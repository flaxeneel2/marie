// Reward area bounds as fractions of screen dimensions.
// Measured on 2560×1440: x [635, 1920], y [300, 600].
// Warframe scales its UI proportionally, so these fractions hold at any resolution.
const REWARD_X_START: f32 = 635.0 / 2560.0;  // ≈ 0.248
const REWARD_X_END: f32   = 1920.0 / 2560.0; // = 0.750
const REWARD_Y_START: f32 = 300.0 / 1440.0;  // ≈ 0.208
const REWARD_Y_END: f32   = 600.0 / 1440.0;  // ≈ 0.417
const CARD_COUNT: u32 = 4;

/// Compute the four per-card crop rectangles (x, y, w, h) in pixels for a
/// window of the given dimensions.
fn card_regions(win_w: u32, win_h: u32) -> [(u32, u32, u32, u32); 4] {
    let x0 = (REWARD_X_START * win_w as f32) as u32;
    let x1 = (REWARD_X_END   * win_w as f32) as u32;
    let y0 = (REWARD_Y_START * win_h as f32) as u32;
    let y1 = (REWARD_Y_END   * win_h as f32) as u32;
    let card_w = (x1 - x0) / CARD_COUNT;
    let card_h = y1 - y0;
    std::array::from_fn(|i| (x0 + i as u32 * card_w, y0, card_w, card_h))
}

/// Capture the four reward-name regions from within the Warframe window.
/// `win_x/y` is the window's screen origin; `win_w/h` its physical pixel size.
pub async fn capture_reward_regions(
    win_x: i32,
    win_y: i32,
    win_w: u32,
    win_h: u32,
) -> Result<Vec<(Vec<u8>, u32, u32)>, String> {
    platform::capture(win_x, win_y, win_w, win_h).await
}

// ── Platform implementations ──────────────────────────────────────────────────

#[cfg(windows)]
mod platform {
    use super::card_regions;
    use screenshots::Screen;

    pub async fn capture(
        win_x: i32,
        win_y: i32,
        win_w: u32,
        win_h: u32,
    ) -> Result<Vec<(Vec<u8>, u32, u32)>, String> {
        let screens = Screen::all().map_err(|e| e.to_string())?;
        let screen = screens
            .into_iter()
            .find(|s| {
                let di = &s.display_info;
                win_x >= di.x
                    && win_x < di.x + di.width as i32
                    && win_y >= di.y
                    && win_y < di.y + di.height as i32
            })
            .ok_or("could not find screen containing Warframe window")?;

        let mut regions = Vec::with_capacity(4);
        for (rx, ry, rw, rh) in card_regions(win_w, win_h) {
            let x = win_x + rx as i32;
            let y = win_y + ry as i32;
            let img = screen.capture_area(x, y, rw, rh).map_err(|e| e.to_string())?;
            regions.push((img.rgba().to_vec(), img.width(), img.height()));
        }
        Ok(regions)
    }
}

/// Linux: capture only the Warframe window area, then crop to reward regions.
///
/// Uses `grim -g "x,y WxH"` (zwlr_screencopy_manager_v1) to capture exactly the
/// Warframe window rectangle on its monitor. Falls back to XDG portal full-screen
/// capture (then crops using the window origin) if grim is not installed.
#[cfg(not(windows))]
mod platform {
    use super::card_regions;
    use image::GenericImageView;

    pub async fn capture(
        win_x: i32,
        win_y: i32,
        win_w: u32,
        win_h: u32,
    ) -> Result<Vec<(Vec<u8>, u32, u32)>, String> {
        // grim captures exactly the requested rectangle; the resulting image's
        // (0,0) corresponds to (win_x, win_y) on screen, so crop relative to (0,0).
        let png_bytes = capture_window_png(win_x, win_y, win_w, win_h).await?;
        let img = image::load_from_memory(&png_bytes).map_err(|e| e.to_string())?;
        let (img_w, img_h) = img.dimensions();

        let mut regions = Vec::with_capacity(4);
        for (rx, ry, rw, rh) in card_regions(img_w, img_h) {
            let cropped = img.crop_imm(rx, ry, rw, rh);
            let cw = cropped.width();
            let ch = cropped.height();
            eprintln!("[screenshot] card region {rx},{ry} {rw}×{rh} → cropped {cw}×{ch}");
            regions.push((cropped.to_rgba8().into_raw(), cw, ch));
        }
        Ok(regions)
    }

    async fn capture_window_png(
        win_x: i32,
        win_y: i32,
        win_w: u32,
        win_h: u32,
    ) -> Result<Vec<u8>, String> {
        let geom = format!("{win_x},{win_y} {win_w}x{win_h}");
        if let Ok(bytes) = capture_via_grim(&geom) {
            return Ok(bytes);
        }
        // Portal fallback: full screen, then crop to window bounds.
        let full = capture_via_portal_full().await?;
        let img = image::load_from_memory(&full).map_err(|e| e.to_string())?;
        let cropped = img.crop_imm(win_x.max(0) as u32, win_y.max(0) as u32, win_w, win_h);
        let mut buf = std::io::Cursor::new(Vec::new());
        cropped
            .write_to(&mut buf, image::ImageFormat::Png)
            .map_err(|e| e.to_string())?;
        Ok(buf.into_inner())
    }

    /// Spawn `grim -g "x,y WxH" -` to capture just the Warframe window area.
    fn capture_via_grim(geometry: &str) -> Result<Vec<u8>, String> {
        let out = std::process::Command::new("grim")
            .args(["-g", geometry, "-"])
            .output()
            .map_err(|e| e.to_string())?;
        if out.status.success() {
            Ok(out.stdout)
        } else {
            Err(String::from_utf8_lossy(&out.stderr).into_owned())
        }
    }

    async fn capture_via_portal_full() -> Result<Vec<u8>, String> {
        use ashpd::desktop::screenshot::Screenshot;

        let response = Screenshot::request()
            .interactive(false)
            .modal(false)
            .send()
            .await
            .map_err(|e| e.to_string())?
            .response()
            .map_err(|e| e.to_string())?;

        let uri = response.uri().to_string();
        let path = uri.strip_prefix("file://").unwrap_or(&uri);
        let png_bytes = std::fs::read(path).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_file(path);
        Ok(png_bytes)
    }
}
