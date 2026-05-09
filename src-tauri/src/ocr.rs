use ocrs::{ImageSource, OcrEngine, OcrEngineParams};
use once_cell::sync::Lazy;
use rten::Model;

// Models are downloaded at build time by build.rs and embedded into the binary.
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

/// Runs OCR on a list of RGBA image regions and returns recognised text for each.
pub async fn recognise_regions(regions: Vec<(Vec<u8>, u32, u32)>) -> Vec<String> {
    let mut out = Vec::with_capacity(regions.len());
    for (pixels, w, h) in regions {
        eprintln!("[ocr] region {w}×{h}, {} bytes", pixels.len());
        out.push(recognise_one(pixels, w, h).await);
    }
    eprintln!("[ocr] final: {out:?}");
    out
}

async fn recognise_one(pixels: Vec<u8>, width: u32, height: u32) -> String {
    tokio::task::spawn_blocking(move || ocr_pixels(&pixels, width, height))
        .await
        .unwrap_or_default()
        .unwrap_or_default()
}

fn ocr_pixels(pixels: &[u8], width: u32, height: u32) -> Option<String> {
    let engine = match ENGINE.as_ref() {
        Ok(e) => e,
        Err(e) => {
            eprintln!("[ocr] engine init failed: {e}");
            return None;
        }
    };

    // Drop alpha, invert luminance — Warframe UI is white-on-dark;
    // ocrs was trained on dark-on-light (document) images.
    let rgb: Vec<u8> = pixels
        .chunks(4)
        .flat_map(|p| [255 - p[0], 255 - p[1], 255 - p[2]])
        .collect();

    let source = match ImageSource::from_bytes(&rgb, (width, height)) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("[ocr] ImageSource::from_bytes failed ({width}×{height}, {} bytes): {e}", rgb.len());
            return None;
        }
    };

    let input = match engine.prepare_input(source) {
        Ok(i) => i,
        Err(e) => { eprintln!("[ocr] prepare_input failed: {e}"); return None; }
    };

    let words = match engine.detect_words(&input) {
        Ok(w) => w,
        Err(e) => { eprintln!("[ocr] detect_words failed: {e}"); return None; }
    };
    eprintln!("[ocr] detected {} words in {width}×{height} region", words.len());

    let lines = engine.find_text_lines(&input, &words);
    eprintln!("[ocr] found {} lines", lines.len());

    let texts = match engine.recognize_text(&input, &lines) {
        Ok(t) => t,
        Err(e) => { eprintln!("[ocr] recognize_text failed: {e}"); return None; }
    };

    // Return only the first recognised line — the item name.
    let result = texts
        .into_iter()
        .flatten()
        .next()
        .map(|line| line.to_string())
        .filter(|s| !s.trim().is_empty());

    eprintln!("[ocr] result: {result:?}");
    result
}
