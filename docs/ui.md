# UI layout

Marie's main app window (`src/routes/+page.svelte`) uses a sidebar-nav shell
with multiple top-level pages. The overlay window is separate and unchanged.

## App shell

```
┌─────────────┬──────────────────────────────────────────┐
│  sidebar    │  content area (scrollable)               │
│  (180 px)   │                                          │
│             │                                          │
│  Inventory  │  [tab bar]  [item grid / dev panel]      │
│  Foundry    │                                          │
│  ─────────  │                                          │
│  Dev        │                                          │
└─────────────┴──────────────────────────────────────────┘
```

`body` has `height: 100vh; overflow: hidden` (set in `src/app.html`) so the
shell fills the window without a page-level scrollbar.

### Nav items

| `NavItem` value | Sidebar label |
|-----------------|---------------|
| `'inventory'`   | Inventory      |
| `'foundry'`     | Foundry        |
| `'dev'`         | Dev            |

Active nav is `$state<NavItem>`.

---

## Inventory tab

Triggered on first navigation to `'inventory'`; `loadInventory()` calls
`invoke<InventoryView>('get_inventory')` once and caches in `$state`.

### Sub-tabs

| `InventoryTab` value | Content source |
|----------------------|----------------|
| `'warframes'`        | `view.warframes` |
| `'gear'`             | `view.gear` |
| `'relics'`           | `view.relics` (filtered + sorted) |
| `'mods'`             | `view.mods` |
| `'resources'`        | `view.resources` |
| `'blueprints'`       | `view.blueprints` |

### Relic tier sub-bar

Rendered only when `inventoryTab === 'relics'`. Tabs:

| `RelicTier` | Filter applied |
|-------------|----------------|
| `'all'`     | none |
| `'lith'`    | `displayName.startsWith("Lith")` |
| `'meso'`    | `displayName.startsWith("Meso")` |
| `'neo'`     | `displayName.startsWith("Neo")` |
| `'axi'`     | `displayName.startsWith("Axi")` |
| `'requiem'` | `displayName.startsWith("Requiem")` |

Relics are always sorted alphabetically by `displayName` regardless of tier.

`currentItems()` returns the filtered + sorted array for the active tab/tier.

### Item grid

Each item renders as a card with:
- Image stack when `overlayImageName` is non-empty:
  - base: `blueprint.avif` at 100%
  - overlay: parent item image at `inset: 10%; object-fit: contain`
- Single `<img>` when `imageName` is non-empty
- Grey placeholder box when both are empty

Image path: `/img/wf-assets/{imageName.replace('.png', '.avif')}`

Card footer shows `displayName` and `count` (count omitted for OwnedItems where
count is `null`).

### Mod grid

Mods tab uses `<ModCard>` (`src/lib/components/ModCard.svelte`) instead of the
generic card. Grid is 4 columns (`grid-template-columns: repeat(4, 1fr)`).

Each `ModCard`:
- Renders a collapsed card (256×150 px) via POST to `/api/mod-card` (Vite plugin)
- On hover: shows a full card (256×380 px) in a `position: fixed` overlay
- Displays a count badge (top-left) when the player owns more than one copy

The POST response is a PNG blob; the component stores a `URL.createObjectURL`
result to avoid URL-length limits from `levelStats` JSON in query strings.

See [mod-cards.md](mod-cards.md) for the full rendering pipeline.

---

## Foundry tab

Placeholder — not yet implemented.

## Dev tab

Contains all developer tooling from the original single-page layout:
- EE.log path display
- Relic trigger test buttons
- Riven trigger test buttons
- Player count override
- Keybind recorder for the overlay interaction shortcut
- Overlay interactive state indicator
