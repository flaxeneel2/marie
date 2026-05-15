# OCR & Screenshot Pipeline

## Overview

```
focus check → N region captures → pixel preprocessing → ocr-rs inference → item name strings
```

`N` is the squad size (1–4), determined by `ee_log.rs` and passed through to the screenshot stage.

## Warframe Window Focus Check (`focus.rs`)

`focus::is_warframe_focused()` checks whether Warframe is the foreground window before any capture is attempted. If it returns `false`, `detect_relic_rewards` can proceed anyway — the check is advisory and fails open so that environments where active-window queries are unavailable (e.g. pure Wayland without XWayland) don't silently suppress captures.

| Platform | Implementation |
|----------|---------------|
| Windows | `GetForegroundWindow` + `GetWindowTextW` — title must contain "Warframe" |
| Linux | `x11rb` queries `_NET_ACTIVE_WINDOW` → `_NET_WM_NAME` via X11. Returns `None` (→ allow) if X11 is unreachable or pure Wayland session. |

> **Note**: `focus.rs` exists and compiles on all platforms but is not yet wired into `detect_relic_rewards` in `lib.rs`. It is available for future use.

## Screen Capture (`screenshot.rs`)

`capture_card_strips(win_x, win_y, win_w, win_h, player_count)` is async on all platforms.  
Returns `Vec<(rgba: Vec<u8>, w: u32, h: u32)>` — one entry per reward card (length = `player_count`).

### Reward Region Coordinates

Proportional bounds measured on a 2560×1440 reference display; scaled to the actual game window size at runtime.

| Dimension | Fraction of window | Notes |
|-----------|-------------------|-------|
| x start   | 635 / 2560 ≈ 24.8% | Left edge of leftmost card name text |
| x end     | 1920 / 2560 = 75%   | Right edge of rightmost card name text |
| y start   | 550 / 1440 ≈ 38.2% | Top of item name text row |
| y end     | 612 / 1440 ≈ 42.5% | Bottom of item name text row |

The full x range is divided into `player_count` equal strips. Card centres for each count:

| Players | Card x centres (% of range) |
|---------|------------------------------|
| 1       | 50% |
| 2       | 25%, 75% |
| 3       | ~17%, 50%, ~83% |
| 4       | 12.5%, 37.5%, 62.5%, 87.5% |

### Linux — `grim` command-line tool

```
grim -g "X,Y WxH" -
```

Captures a single region as a PNG written to stdout. The `image` crate loads it into an RGBA buffer.  
Works on wlroots compositors (Hyprland, Sway). Requires `grim` to be installed.

### Windows — direct region capture (`screenshots` crate)

`Screen::capture_area(x, y, w, h)` — one call per card strip. Uses GDI/DXGI internally; no full-screen intermediary.

## OCR Engine (`ocr.rs`)

Cross-platform OCR using [`ocr-rs`](https://crates.io/crates/ocr-rs) — a Rust wrapper around PaddleOCR with MNN inference. The same code path runs on all platforms.

### Model loading

Three model files are downloaded at first build by `build.rs` (via `ureq`) into `src-tauri/models/` (gitignored), then embedded into the binary with `include_bytes!`:

| File | Size | Role |
|------|------|------|
| `PP-OCRv5_mobile_det.mnn` | ~4.6 MB | Detects text region bounding boxes |
| `en_PP-OCRv5_mobile_rec_infer.mnn` | ~3.8 MB | Recognises English text from cropped regions |
| `ppocr_keys_en.txt` | ~1.4 KB | Character set index for the recognition model |

Models are loaded once at first use via `once_cell::Lazy<OcrEngine>` and reused for every call.

**Build requirement**: `bindgen` (used by `ocr-rs`) requires `libclang` at build time. In the Nix dev shell `LIBCLANG_PATH` is set automatically via `shellHook`.

### Pixel pipeline

```
RGBA u8 pixels (from grim / screenshots crate)
  → preprocess: strip alpha → RGB, upscale 3×
      riven panels additionally: grayscale → invert → binarize (dark bg → white bg)
  → image::DynamicImage::ImageRgba8
  → OcrEngine::recognize(&img)
      internally: PP-OCRv5 detect → crop text regions → PP-OCRv5 English rec
  → Vec<OcrResult> text fields joined with " "
  → one String per card (empty string if no text detected)
```

All card strips are processed in parallel via `tokio::task::spawn_blocking` (one task per strip), running on the Tokio blocking thread pool.

### Accuracy notes

PP-OCRv5 with the English-specific recognition model improves accuracy over the previous `ocrs` models. Multi-line item names (e.g. "Vauban Prime / Chassis Blueprint" word-wrapped across two rows) are handled by joining all recognised text results. Jaro-Winkler + Jaccard fuzzy matching in `wfm.rs` (threshold 0.75) corrects residual OCR mis-reads.
