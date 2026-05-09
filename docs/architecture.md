# Architecture

## Stack

- **Frontend**: SvelteKit v2 + Svelte 5 (runes), TypeScript, Vite, `adapter-static` (SPA, `ssr = false`)
- **Backend**: Rust + Tauri v2
- **Package manager**: Bun
- **Dev environment**: Nix (flake.nix + common.nix) with direnv

## Tauri windows

| label | purpose | transparent | always-on-top | starts visible |
|-------|---------|-------------|---------------|----------------|
| `main` | Control panel — EE.log path, test trigger | no | no | yes |
| `overlay` | Price overlay (1920×180 px, top of screen) | yes | yes | no |

The overlay calls `window.setIgnoreCursorEvents(true)` on mount so clicks pass through to the game. Auto-hides after 30 seconds.

## Rust modules (`src-tauri/src/`)

| file | responsibility |
|------|---------------|
| `lib.rs` | Tauri commands, app setup, wires all modules |
| `ee_log.rs` | Tails EE.log; emits `relic-trigger` on `"Relic rewards initialized"` |
| `focus.rs` | Active-window check — skips capture if Warframe is not focused |
| `screenshot.rs` | Full-screen capture via xdg-desktop-portal (Linux) or direct region capture (Windows) |
| `ocr.rs` | Runs ocrs neural-net OCR on each reward region; returns first text line |
| `wfm.rs` | WFM items cache (startup fetch, `RwLock<HashMap>`); live plat price fetch; Jaro-Winkler fuzzy matching |

## Tauri commands

| command | called by | returns |
|---------|-----------|---------|
| `detect_relic_rewards` | overlay on `relic-trigger` | `Vec<ItemPriceResult>` |
| `test_trigger` | main window button | — (emits `relic-trigger`) |
| `ee_log_path` | main window on mount | `String` |

## IPC / event flow

```
EE.log "Relic rewards initialized"
  → ee_log::start_watcher emits relic-trigger
  → overlay +page.svelte calls detect_relic_rewards
  → focus check → screenshot → OCR → wfm cache lookup + live plat fetch
  → 4 ItemPriceResult cards rendered, window shown
  → auto-hides after 30 s
```

## Key environment variable

`WEBKIT_DISABLE_DMABUF_RENDERER=1` is set in the Nix shell to prevent WebKit crashes on Linux — do not remove this from `common.nix`.
