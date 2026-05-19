# Inventory feature (`--features memory`)

Fetches the player's full account inventory from the Warframe API, resolves
`uniqueName` paths to friendly display names via a warframestat.us items cache,
and returns a categorized `InventoryView` to the frontend.

Requires the `memory` feature (Linux only) — needs the nonce from
`account_memory`. See [memory.md](memory.md).

## Pipeline overview

```
/proc/<pid>/mem scan
  └─ nonce + account_id (ACCOUNT_INFO)
         │
         ▼
inventory::get_or_refresh_inventory
  └─ GET https://api.warframe.com/api/inventory.php?accountId=…&nonce=…
  └─ {app_data_dir}/inventory_cache.json  (5-min TTL)
         │
         ▼
items_cache::get_maps
  └─ GET https://api.warframestat.us/items/?remove=patchlogs,introduced
  └─ {app_data_dir}/items_name_cache.json  (7-day TTL)
         │
         ▼
inventory::build_view  (InventoryCache × ItemMaps → InventoryView)
         │
         ▼
Tauri command get_inventory → frontend DisplayItem[]
```

## Rust modules

### `src-tauri/src/inventory.rs`

#### BSON primitives

| Type | JSON shape | Helper |
|------|-----------|--------|
| `MongoOid` | `{"$oid":"…"}` | `.oid: String` |
| `MongoDate` | `{"$date":{"$numberLong":"…"}}` | `.as_unix_ms() -> Option<i64>` |

#### Shared item types

| Type | Fields | Used for |
|------|--------|---------|
| `Bin` | `slots`, `extra` | Warframe/weapon slot counts |
| `CountedItem` | `item_type`, `item_count` | Resources, blueprints, consumables |
| `XpEntry` | `item_type`, `xp` | Per-item mastery XP (`XPInfo` array) |
| `RawUpgrade` | `item_type`, `item_count`, `last_added` | Unranked mod stacks |
| `Upgrade` | `item_type`, `item_id`, `fingerprint` | Ranked mod instances |
| `OwnedItem` | `item_type`, `item_id`, `xp`, `upgrade_ver`, `features`, `infestation_date`, `configs`, `modular_parts` | Gear (warframes, weapons, sentinels, …) |
| `FusionTreasure` | `item_type`, `item_count`, `sockets` | Primed mod materials |

#### `Inventory` struct

Top-level struct with `#[serde(default)]` on all fields. Opaque nested objects
(loadout presets, nemesis, helminth, etc.) are typed as `serde_json::Value`.

Key field groups:

| Group | Notable fields |
|-------|---------------|
| Currency | `credits`, `platinum`, `endo`, `prime_tokens` |
| Mastery | `mastery_rank`, `xp_info`, `daily_focus` |
| Gear arrays | `warframes` (Suits), `primaries` (LongGuns), `secondaries` (Pistols), `melee`, `sentinels`, `archwings`, `mechs`, `amps`, `companions`, `kaithe`, `railjacks`, … |
| Mods | `raw_upgrades` (unranked stacks), `ranked_mods` (Upgrades) |
| Resources | `misc_items` (MiscItems), `consumables`, `fusion_treasures`, `railjack_resources` |
| Foundry | `blueprints` (Recipes), `pending_recipes` |
| Social | `affiliations`, `boosters`, `quests`, `missions` |

#### `InventoryCache`

```rust
pub struct InventoryCache {
    pub fetched_at: u64,   // Unix seconds
    pub data: Inventory,
}
```

Serialized to `{app_data_dir}/inventory_cache.json`. TTL: 5 minutes.

`get_or_refresh_inventory` logic:
1. Load `inventory_cache.json`; return if `now − fetched_at < 300s`.
2. Read `account_id` + `nonce` from `ACCOUNT_INFO`.
3. `GET /api/inventory.php?accountId=…&nonce=…` via reqwest.
4. Write new `InventoryCache` to disk and return.

#### View types (sent to frontend)

```rust
pub struct DisplayItem {
    pub item_type: String,           // raw uniqueName path
    pub display_name: String,        // resolved friendly name
    pub image_name: String,          // filename e.g. "excaliburprime.avif"
    pub overlay_image_name: String,  // non-empty only for blueprint.png parts
    pub count: Option<i32>,          // None for OwnedItems (warframes, weapons)
    pub rank: Option<i32>,           // highest rank among ranked copies (mods only)
    pub max_rank: Option<i32>,       // fusionLimit from warframestat (mods only)
    pub rarity: Option<String>,      // "Common" / "Uncommon" / "Rare" / "Legendary" (mods only)
    pub polarity: Option<String>,    // "naramon" / "madurai" / … (mods only)
    pub compat_name: Option<String>, // mod compat group e.g. "Warframe" (mods only)
    pub description: Option<String>, // mod description text (mods only)
    pub level_stats: Option<String>, // JSON string of levelStats array (mods only)
    pub base_drain: Option<i32>,     // base mod drain (mods only)
}

pub struct InventoryView {
    pub fetched_at: u64,
    pub warframes: Vec<DisplayItem>,
    pub gear: Vec<DisplayItem>,
    pub relics: Vec<DisplayItem>,
    pub mods: Vec<DisplayItem>,
    pub resources: Vec<DisplayItem>,
    pub blueprints: Vec<DisplayItem>,
}
```

All mod-specific fields (`rank`, `max_rank`, `rarity`, `polarity`, `compat_name`,
`description`, `level_stats`, `base_drain`) are `None` for non-mod items.

`level_stats` is serialized to a JSON string in Rust (from `serde_json::Value`);
the frontend and the mod-card plugin parse it back as needed.

#### `build_view`

```rust
pub fn build_view(
    cache, names, categories, types, images, overlay_images,
    fusion_limits, rarities, polarities,
    compat_names, descriptions, level_stats, base_drains,
) -> InventoryView
```

Categorization rules:

| Tab | Source |
|-----|--------|
| `warframes` | `d.warframes` (Suits) |
| `gear` | primaries, secondaries, melee, sentinels, sentinel_weapons, archwings, arch_guns, arch_melee, mechs, companions, amps, kaithe, railjacks, drifter_melee, plexus |
| `relics` | `misc_items` where `types[item_type] == "Relic"` (or `categories[…] == "Relics"`) |
| `resources` | non-relic `misc_items` + `consumables` + `railjack_resources` |
| `mods` | merged `raw_upgrades` (unranked stacks) + `ranked_mods` (individual ranked copies), grouped by `item_type` |
| `blueprints` | `d.blueprints` (Recipes) |

**Mod merging:** counts are summed across both sources; `rank` tracks the highest
`lvl` found in `UpgradeFingerprint` JSON across all ranked copies (`parse_rank`
helper). Order: `raw_upgrades` item types first, then any ranked-only types.

#### `normalize_recipe_path`

Warframe inventory uses `*Blueprint` suffix for part recipes, but warframestat
indexes them under `*Component`. A lookup table patches these before map lookup:

| Inventory path suffix | Lookup suffix |
|----------------------|--------------|
| `SystemsBlueprint` | `SystemsComponent` |
| `ChassisBlueprint` | `ChassisComponent` |
| `HelmetBlueprint` | `HelmetComponent` |
| `NeuropticBlueprint` | `NeuropticComponent` |

---

### `src-tauri/src/items_cache.rs`

Fetches and caches item metadata from warframestat.us to resolve `uniqueName`
paths to display names, images, and overlay images.

#### API

```
GET https://api.warframestat.us/items/?remove=patchlogs,introduced
```

Returns an array of items including a `components` array for each item. The
`components` field is included (not removed) so blueprint part names can be
derived from the parent item name.

#### Disk cache

File: `{app_data_dir}/items_name_cache.json`  
TTL: 7 days (604 800 s)

Format stores processed maps (not raw items), so format changes automatically
invalidate old caches via serde parse failure.

```rust
struct ItemsFileCache {
    fetched_at: u64,
    names:          HashMap<String, String>,  // uniqueName → display name
    categories:     HashMap<String, String>,  // uniqueName → category
    types:          HashMap<String, String>,  // uniqueName → item type
    images:         HashMap<String, String>,  // uniqueName → imageName (.png)
    overlay_images: HashMap<String, String>,  // uniqueName → parent imageName
    fusion_limits:  HashMap<String, i32>,     // uniqueName → fusionLimit (mod max rank)
    rarities:       HashMap<String, String>,  // uniqueName → rarity string
    polarities:     HashMap<String, String>,  // uniqueName → polarity string
    compat_names:   HashMap<String, String>,  // uniqueName → compatName
    descriptions:   HashMap<String, String>,  // uniqueName → description
    level_stats:    HashMap<String, String>,  // uniqueName → levelStats JSON string
    base_drains:    HashMap<String, i32>,     // uniqueName → baseDrain
}
```

All fields added after the initial schema have `#[serde(default)]` so existing
cache files deserialize without error and a fresh fetch is not forced.

#### `build_maps` — component name generation

For each item, components are processed **only** when their `uniqueName` starts
with `/Lotus/Types/Recipes/` (crafting ingredient components share uniqueNames
with top-level items and must not be overwritten).

Name generated:
- `comp.name == "Blueprint"` → `"{parent.name} Blueprint"`
- otherwise → `"{parent.name} {comp.name} Blueprint"`

Overlay image rule: `overlay_images[comp.uniqueName] = parent.imageName` **only
when** `comp.imageName == "blueprint.png"`. Component parts (Chassis, Systems,
Neuroptics) have their own distinct images and do not get an overlay.

#### `pub struct ItemMaps`

```rust
pub struct ItemMaps {
    pub names:          HashMap<String, String>,
    pub categories:     HashMap<String, String>,
    pub types:          HashMap<String, String>,
    pub images:         HashMap<String, String>,
    pub overlay_images: HashMap<String, String>,
    pub fusion_limits:  HashMap<String, i32>,
    pub rarities:       HashMap<String, String>,
    pub polarities:     HashMap<String, String>,
    pub compat_names:   HashMap<String, String>,
    pub descriptions:   HashMap<String, String>,
    pub level_stats:    HashMap<String, String>,  // JSON strings
    pub base_drains:    HashMap<String, i32>,
}
```

`get_maps(app) -> ItemMaps` — returns from disk cache if valid, otherwise fetches
and saves.

---

## Tauri command

```ts
import { invoke } from "@tauri-apps/api/core";

interface DisplayItem {
  itemType:        string;
  displayName:     string;
  imageName:       string;          // e.g. "excaliburprime.avif" — use as /img/wf-assets/{imageName.replace('.png','.avif')}
  overlayImageName: string;         // non-empty for main blueprint items
  count:           number | null;
  rank:            number | null;   // mods only: highest rank among owned copies
  maxRank:         number | null;   // mods only: fusionLimit from warframestat
  rarity:          string | null;   // mods only
  polarity:        string | null;   // mods only
  compatName:      string | null;   // mods only
  description:     string | null;   // mods only
  levelStats:      string | null;   // mods only: JSON string of levelStats array
  baseDrain:       number | null;   // mods only
}

interface InventoryView {
  fetchedAt: number;
  warframes: DisplayItem[];
  gear: DisplayItem[];
  relics: DisplayItem[];
  mods: DisplayItem[];
  resources: DisplayItem[];
  blueprints: DisplayItem[];
}

const view = await invoke<InventoryView>("get_inventory");
```

`get_inventory` runs `get_or_refresh_inventory` and `items_cache::get_maps` in
parallel via `tokio::join!`, then calls `build_view`.

Returns an error string when the memory feature is unavailable, the game is not
running, or the nonce was not found.

## Item images

Images live in `static/img/wf-assets/` as `.avif` files. The `imageName` field
from warframestat ends in `.png`; replace with `.avif` to get the asset path:

```ts
`/img/wf-assets/${item.imageName.replace('.png', '.avif')}`
```

Blueprint items where `overlayImageName` is non-empty render as a stack: the
generic `blueprint.avif` base with the parent item's image inset on top (CSS
`position: absolute`, `inset: 10%`, `object-fit: contain`).

## Feature flag

The `inventory` and `items_cache` modules are compiled only under
`--features memory`. The `get_inventory` Tauri command always exists in the
standard build and returns an error immediately.

## Future work

- Re-fetch on EE.log `Logged in` events (handles relog / new nonce).
- Expose mastery XP totals for the mastery overview module.
- Cache versioning to avoid needing manual deletion on format changes.
