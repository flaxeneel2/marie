# OCR & Screenshot Pipeline

## Overview

```
focus check → full-screen capture → crop 4 regions → ocrs inference → item name strings
```

## Warframe Window Focus Check (`focus.rs`)

Before any capture, `focus::is_warframe_focused()` checks that Warframe is the active window. If it is not (player alt-tabbed), `detect_relic_rewards` returns an error immediately and no screenshot is taken.

The check fails open — if the platform check is unavailable, capture proceeds normally.

| Platform | Implementation |
|----------|---------------|
| Windows | `GetForegroundWindow` + `GetWindowTextW` — checks title contains "Warframe" |
| Linux | `x11rb` queries `_NET_ACTIVE_WINDOW` then `_NET_WM_NAME` via X11. Works for Warframe running under XWayland. Returns `None` (→ allow) on pure Wayland sessions without XWayland. |

## Screen Capture (`screenshot.rs`)

`capture_reward_regions()` is async on all platforms. Returns `Vec<(rgba: Vec<u8>, w: u32, h: u32)>` — one entry per reward card.

### Linux — xdg-desktop-portal (`ashpd` crate)

Uses the `org.freedesktop.portal.Screenshot` D-Bus portal:
1. Sends a screenshot request with `interactive = false`
2. Portal saves a full-screen PNG to a temp file and returns the `file://` URI
3. We load the PNG with the `image` crate, derive scale factors from its dimensions, and crop to the 4 reward regions
4. Temp file is deleted after loading

Works on GNOME, KDE, and all wlroots compositors (Sway, Hyprland). The first request per session may show an OS permission prompt.

### Windows — direct region capture (`screenshots` crate)

Calls `Screen::capture_area(x, y, w, h)` once per region — no full-screen intermediary. The `screenshots` crate uses GDI/DXGI internally.

## Reward Region Coordinates

Hardcoded for 1920×1080; linearly scaled to actual display resolution via scale factors `(screen_w / 1920, screen_h / 1080)`.

| Card | x | y | w | h |
|------|---|---|---|---|
| 1 | 60 | 310 | 400 | 70 |
| 2 | 540 | 310 | 400 | 70 |
| 3 | 1020 | 310 | 400 | 70 |
| 4 | 1500 | 310 | 400 | 70 |

Derived from WFinfo's analysis of the Warframe 1080p reward screen layout. May need adjustment if DE changes the UI.

## OCR Engine (`ocr.rs`)

Cross-platform pure-Rust OCR using [`ocrs`](https://github.com/robertknight/ocrs) (RTen neural-network inference).

### Model loading

Two `.rten` model files are downloaded at first build by `build.rs` (via `ureq`) into `src-tauri/models/` (gitignored), then embedded into the binary with `include_bytes!`:

| File | Size | Role |
|------|------|------|
| `text-detection.rten` | ~5 MB | Locates text word bounding boxes |
| `text-recognition.rten` | ~8 MB | Reads text from each located region |

The `OcrEngine` is initialised once at first use via `once_cell::Lazy` and reused across all subsequent calls. Model download URL: `https://ocrs-models.s3.ap-southeast-2.amazonaws.com/`

### Pixel pipeline

```
RGBA u8 pixels
  → grayscale f32, manual (0.299R + 0.587G + 0.114B) / 255
  → NdTensor<f32, 3> shape [1, H, W]  (CHW layout)
  → OcrEngine::prepare_input
  → detect_words → find_text_lines → recognize_text
  → first non-empty TextLine → String
```

### Accuracy notes

Warframe's reward card text is high-contrast white-on-dark at a consistent size — a good fit for `ocrs`'s neural-net approach. Jaro-Winkler fuzzy matching in `wfm.rs` (threshold 0.70) corrects residual OCR mis-reads when matching against WFM item names.
