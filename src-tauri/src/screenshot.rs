/// Reward item name regions at the reference 1920×1080 resolution (x, y, w, h).
const REF_REGIONS: [(f32, f32, f32, f32); 4] = [
    (60.0, 310.0, 400.0, 70.0),
    (540.0, 310.0, 400.0, 70.0),
    (1020.0, 310.0, 400.0, 70.0),
    (1500.0, 310.0, 400.0, 70.0),
];
const REF_W: f32 = 1920.0;
const REF_H: f32 = 1080.0;

pub async fn capture_reward_regions() -> Result<Vec<(Vec<u8>, u32, u32)>, String> {
    platform::capture().await
}

// ── Platform implementations ──────────────────────────────────────────────────

#[cfg(windows)]
mod platform {
    use super::{REF_H, REF_REGIONS, REF_W};
    use screenshots::Screen;

    pub async fn capture() -> Result<Vec<(Vec<u8>, u32, u32)>, String> {
        let screens = Screen::all().map_err(|e| e.to_string())?;
        let screen = screens.into_iter().next().ok_or("no display found")?;

        let sw = screen.display_info.width as f32;
        let sh = screen.display_info.height as f32;
        let sx = sw / REF_W;
        let sy = sh / REF_H;

        let mut regions = Vec::with_capacity(4);
        for (rx, ry, rw, rh) in REF_REGIONS {
            let x = (rx * sx) as i32;
            let y = (ry * sy) as i32;
            let w = (rw * sx) as u32;
            let h = (rh * sy) as u32;
            let img = screen.capture_area(x, y, w, h).map_err(|e| e.to_string())?;
            regions.push((img.rgba().to_vec(), img.width(), img.height()));
        }
        Ok(regions)
    }
}

/// Linux: full-screen capture via xdg-desktop-portal, then crop to reward regions.
#[cfg(not(windows))]
mod platform {
    use super::{REF_H, REF_REGIONS, REF_W};
    use ashpd::desktop::screenshot::Screenshot;
    use image::GenericImageView;

    pub async fn capture() -> Result<Vec<(Vec<u8>, u32, u32)>, String> {
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

        // Clean up the temp file the portal created.
        let _ = std::fs::remove_file(path);

        let img = image::load_from_memory(&png_bytes).map_err(|e| e.to_string())?;
        let (screen_w, screen_h) = img.dimensions();
        let sx = screen_w as f32 / REF_W;
        let sy = screen_h as f32 / REF_H;

        let mut regions = Vec::with_capacity(4);
        for (rx, ry, rw, rh) in REF_REGIONS {
            let x = (rx * sx) as u32;
            let y = (ry * sy) as u32;
            let w = (rw * sx) as u32;
            let h = (rh * sy) as u32;
            let cropped = img.crop_imm(x, y, w, h);
            regions.push((cropped.to_rgba8().into_raw(), w, h));
        }
        Ok(regions)
    }
}
