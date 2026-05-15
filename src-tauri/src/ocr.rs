use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use once_cell::sync::Lazy;
use rten::Model;

static DETECTION_MODEL: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/models/text-detection.rten"));
static RECOGNITION_MODEL: &[u8] =
    include_bytes!(concat!(env!("CARGO_MANIFEST_DIR"), "/models/text-recognition.rten"));

static ENGINE: Lazy<Result<OcrEngine, String>> = Lazy::new(|| {
    let det = Model::load(DETECTION_MODEL.to_vec()).map_err(|e| e.to_string())?;
    let rec = Model::load(RECOGNITION_MODEL.to_vec()).map_err(|e| e.to_string())?;
    OcrEngine::new(OcrEngineParams {
        detection_model: Some(det),
        recognition_model: Some(rec),
        ..Default::default()
    })
    .map_err(|e| e.to_string())
});

/// Run OCR on riven panels and return individual text lines per panel.
/// Unlike recognise_cards, lines are NOT joined — the caller parses them.
pub async fn recognise_riven_panels(regions: Vec<(Vec<u8>, u32, u32)>) -> Vec<Vec<String>> {
    let handles: Vec<_> = regions
        .into_iter()
        .enumerate()
        .map(|(i, (pixels, w, h))| {
            tokio::task::spawn_blocking(move || {
                let lines = ocr_riven_panel(&pixels, w, h, i);
                eprintln!("[ocr] riven panel {i} ({w}×{h}): {} lines", lines.len());
                lines
            })
        })
        .collect();

    let mut out = Vec::with_capacity(handles.len());
    for h in handles {
        out.push(h.await.unwrap_or_default());
    }
    out
}

/// OCR a riven panel and return each text line separately (not joined).
fn ocr_riven_panel(pixels: &[u8], width: u32, height: u32, idx: usize) -> Vec<String> {
    let engine = match ENGINE.as_ref() {
        Ok(e) => e,
        Err(e) => { eprintln!("[ocr] engine init failed: {e}"); return vec![]; }
    };

    save_ppm_rgba(pixels, width, height, &format!("/tmp/marie_riven{idx}_raw.ppm"));
    let (rgb, w, h) = preprocess_riven(pixels, width, height);
    save_ppm_rgb(&rgb, w, h, &format!("/tmp/marie_riven{idx}_proc.ppm"));

    let Some(source) = ImageSource::from_bytes(&rgb, (w, h)).ok() else { return vec![]; };
    let Some(input)  = engine.prepare_input(source).ok()          else { return vec![]; };
    let Some(words)  = engine.detect_words(&input).ok()            else { return vec![]; };
    let lines = engine.find_text_lines(&input, &words);
    let Some(texts)  = engine.recognize_text(&input, &lines).ok()  else { return vec![]; };

    texts.into_iter()
        .flatten()
        .map(|line| line.to_string())
        .filter(|s| !s.trim().is_empty())
        .collect()
}

/// Run OCR on all card strips in parallel and return one name per card.
pub async fn recognise_cards(regions: Vec<(Vec<u8>, u32, u32)>) -> Vec<String> {
    let handles: Vec<_> = regions
        .into_iter()
        .enumerate()
        .map(|(i, (pixels, w, h))| {
            tokio::task::spawn_blocking(move || {
                let result = ocr_card(&pixels, w, h, i);
                eprintln!("[ocr] card {i} ({w}×{h}): {result:?}");
                result
            })
        })
        .collect();

    let mut out = Vec::with_capacity(handles.len());
    for h in handles {
        out.push(h.await.unwrap_or_default().unwrap_or_default());
    }
    out
}

/// OCR a single card strip and return the item name (all text lines joined,
/// to handle word-wrapped names like "Vauban Prime Chassis / Blueprint").
fn ocr_card(pixels: &[u8], width: u32, height: u32, idx: usize) -> Option<String> {
    let engine = match ENGINE.as_ref() {
        Ok(e) => e,
        Err(e) => { eprintln!("[ocr] engine init failed: {e}"); return None; }
    };

    // Save the raw capture before any processing.
    save_ppm_rgba(pixels, width, height, &format!("/tmp/marie_card{idx}_raw.ppm"));

    let (rgb, w, h) = preprocess(pixels, width, height);

    // Save what the OCR engine actually sees.
    save_ppm_rgb(&rgb, w, h, &format!("/tmp/marie_card{idx}_proc.ppm"));

    let source = ImageSource::from_bytes(&rgb, (w, h)).ok()?;
    let input = engine.prepare_input(source).ok()?;
    let words = engine.detect_words(&input).ok()?;
    let lines = engine.find_text_lines(&input, &words);
    let texts = engine.recognize_text(&input, &lines).ok()?;

    let joined: String = texts
        .into_iter()
        .flatten()
        .map(|line| line.to_string())
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    if joined.trim().is_empty() { None } else { Some(joined) }
}

/// Strip alpha and upscale 3×. No colour manipulation — the text colour is
/// user-configurable so any colour-based heuristic will break for some players,
/// and the raw image already gives ocrs more signal than our previous attempts
/// at contrast enhancement.
fn preprocess(pixels: &[u8], width: u32, height: u32) -> (Vec<u8>, u32, u32) {
    let w = width as usize;
    let h = height as usize;
    let rgb: Vec<u8> = pixels.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    upscale3x_rgb(&rgb, w, h)
}

/// Riven-specific preprocessing: dark-background mod cards need inversion +
/// binarization so OCR gets clean dark-on-white glyphs. The 1/7 confusion and
/// phantom digits are caused by low-contrast gray-on-dark rendering.
///
/// Steps: strip alpha → grayscale → invert → threshold → upscale 3×.
fn preprocess_riven(pixels: &[u8], width: u32, height: u32) -> (Vec<u8>, u32, u32) {
    let w = width as usize;
    let h = height as usize;

    // Grayscale (luma), invert, then binarize at 128.
    // After inversion: originally-bright text becomes dark, dark bg becomes white.
    let gray_inv_bin: Vec<u8> = pixels.chunks(4).map(|p| {
        let luma = (p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114) / 1000;
        let inverted = 255 - luma as u8;
        if inverted >= 128 { 0 } else { 255 }
    }).collect();

    // Expand grayscale to RGB (ocrs needs 3 channels).
    let rgb: Vec<u8> = gray_inv_bin.iter().flat_map(|&v| [v, v, v]).collect();
    upscale3x_rgb(&rgb, w, h)
}

fn upscale3x_rgb(rgb: &[u8], w: usize, h: usize) -> (Vec<u8>, u32, u32) {
    let nw = w * 3;
    let nh = h * 3;
    let mut out = vec![0u8; nw * nh * 3];
    for y in 0..h {
        for x in 0..w {
            let src = &rgb[(y * w + x) * 3..(y * w + x) * 3 + 3];
            for dy in 0..3usize {
                for dx in 0..3usize {
                    let dst = ((y * 3 + dy) * nw + (x * 3 + dx)) * 3;
                    out[dst..dst + 3].copy_from_slice(src);
                }
            }
        }
    }
    (out, nw as u32, nh as u32)
}

/// Write an RGBA buffer as a plain-PPM (P6) file, dropping the alpha channel.
/// PPM needs no external crate and opens in any image viewer.
fn save_ppm_rgba(pixels: &[u8], w: u32, h: u32, path: &str) {
    use std::io::Write;
    let Ok(mut f) = std::fs::File::create(path) else { return };
    let _ = write!(f, "P6\n{w} {h}\n255\n");
    let rgb: Vec<u8> = pixels.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    let _ = f.write_all(&rgb);
    eprintln!("[ocr] saved raw  → {path}");
}

/// Write an RGB buffer as a plain-PPM (P6) file.
fn save_ppm_rgb(pixels: &[u8], w: u32, h: u32, path: &str) {
    use std::io::Write;
    let Ok(mut f) = std::fs::File::create(path) else { return };
    let _ = write!(f, "P6\n{w} {h}\n255\n");
    let _ = f.write_all(pixels);
    eprintln!("[ocr] saved proc → {path}");
}
