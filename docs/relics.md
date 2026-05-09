# Relic Reward Overlay

## Overview

The relic overlay detects when the Warframe void fissure reward selection screen appears, identifies the four presented items, and shows their current platinum and ducat values in a transparent always-on-top window.

## Trigger Detection (EE.log)

Warframe writes to `EE.log` continuously. The string that fires when the 4-choice reward UI becomes visible is:

```
Script [Info]: Relic rewards initialized
```

This appears **before** `Got rewards`, meaning prices are displayed while the player is still choosing. The full log sequence around a relic reward:

```
854.794  Sys    [Info]: Created /Lotus/Interface/ProjectionsCountdown.swf
854.796  Script [Info]: Initialize timer true  5
854.796  Sys    [Info]: Created /Lotus/Interface/ProjectionRewardChoice.swf
859.797  Script [Info]: Countdown timer expired
859.797  Script [Info]: Relic timer closed
859.832  Script [Info]: Relic rewards initialized      ← TRIGGER
859.832  Sys    [Info]: Created /Lotus/Interface/Backgrounds/Overlay/OverlayBackground.swf
860.258  Script [Info]: Got rewards
```

The item names chosen by each player are **not** written to EE.log. OCR on the game window is required to read them.

### EE.log path

| Platform | Path |
|----------|------|
| Windows (production) | `%LOCALAPPDATA%\Warframe\EE.log` |
| Linux dev override | `$WF_EE_LOG` env var, fallback `./EE.log` |

The watcher (`src-tauri/src/ee_log.rs`) seeks to the end of the file on startup so it only reacts to new lines written during the current session.

## OCR — Screen Regions

Once the trigger fires, `src-tauri/src/screenshot.rs` captures four image regions from the primary display, one per reward card. Coordinates are defined for a 1920×1080 reference and scaled to the actual display resolution.

| Card | x (ref) | y (ref) | w (ref) | h (ref) |
|------|---------|---------|---------|---------|
| 1 | 60 | 310 | 400 | 70 |
| 2 | 540 | 310 | 400 | 70 |
| 3 | 1020 | 310 | 400 | 70 |
| 4 | 1500 | 310 | 400 | 70 |

These coordinates were derived from WFinfo's OCR region analysis of the Warframe 1080p UI. They may need adjustment if DE changes the reward screen layout.

`src-tauri/src/ocr.rs` runs Windows OCR (`Windows.Media.Ocr`) on each region via `SoftwareBitmap` (RGBA8 format). Only the **first non-empty line** of each result is kept — the item name. The remainder (rarity text, percentage) is discarded.

On non-Windows builds OCR returns empty strings; use `test_trigger` from the main window to simulate a reward event.

## Item Name Resolution

After OCR, raw strings are fuzzy-matched against the WFM items cache using **Jaro-Winkler similarity** (threshold 0.70). This tolerates common OCR mis-reads (e.g. `Ash Prim3 Blueprint` → `Ash Prime Blueprint`).

The cache is built from the WFM `/v2/items` response on startup and held in a `tokio::sync::RwLock<HashMap<String, CachedItem>>`.

## Warframe Market API

### Items cache — ducat values and slug mapping

```
GET https://api.warframe.market/v2/items
Header: Language: en
```

Relevant response fields per item:

```jsonc
{
  "slug": "ash_prime_blueprint",  // used as URL key for orders
  "ducats": 45,                   // null for non-prime / non-tradeable items
  "i18n": {
    "en": { "name": "Ash Prime Blueprint" }
  }
}
```

The cache maps **lowercased English name → `{ slug, ducats }`**. Loaded once on startup; no TTL (restart to refresh).

### Live platinum prices — top orders

```
GET https://api.warframe.market/v2/orders/item/{slug}/top
Header: Language: en
```

Relevant response fields:

```jsonc
{
  "data": {
    "sell": [
      { "platinum": 25, "quantity": 1, "visible": true }
    ]
  }
}
```

`plat_min_sell` is taken as `min(sell[*].platinum)`. The call is made per-item on every relic reward detection event; results are not cached between events.

## Overlay Window

The overlay is a second Tauri window (`label: "overlay"`) configured in `tauri.conf.json`:

| Property | Value |
|----------|-------|
| transparent | true |
| decorations | false |
| alwaysOnTop | true |
| visible | false (shown on trigger) |
| skipTaskbar | true |
| size | 1920 × 180 px |
| position | 0, 0 (top-left) |

On mount the overlay calls `window.setIgnoreCursorEvents(true)` so mouse clicks pass through to the game. The window auto-hides after **30 seconds**. The card with the lowest platinum price is highlighted in gold.

## Data Flow

```
EE.log "Relic rewards initialized"
    │
    ▼
ee_log::start_watcher  →  app.emit("relic-trigger")
    │
    ▼  (overlay window receives event)
invoke("detect_relic_rewards")
    │
    ├─ screenshot::capture_reward_regions()   [4× capture_area]
    ├─ ocr::recognise_regions()               [4× Windows OCR]
    └─ wfm::prices_for_names()
           ├─ fuzzy-match name → slug + ducats  (cache)
           └─ fetch plat price  (live API × 4)
    │
    ▼
overlay renders 4 ItemPriceResult cards
auto-hides after 30 s
```

## Known Limitations & Future Work

- OCR region coordinates are hardcoded for 1920×1080; other resolutions use linear scaling which may be imprecise.
- OCR accuracy depends on the Windows language pack. The engine uses `OcrEngine::TryCreateFromUserProfileLanguages()` which picks the first installed language.
- Plat price shows the raw minimum sell order; it does not filter for in-game or online sellers.
- The WFM items cache is never refreshed while the app is running; restart to pick up newly added items.
- No handling yet for the Baro Ki'Teer (non-standard reward screen) or Arbitration rewards.
