# Relic Reward Overlay

## Overview

The relic overlay detects when the Warframe void fissure reward selection screen appears, identifies the presented items (1–4 depending on squad size), and shows their current platinum and ducat values in a transparent always-on-top window.

## Trigger Detection (EE.log)

Warframe writes to `EE.log` continuously. The string that fires when the reward UI becomes visible is:

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
| Linux (Proton default) | `~/.local/share/Steam/steamapps/compatdata/230410/pfx/drive_c/users/steamuser/AppData/Local/Warframe/EE.log` |
| Linux dev override | `$WF_EE_LOG` env var |

The watcher (`src-tauri/src/ee_log.rs`) seeks to the end of the file on startup so it only reacts to new lines written during the current session.

## Squad Size Detection

`ee_log.rs` maintains a rolling buffer of the last 200 log lines. When the trigger fires, it scans recent lines for known player-count patterns:

| Log pattern (case-insensitive) | Example |
|-------------------------------|---------|
| `relic reward choices: N` | `Net [Info]: relic reward choices: 3` |
| `playercount` followed by digits | `Script [Info]: PlayerCount = 2` |

If no pattern matches, squad size defaults to **4** (same as the legacy behaviour). The detected count is emitted as the `relic-trigger` event payload (`u32`) so the frontend can pass it to `detect_relic_rewards`.

> If the patterns don't match your EE.log version, adjust `detect_squad_size` in `ee_log.rs`.

## OCR — Screen Regions

Once the trigger fires, `src-tauri/src/screenshot.rs` captures N image strips from the primary display (N = squad size, 1–4). Each strip covers only the item-name text row of one reward card.

Proportional bounds are measured on a 2560×1440 reference and scaled to the actual game window at runtime:

| Boundary | Value |
|----------|-------|
| x start  | 635 / 2560 ≈ 24.8% of window width |
| x end    | 1920 / 2560 = 75% of window width |
| y start  | 550 / 1440 ≈ 38.2% of window height |
| y end    | 612 / 1440 ≈ 42.5% of window height |

The x range is divided equally into N strips (one per card). For detail on the OCR engine, pixel pipeline, and platform capture implementations, see [docs/ocr.md](ocr.md).

## Item Name Resolution

After OCR, raw strings are fuzzy-matched against the WFM items cache using **Jaro-Winkler (60%) + Jaccard word-token similarity (40%)** with a combined threshold of 0.75. This tolerates common OCR mis-reads (e.g. `Ash Prim3 Blueprint` → `Ash Prime Blueprint`). A word-count gate (±1 word) prevents short OCR fragments from matching multi-word names.

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
| visible | true (always — see linux-wayland.md) |
| skipTaskbar | true |
| size | resized to match Warframe's physical window |
| position | repositioned to match Warframe's screen origin |

On mount the overlay calls `setIgnoreCursorEvents(true)` (fire-and-forget, not awaited) so mouse clicks pass through to the game. Content auto-hides after **30 seconds**. The card with the highest platinum value is highlighted in gold. The card grid uses `grid-template-columns: repeat(N, 1fr)` where N is the number of items returned, so it adapts automatically to 1–4 players.

## Data Flow

```
EE.log "Relic rewards initialized"
    │
    ▼
ee_log::start_watcher
    ├─ scan recent lines for squad size (fallback: 4)
    └─ app.emit("relic-trigger", player_count)
    │
    ▼  (overlay window receives event)
playerCount = event.payload
invoke("detect_relic_rewards", { playerCount })
    │
    ├─ screenshot::capture_card_strips(…, player_count)  [N captures]
    ├─ ocr::recognise_cards(regions)                     [N × ocrs inference]
    └─ wfm::prices_for_names(names)
           ├─ fuzzy-match name → slug + ducats  (cache)
           └─ fetch plat price  (live API × N)
    │
    ▼
overlay renders N ItemPriceResult cards (dynamic grid)
auto-hides after 30 s
```

## Known Limitations & Future Work

- Squad size detection from EE.log is best-effort; if the log patterns don't match, it falls back to 4. The patterns in `detect_squad_size` (`ee_log.rs`) may need adjustment after game updates.
- OCR region coordinates are calibrated for 2560×1440; other resolutions use proportional scaling which may be slightly off at unusual aspect ratios.
- Plat price shows the raw minimum sell order; it does not filter for in-game or online sellers.
- The WFM items cache is never refreshed while the app is running; restart to pick up newly added items.
- No handling yet for Baro Ki'Teer (non-standard reward screen) or Arbitration rewards.
- `focus.rs` exists and checks whether Warframe is focused but is not yet wired into `detect_relic_rewards` — captures proceed regardless of window focus.
