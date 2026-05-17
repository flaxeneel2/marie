# Architecture

## Stack

- **Frontend**: SvelteKit v2 + Svelte 5 (runes), TypeScript, Vite, `adapter-static` (SPA, `ssr = false`)
- **Backend**: Rust + Tauri v2
- **Package manager**: Bun
- **Dev environment**: Nix (flake.nix + common.nix) with direnv

## Tauri windows

| label | purpose | transparent | always-on-top | starts visible |
|-------|---------|-------------|---------------|----------------|
| `main` | Control panel — EE.log path, test buttons | no | no | yes |
| `overlay` | Price overlay — full monitor, layer-shell surface | yes | yes (layer) | yes (always) |

The overlay is always present at the compositor level; Svelte's `{#if visible}` controls whether any content is rendered. On Linux/Wayland it is a wlr-layer-shell `OVERLAY`-layer surface — no window rules needed, input passthrough and z-order above fullscreen are handled by the protocol. See [docs/linux-wayland.md](linux-wayland.md). Content auto-hides after 30 seconds.

## Rust modules (`src-tauri/src/`)

| file | responsibility |
|------|---------------|
| `lib.rs` | Tauri commands, app setup, overlay interactivity, keybind config, portal listener |
| `ee_log.rs` | Tails EE.log; emits `relic-trigger` on `"Relic rewards initialized"` |
| `warframe_window.rs` | Finds Warframe's window and returns its geometry in physical pixels |
| `screenshot.rs` | Captures N reward-card regions or the riven stat region from the screen |
| `ocr.rs` | Runs ocr-rs neural-net OCR on capture regions; returns text lines |
| `wfm.rs` | WFM items cache (startup fetch, `RwLock<HashMap>`); live plat price fetch; Jaro-Winkler fuzzy matching |
| `riven.rs` | Riven data model, grading algorithm, stat weights, fake data for tests |

## Tauri commands

| command | called by | returns |
|---------|-----------|---------|
| `detect_relic_rewards(player_count: u32)` | overlay on `relic-trigger` | `Vec<ItemPriceResult>` |
| `test_trigger` | main window button | — (emits `relic-trigger`) |
| `show_test_overlay` | main window button | — (emits `relic-test-data` with fake data) |
| `ee_log_path` | main window on mount | `String` |
| `capture_current_riven` | overlay on `riven-reroll` (first capture) | `RivenRollGrade` |
| `grade_riven_reroll` | overlay on `riven-reroll` (subsequent rolls) | `RivenRerollResult` |
| `test_riven_trigger` | main window button | — (emits `riven-reroll`) |
| `show_test_riven_overlay` | main window button | — (emits `riven-test-data`) |
| `disable_overlay_interaction` | overlay close button | — |
| `get_interaction_shortcut` | main window on mount | `String` (e.g. `"CTRL SHIFT, i"`) |
| `set_interaction_shortcut(mods_key)` | main window Apply button | `Result<(), String>` |

## IPC / event flow

**Real trigger (EE.log):**
```
EE.log "Relic rewards initialized"
  → ee_log::start_watcher detects squad size (fallback: 4)
  → emits relic-trigger with player_count: u32
  → overlay sets visible = true, calls detect_relic_rewards(player_count)
  → screenshot (N strips) → OCR (N cards) → wfm cache lookup + live plat fetch
  → N ItemPriceResult cards rendered (dynamic grid)
  → auto-hides after 30 s
```

**Test overlay (fake data):**
```
main window "Test overlay" button
  → invoke show_test_overlay
  → Rust: emit relic-test-data with hardcoded items
  → overlay renders 4 cards immediately (no OCR / network)
  → Rust: Tokio timer fires after 5 s → emit hide-overlay
  → overlay hides
```

## Key environment variable

`WEBKIT_DISABLE_DMABUF_RENDERER=1` is set in the Nix shell to prevent WebKit crashes on Linux — do not remove this from `common.nix`.
