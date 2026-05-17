# CLAUDE.md

Marie is a Warframe companion overlay — reads `EE.log` and uses OCR to show relic drop prices in platinum and ducats. Planned future modules: riven roll calculator, mastery calculator.

**Critical constraint**: No DLL injection, no memory reading. EAC must not be triggered. All game data from log parsing or screen capture only.

## Commands

All frontend commands use **bun** (not npm/yarn).

```bash
bun run tauri dev     # dev (Vite + Tauri)
bun run tauri build   # production build
bun run dev           # frontend only (browser)
bun run check         # TypeScript + Svelte type check
bun run check:watch   # watch mode
```

No test infrastructure exists yet.

## Docs

- [Architecture](docs/architecture.md) — stack, Tauri windows, Rust modules, IPC flow
- [OCR & screenshots](docs/ocr.md) — capture pipeline, focus check, ocrs model loading
- [Relics feature](docs/relics.md) — EE.log trigger, WFM API, overlay behaviour, known limitations
- [Conventions](docs/conventions.md) — Svelte 5 runes, Tauri config, ports, platform split patterns
- [Linux / Wayland](docs/linux-wayland.md) — Hyprland window rules, Wayland overlay constraints and workarounds
- [Rivens feature](docs/rivens.md) — riven data model, grading algorithm, stat weights, WFM riven API, OCR challenges
- [Overlay interactivity](docs/overlay-interactivity.md) — XDG GlobalShortcuts portal, hyprctl keybind injection, GTK input shape, keybind recorder UI
