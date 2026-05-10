mod ee_log;
mod ocr;
mod screenshot;
mod warframe_window;
mod wfm;

use tauri::{AppHandle, Emitter, Manager};
use wfm::ItemPriceResult;

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
