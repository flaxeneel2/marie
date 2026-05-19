# Mod cards

Renders Warframe mod cards as PNG blobs using the browser Canvas API
(`HTMLCanvasElement`). All metadata comes from the Rust `items_cache` pipeline.
No server, no Node.js, works in both dev and release builds.

## Architecture

```
Rust items_cache (warframestat /items, 7-day TTL)
  └─ DisplayItem.levelStats / .description / .compatName / .baseDrain
         │
         ▼
ModCard.svelte
  └─ generateModCard(params)  ←  src/lib/mod-card.ts
       │  (browser Canvas API, HTMLCanvasElement)
       ▼
  URL.createObjectURL(blob)
       │
       ▼
  <img src={blobUrl}>
```

## `src/lib/mod-card.ts`

Browser-native port of `@wfcd/mod-generator`. Exports one function:

```ts
export async function generateModCard(params: ModCardParams): Promise<Blob>
```

### `ModCardParams`

| Field | Type | Notes |
|-------|------|-------|
| `name` | string | display name |
| `imageName` | string | warframestat imageName e.g. `"flow.png"` — converted to local AVIF |
| `rarity` | string | `'Common'` / `'Uncommon'` / `'Rare'` / `'Legendary'` |
| `polarity` | string | `'naramon'` / `'madurai'` / … |
| `fusionLimit` | number | max rank |
| `rank` | number | current rank |
| `baseDrain` | number | base mod drain |
| `compatName` | string | mod compat group e.g. `"Warframe"` |
| `description` | string | static description (used if non-empty, otherwise `levelStats` wins) |
| `levelStats` | `unknown[] \| null` | parsed levelStats array from warframestat |
| `full` | boolean | `true` → full card (256×380); `false` → collapsed (256×150) |

### Returns

A `Blob` (`image/png`). Caller does `URL.createObjectURL(blob)` to get a usable src.

### Port notes vs `@wfcd/mod-generator`

| Original | Browser port |
|----------|-------------|
| `createCanvas(w, h)` | `document.createElement('canvas')` |
| `loadImage(path)` | `new Image()` + `onload` promise |
| `canvas.encode('png')` | `canvas.toBlob('image/png')` |
| `new ImageData(data, w, h)` | same (browser has identical API) |
| `GlobalFonts.registerFromPath(...)` | `FontFace` + `document.fonts.add()` via `@fontsource-variable/roboto` CSS import |
| `canvas → encode → loadImage` roundtrip in `flip`/`shadeImage`/`drawHeader` | return `HTMLCanvasElement` directly (valid `CanvasImageSource`, no roundtrip needed) |
| `warframe-items find.findItem(modSet)` | mod sets not supported (modSet always undefined) |

### Font

`@fontsource-variable/roboto` is imported as a side-effect at the top of
`mod-card.ts`. `ensureFont()` calls `document.fonts.load('22px "Roboto"')` once
before any draw call to guarantee the font is ready for canvas text measurement.

## Asset paths

Frame PNGs and polarity icons live in `static/img/mod-frames/` and are served
directly by the Vite/Tauri static file handler:

```
/img/mod-frames/{tier}Background.png
/img/mod-frames/{tier}CornerLights.png
/img/mod-frames/{tier}FrameBottom.png
/img/mod-frames/{tier}FrameTop.png
/img/mod-frames/{tier}SideLight.png
/img/mod-frames/{tier}TopRightBacker.png
/img/mod-frames/{tier}LowerTab.png
/img/mod-frames/RankSlotEmpty.png
/img/mod-frames/RankSlotActive.png
/img/mod-frames/RankCompleteLine.png
/img/mod-frames/polarities/{polarity}.png
```

Tiers: `Bronze` / `Silver` / `Gold` / `Legendary` / `Omega` (Riven uses
`LegendaryBackground.png`, `RivenTopRightBacker.png`, `RivenLowerTab.png`).

The `genesis-assets/` symlinks at the project root are no longer needed and can
be removed if desired.

## `ModCard.svelte`

`src/lib/components/ModCard.svelte`

### Props

```ts
type ModItem = {
  itemType:    string;
  displayName: string;
  imageName:   string;
  count:       number | null;
  rank:        number | null;
  maxRank:     number | null;
  rarity:      string | null;
  polarity:    string | null;
  compatName:  string | null;
  description: string | null;
  levelStats:  string | null;  // JSON string of levelStats array
  baseDrain:   number | null;
};
```

### Rendering

Cards are fetched via POST (not `<img src="url">`) to avoid URL length limits
for large `levelStats` JSON payloads. Each fetch returns a PNG blob; the
component creates an object URL from it.

```
fetchCard(false)  →  thumbUrl  (fetched on mount, lazy via $effect)
fetchCard(true)   →  fullUrl   (fetched on first hover, cached in $state)
```

Collapsed card: `generateCollapsed()` — 256×150 px strip.  
Full card: `generate()` — 256×380 px card.

### Hover overlay

On `mouseenter`, the full card URL is fetched (once) and a `position: fixed`
overlay div is shown. Fixed positioning escapes any `overflow: hidden` / `auto`
ancestor containers that would clip a CSS transform-based approach.

Position logic:
- X: `clamp(mouseX - 128, 8, innerWidth - 264)` (centres card on cursor, clamped to viewport)
- Y: if cursor is in the bottom half → `mouseY + 12`; top half → `mouseY - 392` (above cursor)

The overlay has `pointer-events: none` so it doesn't block interaction with
underlying elements, and `animation: mod-pop 0.1s ease` for a subtle scale-in.

### Count badge

When `count > 1`, a gold `#c9a227` badge is rendered top-left of the card
(position: absolute, z-layered above the image). Mimics in-game mod count style.

## Mod grid

The mods tab in `+page.svelte` renders `view.mods` as a 4-column grid of
`<ModCard>` components. Mods are merged from two inventory sources:

| Source | Rust field | Description |
|--------|-----------|-------------|
| `RawUpgrades` | `raw_upgrades` | Unranked mod stacks — has `ItemCount` |
| `Upgrades` | `ranked_mods` | Ranked individual copies — rank from `UpgradeFingerprint: {"lvl": N}` |

`build_view` groups both by `item_type`, summing counts and tracking the highest
rank among ranked instances. `DisplayItem.rank` is `None` when all copies are
unranked (rank 0 is the default rendered by the generator).
