use image::{DynamicImage, RgbaImage};
use ocr_rs::OcrEngine;
use once_cell::sync::Lazy;

static DET_MODEL: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/models/PP-OCRv5_mobile_det.mnn"
));
static REC_MODEL: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/models/en_PP-OCRv5_mobile_rec_infer.mnn"
));
static KEYS: &[u8] = include_bytes!(concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/models/ppocr_keys_en.txt"
));

static ENGINE: Lazy<Result<OcrEngine, String>> = Lazy::new(|| {
    OcrEngine::from_bytes(DET_MODEL, REC_MODEL, KEYS, None).map_err(|e| e.to_string())
});

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

fn ocr_riven_panel(pixels: &[u8], width: u32, height: u32, idx: usize) -> Vec<String> {
    let engine = match ENGINE.as_ref() {
        Ok(e) => e,
        Err(e) => { eprintln!("[ocr] engine init failed: {e}"); return vec![]; }
    };

    save_ppm_rgba(pixels, width, height, &format!("/tmp/marie_riven{idx}_raw.ppm"));
    let (proc_pixels, w, h) = preprocess_riven(pixels, width, height);
    save_ppm_rgb(&proc_pixels, w, h, &format!("/tmp/marie_riven{idx}_proc.ppm"));

    let img = to_dynamic_image_rgb(&proc_pixels, w, h);
    match engine.recognize(&img) {
        Ok(results) => results
            .into_iter()
            .map(|r| r.text)
            .filter(|s| !s.trim().is_empty())
            .collect(),
        Err(e) => { eprintln!("[ocr] riven panel {idx} recognition failed: {e}"); vec![] }
    }
}

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

fn ocr_card(pixels: &[u8], width: u32, height: u32, idx: usize) -> Option<String> {
    let engine = match ENGINE.as_ref() {
        Ok(e) => e,
        Err(e) => { eprintln!("[ocr] engine init failed: {e}"); return None; }
    };

    save_ppm_rgba(pixels, width, height, &format!("/tmp/marie_card{idx}_raw.ppm"));
    let (proc_pixels, w, h) = preprocess(pixels, width, height);
    save_ppm_rgb(&proc_pixels, w, h, &format!("/tmp/marie_card{idx}_proc.ppm"));

    let img = to_dynamic_image_rgb(&proc_pixels, w, h);
    let results = engine.recognize(&img).ok()?;

    let joined: String = results
        .into_iter()
        .map(|r| r.text)
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    if joined.trim().is_empty() { None } else { Some(joined) }
}

fn to_dynamic_image_rgb(rgb: &[u8], w: u32, h: u32) -> DynamicImage {
    let mut rgba = Vec::with_capacity(rgb.len() / 3 * 4);
    for chunk in rgb.chunks(3) {
        rgba.extend_from_slice(chunk);
        rgba.push(255);
    }
    let buf = RgbaImage::from_raw(w, h, rgba).expect("image buffer size mismatch");
    DynamicImage::ImageRgba8(buf)
}

fn preprocess(pixels: &[u8], width: u32, height: u32) -> (Vec<u8>, u32, u32) {
    let w = width as usize;
    let h = height as usize;
    let rgb: Vec<u8> = pixels.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    upscale3x_rgb(&rgb, w, h)
}

fn preprocess_riven(pixels: &[u8], width: u32, height: u32) -> (Vec<u8>, u32, u32) {
    let w = width as usize;
    let h = height as usize;

    let gray_inv_bin: Vec<u8> = pixels.chunks(4).map(|p| {
        let luma = (p[0] as u32 * 299 + p[1] as u32 * 587 + p[2] as u32 * 114) / 1000;
        let inverted = 255 - luma as u8;
        if inverted >= 128 { 0 } else { 255 }
    }).collect();

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

fn save_ppm_rgba(pixels: &[u8], w: u32, h: u32, path: &str) {
    use std::io::Write;
    let Ok(mut f) = std::fs::File::create(path) else { return };
    let _ = write!(f, "P6\n{w} {h}\n255\n");
    let rgb: Vec<u8> = pixels.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    let _ = f.write_all(&rgb);
    eprintln!("[ocr] saved raw  → {path}");
}

fn save_ppm_rgb(pixels: &[u8], w: u32, h: u32, path: &str) {
    use std::io::Write;
    let Ok(mut f) = std::fs::File::create(path) else { return };
    let _ = write!(f, "P6\n{w} {h}\n255\n");
    let _ = f.write_all(pixels);
    eprintln!("[ocr] saved proc → {path}");
}
