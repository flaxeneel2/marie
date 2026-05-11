// Proportional bounds measured on 2560×1440.
// Reward name text: x=[635,1920], y=[550,612].
const TEXT_X_START: f32 = 635.0  / 2560.0;
const TEXT_X_END:   f32 = 1920.0 / 2560.0;
const TEXT_Y_START: f32 = 550.0  / 1440.0;
const TEXT_Y_END:   f32 = 612.0  / 1440.0;

// Riven stat text region — calibrated on a 2560×1440 display from a real session log.
// Covers the OmegaRerollSelection stat panel: the same region is captured twice —
// once when the screen opens (current/old stats) and once after rolling (new stats).
const RIVEN_STAT_X_START: f32 = 1110.0 / 2560.0;
const RIVEN_STAT_X_END:   f32 = 1460.0 / 2560.0;
const RIVEN_STAT_Y_START: f32 =  915.0 / 1440.0;
const RIVEN_STAT_Y_END:   f32 = 1100.0 / 1440.0;

/// Capture the riven stat text region from the OmegaRerollSelection screen.
/// Called twice per roll cycle: once on screen-open (current stats) and once
/// after the roll (new stats). Same region both times; content changes.
pub async fn capture_riven_stat_region(
    win_x: i32, win_y: i32, win_w: u32, win_h: u32,
) -> Result<(Vec<u8>, u32, u32), String> {
    let (rx, ry, rw, rh) = frac_rect(
        RIVEN_STAT_X_START, RIVEN_STAT_X_END,
        RIVEN_STAT_Y_START, RIVEN_STAT_Y_END,
        win_w, win_h,
    );
    eprintln!("[screenshot] riven stat region: {rw}×{rh} at ({rx},{ry})");
    platform::capture_rect(win_x, win_y, (rx, ry, rw, rh)).await
}

/// Capture one RGBA strip per reward card, all covering the item-name text rows.
/// Cards are equally spaced across the measured x range.
/// `player_count` must be 1–4; values outside that range are clamped.
pub async fn capture_card_strips(
    win_x: i32, win_y: i32, win_w: u32, win_h: u32,
    player_count: u32,
) -> Result<Vec<(Vec<u8>, u32, u32)>, String> {
    let n = player_count.clamp(1, 4);
    let card_w_frac = (TEXT_X_END - TEXT_X_START) / n as f32;
    let mut out = Vec::with_capacity(n as usize);
    for i in 0..n {
        let x0 = TEXT_X_START + i as f32 * card_w_frac;
        let x1 = x0 + card_w_frac;
        let (rx, ry, rw, rh) = frac_rect(x0, x1, TEXT_Y_START, TEXT_Y_END, win_w, win_h);
        eprintln!("[screenshot] card {i} strip {rw}×{rh} at ({rx},{ry})");
        out.push(platform::capture_rect(win_x, win_y, (rx, ry, rw, rh)).await?);
    }
    Ok(out)
}

fn frac_rect(x0: f32, x1: f32, y0: f32, y1: f32, ww: u32, wh: u32) -> (u32, u32, u32, u32) {
    (
        (x0 * ww as f32) as u32,
        (y0 * wh as f32) as u32,
        ((x1 - x0) * ww as f32) as u32,
        ((y1 - y0) * wh as f32) as u32,
    )
}

// ── Platform implementations ──────────────────────────────────────────────────

#[cfg(windows)]
mod platform {
    use screenshots::Screen;

    pub async fn capture_rect(
        win_x: i32, win_y: i32,
        (rx, ry, rw, rh): (u32, u32, u32, u32),
    ) -> Result<(Vec<u8>, u32, u32), String> {
        let screens = Screen::all().map_err(|e| e.to_string())?;
        let screen = screens
            .into_iter()
            .find(|s| {
                let di = &s.display_info;
                win_x >= di.x && win_x < di.x + di.width as i32
                    && win_y >= di.y && win_y < di.y + di.height as i32
            })
            .ok_or("could not find screen containing Warframe window")?;
        let img = screen
            .capture_area(win_x + rx as i32, win_y + ry as i32, rw, rh)
            .map_err(|e| e.to_string())?;
        Ok((img.rgba().to_vec(), img.width(), img.height()))
    }
}

#[cfg(not(windows))]
mod platform {
    use image::GenericImageView;

    pub async fn capture_rect(
        win_x: i32, win_y: i32,
        (rx, ry, rw, rh): (u32, u32, u32, u32),
    ) -> Result<(Vec<u8>, u32, u32), String> {
        let geom = format!("{},{} {}x{}", win_x + rx as i32, win_y + ry as i32, rw, rh);
        let out = std::process::Command::new("grim")
            .args(["-g", &geom, "-"])
            .output()
            .map_err(|e| e.to_string())?;
        if !out.status.success() {
            return Err(String::from_utf8_lossy(&out.stderr).into_owned());
        }
        let img = image::load_from_memory(&out.stdout).map_err(|e| e.to_string())?;
        let (w, h) = img.dimensions();
        Ok((img.to_rgba8().into_raw(), w, h))
    }
}
