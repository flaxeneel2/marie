mod ee_log;
mod ocr;
mod screenshot;
mod warframe_window;
mod wfm;

use tauri::{AppHandle, Emitter, Manager};
use warframe_window::WindowGeometry;
use wfm::ItemPriceResult;

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Convert a Tauri `Monitor` to `WindowGeometry` in **physical** pixels.
///
/// `Monitor::size()` returns logical pixels on Wayland/HiDPI; multiplying by
/// `scale_factor()` gives the true physical resolution that `set_size(PhysicalSize)`
/// expects.
fn monitor_geometry(m: tauri::Monitor) -> WindowGeometry {
    let scale = m.scale_factor();
    WindowGeometry {
        x: m.position().x,
        y: m.position().y,
        width:  (m.size().width  as f64 * scale).round() as u32,
        height: (m.size().height as f64 * scale).round() as u32,
    }
}

/// Geometry to cover the game window, falling back to the primary monitor.
fn overlay_geometry(overlay: &tauri::WebviewWindow) -> Option<WindowGeometry> {
    warframe_window::find_warframe_geometry()
        .or_else(|| overlay.primary_monitor().ok().flatten().map(monitor_geometry))
}

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
fn get_warframe_geometry() -> Option<WindowGeometry> {
    warframe_window::find_warframe_geometry()
}

#[tauri::command]
fn test_trigger(app: AppHandle, player_count: u32) {
    app.emit("relic-trigger", player_count.clamp(1, 4)).ok();
}

/// Positions the overlay over Warframe then emits fake price data — bypasses
/// OCR so the overlay UI can be tested without the game running.
#[tauri::command]
fn show_test_overlay(app: AppHandle) {
    if let Some(overlay) = app.get_webview_window("overlay") {
        if let Some(g) = overlay_geometry(&overlay) {
            overlay.set_position(tauri::PhysicalPosition::new(g.x, g.y)).ok();
            overlay.set_size(tauri::PhysicalSize::new(g.width, g.height)).ok();
        }
    }
    app.emit("relic-test-data", vec![
        ItemPriceResult { ocr_text: "Ash Prime Blueprint".into(),      matched_name: "Ash Prime Blueprint".into(),      slug: "ash_prime_blueprint".into(),      ducats: Some(45), plat_min_sell: Some(12) },
        ItemPriceResult { ocr_text: "Volt Prime Chassis".into(),       matched_name: "Volt Prime Chassis".into(),       slug: "volt_prime_chassis".into(),       ducats: Some(65), plat_min_sell: Some(8)  },
        ItemPriceResult { ocr_text: "Forma Blueprint".into(),          matched_name: "Forma Blueprint".into(),          slug: "forma_blueprint".into(),          ducats: None,     plat_min_sell: Some(3)  },
        ItemPriceResult { ocr_text: "Orokin Reactor Blueprint".into(), matched_name: "Orokin Reactor Blueprint".into(), slug: "orokin_reactor_blueprint".into(), ducats: None,     plat_min_sell: Some(25) },
    ]).ok();

    // Timer lives in Tokio, not WebKit — the overlay's nofocus window rule causes
    // WebKitGTK to throttle setTimeout in background pages, so JS timers never fire.
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
            get_warframe_geometry,
            test_trigger,
            show_test_overlay,
            ee_log_path,
        ])
        .setup(|app| {
            // Size the overlay to cover Warframe, falling back to the primary monitor
            // when the game isn't running yet. This runs before the webview renders,
            // so the 1×1 placeholder size in tauri.conf.json is never visible.
            if let Some(overlay) = app.get_webview_window("overlay") {
                if let Some(g) = overlay_geometry(&overlay) {
                    overlay.set_position(tauri::PhysicalPosition::new(g.x, g.y)).ok();
                    overlay.set_size(tauri::PhysicalSize::new(g.width, g.height)).ok();
                    eprintln!("[marie] overlay sized to {}×{} at ({},{})", g.width, g.height, g.x, g.y);
                }
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
