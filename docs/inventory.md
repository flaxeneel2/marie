# Inventory feature (`--features memory`)

Fetches the player's full account inventory from the Warframe API and caches it
on disk. Requires the `memory` feature (Linux only) because it uses the nonce
extracted from process memory to authenticate the request.

## API endpoint

```
GET https://api.warframe.com/api/inventory.php?accountId={account_id}&nonce={nonce}
```

Both parameters come from `account_memory::ACCOUNT_INFO`, populated at startup
by the privileged child's `ScanAccount` scan. See [memory.md](memory.md).

The response is a large JSON object using MongoDB BSON conventions — dates as
`{"$date":{"$numberLong":"<ms>"}}`, OIDs as `{"$oid":"<hex>"}`.

## Rust module — `src-tauri/src/inventory.rs`

### BSON primitives

| Type | JSON shape | Helper |
|------|-----------|--------|
| `MongoOid` | `{"$oid":"…"}` | `.oid: String` |
| `MongoDate` | `{"$date":{"$numberLong":"…"}}` | `.as_unix_ms() -> Option<i64>` |

### Shared item types

| Type | Fields | Used for |
|------|--------|---------|
| `Bin` | `slots`, `extra` | Warframe/weapon slot counts |
| `CountedItem` | `item_type`, `item_count` | Resources, blueprints, consumables |
| `XpEntry` | `item_type`, `xp` | Per-item mastery XP (`XPInfo` array) |
| `RawUpgrade` | `item_type`, `item_count`, `last_added` | Unranked mod stacks |
| `Upgrade` | `item_type`, `item_id`, `fingerprint` | Ranked mod instances |
| `OwnedItem` | `item_type`, `item_id`, `xp`, `upgrade_ver`, `features`, `infestation_date`, `configs`, `modular_parts` | All gear (warframes, weapons, sentinels, etc.) |
| `FusionTreasure` | `item_type`, `item_count`, `sockets` | Primed mod materials |

### Domain types

| Type | Purpose |
|------|---------|
| `Affiliation` | Syndicate standing (`tag`, `standing`, `initiated`, `title`) |
| `Booster` | Active booster (`item_type`, `expiry: i64` unix seconds) |
| `QuestKey` | Quest state (`unlock`, `completed`, `progress`) |
| `PendingRecipe` | In-progress foundry build (`item_type`, `completion_date`, `item_id`) |
| `MissionEntry` | Node completion count (`tag`, `completes`, `tier`) |
| `FocusXp` | Focus school XP (`power`, `attack`, `tactic`, `ward`, `defense`) |
| `AlignmentData` | Tenno alignment (`alignment`, `wisdom`) |
| `DuviriInfo` | Duviri state (`seed`, `completions`, `stalker_chance`) |
| `PlayerSettings` | Account privacy settings |
| `ChallengeProgress` | Nightwave / challenge progress |

### `Inventory` struct

Top-level struct with `#[serde(default)]` on all array/scalar fields so missing
keys never fail parsing. Opaque nested objects (loadout presets, nemesis,
helminth, etc.) are typed as `serde_json::Value` to avoid maintenance burden.

Key field groups:

| Group | Fields |
|-------|--------|
| Currency | `credits`, `platinum`, `platinum_free`, `endo`, `prime_tokens` |
| Mastery | `mastery_rank`, `xp_info`, `daily_focus` |
| Trading | `trades_remaining`, `gifts_remaining` |
| Slot bins | `suit_bin`, `weapon_bin`, `sentinel_bin`, `archwing_bin`, … |
| Gear arrays | `warframes`, `primaries`, `secondaries`, `melee`, `sentinels`, `archwings`, `mechs`, `amps`, … |
| Mods | `raw_upgrades`, `ranked_mods` |
| Resources | `misc_items`, `consumables`, `fusion_treasures`, `blueprints`, `pending_recipes` |
| Social | `affiliations`, `boosters`, `quests`, `missions` |
| Focus | `focus_xp`, `focus_ability` |

## Cache

### `InventoryCache`

```rust
pub struct InventoryCache {
    pub fetched_at: u64,   // Unix seconds (std::time::SystemTime)
    pub data: Inventory,
}
```

Serialized to / deserialized from `{app_data_dir}/inventory_cache.json` using
serde_json.

### TTL — 5 minutes

`get_or_refresh_inventory` logic:

1. Try to load `inventory_cache.json` from disk.
2. If cache exists and `now - fetched_at < 300s` → return cached data, no HTTP.
3. Otherwise:
   a. Read `account_id` and `nonce` from `ACCOUNT_INFO` (error if missing).
   b. `GET` the inventory endpoint via `reqwest`.
   c. Parse response into `Inventory`.
   d. Write new `InventoryCache` to disk.
   e. Return fresh cache.

Cache file is written atomically via `fs::write` (single syscall on Linux).

## Tauri command

```ts
import { invoke } from "@tauri-apps/api/core";

interface InventoryCache {
  fetched_at: number;           // Unix seconds
  data: Inventory;              // full inventory object
}

const cache = await invoke<InventoryCache>("get_inventory");
```

Returns an error string in the standard (non-memory) build or when the game
is not running / nonce was not found.

## Integration with memory pipeline

```
EE.log
  └─ account_id, username
         │
         ▼
/proc/<pid>/mem scan
  └─ nonce
         │
         ▼
ACCOUNT_INFO (Mutex<Option<AccountInfo>>)
         │
         ▼
inventory::get_or_refresh_inventory
  └─ GET /api/inventory.php?accountId=…&nonce=…
         │
         ▼
{app_data_dir}/inventory_cache.json
         │
         ▼
Tauri frontend (get_inventory command)
```

## Feature flag

The entire `inventory` module is compiled only when `--features memory` is set
(same gate as `account_memory`). The `get_inventory` Tauri command always exists
in both builds — the standard build stub returns an error immediately so the
frontend can handle it gracefully.

## Future work

- Re-fetch on EE.log `Logged in` events (handles relog / new nonce).
- Expose mastery XP totals to the UI for the mastery overview module.
- Incremental cache invalidation using `LastInventorySync` OID.
