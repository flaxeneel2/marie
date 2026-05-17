# Overlay Interactivity & Global Hotkey

Marie's overlay is normally fully click-through (pointer-events pass to the game beneath it).
A hotkey toggles it into interactive mode so the user can select/copy text from riven cards or
interact with overlay content.

## Architecture overview

```
Hyprland keybind (injected via hyprctl)
  → Hyprland global-shortcuts-v1 Wayland protocol
    → xdg-desktop-portal-hyprland
      → XDG GlobalShortcuts D-Bus portal (org.freedesktop.portal.GlobalShortcuts)
        → ashpd Rust bindings (Activated signal)
          → toggle_overlay_interaction(&app)
            → GTK: KeyboardMode + input_shape_combine_region
            → Tauri: overlay-interactive-changed event
              → Svelte: overlayInteractive state, badge rendered
```

## Why XDG GlobalShortcuts portal (not evdev/X11)

Marie targets Wayland (Hyprland) exclusively. The alternatives:

| Approach | Pros | Cons |
|----------|------|------|
| `tauri-plugin-global-shortcut` | Easy | X11 grab — doesn't work on pure Wayland |
| rdev + evdev-rs | Works on Wayland | Needs `input` group; reads raw kernel events; feels invasive |
| **XDG GlobalShortcuts portal** | No permissions, proper Wayland API, compositor-managed | Hyprland needs a `bind` line to trigger the protocol |

The portal is the correct Wayland approach. The only wrinkle (Hyprland requires a keybind
declaration to fire the protocol) is solved by injecting it at runtime with `hyprctl keyword bind`.

## Rust side (`src-tauri/src/lib.rs`)

### State

```rust
static OVERLAY_INTERACTIVE: AtomicBool = AtomicBool::new(false);
```

### Toggle function

`toggle_overlay_interaction(&app)` XORs the atomic bool, then (Linux only) calls
`set_overlay_input_linux` via `gtk::glib::idle_add_once` to marshal the GTK operation to the
main GTK thread. It also emits `overlay-interactive-changed: bool` to the frontend.

### GTK input shape (Linux)

When **interactive**:
- `win.set_keyboard_mode(KeyboardMode::OnDemand)` — keyboard events reach WebKit
- `win.input_shape_combine_region(None)` — full window receives pointer events

When **passthrough** (default):
- `win.set_keyboard_mode(KeyboardMode::None)` — keyboard events never reach overlay
- `win.input_shape_combine_region(Some(&cairo::Region::create()))` — empty region = clicks pass through

The `WidgetExt` trait must be in scope for `input_shape_combine_region` to resolve.

### Portal listener

`start_portal_listener(app: AppHandle)` runs as a Tokio task on startup (`#[cfg(not(windows))]`):

1. Creates a `GlobalShortcuts` proxy via `ashpd`
2. Creates a session (`CreateSessionOptions::default()`)
3. Registers shortcut ID `"toggle-overlay"` via `bind_shortcuts` with `BindShortcutsOptions::default()`
4. Subscribes to `receive_activated()` signal stream
5. On each `Activated` event with `shortcut_id() == "toggle-overlay"`, calls `toggle_overlay_interaction`

The session lives for the lifetime of the app. If the portal is unavailable (non-Hyprland), the
function logs a warning and returns early — Marie continues working, just without the hotkey.

### Hyprland keybind injection

Because Hyprland's global-shortcuts implementation requires a `bind =` line to fire the protocol
(the `preferred_trigger` field in the portal spec is stored but ignored), Marie injects the bind
at startup via `hyprctl keyword bind`. This avoids any manual `hyprland.conf` editing.

```
hyprctl keyword bind MODS, KEY, global, net.flaxeneel2.marie:toggle-overlay
```

Constants:
```rust
const DEFAULT_BIND: &str = "CTRL SHIFT, i";
const SHORTCUT_ID: &str  = "toggle-overlay";
const APP_ID: &str        = "net.flaxeneel2.marie";
```

### Config persistence

Bind is stored in `~/{app_config_dir}/config.json` as `{ "bind": "CTRL SHIFT, i" }`.

| Function | Purpose |
|----------|---------|
| `config_path(app)` | Returns path to `config.json` in the Tauri app config dir |
| `load_shortcut_config(app)` | Reads bind from config; falls back to `DEFAULT_BIND` |
| `save_shortcut_config(app, mods_key)` | Writes bind to config |
| `apply_hyprland_bind(mods_key)` | Runs `hyprctl keyword bind <bind>` |

At startup (in `setup`), Marie calls `apply_hyprland_bind(load_shortcut_config(app))` so the
previously saved bind is always active without user intervention.

### Tauri commands

| Command | Signature | Purpose |
|---------|-----------|---------|
| `disable_overlay_interaction` | `(app: AppHandle)` | Force-exit interactive mode (close button in overlay) |
| `get_interaction_shortcut` | `(app) -> String` | Return current stored bind string |
| `set_interaction_shortcut` | `(app, mods_key: String) -> Result<(), String>` | Save + apply new bind via hyprctl |

Non-Linux stubs exist for `get_interaction_shortcut` (returns `DEFAULT_BIND`) and
`set_interaction_shortcut` (no-op `Ok(())`) so the frontend compiles cross-platform.

## Frontend — overlay (`src/routes/overlay/+page.svelte`)

```svelte
let overlayInteractive = $state(false);
listen('overlay-interactive-changed', e => overlayInteractive = e.payload);
```

When `overlayInteractive` is true, an `INTERACTIVE` badge appears in the top-right with a `×`
close button that calls `invoke('disable_overlay_interaction')`.

Interactive mode is also cleared automatically when `hideOverlay()` or `hideRivenOverlay()` fires.

### CSS — pointer-events

The overlay `<body>` covers the whole screen with `pointer-events: none` by default.
Individual content areas opt back in explicitly:

```css
/* passthrough by default */
.overlay      { pointer-events: none; }
.riven-overlay { pointer-events: none; }

/* content areas receive clicks */
.card         { pointer-events: auto; user-select: text; cursor: default; }
.riven-side   { pointer-events: auto; }
.riven-panel  { user-select: text; cursor: default; }
```

`user-select: text` is required because WebKitGTK does not enable text selection by default in
overlay/layer-shell windows that lack keyboard focus.

## Frontend — settings (`src/routes/+page.svelte`)

A keybind recorder replaces the old static `hyprland.conf` instruction:

1. User clicks the recorder box → `recordingBind = true`
2. Next keydown is captured via `onkeydown`; modifier-only events are ignored
3. `hyprlandKey(e)` maps `KeyboardEvent.key` to xkb key names Hyprland understands
4. Result written to `currentBind` in format `"CTRL SHIFT, i"`; recording stops
5. "Apply" button calls `invoke('set_interaction_shortcut', { modsKey: currentBind })`

### Key name mapping (notable cases)

| JS `event.key` | Hyprland name |
|----------------|---------------|
| `" "` | `SPACE` |
| `Enter` | `Return` |
| `ArrowLeft` | `Left` |
| `PageUp` | `Prior` |
| `PageDown` | `Next` |
| `";"` | `semicolon` |
| Single letter/digit | lowercased as-is |
| `F1`–`F12` | passed through unchanged |

## Dependencies

| Crate | Added for |
|-------|-----------|
| `ashpd 0.13` (upgraded from 0.9) | XDG GlobalShortcuts portal API; 0.9 lacked `global_shortcuts` feature |
| `futures-util 0.3` | `StreamExt::next()` on the `receive_activated()` signal stream |

`rdev` and `libevdev` (used in an earlier evdev-based approach) were removed entirely.

## Sequence: startup

```
app setup
  ├─ init_layer_shell (overlay → wlr-layer-shell, KeyboardMode::None, empty input region)
  ├─ load_shortcut_config → apply_hyprland_bind  (inject keybind into Hyprland live)
  └─ start_portal_listener (Tokio task)
       └─ GlobalShortcuts::new → create_session → bind_shortcuts("toggle-overlay")
            └─ loop: receive_activated → toggle_overlay_interaction
```

## Sequence: user presses hotkey

```
Hyprland detects keybind
  → fires global-shortcuts-v1 activated event
    → xdg-desktop-portal-hyprland forwards to D-Bus Activated signal
      → ashpd stream yields Activated { shortcut_id: "toggle-overlay" }
        → toggle_overlay_interaction(&app)
          ├─ OVERLAY_INTERACTIVE XOR true
          ├─ idle_add_once → GTK thread:
          │    if now interactive: KeyboardMode::OnDemand, full input region
          │    else:               KeyboardMode::None,     empty input region
          └─ app.emit("overlay-interactive-changed", now)
               → Svelte: overlayInteractive flipped, badge shown/hidden
```
