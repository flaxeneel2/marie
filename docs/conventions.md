# Conventions

## Frontend

- Svelte 5 runes (`$state`, `$derived`, `$effect`) — not legacy `$:` reactive syntax
- Vite dev server must run on port **1420** (hardcoded in Tauri config)
- New frontend routes that need a Tauri window need `export const prerender = true` and `export const ssr = false` in their `+page.ts`

## Tauri

- App identifier: `net.flaxeneel2.marie`
- New Tauri capabilities (permissions) go in `src-tauri/capabilities/`
- Both windows (`main` and `overlay`) must be listed in `capabilities/default.json`

## Wayland / overlay window

- The overlay window must be `"visible": true` in `tauri.conf.json`. `show()`/`hide()` are no-ops on Wayland XDG shell. Use Svelte `{#if visible}` to toggle content instead.
- Never `await win.setIgnoreCursorEvents(true)` — on Wayland this call never returns a reply, hanging the caller indefinitely.
- Never park the overlay off-screen (e.g. at `(-100000, -100000)`) — WebKitGTK suspends rendering for windows outside the visible area.
- See [docs/linux-wayland.md](linux-wayland.md) for required Hyprland window rules.

## Rust

- `screenshots` and `windows` crates are `[target.'cfg(windows)'.dependencies]`
- `ashpd`, `image`, `x11rb` are `[target.'cfg(not(windows))'.dependencies]`
- `ocrs`, `rten`, `rten_tensor` are general deps (same code path on all platforms)
- Platform splits use `#[cfg(windows)]` / `#[cfg(not(windows))]` module pattern inside a single file rather than separate files

## Nix / environment

- `WEBKIT_DISABLE_DMABUF_RENDERER=1` must stay in `common.nix` — prevents WebKit DMA-BUF crashes on Linux
- ocrs model files live in `src-tauri/models/` (gitignored, downloaded by `build.rs` on first build)
