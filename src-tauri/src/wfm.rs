use once_cell::sync::Lazy;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use tokio::sync::RwLock;

// ── Cache ─────────────────────────────────────────────────────────────────────

static CACHE: Lazy<RwLock<HashMap<String, CachedItem>>> =
    Lazy::new(|| RwLock::new(HashMap::new()));

#[derive(Clone)]
pub struct CachedItem {
    pub slug: String,
    pub ducats: Option<u32>,
}

// ── API response types ────────────────────────────────────────────────────────

#[derive(Deserialize)]
struct ItemsResponse {
    data: Vec<ApiItem>,
}

#[derive(Deserialize)]
struct ApiItem {
    slug: String,
    ducats: Option<u32>,
    i18n: HashMap<String, ApiLang>,
}

#[derive(Deserialize)]
struct ApiLang {
    name: String,
}

#[derive(Deserialize)]
struct TopOrdersResponse {
    data: TopOrdersData,
}

#[derive(Deserialize)]
struct TopOrdersData {
    sell: Vec<OrderEntry>,
}

#[derive(Deserialize)]
struct OrderEntry {
    platinum: u32,
}

// ── Public API ────────────────────────────────────────────────────────────────

pub async fn init_cache() -> Result<usize, reqwest::Error> {
    let client = reqwest::Client::new();
    let resp: ItemsResponse = client
        .get("https://api.warframe.market/v2/items")
        .header("Language", "en")
        .send()
        .await?
        .json()
        .await?;

    let mut map = CACHE.write().await;
    for item in resp.data {
        let name = item
            .i18n
            .get("en")
            .map(|l| l.name.to_lowercase())
            .unwrap_or_default();
        if !name.is_empty() {
            map.insert(name, CachedItem { slug: item.slug, ducats: item.ducats });
        }
    }
    Ok(map.len())
}

#[derive(Serialize, Clone)]
pub struct ItemPriceResult {
    /// Raw OCR text returned by the recogniser
    pub ocr_text: String,
    /// Best-match item name from WFM (empty when unmatched)
    pub matched_name: String,
    pub slug: String,
    pub ducats: Option<u32>,
    /// Lowest visible sell order price in platinum
    pub plat_min_sell: Option<u32>,
}

pub async fn prices_for_names(names: Vec<String>) -> Vec<ItemPriceResult> {
    let cache = CACHE.read().await;
    let client = reqwest::Client::new();
    let mut results = Vec::with_capacity(names.len());

    for raw in names {
        let (matched_name, cached) = best_match(&raw, &cache);

        let plat_min_sell = match cached.as_ref() {
            Some(c) => fetch_min_sell(&client, &c.slug).await,
            None => None,
        };

        results.push(ItemPriceResult {
            ocr_text: raw,
            matched_name,
            slug: cached.as_ref().map(|c| c.slug.clone()).unwrap_or_default(),
            ducats: cached.as_ref().and_then(|c| c.ducats),
            plat_min_sell,
        });
    }

    results
}

// ── Helpers ───────────────────────────────────────────────────────────────────

/// Relic reward items that have no WFM listing and no ducat value.
/// These are fuzzy-matched as a fallback after the market search fails,
/// so OCR noise is tolerated and new entries can be added here freely.
const NON_MARKET: &[&str] = &[
    "forma blueprint",
    "2x forma blueprint",
    "exilus adapter blueprint",
];

/// Combined similarity: 60% Jaro-Winkler (character level) + 40% Jaccard on
/// word tokens (semantic level).  Using both means a candidate needs to look
/// right *and* share the right words, reducing false positives from OCR noise.
fn similarity(query: &str, target: &str) -> f64 {
    let jw = strsim::jaro_winkler(query, target);

    let q: std::collections::HashSet<&str> = query.split_whitespace().collect();
    let t: std::collections::HashSet<&str> = target.split_whitespace().collect();
    let intersection = q.intersection(&t).count();
    let union = q.union(&t).count();
    let jaccard = if union == 0 { 0.0 } else { intersection as f64 / union as f64 };

    0.6 * jw + 0.4 * jaccard
}

/// Fix common OCR character confusions before fuzzy matching.
/// Applied after lowercasing so substitutions are case-insensitive.
fn normalize_ocr(s: &str) -> String {
    let cleaned = s.replace(['|', '!', ';', ':'], "");
    let tokens: Vec<&str> = cleaned.split_whitespace().collect();

    let mut out: Vec<String> = Vec::with_capacity(tokens.len());
    let mut i = 0;
    while i < tokens.len() {
        let tok = tokens[i];
        // Re-join "2 x" → "2x": OCR sometimes splits a quantity prefix like
        // "2x" into two tokens. Merge a purely numeric token with the next
        // token when it is a single letter.
        if tok.chars().all(|c| c.is_ascii_digit()) {
            if let Some(&next) = tokens.get(i + 1) {
                if next.len() == 1 && next.chars().next().map_or(false, |c| c.is_ascii_alphabetic()) {
                    out.push(format!("{tok}{next}"));
                    i += 2;
                    continue;
                }
            }
        }
        out.push(tok.to_string());
        i += 1;
    }

    // Drop remaining isolated single-character tokens — capture-edge noise.
    out.into_iter()
        .filter(|tok| tok.len() > 1)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Two-step fuzzy match:
///   1. Market items (WFM cache) — returns a CachedItem for price lookup.
///   2. Non-market items (NON_MARKET list) — returns the name only; plat/ducats
///      will show as "—" because there is no CachedItem to fetch prices from.
/// Falls back to an empty unmatched result if neither step finds a hit above 0.70.
fn best_match(raw: &str, cache: &HashMap<String, CachedItem>) -> (String, Option<CachedItem>) {
    let query = normalize_ocr(&raw.to_lowercase());
    let q_words = query.split_whitespace().count();

    // ── Step 1: market items ──────────────────────────────────────────────────
    let mut best_score = 0.0_f64;
    let mut best_name = String::new();
    let mut best_item: Option<CachedItem> = None;

    for (name, item) in cache.iter() {
        let t_words = name.split_whitespace().count();
        if q_words.abs_diff(t_words) > 1 { continue; }
        let score = similarity(&query, name);
        if score > best_score {
            best_score = score;
            best_name = name.clone();
            best_item = Some(item.clone());
        }
    }

    eprintln!("[wfm] market match for {query:?}: {best_name:?} (score {best_score:.3})");

    if best_score >= 0.70 {
        return (best_name, best_item);
    }

    // ── Step 2: non-market items ──────────────────────────────────────────────
    let mut best_nm_score = 0.0_f64;
    let mut best_nm_name = String::new();

    for &name in NON_MARKET {
        let t_words = name.split_whitespace().count();
        if q_words.abs_diff(t_words) > 1 { continue; }
        let score = similarity(&query, name);
        if score > best_nm_score {
            best_nm_score = score;
            best_nm_name = name.to_string();
        }
    }

    eprintln!("[wfm] non-market match for {query:?}: {best_nm_name:?} (score {best_nm_score:.3})");

    if best_nm_score >= 0.70 {
        (best_nm_name, None)
    } else {
        (String::new(), None)
    }
}

async fn fetch_min_sell(client: &reqwest::Client, slug: &str) -> Option<u32> {
    let url = format!("https://api.warframe.market/v2/orders/item/{slug}/top");
    let resp: TopOrdersResponse = client
        .get(&url)
        .header("Language", "en")
        .send()
        .await
        .ok()?
        .json()
        .await
        .ok()?;

    resp.data.sell.into_iter().map(|o| o.platinum).min()
}
