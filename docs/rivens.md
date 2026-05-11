# Riven Mod Grading System

## Overview

The riven overlay reads a riven mod from the screen (OCR), evaluates it against a grading model, and displays two independent grades:

- **Market Grade** — how much this riven is likely worth in platinum (demand-driven)
- **Build Grade** — how good this riven is for actually using the weapon (stat-quality-driven)

These diverge often. A near-god-roll riven on a forgotten weapon has high build grade, low market grade. A mediocre riven on a meta weapon can still command 200–400 plat.

---

## Riven Data Model

A fully parsed riven has:

| Field | Type | Notes |
|-------|------|-------|
| `weapon` | `String` | Warframe's internal weapon name, normalised to WFM slug |
| `weapon_class` | `WeaponClass` | `Primary`, `Secondary`, `Melee`, `Archgun` |
| `positives` | `Vec<StatRoll>` | 2 or 3 positive stats |
| `negative` | `Option<StatRoll>` | Present on rivens with a "curse" |
| `disposition` | `u8` | 1–5 (●–●●●●●), fetched from WFM or Warframe API |
| `roll_count` | `u32` | Number of times the riven has been rerolled |
| `mod_rank` | `u8` | 0–8; affects stat magnitudes alongside disposition |
| `mr_requirement` | `u8` | 8–16 depending on weapon class |

```rust
pub struct StatRoll {
    pub stat: RivenStat,   // enum variant
    pub value: f32,        // actual percentage or flat value as shown in-game
    pub is_negative: bool,
}
```

Having a negative stat boosts all positive stat magnitudes by ~25% — this is baked into `value` by the game itself, so the grading system treats the raw `value` fields as authoritative.

---

## Disposition

Disposition (●–●●●●●, internally 0.5–1.55 as a float) is a per-weapon multiplier applied to base riven stat ranges. A disposition-5 weapon has ~3× the stat magnitude of a disposition-1 weapon for the same stat slot.

Disposition **does not** directly scale market value — in fact it inversely correlates with demand, since Warframe gives high disposition to underused weapons. The stat magnitude it produces matters for build quality but not price.

| Stars | Approx float | Player interpretation |
|-------|-------------|----------------------|
| ●○○○○ | ~0.5 | Weak riven, high-demand weapon (meta) |
| ●●○○○ | ~0.75 | |
| ●●●○○ | ~1.0 | Balanced |
| ●●●●○ | ~1.25 | |
| ●●●●● | ~1.55 | Strong riven, niche weapon |

Source of truth: `GET https://api.warframe.market/v2/riven/weapons` returns `disposition` as a float per weapon (see WFM Riven API section).

---

## Weapon Tier (Market Grade Input)

Weapon tier captures player demand independent of stat quality. A tier-S riven is desirable even with mediocre stats; a tier-F riven is nearly unsellable regardless.

Tiers are **not hardcoded** — they are derived from WFM riven market data at runtime by sampling median buyout prices across all listings for a weapon.

### Tier thresholds (median buyout, plat)

| Tier | Median buyout | Description |
|------|--------------|-------------|
| S | ≥ 600 | Consistently meta: Kuva Nukor, Tenet Cycron, Acceltra, Phenmor, etc. |
| A | 300–599 | Strong weapons with active player bases |
| B | 150–299 | Viable weapons, moderate demand |
| C | 75–149 | Situational or outclassed weapons |
| D | 25–74 | Niche, novelty, or newly powercrept |
| F | < 25 | Effectively unsellable at any reasonable price |

These thresholds are re-evaluated each session via the WFM riven auction search endpoint. The tier is cached per weapon for the session duration.

---

## Stat Weight Tables

Each `RivenStat` is assigned a weight per weapon class. Weights reflect how much that stat contributes to a strong build — they are used for Build Grade only.

### Weight scale

| Weight | Label | Meaning |
|--------|-------|---------|
| 1.0 | God | Universally best-in-slot for the class |
| 0.75 | Great | Strong, often sought |
| 0.5 | Good | Useful but not essential |
| 0.25 | Filler | Minor benefit, commonly ignored |
| 0.0 | Dump | No meaningful benefit (or actively useless) |

### Primary (non-shotgun)

| Stat | Weight |
|------|--------|
| Critical Chance | 1.0 |
| Critical Damage | 1.0 |
| Damage | 1.0 |
| Multishot | 1.0 |
| Fire Rate | 0.75 |
| Status Chance | 0.5 |
| Reload Speed | 0.5 |
| Toxin/Cold/Heat/Electric damage | 0.5 |
| Magazine Capacity | 0.25 |
| Punch Through | 0.25 |
| Ammo Maximum | 0.25 |
| Zoom | 0.0 |
| Recoil (positive) | 0.0 |
| Flight Speed (on hitscan) | 0.0 |
| Status Duration | 0.0 |

### Shotgun (Primary subclass)

| Stat | Weight |
|------|--------|
| Multishot | 1.0 |
| Critical Chance | 1.0 |
| Critical Damage | 1.0 |
| Damage | 0.75 |
| Status Chance | 0.75 |
| Fire Rate | 0.5 |
| Reload Speed | 0.5 |
| Magazine Capacity | 0.25 |
| Punch Through | 0.0 |
| Zoom | 0.0 |

### Secondary (pistol)

| Stat | Weight |
|------|--------|
| Critical Chance | 1.0 |
| Critical Damage | 1.0 |
| Damage | 1.0 |
| Multishot | 1.0 |
| Fire Rate | 0.75 |
| Status Chance | 0.5 |
| Reload Speed | 0.5 |
| Magazine Capacity | 0.25 |
| Ammo Maximum | 0.25 |
| Zoom | 0.0 |
| Flight Speed (hitscan) | 0.0 |

### Melee

| Stat | Weight |
|------|--------|
| Critical Chance | 1.0 |
| Critical Damage | 1.0 |
| Damage | 1.0 |
| Attack Speed | 0.75 |
| Status Chance | 0.5 |
| Range | 0.5 |
| Combo Duration | 0.25 |
| Slide Attack | 0.25 |
| Follow Through | 0.25 |
| Heavy Attack Efficiency | 0.25 |
| Finisher Damage | 0.0 |
| Channeling Efficiency | 0.0 |

### Archgun

Same weights as Primary but Flight Speed is relevant (Archgun projectiles are physical).

The tables above are the **class-level fallback**. Per-weapon weights derived from the two sources below override them when available.

---

## Per-Weapon Stat Weight Derivation

The class-level tables treat all rifles the same. In practice a Rubico Prime (crit sniper) and an Ignis Wraith (status flamethrower) want opposite stat priorities. Two data sources are combined to produce weapon-specific weights automatically.

### Source 1 — Warframe Public Export

DE publishes the full weapon stat database to their CDN after each update. The wiki scrapes it; Marie can too.

#### Fetching

```
GET https://content.warframe.com/PublicExport/index_en.txt
```

Returns a plain-text list of filenames with content hashes, one per line:

```
ExportWeapons_en.json@abc123def456
ExportRegions_en.json@...
...
```

Parse out the `ExportWeapons_en.json@{hash}` line, then:

```
GET https://content.warframe.com/PublicExport/Manifest/ExportWeapons_en.json@{hash}
```

Cache the hash between runs; only re-download when the hash changes (i.e. after a game update).

#### Joining to WFM weapons

Each entry in the export has a `uniqueName` field (e.g. `/Lotus/Weapons/Grineer/Pistols/GrnTorpedoPistol/GrnTorpedoPistol`). This matches the `gameRef` field in the WFM `/v2/riven/weapons` response 1:1, so the two datasets join on that key.

#### Relevant export fields

| Field | Type | Used for |
|-------|------|----------|
| `uniqueName` | string | join key to WFM `gameRef` |
| `criticalChance` | float (0–1) | crit profile |
| `criticalMultiplier` | float | crit profile |
| `statusChance` | float (0–1) | status profile |
| `fireRate` | float (rounds/s) | fire rate weight |
| `magazineSize` | int | reload weight |
| `reloadTime` | float (s) | reload weight |
| `projectileSpeed` | float | 0 = hitscan → flight speed is dump stat |
| `damagePerShot` | `[impact, puncture, slash, ...]` | physical distribution |
| `totalDamage` | float | slash ratio denominator |

#### Algorithmic weight derivation

```
// --- crit profile ---
if criticalChance >= 0.25 && criticalMultiplier >= 2.0:
    critical_chance = 1.0,  critical_damage = 1.0,  damage = 1.0
elif criticalChance >= 0.15:
    critical_chance = 0.75, critical_damage = 0.75, damage = 0.75
else:
    critical_chance = 0.25, critical_damage = 0.25

// --- status profile ---
if statusChance >= 0.30:   status_chance = 1.0
elif statusChance >= 0.15: status_chance = 0.5
else:                      status_chance = 0.25

// --- multishot: always tier-1 (linear DPS multiplier) ---
multishot = 1.0

// --- damage floor ---
damage = max(damage, 0.75)

// --- fire rate: diminishing on already-fast weapons ---
if fireRate > 10.0:   fire_rate = 0.25
elif fireRate > 4.0:  fire_rate = 0.5
else:                 fire_rate = 0.75

// --- reload: more valuable on slow/small-mag weapons ---
if reloadTime > 2.5 || magazineSize <= 10:
    reload_speed = 0.75
else:
    reload_speed = 0.25

// --- hitscan check ---
if projectileSpeed == 0: flight_speed = 0.0
else:                    flight_speed = 0.5

// --- slash dominance ---
slash_ratio = damagePerShot[2] / totalDamage
if slash_ratio > 0.5: slash_damage = 0.5   // slash scales well with condition overload
```

Stats not mentioned in the above rules inherit the class-level weight from the fallback tables.

---

### Source 2 — WFM Listing Correlation

For each weapon, the auction endpoint exposes what buyers are actually paying for. Stats that cluster in expensive listings are revealed preferences — more reliable than theory for weapons with non-obvious optimal builds.

#### Algorithm

Fetch two sets of listings for the weapon:

```
GET /v2/auctions/search?type=riven&weapon_url_name={slug}&sort_by=price_desc&buyout_policy=with_buyout
GET /v2/auctions/search?type=riven&weapon_url_name={slug}&sort_by=price_asc&buyout_policy=with_buyout
```

Take the top 50 from each. For each stat slug that can appear on this weapon:

```
freq_high = (listings in top-50 containing this stat as positive) / 50
freq_low  = (listings in bottom-50 containing this stat as positive) / 50

preference_ratio = freq_high / max(freq_low, 0.05)   // floor avoids div-by-zero
```

Normalise all `preference_ratio` values for the weapon to a 0.0–1.0 scale (divide by the max ratio). The result is the **market weight** for each stat on this specific weapon.

#### Validity gate

Only run correlation if the weapon has ≥ 30 total active buyout listings. Below that, the sample is too thin and ratios are noise. Check with a quick `sort_by=price_asc` page and inspect the total count in the response if the API exposes it; otherwise count the returned items (if < 30, skip).

---

### Blending: Export Prior + Market Refinement

```
if listing_count >= 50:
    weight = 0.4 × export_weight + 0.6 × market_weight   // market signal dominates
elif listing_count >= 20:
    weight = 0.7 × export_weight + 0.3 × market_weight   // light refinement
else:
    weight = export_weight                                 // not enough signal
```

The blend result is clamped to [0.0, 1.0] and stored in a per-weapon `HashMap<stat_slug, f32>` in the `riven.rs` cache alongside disposition.

#### Refresh cadence

| Source | When fetched | Invalidated |
|--------|-------------|-------------|
| Export index hash | Startup | Hash changes (game update) |
| Export weapon data | Startup, if hash changed | Same |
| WFM correlation | First grade request per weapon | Session restart |

Correlation is computed lazily per weapon on first use, not for all weapons upfront, to avoid hammering the API at startup.

---

## Negative Stat Evaluation

Negatives boost positive magnitudes by ~25%. Whether the trade-off is worth it depends on what was negated.

### Negative tiers

| Tier | Multiplier | Examples |
|------|-----------|---------|
| Harmless | 1.15 | -Zoom (often desired on snipers), -Flight Speed on hitscan, -Recoil on low-recoil weapons |
| Minor | 1.05 | -Impact (usually a dump physical type), -Puncture (same), -Ammo Maximum (manageable) |
| Neutral | 1.0 | -Reload Speed (varies by weapon), -Magazine (varies), -Slash on non-slash builds |
| Harmful | 0.85 | -Fire Rate on fast weapons, -Range on melee, -Attack Speed on melee |
| Crippling | 0.65 | -Damage, -Multishot, -Critical Chance, -Critical Damage, -Status Chance on status builds |

A riven with a Harmless negative is worth actively seeking over a no-negative riven, because the positive boost outweighs the loss.

The negative tier must be evaluated per weapon class: -Slash is Minor for a pure-impact shotgun but Harmful on a condition overload melee.

---

## Scoring Algorithm

### Build Score

```
stat_score  = Σ (stat_weight[i] × clamp(value[i] / ideal_value[i], 0, 1))  for each positive stat
stat_score /= max_possible (sum of top-N stat weights for this class, N = positive count)

negative_mod = negative_tier_multiplier  (1.0 if no negative)

build_score = stat_score × negative_mod
```

`ideal_value` is the theoretical maximum stat value at max disposition (●●●●●) and max mod rank, for a 3-positive no-negative riven. This normalises across dispositions so a 90% CC roll on a disposition-3 weapon scores similarly to a 90% CC roll on a disposition-5 weapon.

### Market Score

```
weapon_tier_weight = { S:1.0, A:0.8, B:0.6, C:0.4, D:0.2, F:0.05 }

roll_penalty = max(0, (roll_count - 5)) × 0.01    # 1% per roll above 5, capped at 0.15
roll_mod     = 1.0 - roll_penalty

market_score = weapon_tier_weight × build_score × roll_mod
```

Roll count matters more to buyers than sellers: pristine rivens (0–5 rolls) fetch premiums on meta weapons. High roll counts signal extensive rerolling and can reduce perceived value even if the current stats are good.

### Grade thresholds

| Grade | Score range | Display colour |
|-------|------------|---------------|
| S | ≥ 0.85 | Gold |
| A | 0.70 – 0.84 | Green |
| B | 0.55 – 0.69 | Teal |
| C | 0.40 – 0.54 | White |
| D | 0.25 – 0.39 | Grey |
| F | < 0.25 | Red |

Both Market Grade and Build Grade use the same threshold table but are calculated independently.

---

## WFM Riven API

All riven endpoints use `/v2`. The envelope is:

```jsonc
{
  "apiVersion": "0.23.1",
  "data": [ /* array of objects */ ],
  "error": null
}
```

### Weapon list + disposition

```
GET https://api.warframe.market/v2/riven/weapons
Header: Language: en
```

Returns all riven-eligible weapons. Relevant fields per entry:

```jsonc
{
  "slug": "kulstar",
  "group": "secondary",       // "primary" | "secondary" | "melee" | "archgun"
  "rivenType": "pistol",      // riven mod category; more granular than group
  "disposition": 1.3,         // float, ~0.5–1.55
  "reqMasteryRank": 5,
  "i18n": {
    "en": { "name": "Kulstar" }
  }
}
```

Build a `HashMap<slug, WeaponInfo>` from this on startup. `disposition` and `group` are the fields grading needs; cache the rest for display. Re-fetch on startup; disposition changes with DE balance patches.

### Attribute (stat) list

```
GET https://api.warframe.market/v2/riven/attributes
Header: Language: en
```

Returns all possible riven stats. Relevant fields per entry:

```jsonc
{
  "slug": "recoil",
  "exclusiveTo": ["shotgun", "rifle", "pistol", "kitgun"],  // null/absent = universal
  "positiveIsNegative": true,   // "positive" roll of this stat is actually bad (e.g. +recoil = more recoil)
  "positiveOnly": true,         // can only appear as a positive
  "negativeOnly": false,        // can only appear as a negative
  "unit": "percent",            // "percent" | "seconds" | "none"
  "i18n": {
    "en": { "name": "Weapon Recoil" }
  }
}
```

`positiveIsNegative` is important for the negative-tier evaluation: a "positive" Recoil roll makes the weapon kick more, so it should be treated as Crippling even though its sign is `+`. Build a `HashMap<slug, AttributeInfo>` on startup alongside the weapon map.

`exclusiveTo` lists `rivenType` values (e.g. `"pistol"`, `"rifle"`, `"shotgun"`, `"melee"`, `"kitgun"`, `"archgun"`). Stats absent from this list on a parsed riven are an OCR mis-read.

### Auction search (for weapon tier estimation)

```
GET https://api.warframe.market/v2/auctions/search
  ?type=riven
  &weapon_url_name={slug}
  &sort_by=price_asc
  &buyout_policy=with_buyout
```

Sample the top 20 `buyout_price` values from `data[*]`, discard outliers (> 3× median), take the median. This is the weapon's market floor regardless of stat quality — used as the weapon tier signal.

Relevant listing fields:

```jsonc
{
  "item": {
    "weaponUrlName": "kuva_nukor",
    "attributes": [
      { "urlName": "critical_chance", "value": 78.3, "positive": true },
      { "urlName": "multishot",       "value": 95.1, "positive": true },
      { "urlName": "zoom",            "value": 36.2, "positive": false }
    ],
    "reRolls": 3,
    "modRank": 8,
    "name": "Vexi-Agentin"
  },
  "buyoutPrice": 1200,
  "startingPrice": 800
}
```

`urlName` on attributes maps 1:1 to the `slug` field from `/v2/riven/attributes`. `positive: false` is the negative stat. `name` is a cosmetic generated riven name — not useful for grading.

---

## OCR Challenges

Reading rivens from screen is substantially harder than reading relic item names:

### What needs to be read

| Element | Notes |
|---------|-------|
| Weapon name | Top of the mod card, often with "Kuva" / "Tenet" prefix |
| Stat names | Multi-word, partially truncated on small cards |
| Stat values | Float percentages; OCR confuses `+` and digits |
| Stat signs | `+` vs implicit negative; colour-coded green/red |
| Roll count | Small text at card bottom (`Rolls: N`) |
| Mod rank | Star row at bottom |

### Known OCR difficulties

- Stat names wrap to two lines on narrow cards; the OCR region must capture both.
- The `+` prefix on positive stats and the absence of it on negatives must be inferred from text colour or position rather than character recognition.
- Stat values use a custom Warframe UI font with stylised digits — `1` and `7` are frequently confused.
- Multiple rivens can be shown simultaneously when comparing rolls; the active riven must be identified by highlight state.
- Kuva/Tenet weapon names have a coloured prefix that OCR may read as a separate word or drop entirely.

### Screen region strategy

Unlike relic rewards (fixed reward screen), rivens appear in:
- **Inventory / mod menu** — fixed position relative to the focused card
- **Post-mission reward screen** — not applicable (rivens are given sealed, stats hidden)
- **Trading post** — varying card position

For the initial implementation, target the **mod inspection panel** (the large card shown when a mod is selected in inventory). Proportional region bounds should be calibrated to 2560×1440 and scaled at runtime, matching the relic overlay approach.

---

## Planned Module Architecture

```
EE.log "ContainerType=Mod" (or manual trigger from main window)
    │
    ▼
screenshot::capture_riven_card()
    │
    ▼
ocr::parse_riven_card(image) -> RivenOcrResult
    │   ├─ weapon name (fuzzy matched against WFM riven items list)
    │   ├─ stats (sign, name, value) × 2–4
    │   └─ roll_count, mod_rank
    │
    ▼
riven::grade(weapon, stats, roll_count) -> RivenGrade
    │   ├─ resolve disposition from WFM riven weapons cache
    │   ├─ resolve weapon tier from WFM auction median
    │   ├─ resolve per-weapon stat weights
    │   │     ├─ export_weights  (from ExportWeapons_en.json derivation)
    │   │     ├─ market_weights  (from WFM listing correlation, lazy per weapon)
    │   │     └─ blend(export, market, listing_count)
    │   ├─ compute build_score (blended weights × negative mod)
    │   └─ compute market_score (build_score × tier weight × roll mod)
    │
    ▼
overlay renders:
    - Weapon name + disposition dots
    - Each stat with its weight tier colour
    - Build Grade (letter + score)
    - Market Grade (letter + score)
    - Estimated plat range (from WFM auction sample)
```

New Rust module: `src-tauri/src/riven.rs` — grading logic, stat weight tables, WFM riven cache.

New Tauri command: `grade_riven(ocr_result: RivenOcrResult) -> RivenGrade`

---

## Known Limitations & Future Work

- Weapon tier is sampled from WFM listings at session start; price swings from Warframe updates or content drops are not reflected until restart.
- WFM correlation weights are also session-cached; a weapon that spikes in meta popularity mid-session won't reflect updated stat preferences until restart.
- Build Score does not account for specific synergies (e.g. +status chance only matters if the build has the corrosive/viral procs to leverage it). A future `BuildContext` struct could accept a player's existing mod loadout and adjust weights accordingly.
- Export derivation rules are heuristics — they work well for conventional crit/status builds but will misgrade weapons used in niche builds (e.g. a beam weapon used for its unique status ticking, not its crit). WFM correlation corrects this for high-volume weapons but not for obscure ones.
- Negative evaluation is weapon-class-level, not weapon-specific. Some weapons have quirks where a normally-harmful negative is acceptable (e.g. -reload on Ignis Wraith with Fast Hands).
- No handling yet for Zaw/Kitgun/Amp rivens, which use component-based weapon names.
- Roll count penalty is linear and capped; some buyers weigh rolls non-linearly (anything above 10 rolls is often treated as "tainted" regardless of stats).
- The grading system produces a score but not a recommended price — price recommendation requires more market data points (percentile position in current listings, demand velocity).
