use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

// ── API response types ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct RivenWeaponsResponse {
    data: Vec<ApiRivenWeapon>,
}

#[derive(Deserialize)]
struct ApiRivenWeapon {
    slug: String,
    #[serde(default)]
    group: String,
    #[serde(rename = "rivenType", default)]
    riven_type: String,
    disposition: f32,
    #[serde(default)]
    i18n: HashMap<String, ApiRivenWeaponI18n>,
}

#[derive(Deserialize)]
struct ApiRivenWeaponI18n {
    // WFM v2 uses "name" in the JSON example; Go struct says "itemName" — trust the JSON.
    name: String,
}

#[derive(Deserialize)]
struct RivenAttributesResponse {
    data: Vec<ApiRivenAttribute>,
}

#[derive(Deserialize)]
struct ApiRivenAttribute {
    slug: String,
    #[serde(rename = "exclusiveTo", default)]
    exclusive_to: Vec<String>,
    #[serde(rename = "positiveIsNegative", default)]
    positive_is_negative: bool,
    #[serde(rename = "positiveOnly", default)]
    positive_only: bool,
    #[serde(rename = "negativeOnly", default)]
    negative_only: bool,
    #[serde(default)]
    i18n: HashMap<String, ApiRivenAttributeI18n>,
}

#[derive(Deserialize)]
struct ApiRivenAttributeI18n {
    // WFM v2 JSON shows "name"; Go struct says "effect" — trust the JSON.
    name: String,
}

// ── Cached data ───────────────────────────────────────────────────────────────

#[derive(Clone)]
pub struct WeaponInfo {
    pub name: String,
    pub group: String,
    pub riven_type: String,
    pub disposition: f32,
}

#[derive(Clone)]
pub struct AttributeInfo {
    pub name: String,
    pub exclusive_to: Vec<String>,
    pub positive_is_negative: bool,
    pub _positive_only: bool,
    pub _negative_only: bool,
}

static WEAPON_CACHE: Lazy<RwLock<HashMap<String, WeaponInfo>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

static ATTRIBUTE_CACHE: Lazy<RwLock<HashMap<String, AttributeInfo>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

// ── Output types (serialized to frontend) ─────────────────────────────────────

#[derive(Serialize, Clone)]
pub struct ParsedStat {
    pub slug: String,
    pub display_name: String,
    pub value: f32,
    pub is_negative: bool,
    /// True when positiveIsNegative=true and this stat rolled positive (e.g. +Recoil = bad).
    pub effective_negative: bool,
    /// True for 'x'-prefix faction/damage-type multiplier stats (e.g. "x1.7 Grineer Damage").
    /// These display as "x0.49" not "+49%".
    pub is_multiplier: bool,
    pub weight: f32,
    pub weight_label: String,
    /// value / theoretical_max clamped 0–1. None when base value for this stat is unknown.
    pub roll_quality: Option<f32>,
}

#[derive(Serialize, Clone)]
pub struct RivenRollGrade {
    pub weapon_name: String,
    pub weapon_slug: String,
    pub disposition: f32,
    pub weapon_tier: String,
    pub stats: Vec<ParsedStat>,
    pub roll_count: u32,
    pub build_score: f32,
    pub build_grade: String,
    pub market_score: f32,
    pub market_grade: String,
}

#[derive(Serialize, Clone)]
pub struct RivenRerollResult {
    pub old: RivenRollGrade,
    pub new: RivenRollGrade,
}

// ── Public API ────────────────────────────────────────────────────────────────

pub async fn init_riven_cache() -> Result<(usize, usize), reqwest::Error> {
    let client = reqwest::Client::new();

    let weapons_resp: RivenWeaponsResponse = client
        .get("https://api.warframe.market/v2/riven/weapons")
        .header("Language", "en")
        .send().await?
        .json().await?;

    let mut weapons = WEAPON_CACHE.write().await;
    for w in weapons_resp.data {
        let name = w.i18n.get("en").map(|l| l.name.clone()).unwrap_or_default();
        weapons.insert(w.slug, WeaponInfo {
            name,
            group: w.group,
            riven_type: w.riven_type,
            disposition: w.disposition,
        });
    }
    let weapon_count = weapons.len();
    drop(weapons);

    let attrs_resp: RivenAttributesResponse = client
        .get("https://api.warframe.market/v2/riven/attributes")
        .header("Language", "en")
        .send().await?
        .json().await?;

    let mut attrs = ATTRIBUTE_CACHE.write().await;
    for a in attrs_resp.data {
        let name = a.i18n.get("en").map(|l| l.name.clone()).unwrap_or_default();
        attrs.insert(a.slug, AttributeInfo {
            name,
            exclusive_to: a.exclusive_to,
            positive_is_negative: a.positive_is_negative,
            _positive_only: a.positive_only,
            _negative_only: a.negative_only,
        });
    }
    let attr_count = attrs.len();
    drop(attrs);

    Ok((weapon_count, attr_count))
}

/// Grade both panels from the riven cycling comparison screen.
/// Both panels are the same weapon, so weapon tier is fetched once.
pub async fn grade_panels(
    old_lines: Vec<String>,
    new_lines: Vec<String>,
) -> RivenRerollResult {
    // Clone caches before any async work — can't hold RwLock across await.
    let weapons: HashMap<String, WeaponInfo> = WEAPON_CACHE.read().await.clone();
    let attrs: HashMap<String, AttributeInfo> = ATTRIBUTE_CACHE.read().await.clone();

    let client = reqwest::Client::new();

    let (old_weapon_slug, old_weapon) = find_weapon(&old_lines, &weapons);
    let (new_weapon_slug, new_weapon) = find_weapon(&new_lines, &weapons);

    // Use whichever panel had a better weapon match, fetch tier once.
    let primary_slug = if !old_weapon_slug.is_empty() { &old_weapon_slug } else { &new_weapon_slug };
    let (weapon_tier, _median) = if !primary_slug.is_empty() {
        fetch_weapon_tier(&client, primary_slug).await
    } else {
        ("?".to_string(), 0.0)
    };

    let old = build_grade(old_lines, old_weapon_slug, old_weapon, weapon_tier.clone(), &attrs);
    let new = build_grade(new_lines, new_weapon_slug, new_weapon, weapon_tier, &attrs);

    RivenRerollResult { old, new }
}

pub fn fake_reroll_result() -> RivenRerollResult {
    let old = RivenRollGrade {
        weapon_name: "Kuva Nukor".into(),
        weapon_slug: "kuva_nukor".into(),
        disposition: 0.5,
        weapon_tier: "S".into(),
        stats: vec![
            ParsedStat { slug: "fire_rate".into(),    display_name: "Fire Rate".into(),    value: 62.3, is_negative: false, is_multiplier: false, effective_negative: false, weight: 0.75, weight_label: "Great".into(), roll_quality: Some(0.72) },
            ParsedStat { slug: "reload_speed".into(), display_name: "Reload Speed".into(), value: 48.7, is_negative: false, is_multiplier: false, effective_negative: false, weight: 0.5,  weight_label: "Good".into(),  roll_quality: Some(0.45) },
            ParsedStat { slug: "damage".into(),       display_name: "Damage".into(),       value: 54.1, is_negative: true,  is_multiplier: false, effective_negative: false, weight: 0.0,  weight_label: "Dump".into(),  roll_quality: None        },
        ],
        roll_count: 10,
        build_score: 0.38,
        build_grade: "D".into(),
        market_score: 0.19,
        market_grade: "F".into(),
    };
    let new = RivenRollGrade {
        weapon_name: "Kuva Nukor".into(),
        weapon_slug: "kuva_nukor".into(),
        disposition: 0.5,
        weapon_tier: "S".into(),
        stats: vec![
            ParsedStat { slug: "critical_chance".into(), display_name: "Critical Chance".into(), value: 77.3, is_negative: false, is_multiplier: false, effective_negative: false, weight: 1.0,  weight_label: "God".into(),  roll_quality: Some(0.91) },
            ParsedStat { slug: "multishot".into(),        display_name: "Multishot".into(),        value: 88.1, is_negative: false, is_multiplier: false, effective_negative: false, weight: 1.0,  weight_label: "God".into(),  roll_quality: Some(0.58) },
            ParsedStat { slug: "zoom".into(),             display_name: "Zoom".into(),             value: 34.6, is_negative: true,  is_multiplier: false, effective_negative: false, weight: 0.0,  weight_label: "Dump".into(), roll_quality: None        },
        ],
        roll_count: 11,
        build_score: 1.0,
        build_grade: "S".into(),
        market_score: 0.94,
        market_grade: "S".into(),
    };
    RivenRerollResult { old, new }
}

// ── Grade construction ────────────────────────────────────────────────────────

fn build_grade(
    lines: Vec<String>,
    weapon_slug: String,
    weapon: Option<WeaponInfo>,
    weapon_tier: String,
    attrs: &HashMap<String, AttributeInfo>,
) -> RivenRollGrade {
    let riven_type = weapon.as_ref().map(|w| w.riven_type.as_str()).unwrap_or("rifle");
    let disposition = weapon.as_ref().map(|w| w.disposition).unwrap_or(1.0);
    let roll_count = parse_roll_count(&lines);

    let raw = parse_stat_lines(&lines, attrs);

    // Pre-scan to determine slot configuration for roll_quality computation.
    let n_positive = raw.iter().filter(|(_, _, _, is_neg, _, attr)| {
        let eff = attr.as_ref().map(|a| a.positive_is_negative && !*is_neg).unwrap_or(false);
        !*is_neg && !eff
    }).count();
    let has_negative = raw.iter().any(|(_, _, _, is_neg, _, attr)| {
        let eff = attr.as_ref().map(|a| a.positive_is_negative && !*is_neg).unwrap_or(false);
        *is_neg || eff
    });
    let s_factor = slot_factor(n_positive, has_negative);

    let stats: Vec<ParsedStat> = raw.into_iter().map(|(slug, display_name, value, is_negative, is_multiplier, attr)| {
        let effective_negative = attr.as_ref().map(|a| a.positive_is_negative && !is_negative).unwrap_or(false);
        let weight = if is_negative || effective_negative { 0.0 } else { stat_weight(&slug, riven_type) };
        let roll_quality = if !is_negative && !effective_negative && !is_multiplier {
            base_stat_value(&slug).map(|base| {
                let max_val = base * disposition * s_factor;
                if max_val > 0.0 { (value / max_val).clamp(0.0, 1.0) } else { 0.0 }
            })
        } else {
            None
        };
        ParsedStat { roll_quality, weight_label: weight_label(weight), slug, display_name, value, is_negative, is_multiplier, effective_negative, weight }
    }).collect();

    let positives: Vec<&ParsedStat> = stats.iter().filter(|s| !s.is_negative && !s.effective_negative).collect();
    let negative = stats.iter().find(|s| s.is_negative || s.effective_negative);

    let build_score = compute_build_score(&positives, negative, riven_type);
    let market_score = compute_market_score(build_score, &weapon_tier, roll_count);

    RivenRollGrade {
        weapon_name: weapon.as_ref().map(|w| w.name.clone()).unwrap_or_default(),
        weapon_slug,
        disposition: weapon.as_ref().map(|w| w.disposition).unwrap_or(1.0),
        weapon_tier,
        stats,
        roll_count,
        build_score,
        build_grade: score_to_grade(build_score),
        market_score,
        market_grade: score_to_grade(market_score),
    }
}

// ── Weapon matching ───────────────────────────────────────────────────────────

fn find_weapon(lines: &[String], weapons: &HashMap<String, WeaponInfo>) -> (String, Option<WeaponInfo>) {
    let mut best_score = 0.0_f64;
    let mut best_slug = String::new();
    let mut best_info: Option<WeaponInfo> = None;

    // Only check the first few lines; weapon name is always near the top of the card.
    for line in lines.iter().take(6) {
        let q = normalize_for_match(line);
        if q.is_empty() { continue; }
        for (slug, info) in weapons.iter() {
            let t = info.name.to_lowercase();
            let score = similarity(&q, &t);
            if score > best_score {
                best_score = score;
                best_slug = slug.clone();
                best_info = Some(info.clone());
            }
        }
    }

    eprintln!("[riven] weapon match: {best_slug:?} (score {best_score:.3})");

    if best_score >= 0.70 { (best_slug, best_info) } else { (String::new(), None) }
}

// ── Stat line parsing ─────────────────────────────────────────────────────────

/// Parse OCR lines into (slug, display_name, value, is_negative, is_multiplier, attr) tuples.
/// Formats:
///   "+NUMBER[%] Stat Name"   — positive percentage stat
///   "-NUMBER[%] Stat Name"   — negative percentage stat
///   "xNUMBER Stat Name"      — faction/damage-type multiplier (no %, keep raw value)
fn parse_stat_lines(
    lines: &[String],
    attrs: &HashMap<String, AttributeInfo>,
) -> Vec<(String, String, f32, bool, bool, Option<AttributeInfo>)> {
    let mut out = Vec::new();

    for line in lines {
        let t = line.trim();
        let (is_negative, is_multiplier, rest) = if t.starts_with('-') {
            (true, false, &t[1..])
        } else if t.starts_with('+') {
            (false, false, &t[1..])
        } else if t.starts_with('x') || t.starts_with('X') {
            (false, true, &t[1..])
        } else {
            continue;
        };

        let rest = rest.trim_start();
        let num_end = rest.chars().take_while(|c| c.is_ascii_digit() || *c == '.').count();
        if num_end == 0 { continue; }

        let value: f32 = match rest[..num_end].parse() {
            Ok(v) => v,
            Err(_) => continue,
        };

        // Multiplier lines have no '%' suffix; percentage lines may or may not.
        let name_part = if is_multiplier {
            rest[num_end..].trim()
        } else {
            rest[num_end..].trim_start_matches('%').trim()
        };
        if name_part.is_empty() { continue; }

        let (slug, attr) = match_attribute(name_part, attrs);
        let display_name = attr.as_ref().map(|a| a.name.clone()).unwrap_or_else(|| name_part.to_string());

        eprintln!(
            "[riven] stat parse: {:?} → slug={slug:?} val={value} neg={is_negative} multiplier={is_multiplier}",
            name_part
        );
        out.push((slug, display_name, value, is_negative, is_multiplier, attr));
    }

    out
}

fn match_attribute(name_ocr: &str, attrs: &HashMap<String, AttributeInfo>) -> (String, Option<AttributeInfo>) {
    let q = name_ocr.to_lowercase();
    let mut best_score = 0.0_f64;
    let mut best_slug = String::new();
    let mut best_attr: Option<AttributeInfo> = None;

    for (slug, attr) in attrs.iter() {
        let score = similarity(&q, &attr.name.to_lowercase());
        if score > best_score {
            best_score = score;
            best_slug = slug.clone();
            best_attr = Some(attr.clone());
        }
    }

    eprintln!("[riven] attr match: {:?} → {:?} (score {best_score:.3})", q, best_slug);
    if best_score >= 0.60 { (best_slug, best_attr) } else { (String::new(), None) }
}

fn parse_roll_count(lines: &[String]) -> u32 {
    for line in lines {
        let lower = line.to_lowercase();
        if lower.contains("roll") {
            let digits: String = lower
                .chars()
                .skip_while(|c| !c.is_ascii_digit())
                .take_while(|c| c.is_ascii_digit())
                .collect();
            if let Ok(n) = digits.parse::<u32>() {
                return n;
            }
        }
    }
    0
}

// ── Scoring ───────────────────────────────────────────────────────────────────

fn compute_build_score(positives: &[&ParsedStat], negative: Option<&ParsedStat>, riven_type: &str) -> f32 {
    if positives.is_empty() { return 0.0; }
    let max = top_n_weights(riven_type, positives.len());
    if max <= 0.0 { return 0.0; }
    let raw = (positives.iter().map(|s| s.weight).sum::<f32>() / max).clamp(0.0, 1.0);
    let neg_mod = negative.map(|s| negative_multiplier(&s.slug, riven_type)).unwrap_or(1.0);
    (raw * neg_mod).clamp(0.0, 1.0)
}

fn compute_market_score(build_score: f32, tier: &str, roll_count: u32) -> f32 {
    let tier_w = match tier { "S" => 1.0, "A" => 0.8, "B" => 0.6, "C" => 0.4, "D" => 0.2, _ => 0.05 };
    let roll_mod = 1.0 - ((roll_count.saturating_sub(5)) as f32 * 0.01).min(0.15);
    (tier_w * build_score * roll_mod).clamp(0.0, 1.0)
}

fn score_to_grade(score: f32) -> String {
    match score {
        s if s >= 0.85 => "S",
        s if s >= 0.70 => "A",
        s if s >= 0.55 => "B",
        s if s >= 0.40 => "C",
        s if s >= 0.25 => "D",
        _ => "F",
    }.to_string()
}

fn weight_label(w: f32) -> String {
    match w {
        x if x >= 1.0  => "God",
        x if x >= 0.75 => "Great",
        x if x >= 0.5  => "Good",
        x if x > 0.0   => "Filler",
        _ => "Dump",
    }.to_string()
}

// ── Roll quality ──────────────────────────────────────────────────────────────

/// Theoretical max % value at disposition 1.0 with 3P+1N (the community reference config).
/// Returns None for stats that don't roll as a simple percentage (e.g. punch_through, multipliers).
fn base_stat_value(slug: &str) -> Option<f32> {
    Some(match slug {
        "damage"                  => 99.0,
        "critical_chance"         => 66.0,
        "critical_damage"         => 99.0,
        "multishot"               => 99.0,
        "fire_rate"               => 60.0,
        "status_chance"           => 60.0,
        "reload_speed"            => 40.0,
        "heat_damage"             => 90.0,
        "cold_damage"             => 90.0,
        "electric_damage"         => 90.0,
        "toxin_damage"            => 90.0,
        "radiation_damage"        => 90.0,
        "magnetic_damage"         => 90.0,
        "viral_damage"            => 90.0,
        "corrosive_damage"        => 90.0,
        "blast_damage"            => 90.0,
        "gas_damage"              => 90.0,
        "magazine_capacity"       => 40.0,
        "ammo_maximum"            => 40.0,
        "attack_speed"            => 55.0,
        "range"                   => 40.0,
        "combo_duration"          => 40.0,
        "heavy_attack_efficiency" => 40.0,
        "slide_attack"            => 90.0,
        "finisher_damage"         => 60.0,
        "channeling_efficiency"   => 40.0,
        "status_duration"         => 40.0,
        "flight_speed"            => 60.0,
        "zoom"                    => 44.0,
        _ => return None,
    })
}

/// Multiplier on top of base × disposition for different slot configurations.
/// Reference (1.0) = 3 positives + 1 negative.
fn slot_factor(n_positive: usize, has_negative: bool) -> f32 {
    match (n_positive, has_negative) {
        (2, false) => 1.00,
        (2, true)  => 1.25,
        (3, false) => 0.75,
        (3, true)  => 1.00,
        _          => 1.00,
    }
}

// ── Stat weight tables ────────────────────────────────────────────────────────

fn stat_weight(slug: &str, riven_type: &str) -> f32 {
    match slug {
        // Universal god tier
        "critical_chance" | "critical_damage" | "damage" => 1.0,

        // Multishot: god on all ranged types, not applicable on melee
        "multishot" if riven_type != "melee" && riven_type != "zaw" => 1.0,

        // Melee-specific
        "attack_speed" if matches!(riven_type, "melee" | "zaw") => 0.75,
        "range"        if matches!(riven_type, "melee" | "zaw") => 0.5,
        "status_chance" if matches!(riven_type, "melee" | "zaw") => 0.5,
        "combo_duration" | "slide_attack" | "follow_through" | "heavy_attack_efficiency"
            if matches!(riven_type, "melee" | "zaw") => 0.25,
        "finisher_damage" | "channeling_efficiency" => 0.0,

        // Shotgun elevates status chance
        "status_chance" if riven_type == "shotgun" => 0.75,
        "punch_through" if riven_type == "shotgun" => 0.0,

        // Ranged generics
        "fire_rate"       => 0.75,
        "status_chance"   => 0.5,
        "reload_speed"    => 0.5,
        "heat_damage" | "cold_damage" | "electric_damage" | "toxin_damage" => 0.5,
        "magazine_capacity" | "punch_through" | "ammo_maximum" => 0.25,

        // Archgun projectiles are physical; flight speed matters
        "flight_speed" if riven_type == "archgun" => 0.5,
        "flight_speed" => 0.0,

        "zoom" | "status_duration" => 0.0,
        _ => 0.1,
    }
}

/// Sum of the top-N ideal weights for a given riven_type (used to normalise build score).
fn top_n_weights(riven_type: &str, n: usize) -> f32 {
    // Best possible weights for any riven_type, ordered descending.
    let top: &[f32] = match riven_type {
        "melee" | "zaw" => &[1.0, 1.0, 1.0, 0.75, 0.5],
        _ => &[1.0, 1.0, 1.0, 0.75, 0.5],
    };
    top.iter().take(n).sum()
}

fn negative_multiplier(slug: &str, riven_type: &str) -> f32 {
    match slug {
        "zoom"                                                          => 1.15,
        "flight_speed" if riven_type != "archgun"                      => 1.15,
        "recoil" | "impact_damage" | "puncture_damage" | "ammo_maximum" => 1.05,
        "damage" | "multishot" | "critical_chance" | "critical_damage" => 0.65,
        "status_chance"                                                 => 0.85,
        "fire_rate" if matches!(riven_type, "rifle" | "pistol" | "kitgun") => 0.85,
        "range"         if matches!(riven_type, "melee" | "zaw") => 0.85,
        "attack_speed"  if matches!(riven_type, "melee" | "zaw") => 0.85,
        _                                                               => 1.0,
    }
}

// ── WFM weapon tier ───────────────────────────────────────────────────────────

async fn fetch_weapon_tier(client: &reqwest::Client, weapon_slug: &str) -> (String, f32) {
    #[derive(Deserialize)]
    struct AuctionResp { data: Vec<AuctionEntry> }
    #[derive(Deserialize)]
    struct AuctionEntry {
        #[serde(rename = "buyoutPrice")]
        buyout_price: Option<u32>,
    }

    let url = format!(
        "https://api.warframe.market/v2/auctions/search\
         ?type=riven&weapon_url_name={weapon_slug}&sort_by=price_asc&buyout_policy=with_buyout"
    );
    let Ok(resp) = client.get(&url).header("Language", "en").send().await else {
        return ("?".to_string(), 0.0);
    };
    let Ok(body) = resp.json::<AuctionResp>().await else {
        return ("?".to_string(), 0.0);
    };

    let mut prices: Vec<u32> = body.data.iter()
        .filter_map(|e| e.buyout_price)
        .take(20)
        .collect();
    prices.sort_unstable();

    if prices.is_empty() { return ("?".to_string(), 0.0); }

    let median = prices[prices.len() / 2] as f32;
    let filtered: Vec<u32> = prices.into_iter().filter(|&p| p as f32 <= median * 3.0).collect();
    let median = filtered[filtered.len() / 2] as f32;

    let tier = match median as u32 {
        p if p >= 600 => "S",
        p if p >= 300 => "A",
        p if p >= 150 => "B",
        p if p >= 75  => "C",
        p if p >= 25  => "D",
        _             => "F",
    };
    eprintln!("[riven] weapon tier for {weapon_slug}: {tier} (median {median:.0}p)");
    (tier.to_string(), median)
}

// ── Text utilities ────────────────────────────────────────────────────────────

fn normalize_for_match(s: &str) -> String {
    s.to_lowercase()
        .split_whitespace()
        .filter(|t| t.len() > 1)
        .collect::<Vec<_>>()
        .join(" ")
}

fn similarity(a: &str, b: &str) -> f64 {
    let jw = strsim::jaro_winkler(a, b);
    let a_set: std::collections::HashSet<&str> = a.split_whitespace().collect();
    let b_set: std::collections::HashSet<&str> = b.split_whitespace().collect();
    let inter = a_set.intersection(&b_set).count();
    let union = a_set.union(&b_set).count();
    let jaccard = if union == 0 { 0.0 } else { inter as f64 / union as f64 };
    0.6 * jw + 0.4 * jaccard
}
