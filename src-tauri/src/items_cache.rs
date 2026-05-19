use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

const CACHE_TTL_SECS: u64 = 7 * 24 * 3600;
const API_URL: &str =
    "https://api.warframestat.us/items/?remove=patchlogs,introduced";

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

// ── Raw API types (fetch only, not stored) ────────────────────────────────────

#[derive(Debug, Deserialize)]
struct RawComponent {
    #[serde(rename = "uniqueName")]
    unique_name: String,
    name: String,
    #[serde(rename = "type", default)]
    item_type: String,
    #[serde(rename = "imageName", default)]
    image_name: String,
}

#[derive(Debug, Deserialize)]
struct RawItem {
    #[serde(rename = "uniqueName")]
    unique_name: String,
    name: String,
    #[serde(default)]
    category: String,
    #[serde(rename = "type", default)]
    item_type: String,
    #[serde(rename = "imageName", default)]
    image_name: String,
    #[serde(rename = "fusionLimit")]
    fusion_limit: Option<i32>,
    rarity: Option<String>,
    polarity: Option<String>,
    #[serde(default)]
    components: Vec<RawComponent>,
}

// ── Disk cache (processed maps only) ─────────────────────────────────────────

#[derive(Debug, Deserialize, Serialize)]
struct ItemsFileCache {
    fetched_at: u64,
    names: HashMap<String, String>,
    categories: HashMap<String, String>,
    types: HashMap<String, String>,
    images: HashMap<String, String>,
    // For recipe components: the parent item's imageName to overlay on the blueprint bg
    overlay_images: HashMap<String, String>,
    #[serde(default)]
    fusion_limits: HashMap<String, i32>,
    #[serde(default)]
    rarities: HashMap<String, String>,
    #[serde(default)]
    polarities: HashMap<String, String>,
}

// ── Public output ─────────────────────────────────────────────────────────────

pub struct ItemMaps {
    pub names: HashMap<String, String>,
    pub categories: HashMap<String, String>,
    pub types: HashMap<String, String>,
    pub images: HashMap<String, String>,
    pub overlay_images: HashMap<String, String>,
    pub fusion_limits: HashMap<String, i32>,
    pub rarities: HashMap<String, String>,
    pub polarities: HashMap<String, String>,
}

// ── Processing ────────────────────────────────────────────────────────────────

fn build_maps(items: Vec<RawItem>) -> ItemMaps {
    let cap = items.len() * 5;
    let mut names = HashMap::with_capacity(cap);
    let mut categories = HashMap::with_capacity(cap);
    let mut types = HashMap::with_capacity(cap);
    let mut images = HashMap::with_capacity(cap);
    let mut overlay_images = HashMap::new();
    let mut fusion_limits: HashMap<String, i32> = HashMap::new();
    let mut rarities: HashMap<String, String> = HashMap::new();
    let mut polarities: HashMap<String, String> = HashMap::new();

    for item in items {
        // Only process recipe blueprint parts, not crafting ingredient components.
        // Ingredient components (resources, cells, etc.) share uniqueNames with
        // top-level items and must not be overwritten with generated blueprint names.
        for comp in &item.components {
            if !comp.unique_name.starts_with("/Lotus/Types/Recipes/") {
                continue;
            }
            let display_name = if comp.name == "Blueprint" {
                format!("{} Blueprint", item.name)
            } else {
                format!("{} {} Blueprint", item.name, comp.name)
            };
            names.insert(comp.unique_name.clone(), display_name);
            categories.insert(comp.unique_name.clone(), item.category.clone());
            types.insert(comp.unique_name.clone(), comp.item_type.clone());
            if !comp.image_name.is_empty() {
                images.insert(comp.unique_name.clone(), comp.image_name.clone());
            }
            // Only overlay parent image when component itself is the generic blueprint.png
            if comp.image_name == "blueprint.png" && !item.image_name.is_empty() {
                overlay_images.insert(comp.unique_name.clone(), item.image_name.clone());
            }
        }

        names.insert(item.unique_name.clone(), item.name);
        categories.insert(item.unique_name.clone(), item.category);
        types.insert(item.unique_name.clone(), item.item_type);
        if !item.image_name.is_empty() {
            images.insert(item.unique_name.clone(), item.image_name);
        }
        if let Some(fl) = item.fusion_limit {
            fusion_limits.insert(item.unique_name.clone(), fl);
        }
        if let Some(r) = item.rarity {
            rarities.insert(item.unique_name.clone(), r);
        }
        if let Some(p) = item.polarity {
            polarities.insert(item.unique_name, p);
        }
    }

    ItemMaps { names, categories, types, images, overlay_images, fusion_limits, rarities, polarities }
}

// ── Disk I/O ──────────────────────────────────────────────────────────────────

fn cache_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|d| d.join("items_name_cache.json"))
}

fn load_disk_cache(app: &AppHandle) -> Option<ItemsFileCache> {
    let path = cache_path(app)?;
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn save_disk_cache(app: &AppHandle, cache: &ItemsFileCache) {
    let Some(path) = cache_path(app) else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(cache) {
        let _ = std::fs::write(path, json);
    }
}

// ── Public API ────────────────────────────────────────────────────────────────

fn empty_maps() -> ItemMaps {
    ItemMaps {
        names: HashMap::new(),
        categories: HashMap::new(),
        types: HashMap::new(),
        images: HashMap::new(),
        overlay_images: HashMap::new(),
        fusion_limits: HashMap::new(),
        rarities: HashMap::new(),
        polarities: HashMap::new(),
    }
}

pub async fn get_maps(app: &AppHandle) -> ItemMaps {
    if let Some(cache) = load_disk_cache(app) {
        let age = now_secs().saturating_sub(cache.fetched_at);
        if age < CACHE_TTL_SECS {
            eprintln!("[items_cache] cache hit ({age}s old, {} entries)", cache.names.len());
            return ItemMaps {
                names: cache.names,
                categories: cache.categories,
                types: cache.types,
                images: cache.images,
                overlay_images: cache.overlay_images,
                fusion_limits: cache.fusion_limits,
                rarities: cache.rarities,
                polarities: cache.polarities,
            };
        }
        eprintln!("[items_cache] cache expired ({age}s old), refreshing");
    }

    eprintln!("[items_cache] fetching from warframestat.us");
    let items = match reqwest::get(API_URL).await {
        Ok(resp) => match resp.json::<Vec<RawItem>>().await {
            Ok(v) => { eprintln!("[items_cache] fetched {} items", v.len()); v }
            Err(e) => { eprintln!("[items_cache] parse failed: {e}"); return empty_maps(); }
        },
        Err(e) => { eprintln!("[items_cache] fetch failed: {e}"); return empty_maps(); }
    };

    let maps = build_maps(items);
    let cache = ItemsFileCache {
        fetched_at: now_secs(),
        names: maps.names,
        categories: maps.categories,
        types: maps.types,
        images: maps.images,
        overlay_images: maps.overlay_images,
        fusion_limits: maps.fusion_limits,
        rarities: maps.rarities,
        polarities: maps.polarities,
    };
    save_disk_cache(app, &cache);
    ItemMaps {
        names: cache.names,
        categories: cache.categories,
        types: cache.types,
        images: cache.images,
        overlay_images: cache.overlay_images,
        fusion_limits: cache.fusion_limits,
        rarities: cache.rarities,
        polarities: cache.polarities,
    }
}
