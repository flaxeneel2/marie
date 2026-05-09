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

/// Fuzzy-match `raw` against all cached item names, returning the best hit above
/// a similarity threshold. Falls back to an empty match on failure.
fn best_match(raw: &str, cache: &HashMap<String, CachedItem>) -> (String, Option<CachedItem>) {
    let query = raw.to_lowercase();
    let mut best_score = 0.0_f64;
    let mut best_name = String::new();
    let mut best_item: Option<CachedItem> = None;

    for (name, item) in cache.iter() {
        let score = strsim::jaro_winkler(&query, name);
        if score > best_score {
            best_score = score;
            best_name = name.clone();
            best_item = Some(item.clone());
        }
    }

    // Require at least 70% similarity to consider a match valid.
    if best_score >= 0.70 {
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
