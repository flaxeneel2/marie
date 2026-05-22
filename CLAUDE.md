# CLAUDE.md

Marie is a Warframe companion overlay — reads `EE.log` and uses OCR to show relic drop prices in platinum and ducats. Planned future modules: riven roll calculator, mastery calculator.

**Two build variants:**

- **Standard build** (default): no DLL injection, no memory reading. EAC-safe. All game data from log parsing or screen capture only.
- **Memory build** (`--features memory`, Linux only, must run as root): reads `/proc/<pid>/mem` without ptrace to extract the authentication nonce; unlocks account-gated features (mastery overview, etc.). See [docs/memory.md](docs/memory.md).

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
- [Memory feature](docs/memory.md) — `/proc/<pid>/mem` nonce scan, root requirement, EAC safety, account data flow
- [Riven memory](docs/riven-memory.md) — external riven detection via pattern scan of Warframe .text, IPC tags, struct layout (partial)
- [Inventory feature](docs/inventory.md) — Warframe inventory API, BSON types, 5-min disk cache, items_cache (7-day TTL), DisplayItem/InventoryView pipeline, blueprint name resolution
- [Mod cards](docs/mod-cards.md) — mod-card-plugin.ts (Vite POST endpoint), @wfcd/mod-generator, genesis-assets symlinks, font fix, ModCard.svelte hover overlay
- [UI layout](docs/ui.md) — sidebar nav, inventory sub-tabs, relic tier filter, item image rendering, mod grid
