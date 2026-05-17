mod ee_log;
mod ocr;
mod riven;
mod screenshot;
mod warframe_window;
mod wfm;

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

/// Parsed representation of a keyboard shortcut for use in the rdev listener.
#[derive(Clone)]
struct ParsedShortcut {
    ctrl: bool,
    alt: bool,
    shift: bool,
    super_: bool,
    key: rdev::Key,
}

struct ShortcutState {
    raw: String,
    parsed: Option<ParsedShortcut>,
}

static SHORTCUT: Lazy<std::sync::Mutex<ShortcutState>> = Lazy::new(|| {
    let default = "Ctrl+Shift+I".to_string();
    let parsed = parse_shortcut(&default);
    std::sync::Mutex::new(ShortcutState { raw: default, parsed })
});

fn parse_shortcut(s: &str) -> Option<ParsedShortcut> {
    let mut ctrl = false;
    let mut alt = false;
    let mut shift = false;
    let mut super_ = false;
    let mut key: Option<rdev::Key> = None;

    for part in s.split('+') {
        match part.trim() {
            "Ctrl"  => ctrl  = true,
            "Alt"   => alt   = true,
            "Shift" => shift  = true,
            "Super" => super_ = true,
            k       => key = Some(parse_key_name(k)?),
        }
    }

    Some(ParsedShortcut { ctrl, alt, shift, super_, key: key? })
}

fn parse_key_name(s: &str) -> Option<rdev::Key> {
    use rdev::Key::*;
    Some(match s {
        "A" => KeyA, "B" => KeyB, "C" => KeyC, "D" => KeyD, "E" => KeyE,
        "F" => KeyF, "G" => KeyG, "H" => KeyH, "I" => KeyI, "J" => KeyJ,
        "K" => KeyK, "L" => KeyL, "M" => KeyM, "N" => KeyN, "O" => KeyO,
        "P" => KeyP, "Q" => KeyQ, "R" => KeyR, "S" => KeyS, "T" => KeyT,
        "U" => KeyU, "V" => KeyV, "W" => KeyW, "X" => KeyX, "Y" => KeyY,
        "Z" => KeyZ,
        "0" => Num0, "1" => Num1, "2" => Num2, "3" => Num3, "4" => Num4,
        "5" => Num5, "6" => Num6, "7" => Num7, "8" => Num8, "9" => Num9,
        "F1"  => F1,  "F2"  => F2,  "F3"  => F3,  "F4"  => F4,
        "F5"  => F5,  "F6"  => F6,  "F7"  => F7,  "F8"  => F8,
        "F9"  => F9,  "F10" => F10, "F11" => F11, "F12" => F12,
        ";" => SemiColon, "=" => Equal, "-" => Minus,
        "." => Dot, "," => Comma, "/" => Slash, "\\" => BackSlash,
        "[" => LeftBracket, "]" => RightBracket, "'" => Quote, "`" => BackQuote,
        "Space"     => Space,
        "Enter"     => Return,
        "Backspace"  => Backspace,
        "Delete"    => Delete,
        "Escape"    => Escape,
        "Tab"       => Tab,
        "Up"        => UpArrow,
        "Down"      => DownArrow,
        "Left"      => LeftArrow,
        "Right"     => RightArrow,
        "Home"      => Home,
        "End"       => End,
        "PageUp"    => PageUp,
        "PageDown"  => PageDown,
        _ => return None,
    })
}

fn is_modifier(key: rdev::Key) -> bool {
    matches!(
        key,
        rdev::Key::ControlLeft | rdev::Key::ControlRight
        | rdev::Key::Alt | rdev::Key::AltGr
        | rdev::Key::ShiftLeft | rdev::Key::ShiftRight
        | rdev::Key::MetaLeft | rdev::Key::MetaRight
    )
}

fn start_shortcut_listener(app: AppHandle) {
    std::thread::spawn(move || {
        let mut ctrl  = false;
        let mut alt   = false;
        let mut shift = false;
        let mut super_ = false;

        let callback = move |event: rdev::Event| {
            match event.event_type {
                rdev::EventType::KeyPress(key) => {
                    match key {
                        rdev::Key::ControlLeft | rdev::Key::ControlRight => ctrl  = true,
                        rdev::Key::Alt | rdev::Key::AltGr               => alt   = true,
                        rdev::Key::ShiftLeft | rdev::Key::ShiftRight     => shift = true,
                        rdev::Key::MetaLeft | rdev::Key::MetaRight       => super_ = true,
                        _ if !is_modifier(key) => {
                            let matched = SHORTCUT.lock().ok().and_then(|s| {
                                s.parsed.as_ref().map(|p| {
                                    p.ctrl == ctrl
                                        && p.alt == alt
                                        && p.shift == shift
                                        && p.super_ == super_
                                        && p.key == key
                                })
                            }).unwrap_or(false);

                            if matched {
                                toggle_overlay_interaction(&app);
                            }
                        }
                        _ => {}
                    }
                }
                rdev::EventType::KeyRelease(key) => {
                    match key {
                        rdev::Key::ControlLeft | rdev::Key::ControlRight => ctrl  = false,
                        rdev::Key::Alt | rdev::Key::AltGr               => alt   = false,
                        rdev::Key::ShiftLeft | rdev::Key::ShiftRight     => shift = false,
                        rdev::Key::MetaLeft | rdev::Key::MetaRight       => super_ = false,
                        _ => {}
                    }
                }
                _ => {}
            }
        };

        if let Err(e) = rdev::listen(callback) {
            eprintln!("[marie] shortcut listener failed: {e:?}");
            eprintln!("[marie] on Linux make sure you are in the `input` group: sudo usermod -aG input $USER");
        }
    });
}

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
                    win.set_keyboard_mode(KeyboardMode::OnDemand);
                    win.input_shape_combine_region(None);
                } else {
                    win.set_keyboard_mode(KeyboardMode::None);
                    let empty = cairo::Region::create();
                    win.input_shape_combine_region(Some(&empty));
                }
            }
        }
    });
}

// ── Config persistence ────────────────────────────────────────────────────────

fn load_saved_shortcut(app: &AppHandle) -> Option<String> {
    let path = app.path().app_config_dir().ok()?.join("config.json");
    let content = std::fs::read_to_string(path).ok()?;
    let v: serde_json::Value = serde_json::from_str(&content).ok()?;
    v["interaction_shortcut"].as_str().map(str::to_string)
}

fn save_shortcut_config(app: &AppHandle, shortcut: &str) {
    let Ok(dir) = app.path().app_config_dir() else { return };
    let _ = std::fs::create_dir_all(&dir);
    let v = serde_json::json!({ "interaction_shortcut": shortcut });
    let _ = std::fs::write(dir.join("config.json"), v.to_string());
}

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
fn get_interaction_shortcut() -> String {
    SHORTCUT.lock().map(|s| s.raw.clone()).unwrap_or_default()
}

#[tauri::command]
fn set_interaction_shortcut(app: AppHandle, shortcut: String) -> Result<(), String> {
    let parsed = parse_shortcut(&shortcut)
        .ok_or_else(|| format!("unrecognised shortcut: {shortcut}"))?;
    {
        let mut s = SHORTCUT.lock().map_err(|e| e.to_string())?;
        s.raw = shortcut.clone();
        s.parsed = Some(parsed);
    }
    save_shortcut_config(&app, &shortcut);
    app.emit("shortcut-changed", shortcut).ok();
    Ok(())
}

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
            get_interaction_shortcut,
            set_interaction_shortcut,
            disable_overlay_interaction,
        ])
        .setup(|app| {
            #[cfg(target_os = "linux")]
            if let Some(overlay) = app.get_webview_window("overlay") {
                init_layer_shell(&overlay);
            }

            // Load saved shortcut (or keep default).
            if let Some(saved) = load_saved_shortcut(app.handle()) {
                if let Some(parsed) = parse_shortcut(&saved) {
                    if let Ok(mut s) = SHORTCUT.lock() {
                        s.raw = saved;
                        s.parsed = Some(parsed);
                    }
                }
            }

            // Start global key listener in its own OS thread.
            start_shortcut_listener(app.handle().clone());

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
