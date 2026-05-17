# Linux / Wayland Notes

Marie targets wlr-layer-shell compositors (Hyprland, Sway, river, niri, …). The overlay is a
layer-shell surface rather than a normal XDG-shell window, so **no window rules are required**.

## How the overlay surface works

At startup, `lib.rs::init_layer_shell()` converts the Tauri GTK window into a wlr-layer-shell
surface before it is first shown:

| Setting | Value | Why |
|---------|-------|-----|
| Layer | `OVERLAY` | Rendered above fullscreen clients (e.g. Warframe) |
| Anchors | all 4 edges | Fills the entire monitor |
| Exclusive zone | `-1` | Doesn't push panels or taskbars |
| Keyboard mode | `None` | Never steals keyboard focus |
| Namespace | `marie-overlay` | Identifies the surface to the compositor |

`gtk_layer_shell::is_supported()` is checked at runtime. If layer shell is not available (e.g.
a non-wlr compositor), a warning is printed and the surface falls back to a plain XDG-shell
window — but that path is not officially supported.

## `visible: true` in tauri.conf.json

The overlay window is configured with `"visible": true`. The overlay is **always present at the
compositor level**; the Svelte `{#if visible}` block controls whether any content is rendered
inside it. This is simpler than show/hide cycling and avoids any compositor-side quirks.

## JS dismiss timer throttling

WebKitGTK throttles `setTimeout` on windows that never receive keyboard focus. Since layer shell
with `KeyboardMode::None` guarantees the overlay never gets focus, the `show_test_overlay`
dismiss timer runs in a Tokio task on the Rust side and emits a `hide-overlay` event instead
of relying on a JS timer. The real trigger path (`showOverlay()`) still uses `setTimeout` for
the 30 s auto-hide — if that proves unreliable in practice, move it to Tokio as well.

## `WEBKIT_DISABLE_DMABUF_RENDERER=1`

Set in `common.nix` via `shellHook`. Prevents WebKitGTK crashes on Linux when the DMA-BUF
renderer is active — do not remove.

## Overlay interactivity

By default the overlay surface is fully click-through (`KeyboardMode::None`, empty input shape
region). A global hotkey — injected into Hyprland via `hyprctl keyword bind` at startup — toggles
it to interactive mode, allowing the user to select text from overlay panels.

Full details including the XDG GlobalShortcuts portal flow, keybind injection, and CSS
pointer-events handling: [docs/overlay-interactivity.md](overlay-interactivity.md).

## Multi-monitor

The layer-shell surface is assigned to whichever output the compositor picks (typically the
primary monitor). On multi-monitor setups where Warframe runs on a secondary display, the
overlay will appear on the wrong screen. A fix would be to call
`gtk_layer_shell::set_monitor()` at trigger time, using the monitor derived from
`find_warframe_geometry()`. This is not implemented yet.
