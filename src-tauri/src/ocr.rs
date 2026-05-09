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
        out.push(recognise_one(pixels, w, h).await);
    }
    out
}

async fn recognise_one(pixels: Vec<u8>, width: u32, height: u32) -> String {
    tokio::task::spawn_blocking(move || ocr_pixels(&pixels, width, height))
        .await
        .unwrap_or_default()
        .unwrap_or_default()
}

fn ocr_pixels(pixels: &[u8], width: u32, height: u32) -> Option<String> {
    let engine = ENGINE.as_ref().ok()?;

    // Drop alpha channel — ocrs expects interleaved RGB (HWC), 3 bytes per pixel.
    let rgb: Vec<u8> = pixels.chunks(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
    let source = ImageSource::from_bytes(&rgb, (width, height)).ok()?;

    let input = engine.prepare_input(source).ok()?;
    let words = engine.detect_words(&input).ok()?;
    let lines = engine.find_text_lines(&input, &words);
    let texts = engine.recognize_text(&input, &lines).ok()?;

    // Return only the first recognised line — the item name.
    texts
        .into_iter()
        .flatten()
        .next()
        .map(|line| line.to_string())
        .filter(|s| !s.trim().is_empty())
}
