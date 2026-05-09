mod ee_log;
mod ocr;
mod screenshot;
mod warframe_window;
mod wfm;

use tauri::{AppHandle, Emitter, Manager};
use warframe_window::WindowGeometry;
use wfm::ItemPriceResult;

// ── Tauri commands ────────────────────────────────────────────────────────────

/// Called by the overlay when a relic-trigger event fires.
/// Verifies Warframe's window exists, captures the 4 reward regions,
/// runs OCR, fuzzy-matches item names, then fetches live plat prices.
#[tauri::command]
async fn detect_relic_rewards() -> Result<Vec<ItemPriceResult>, String> {
    if warframe_window::find_warframe_geometry().is_none() {
        return Err("Warframe window not found — is the game running?".to_string());
    }
    let regions = screenshot::capture_reward_regions().await?;
    let names = ocr::recognise_regions(regions).await;
    Ok(wfm::prices_for_names(names).await)
}

/// Returns Warframe's window geometry in physical pixels, or null if the game
/// is not running. Used by the overlay to position itself over the game window.
#[tauri::command]
fn get_warframe_geometry() -> Option<WindowGeometry> {
    warframe_window::find_warframe_geometry()
}

/// Fires the relic-trigger event manually – useful for UI testing or on
/// platforms where OCR/screenshot are unavailable during development.
#[tauri::command]
fn test_trigger(app: AppHandle) {
    app.emit("relic-trigger", ()).ok();
}

/// Shows the overlay immediately with hardcoded fake price data, bypassing
/// OCR and the Warframe focus check. Used to verify the overlay renders correctly.
/// Positioning is done from Rust before the event is emitted so the JS side
/// only needs to flip `visible` — no async IPC chain that can silently fail.
#[tauri::command]
fn show_test_overlay(app: AppHandle) {
    let geo = warframe_window::find_warframe_geometry();
    eprintln!("[marie] show_test_overlay: warframe = {:?}",
        geo.map(|g| format!("({},{}) {}x{}", g.x, g.y, g.width, g.height)));

    if let Some(g) = geo {
        if let Some(overlay) = app.get_webview_window("overlay") {
            match overlay.set_position(tauri::PhysicalPosition::new(g.x, g.y)) {
                Ok(_)  => eprintln!("[marie] show_test_overlay: set_position ok"),
                Err(e) => eprintln!("[marie] show_test_overlay: set_position failed: {e}"),
            }
            match overlay.set_size(tauri::PhysicalSize::new(g.width, g.height)) {
                Ok(_)  => eprintln!("[marie] show_test_overlay: set_size ok"),
                Err(e) => eprintln!("[marie] show_test_overlay: set_size failed: {e}"),
            }
        } else {
            eprintln!("[marie] show_test_overlay: overlay window not found in app");
        }
    }

    eprintln!("[marie] show_test_overlay: emitting relic-test-data");
    let fake = vec![
        ItemPriceResult {
            ocr_text: "Ash Prime Blueprint".into(),
            matched_name: "Ash Prime Blueprint".into(),
            slug: "ash_prime_blueprint".into(),
            ducats: Some(45),
            plat_min_sell: Some(12),
        },
        ItemPriceResult {
            ocr_text: "Volt Prime Chassis".into(),
            matched_name: "Volt Prime Chassis".into(),
            slug: "volt_prime_chassis".into(),
            ducats: Some(65),
            plat_min_sell: Some(8),
        },
        ItemPriceResult {
            ocr_text: "Forma Blueprint".into(),
            matched_name: "Forma Blueprint".into(),
            slug: "forma_blueprint".into(),
            ducats: None,
            plat_min_sell: Some(3),
        },
        ItemPriceResult {
            ocr_text: "Orokin Reactor Blueprint".into(),
            matched_name: "Orokin Reactor Blueprint".into(),
            slug: "orokin_reactor_blueprint".into(),
            ducats: None,
            plat_min_sell: Some(25),
        },
    ];
    app.emit("relic-test-data", fake).ok();
}

/// Returns the expected EE.log path so the UI can display it.
#[tauri::command]
fn ee_log_path() -> String {
    ee_log::log_path().display().to_string()
}

/// JS-side logging bridge — prints to the Rust terminal since devtools are
/// not reliably accessible for the overlay window on Hyprland.
#[tauri::command]
fn overlay_log(message: String) {
    eprintln!("[marie/overlay] {message}");
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
            overlay_log,
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
