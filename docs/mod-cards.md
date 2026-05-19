# Mod cards

Renders Warframe mod cards as PNG images using `@wfcd/mod-generator` and serves
them from a Vite dev-server middleware. All mod metadata (stats, description,
compat name) comes from the Rust `items_cache` pipeline — no client-side API
calls.

## Architecture

```
Rust items_cache (warframestat /items, 7-day TTL)
  └─ DisplayItem.levelStats / .description / .compatName / .baseDrain
         │
         ▼
ModCard.svelte  ──POST /api/mod-card──▶  mod-card-plugin.ts (Vite middleware)
  │                                            │
  │  blob URL ◀─────────────────────────────  └─ @wfcd/mod-generator
  ▼                                                 └─ @napi-rs/canvas
<img src={blobUrl}>
```

## Vite plugin (`mod-card-plugin.ts`)

File lives at the project root (outside `src/`) so `svelte-check` ignores it.
Registered in `vite.config.js` as `modCardPlugin()`.

Exposes a single endpoint: `POST /api/mod-card`

### Request body (JSON)

| Field | Type | Notes |
|-------|------|-------|
| `itemType` | string | uniqueName path — used for asset lookup only |
| `imageName` | string | warframestat imageName e.g. `"flow.png"` |
| `name` | string | display name |
| `rarity` | string | `"Common"` / `"Uncommon"` / `"Rare"` / `"Legendary"` |
| `polarity` | string | `"naramon"` / `"madurai"` / `"vazarin"` etc. |
| `maxRank` | number | fallback when `levelStats` not present |
| `rank` | number | current rank to render rank pips |
| `full` | boolean | `true` → `generate()` (full card 256×380); `false` → `generateCollapsed()` (256×150) |
| `compatName` | string | mod compat group e.g. `"Warframe"` |
| `description` | string | mod description text |
| `levelStats` | string \| null | JSON string of `levelStats` array from warframestat |
| `baseDrain` | number | base mod drain |

### Response

`image/png` binary, `Cache-Control: public, max-age=3600`.

### Image resolution

The plugin looks for a local AVIF file in `static/img/wf-assets/`:

```ts
const avif = imageName.replace(/\.(png|jpg|jpeg|webp)$/i, '.avif');
const candidate = path.join(ASSETS_DIR, avif);
if (existsSync(candidate)) imageArg = candidate;
```

`@napi-rs/canvas`'s `loadImage` accepts raw absolute paths (not `file://` URIs).
If the file doesn't exist it falls back to the generator's own lookup (CDN).
The `existsSync` check is required — passing a missing path causes `loadImage` to
call `new URL(source)` which throws for bare Unix paths.

### Font registration

`@wfcd/mod-generator`'s `registerFonts()` has a path bug: it resolves via
`createRequire(import.meta.url)` → `require.resolve('@fontsource-variable/roboto')`
which returns `index.css`, then navigates `../../files/…` — one directory too
high. Fix: pre-register Roboto at the correct absolute path before any generate
call:

```ts
const ROBOTO_WOFF2 = path.join(ROOT, 'node_modules/@fontsource-variable/roboto/files/roboto-latin-wght-normal.woff2');
if (!GlobalFonts.has('Roboto')) GlobalFonts.registerFromPath(ROBOTO_WOFF2, 'Roboto');
```

This runs once at plugin load time (module top-level).

## genesis-assets symlinks

`@wfcd/mod-generator` hardcodes `./genesis-assets/` relative to CWD to find
frame PNGs (Bronze/Silver/Gold/Legendary/Omega tiers, rank slot images, polarity
icons). The assets are downloaded to `static/img/mod-frames/` and exposed via
two symlinks at the project root:

```
genesis-assets/modFrames      → static/img/mod-frames/
genesis-assets/img/polarities → static/img/mod-frames/polarities/
```

These must exist for the generator to render frames and polarity icons.

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
