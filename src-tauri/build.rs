use std::{fs, io::Read, path::Path};

const MODELS: &[(&str, &str)] = &[
    (
        "text-detection.rten",
        "https://ocrs-models.s3.ap-southeast-2.amazonaws.com/text-detection.rten",
    ),
    (
        "text-recognition.rten",
        "https://ocrs-models.s3.ap-southeast-2.amazonaws.com/text-recognition.rten",
    ),
];

fn main() {
    tauri_build::build();

    let models_dir = Path::new("models");
    fs::create_dir_all(models_dir).expect("failed to create models/");

    for (name, url) in MODELS {
        let path = models_dir.join(name);
        if !path.exists() {
            println!("cargo:warning=Downloading OCR model: {name}");
            download(url, &path);
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
