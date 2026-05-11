mod ee_log;
mod ocr;
mod riven;
mod screenshot;
mod warframe_window;
mod wfm;

use once_cell::sync::Lazy;
use tauri::{AppHandle, Emitter, Manager};
use tokio::sync::Mutex;
use wfm::ItemPriceResult;

// Last successfully OCR'd riven stat lines. On each roll this becomes the "old"
// snapshot — the fresh OCR result is stored here for the next roll to use as old.
static CURRENT_RIVEN_LINES: Lazy<Mutex<Option<Vec<String>>>> = Lazy::new(|| Mutex::new(None));

// ── Tauri commands ────────────────────────────────────────────────────────────

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
    // Grade the current state for immediate overlay display.
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

    // Rotate: whatever was current becomes old, new OCR becomes the new current.
    let old_lines = {
        let mut guard = CURRENT_RIVEN_LINES.lock().await;
        let old = guard.take().unwrap_or_default();
        *guard = Some(new_lines.clone());
        old
    };

    eprintln!("[riven] grading: {} old lines, {} new lines", old_lines.len(), new_lines.len());
    Ok(riven::grade_panels(old_lines, new_lines).await)
}

/// Fires the riven-reroll event so the overlay exercises the real OCR + grading
/// pipeline. Mirrors how test_trigger works for relics.
#[tauri::command]
fn test_riven_trigger(app: AppHandle) {
    app.emit("riven-reroll", ()).ok();
}

/// Sends pre-baked fake data directly to the overlay, bypassing OCR entirely.
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
        ])
        .setup(|app| {
            #[cfg(target_os = "linux")]
            if let Some(overlay) = app.get_webview_window("overlay") {
                init_layer_shell(&overlay);
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
    // -1: don't push panels/taskbars; surface floats above everything
    win.set_exclusive_zone(-1);
    win.set_keyboard_mode(KeyboardMode::None);
    win.set_namespace("marie-overlay");

    // Window was created hidden (visible:false in tauri.conf.json) so that
    // init_layer_shell could run before the GTK window was mapped.  Show it now.
    use gtk::prelude::WidgetExt;
    win.show_all();

    // Make the entire surface click-through by setting an empty input region.
    // On Wayland this becomes wl_surface.set_input_region(empty), so all pointer
    // events pass straight through to whatever is underneath.
    let empty = cairo::Region::create();
    win.input_shape_combine_region(Some(&empty));

    eprintln!("[marie] overlay: wlr-layer-shell initialised (OVERLAY layer, full-screen, no keyboard)");
}
