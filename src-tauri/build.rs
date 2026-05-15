use std::{fs, io::Read, path::Path};

const MODEL_BASE: &str =
    "https://github.com/zibo-chen/rust-paddle-ocr/raw/next/models";

const MODELS: &[(&str, u64); 3] = &[
    ("PP-OCRv5_mobile_det.mnn",         1024 * 1024),
    ("en_PP-OCRv5_mobile_rec_infer.mnn", 1024 * 1024),
    ("ppocr_keys_en.txt",               512),
];

fn main() {
    tauri_build::build();

    let models_dir = Path::new("models");
    fs::create_dir_all(models_dir).expect("failed to create models/");

    for (name, min_size) in MODELS {
        let path = models_dir.join(name);
        let too_small = fs::metadata(&path)
            .map(|m| m.len() < *min_size)
            .unwrap_or(true);
        if too_small {
            println!("cargo:warning=Downloading OCR model: {name}");
            let url = format!("{MODEL_BASE}/{name}");
            download(&url, &path);
        }
    }
}

fn download(url: &str, dest: &Path) {
    let mut body = Vec::new();
    ureq::get(url)
        .call()
        .unwrap_or_else(|e| panic!("failed to download {url}: {e}"))
        .into_reader()
        .read_to_end(&mut body)
        .expect("failed to read download body");
    fs::write(dest, body).expect("failed to write model file");
}
