mod ee_log;
mod ocr;
mod riven;
mod screenshot;
mod warframe_window;
mod wfm;

#[cfg(all(feature = "memory", target_os = "linux"))]
mod account_memory;

#[cfg(all(feature = "memory", target_os = "linux"))]
mod inventory;

#[cfg(all(feature = "memory", target_os = "linux"))]
mod items_cache;

use once_cell::sync::Lazy;
use std::sync::atomic::{AtomicBool, Ordering};
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;
use wfm::ItemPriceResult;

// Last successfully OCR'd riven stat lines. On each roll this becomes the "old"
// snapshot — the fresh OCR result is stored here for the next roll to use as old.
static CURRENT_RIVEN_LINES: Lazy<Mutex<Option<Vec<String>>>> = Lazy::new(|| Mutex::new(None));

// ── Overlay interaction toggle ────────────────────────────────────────────────

static OVERLAY_INTERACTIVE: AtomicBool = AtomicBool::new(false);

fn toggle_overlay_interaction(app: &AppHandle) {
    let was = OVERLAY_INTERACTIVE.fetch_xor(true, Ordering::SeqCst);
    let now = !was;
    eprintln!("[marie] overlay interaction: {}", if now { "on" } else { "off" });

    #[cfg(target_os = "linux")]
    set_overlay_input_linux(app, now);

    app.emit("overlay-interactive-changed", now).ok();
}

#[cfg(target_os = "linux")]
fn set_overlay_input_linux(app: &AppHandle, interactive: bool) {
    let app = app.clone();
    gtk::glib::idle_add_once(move || {
        use gtk::prelude::WidgetExt;
        use gtk_layer_shell::{KeyboardMode, LayerShell};
        if let Some(overlay) = app.get_webview_window("overlay") {
            if let Ok(win) = overlay.gtk_window() {
                if interactive {
                    // Exclusive: steals keyboard from Warframe/XWayland so the overlay
                    // can receive text selection shortcuts (ctrl+c etc).
                    // Input region stays passthrough until frontend calls set_interactive_region.
                    win.set_keyboard_mode(KeyboardMode::Exclusive);
                } else {
                    win.set_keyboard_mode(KeyboardMode::None);
                    let empty = cairo::Region::create();
                    win.input_shape_combine_region(Some(&empty));
                }
            }
        }
    });
}

// ── Keybind config (Linux only) ───────────────────────────────────────────────

// Stores just the "MODS, key" portion, e.g. "CTRL SHIFT, i".
// Marie injects the full bind via `hyprctl keyword bind` at startup.
const DEFAULT_BIND: &str = "CTRL SHIFT, i";
const SHORTCUT_ID: &str  = "toggle-overlay";
const APP_ID: &str        = "net.flaxeneel2.marie";

#[cfg(target_os = "linux")]
fn config_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("config.json"))
}

#[cfg(target_os = "linux")]
fn load_shortcut_config(app: &AppHandle) -> String {
    config_path(app)
        .and_then(|p| std::fs::read_to_string(p).ok())
        .and_then(|s| serde_json::from_str::<serde_json::Value>(&s).ok())
        .and_then(|v| v["bind"].as_str().map(str::to_string))
        .unwrap_or_else(|| DEFAULT_BIND.to_string())
}

#[cfg(target_os = "linux")]
fn save_shortcut_config(app: &AppHandle, mods_key: &str) {
    if let Some(path) = config_path(app) {
        let _ = std::fs::create_dir_all(path.parent().unwrap());
        let _ = std::fs::write(path, serde_json::json!({ "bind": mods_key }).to_string());
    }
}

// Removes ALL accumulated binds for `mods_key` (e.g. "CTRL, semicolon").
// hyprctl keyword bind always appends — without flushing first, repeated
// Apply clicks or restarts pile up duplicates that cancel each other out.
#[cfg(target_os = "linux")]
fn flush_hyprland_bind(mods_key: &str) {
    for _ in 0..32 {
        let Ok(out) = std::process::Command::new("hyprctl")
            .args(["keyword", "unbind", mods_key])
            .output() else { break };
        // hyprctl prints "ok" on success; anything else means no bind was found.
        if out.stdout.trim_ascii() != b"ok" {
            break;
        }
    }
}

// Injects a fresh single bind into the running Hyprland session.
#[cfg(target_os = "linux")]
fn apply_hyprland_bind(mods_key: &str) -> Result<(), String> {
    flush_hyprland_bind(mods_key);

    let bind_val = format!("{mods_key}, global, {APP_ID}:{SHORTCUT_ID}");
    let status = std::process::Command::new("hyprctl")
        .args(["keyword", "bind", &bind_val])
        .status()
        .map_err(|e| format!("hyprctl not found: {e}"))?;

    if status.success() {
        eprintln!("[marie] hyprctl bind set: {bind_val}");
        Ok(())
    } else {
        Err(format!("hyprctl exited with {status}"))
    }
}

// ── XDG GlobalShortcuts portal listener ──────────────────────────────────────

#[cfg(not(windows))]
async fn start_portal_listener(app: AppHandle) {
    use ashpd::desktop::global_shortcuts::{BindShortcutsOptions, GlobalShortcuts, NewShortcut};
    use ashpd::desktop::CreateSessionOptions;
    use futures_util::StreamExt;

    let proxy = match GlobalShortcuts::new().await {
        Ok(p) => p,
        Err(e) => {
            eprintln!("[marie] GlobalShortcuts portal unavailable: {e}");
            return;
        }
    };

    let session = match proxy.create_session(CreateSessionOptions::default()).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[marie] failed to create GlobalShortcuts session: {e}");
            return;
        }
    };

    let shortcut = NewShortcut::new("toggle-overlay", "Toggle overlay interactive mode");
    match proxy.bind_shortcuts(&session, &[shortcut], None, BindShortcutsOptions::default()).await {
        Ok(req) => {
            if let Err(e) = req.response() {
                eprintln!("[marie] bind_shortcuts response error: {e}");
                return;
            }
            eprintln!("[marie] GlobalShortcuts: registered 'toggle-overlay'");
        }
        Err(e) => {
            eprintln!("[marie] bind_shortcuts failed: {e}");
            return;
        }
    }

    let mut stream = match proxy.receive_activated().await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[marie] failed to subscribe to Activated signal: {e}");
            return;
        }
    };

    while let Some(activated) = stream.next().await {
        if activated.shortcut_id() == "toggle-overlay" {
            toggle_overlay_interaction(&app);
        }
    }

    eprintln!("[marie] GlobalShortcuts stream ended");
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[derive(serde::Deserialize)]
struct PhysRect {
    x: i32,
    y: i32,
    width: i32,
    height: i32,
}

/// Called by the overlay frontend after entering interactive mode.
/// Builds a Cairo input-shape region from the measured content element bounds so that
/// only those pixels capture pointer events — the rest of the screen stays passthrough.
#[cfg(target_os = "linux")]
#[tauri::command]
fn set_interactive_region(app: AppHandle, rects: Vec<PhysRect>) {
    gtk::glib::idle_add_once(move || {
        use gtk::prelude::WidgetExt;
        if let Some(overlay) = app.get_webview_window("overlay") {
            if let Ok(win) = overlay.gtk_window() {
                let region = cairo::Region::create();
                for r in &rects {
                    let rect = cairo::RectangleInt::new(r.x, r.y, r.width, r.height);
                    region.union_rectangle(&rect).ok();
                }
                // Empty region when no rects: stays passthrough.
                win.input_shape_combine_region(Some(&region));
            }
        }
    });
}

#[cfg(not(target_os = "linux"))]
#[tauri::command]
fn set_interactive_region(_app: AppHandle, _rects: Vec<PhysRect>) {}

#[tauri::command]
fn disable_overlay_interaction(app: AppHandle) {
    OVERLAY_INTERACTIVE.store(false, Ordering::SeqCst);
    #[cfg(target_os = "linux")]
    set_overlay_input_linux(&app, false);
    app.emit("overlay-interactive-changed", false).ok();
}

#[tauri::command]
async fn detect_relic_rewards(player_count: u32) -> Result<Vec<ItemPriceResult>, String> {
    let geo = warframe_window::find_warframe_geometry()
        .ok_or_else(|| "Warframe window not found — is the game running?".to_string())?;

    let regions = screenshot::capture_card_strips(geo.x, geo.y, geo.width, geo.height, player_count).await?;
    let names = ocr::recognise_cards(regions).await;
    Ok(wfm::prices_for_names(names).await)
}

#[tauri::command]
fn test_trigger(app: AppHandle, player_count: u32) {
    app.emit("relic-trigger", player_count.clamp(1, 4)).ok();
}

/// Emits fake price data so the overlay UI can be tested without the game running.
#[tauri::command]
fn show_test_overlay(app: AppHandle) {
    app.emit("relic-test-data", vec![
        ItemPriceResult { ocr_text: "Ash Prime Blueprint".into(),      matched_name: "Ash Prime Blueprint".into(),      slug: "ash_prime_blueprint".into(),      ducats: Some(45), plat_min_sell: Some(12) },
        ItemPriceResult { ocr_text: "Volt Prime Chassis".into(),       matched_name: "Volt Prime Chassis".into(),       slug: "volt_prime_chassis".into(),       ducats: Some(65), plat_min_sell: Some(8)  },
        ItemPriceResult { ocr_text: "Forma Blueprint".into(),          matched_name: "Forma Blueprint".into(),          slug: "forma_blueprint".into(),          ducats: None,     plat_min_sell: Some(3)  },
        ItemPriceResult { ocr_text: "Orokin Reactor Blueprint".into(), matched_name: "Orokin Reactor Blueprint".into(), slug: "orokin_reactor_blueprint".into(), ducats: None,     plat_min_sell: Some(25) },
    ]).ok();

    // Dismiss timer runs in Tokio: WebKitGTK throttles JS timers on windows
    // that never receive keyboard focus (which layer-shell KeyboardMode::None guarantees).
    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(5)).await;
        app2.emit("hide-overlay", ()).ok();
    });
}

#[tauri::command]
fn ee_log_path() -> String {
    ee_log::log_path().display().to_string()
}

/// Called when the riven reroll screen opens (before any rolling).
/// OCRs the stat region, stores lines as baseline, returns the graded result for immediate display.
#[tauri::command]
async fn capture_current_riven() -> Result<riven::RivenRollGrade, String> {
    eprintln!("[riven] capture_current_riven called — OCR starting");
    let geo = warframe_window::find_warframe_geometry()
        .ok_or_else(|| "Warframe window not found — is the game running?".to_string())?;

    eprintln!("[riven] window found at ({},{}) {}×{}", geo.x, geo.y, geo.width, geo.height);
    let region = screenshot::capture_riven_stat_region(geo.x, geo.y, geo.width, geo.height).await?;
    eprintln!("[riven] screenshot captured, running OCR…");
    let lines = ocr::recognise_riven_panels(vec![region]).await
        .into_iter().next().unwrap_or_default();

    eprintln!("[riven] capture_current_riven done — {} lines: {:?}", lines.len(), lines);
    *CURRENT_RIVEN_LINES.lock().await = Some(lines.clone());
    Ok(riven::grade_panels(lines.clone(), lines).await.new)
}

/// Called when new stats are on screen (after each roll).
/// Rotates: CURRENT becomes old, fresh OCR becomes the new CURRENT, grades both.
#[tauri::command]
async fn grade_riven_reroll() -> Result<riven::RivenRerollResult, String> {
    eprintln!("[riven] grade_riven_reroll called — OCR starting for new stats");
    let geo = warframe_window::find_warframe_geometry()
        .ok_or_else(|| "Warframe window not found — is the game running?".to_string())?;

    eprintln!("[riven] window found at ({},{}) {}×{}", geo.x, geo.y, geo.width, geo.height);
    let region = screenshot::capture_riven_stat_region(geo.x, geo.y, geo.width, geo.height).await?;
    eprintln!("[riven] screenshot captured, running OCR…");
    let new_lines = ocr::recognise_riven_panels(vec![region]).await
        .into_iter().next().unwrap_or_default();

    eprintln!("[riven] new OCR lines ({}): {:?}", new_lines.len(), new_lines);

    let old_lines = {
        let mut guard = CURRENT_RIVEN_LINES.lock().await;
        let old = guard.take().unwrap_or_default();
        *guard = Some(new_lines.clone());
        old
    };

    eprintln!("[riven] grading: {} old lines, {} new lines", old_lines.len(), new_lines.len());
    Ok(riven::grade_panels(old_lines, new_lines).await)
}

#[cfg(target_os = "linux")]
#[tauri::command]
fn get_interaction_shortcut(app: AppHandle) -> String {
    load_shortcut_config(&app)
}

#[cfg(not(target_os = "linux"))]
#[tauri::command]
fn get_interaction_shortcut(_app: AppHandle) -> String {
    DEFAULT_BIND.to_string()
}

#[cfg(target_os = "linux")]
#[tauri::command]
fn set_interaction_shortcut(app: AppHandle, mods_key: String) -> Result<(), String> {
    // If the key changed, flush the old bind so it doesn't linger.
    let old = load_shortcut_config(&app);
    if old != mods_key {
        flush_hyprland_bind(&old);
    }
    save_shortcut_config(&app, &mods_key);
    apply_hyprland_bind(&mods_key) // apply_hyprland_bind also flushes mods_key itself
}

#[cfg(not(target_os = "linux"))]
#[tauri::command]
fn set_interaction_shortcut(_app: AppHandle, _mods_key: String) -> Result<(), String> {
    Ok(())
}

#[cfg(all(feature = "memory", target_os = "linux"))]
#[tauri::command]
fn get_account_info() -> Option<account_memory::AccountInfo> {
    account_memory::ACCOUNT_INFO.lock().unwrap().clone()
}

#[cfg(not(all(feature = "memory", target_os = "linux")))]
#[tauri::command]
fn get_account_info() -> Option<serde_json::Value> {
    None
}

#[cfg(all(feature = "memory", target_os = "linux"))]
#[tauri::command]
async fn get_inventory(app: AppHandle) -> Result<inventory::InventoryView, String> {
    let (inv_result, maps) = tokio::join!(
        inventory::get_or_refresh_inventory(&app),
        items_cache::get_maps(&app),
    );
    let cache = inv_result?;
    Ok(inventory::build_view(&cache, &maps.names, &maps.categories, &maps.types, &maps.images, &maps.overlay_images, &maps.fusion_limits, &maps.rarities, &maps.polarities, &maps.compat_names, &maps.descriptions, &maps.level_stats, &maps.base_drains))
}

#[cfg(not(all(feature = "memory", target_os = "linux")))]
#[tauri::command]
async fn get_inventory() -> Result<serde_json::Value, String> {
    Err("inventory requires the memory feature (Linux only)".into())
}

#[tauri::command]
fn test_riven_trigger(app: AppHandle) {
    app.emit("riven-reroll", ()).ok();
}

#[tauri::command]
fn show_test_riven_overlay(app: AppHandle) {
    app.emit("riven-test-data", riven::fake_reroll_result()).ok();

    let app2 = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(std::time::Duration::from_secs(10)).await;
        app2.emit("hide-riven-overlay", ()).ok();
    });
}

// ── Entry point ───────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    // Fork privileged child, do initial account scan, then drop to real UID —
    // all before Tauri / D-Bus / GTK initialise. See docs/memory.md.
    #[cfg(all(feature = "memory", target_os = "linux"))]
    account_memory::init();

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .invoke_handler(tauri::generate_handler![
            detect_relic_rewards,
            test_trigger,
            show_test_overlay,
            ee_log_path,
            capture_current_riven,
            grade_riven_reroll,
            test_riven_trigger,
            show_test_riven_overlay,
            disable_overlay_interaction,
            set_interactive_region,
            get_interaction_shortcut,
            set_interaction_shortcut,
            get_account_info,
            get_inventory,
        ])
        .setup(|app| {
            #[cfg(target_os = "linux")]
            if let Some(overlay) = app.get_webview_window("overlay") {
                init_layer_shell(&overlay);
            }

            #[cfg(target_os = "linux")]
            {
                let bind = load_shortcut_config(app.handle());
                if let Err(e) = apply_hyprland_bind(&bind) {
                    eprintln!("[marie] hyprctl bind failed at startup: {e}");
                }
            }

            #[cfg(not(windows))]
            {
                let handle = app.handle().clone();
                tauri::async_runtime::spawn(async move { start_portal_listener(handle).await });
            }

            tauri::async_runtime::spawn(async move {
                match wfm::init_cache().await {
                    Ok(n) => eprintln!("[marie] WFM cache ready: {n} items"),
                    Err(e) => eprintln!("[marie] WFM cache failed: {e}"),
                }
            });
            tauri::async_runtime::spawn(async move {
                match riven::init_riven_cache().await {
                    Ok((w, a)) => eprintln!("[marie] riven cache ready: {w} weapons, {a} attributes"),
                    Err(e) => eprintln!("[marie] riven cache failed: {e}"),
                }
            });
            ee_log::start_watcher(app.handle().clone());
            Ok(())
        })
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}

// ── Layer shell init (Linux / Wayland only) ───────────────────────────────────

#[cfg(target_os = "linux")]
fn init_layer_shell(overlay: &tauri::WebviewWindow) {
    use gtk_layer_shell::{Edge, KeyboardMode, Layer, LayerShell};

    let win = match overlay.gtk_window() {
        Ok(w) => w,
        Err(e) => { eprintln!("[marie] overlay: could not get GTK window: {e}"); return; }
    };

    if !gtk_layer_shell::is_supported() {
        eprintln!("[marie] overlay: wlr-layer-shell not available — falling back to XDG shell (add Hyprland window rules)");
        return;
    }

    win.init_layer_shell();
    win.set_layer(Layer::Overlay);
    win.set_anchor(Edge::Left,   true);
    win.set_anchor(Edge::Right,  true);
    win.set_anchor(Edge::Top,    true);
    win.set_anchor(Edge::Bottom, true);
    win.set_exclusive_zone(-1);
    win.set_keyboard_mode(KeyboardMode::None);
    win.set_namespace("marie-overlay");

    use gtk::prelude::WidgetExt;
    win.show_all();

    let empty = cairo::Region::create();
    win.input_shape_combine_region(Some(&empty));

    eprintln!("[marie] overlay: wlr-layer-shell initialised (OVERLAY layer, full-screen, no keyboard)");
}
