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

/// Run OCR on all card strips in parallel and return one name per card.
pub async fn recognise_cards(regions: Vec<(Vec<u8>, u32, u32)>) -> Vec<String> {
    // Spawn all blocking tasks before awaiting any — they run concurrently on
    // the Tokio blocking thread pool.
    let handles: Vec<_> = regions
        .into_iter()
        .enumerate()
        .map(|(i, (pixels, w, h))| {
            tokio::task::spawn_blocking(move || {
                let result = ocr_card(&pixels, w, h);
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
fn ocr_card(pixels: &[u8], width: u32, height: u32) -> Option<String> {
    let engine = match ENGINE.as_ref() {
        Ok(e) => e,
        Err(e) => { eprintln!("[ocr] engine init failed: {e}"); return None; }
    };

    // Invert luminance: Warframe UI is white-on-dark; ocrs trained on dark-on-light.
    let rgb: Vec<u8> = pixels
        .chunks(4)
        .flat_map(|p| [255 - p[0], 255 - p[1], 255 - p[2]])
        .collect();

    let source = ImageSource::from_bytes(&rgb, (width, height)).ok()?;
    let input = engine.prepare_input(source).ok()?;
    let words = engine.detect_words(&input).ok()?;
    let lines = engine.find_text_lines(&input, &words);
    let texts = engine.recognize_text(&input, &lines).ok()?;

    // Join all recognised lines — handles two-row word-wrapped names.
    let joined: String = texts
        .into_iter()
        .flatten()
        .map(|line| line.to_string())
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>()
        .join(" ");

    if joined.trim().is_empty() { None } else { Some(joined) }
}
