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

/// Items that appear in relic rewards but have no meaningful market value.
/// They are returned as unmatched so the overlay shows "—" for both plat and ducats.
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

/// Fuzzy-match `raw` against all cached item names, returning the best hit above
/// a similarity threshold. Falls back to an empty match on failure.
fn best_match(raw: &str, cache: &HashMap<String, CachedItem>) -> (String, Option<CachedItem>) {
    let query = raw.to_lowercase();

    if NON_MARKET.iter().any(|&nm| query.contains(nm)) {
        return (String::new(), None);
    }

    let q_words = query.split_whitespace().count();
    let mut best_score = 0.0_f64;
    let mut best_name = String::new();
    let mut best_item: Option<CachedItem> = None;

    for (name, item) in cache.iter() {
        // Hard gate: word count must be within ±1 — prevents short OCR fragments
        // from matching multi-word item names and vice-versa.
        let t_words = name.split_whitespace().count();
        if q_words.abs_diff(t_words) > 1 { continue; }

        let score = similarity(&query, name);
        if score > best_score {
            best_score = score;
            best_name = name.clone();
            best_item = Some(item.clone());
        }
    }

    eprintln!("[wfm] best match for {query:?}: {best_name:?} (score {best_score:.3})");

    if best_score >= 0.75 {
        (best_name, best_item)
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
