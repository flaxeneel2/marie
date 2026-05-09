mod ee_log;
mod ocr;
mod screenshot;
mod warframe_window;
mod wfm;

use tauri::{AppHandle, Emitter, Manager};
use warframe_window::WindowGeometry;
use wfm::ItemPriceResult;

// ── Tauri commands ────────────────────────────────────────────────────────────

#[tauri::command]
async fn detect_relic_rewards() -> Result<Vec<ItemPriceResult>, String> {
    let geo = warframe_window::find_warframe_geometry()
        .ok_or_else(|| "Warframe window not found — is the game running?".to_string())?;
    let regions = screenshot::capture_reward_regions(geo.x, geo.y, geo.width, geo.height).await?;
    let names = ocr::recognise_regions(regions).await;
    Ok(wfm::prices_for_names(names).await)
}

#[tauri::command]
fn get_warframe_geometry() -> Option<WindowGeometry> {
    warframe_window::find_warframe_geometry()
}

#[tauri::command]
fn test_trigger(app: AppHandle) {
    app.emit("relic-trigger", ()).ok();
}

/// Positions the overlay over Warframe then emits fake price data — bypasses
/// OCR so the overlay UI can be tested without the game running.
#[tauri::command]
fn show_test_overlay(app: AppHandle) {
    if let Some(g) = warframe_window::find_warframe_geometry() {
        if let Some(overlay) = app.get_webview_window("overlay") {
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
