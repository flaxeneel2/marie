// Linux + memory feature only — requires the nonce from account_memory.

use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::time::{SystemTime, UNIX_EPOCH};
use tauri::{AppHandle, Manager};

const CACHE_TTL_SECS: u64 = 300;
const API_BASE: &str = "https://api.warframe.com/api/inventory.php";

fn now_secs() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs()
}

fn cache_path(app: &AppHandle) -> Option<std::path::PathBuf> {
    app.path()
        .app_data_dir()
        .ok()
        .map(|d| d.join("inventory_cache.json"))
}

fn load_cache(app: &AppHandle) -> Option<InventoryCache> {
    let path = cache_path(app)?;
    let raw = std::fs::read_to_string(path).ok()?;
    serde_json::from_str(&raw).ok()
}

fn save_cache(app: &AppHandle, cache: &InventoryCache) {
    let Some(path) = cache_path(app) else { return };
    if let Some(parent) = path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }
    if let Ok(json) = serde_json::to_string(cache) {
        let _ = std::fs::write(path, json);
    }
}

// ── MongoDB BSON primitives ───────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct MongoOid {
    #[serde(rename = "$oid")]
    pub oid: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct MongoDateInner {
    #[serde(rename = "$numberLong")]
    ms: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MongoDate {
    #[serde(rename = "$date")]
    date: MongoDateInner,
}

impl MongoDate {
    pub fn as_unix_ms(&self) -> Option<i64> {
        self.date.ms.parse().ok()
    }
}

// ── Shared small types ────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct Bin {
    #[serde(rename = "Slots", default)]
    pub slots: i32,
    #[serde(rename = "Extra", default)]
    pub extra: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CountedItem {
    #[serde(rename = "ItemType")]
    pub item_type: String,
    #[serde(rename = "ItemCount")]
    pub item_count: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct XpEntry {
    #[serde(rename = "ItemType")]
    pub item_type: String,
    #[serde(rename = "XP")]
    pub xp: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawUpgrade {
    #[serde(rename = "ItemType")]
    pub item_type: String,
    #[serde(rename = "ItemCount")]
    pub item_count: i32,
    #[serde(rename = "LastAdded")]
    pub last_added: Option<MongoOid>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Upgrade {
    #[serde(rename = "ItemType")]
    pub item_type: String,
    #[serde(rename = "ItemId")]
    pub item_id: MongoOid,
    #[serde(rename = "UpgradeFingerprint")]
    pub fingerprint: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct OwnedItem {
    #[serde(rename = "ItemType")]
    pub item_type: String,
    #[serde(rename = "ItemId")]
    pub item_id: Option<MongoOid>,
    #[serde(rename = "XP", default)]
    pub xp: i64,
    #[serde(rename = "UpgradeVer")]
    pub upgrade_ver: Option<i32>,
    #[serde(rename = "Features")]
    pub features: Option<i64>,
    #[serde(rename = "InfestationDate")]
    pub infestation_date: Option<MongoDate>,
    #[serde(rename = "Configs")]
    pub configs: Option<Value>,
    #[serde(rename = "ModularParts")]
    pub modular_parts: Option<Vec<String>>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Affiliation {
    #[serde(rename = "Tag")]
    pub tag: String,
    #[serde(rename = "Standing", default)]
    pub standing: i64,
    #[serde(rename = "Initiated", default)]
    pub initiated: bool,
    #[serde(rename = "Title")]
    pub title: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Booster {
    #[serde(rename = "ItemType")]
    pub item_type: String,
    #[serde(rename = "ExpiryDate")]
    pub expiry: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct QuestKey {
    #[serde(rename = "ItemType")]
    pub item_type: String,
    #[serde(rename = "unlock", default)]
    pub unlock: bool,
    #[serde(rename = "Completed", default)]
    pub completed: bool,
    #[serde(rename = "Progress")]
    pub progress: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PendingRecipe {
    #[serde(rename = "ItemType")]
    pub item_type: String,
    #[serde(rename = "CompletionDate")]
    pub completion_date: MongoDate,
    #[serde(rename = "ItemId")]
    pub item_id: MongoOid,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MissionEntry {
    #[serde(rename = "Tag")]
    pub tag: String,
    #[serde(rename = "Completes", default)]
    pub completes: i32,
    #[serde(rename = "Tier")]
    pub tier: Option<i32>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FusionTreasure {
    #[serde(rename = "ItemType")]
    pub item_type: String,
    #[serde(rename = "ItemCount")]
    pub item_count: i32,
    #[serde(rename = "Sockets", default)]
    pub sockets: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct FocusXp {
    #[serde(rename = "AP_POWER", default)]
    pub power: i64,
    #[serde(rename = "AP_ATTACK", default)]
    pub attack: i64,
    #[serde(rename = "AP_TACTIC", default)]
    pub tactic: i64,
    #[serde(rename = "AP_WARD", default)]
    pub ward: i64,
    #[serde(rename = "AP_DEFENSE", default)]
    pub defense: i64,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AlignmentData {
    #[serde(rename = "Alignment", default)]
    pub alignment: i32,
    #[serde(rename = "Wisdom", default)]
    pub wisdom: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DuviriInfo {
    #[serde(rename = "Seed")]
    pub seed: i64,
    #[serde(rename = "NumCompletions", default)]
    pub completions: i32,
    #[serde(rename = "StalkerChance", default)]
    pub stalker_chance: i32,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PlayerSettings {
    #[serde(rename = "FriendInvRestriction")]
    pub friend_inv_restriction: Option<String>,
    #[serde(rename = "GiftMode")]
    pub gift_mode: Option<String>,
    #[serde(rename = "GuildInvRestriction")]
    pub guild_inv_restriction: Option<String>,
    #[serde(rename = "ShowFriendInvNotifications", default)]
    pub show_friend_notifications: bool,
    #[serde(rename = "SubscribedToSurveys", default)]
    pub subscribed_surveys: bool,
    #[serde(rename = "TradingRulesConfirmed", default)]
    pub trading_rules_confirmed: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ChallengeProgress {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Progress", default)]
    pub progress: i64,
}

// ── Main Inventory struct ─────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Inventory {
    // Account metadata
    #[serde(rename = "Created")]
    pub created: Option<MongoDate>,
    #[serde(rename = "TrainingDate")]
    pub training_date: Option<MongoDate>,
    #[serde(rename = "GuildId")]
    pub guild_id: Option<MongoOid>,
    #[serde(rename = "LastInventorySync")]
    pub last_inventory_sync: Option<MongoOid>,
    #[serde(rename = "NextRefill")]
    pub next_refill: Option<MongoDate>,

    // Currency
    #[serde(rename = "RegularCredits", default)]
    pub credits: i64,
    #[serde(rename = "PremiumCredits", default)]
    pub platinum: i64,
    #[serde(rename = "PremiumCreditsFree", default)]
    pub platinum_free: i64,
    #[serde(rename = "FusionPoints", default)]
    pub endo: i64,
    #[serde(rename = "PrimeTokens", default)]
    pub prime_tokens: i32,
    #[serde(rename = "RewardSeed")]
    pub reward_seed: Option<i64>,

    // Mastery / XP
    #[serde(rename = "PlayerLevel", default)]
    pub mastery_rank: i32,
    #[serde(rename = "XPInfo", default)]
    pub xp_info: Vec<XpEntry>,
    #[serde(rename = "DailyFocus", default)]
    pub daily_focus: i64,

    // Trading
    #[serde(rename = "TradesRemaining", default)]
    pub trades_remaining: i32,
    #[serde(rename = "GiftsRemaining", default)]
    pub gifts_remaining: i32,

    // Slots
    #[serde(rename = "SuitBin", default)]
    pub suit_bin: Bin,
    #[serde(rename = "WeaponBin", default)]
    pub weapon_bin: Bin,
    #[serde(rename = "SentinelBin", default)]
    pub sentinel_bin: Bin,
    #[serde(rename = "SpaceSuitBin", default)]
    pub archwing_bin: Bin,
    #[serde(rename = "SpaceWeaponBin", default)]
    pub arch_weapon_bin: Bin,
    #[serde(rename = "MechBin", default)]
    pub mech_bin: Bin,
    #[serde(rename = "OperatorAmpBin", default)]
    pub amp_bin: Bin,
    #[serde(rename = "RandomModBin", default)]
    pub riven_bin: Bin,
    #[serde(rename = "CrewShipSalvageBin", default)]
    pub railjack_salvage_bin: Bin,
    #[serde(rename = "CrewMemberBin", default)]
    pub crew_bin: Bin,
    #[serde(rename = "PvpBonusLoadoutBin", default)]
    pub pvp_bonus_bin: Bin,
    #[serde(rename = "PveBonusLoadoutBin", default)]
    pub pve_bonus_bin: Bin,

    // Owned gear
    #[serde(rename = "Suits", default)]
    pub warframes: Vec<OwnedItem>,
    #[serde(rename = "LongGuns", default)]
    pub primaries: Vec<OwnedItem>,
    #[serde(rename = "Pistols", default)]
    pub secondaries: Vec<OwnedItem>,
    #[serde(rename = "Melee", default)]
    pub melee: Vec<OwnedItem>,
    #[serde(rename = "Sentinels", default)]
    pub sentinels: Vec<OwnedItem>,
    #[serde(rename = "SentinelWeapons", default)]
    pub sentinel_weapons: Vec<OwnedItem>,
    #[serde(rename = "SpaceSuits", default)]
    pub archwings: Vec<OwnedItem>,
    #[serde(rename = "SpaceGuns", default)]
    pub arch_guns: Vec<OwnedItem>,
    #[serde(rename = "SpaceMelee", default)]
    pub arch_melee: Vec<OwnedItem>,
    #[serde(rename = "MechSuits", default)]
    pub mechs: Vec<OwnedItem>,
    #[serde(rename = "OperatorAmps", default)]
    pub amps: Vec<OwnedItem>,
    #[serde(rename = "Scoops", default)]
    pub fishing_spears: Vec<OwnedItem>,
    #[serde(rename = "DataKnives", default)]
    pub datamasses: Vec<OwnedItem>,
    #[serde(rename = "KubrowPets", default)]
    pub companions: Vec<OwnedItem>,
    #[serde(rename = "Horses", default)]
    pub kaithe: Vec<OwnedItem>,
    #[serde(rename = "Motorcycles", default)]
    pub merulina: Vec<OwnedItem>,
    #[serde(rename = "CrewShips", default)]
    pub railjacks: Vec<OwnedItem>,
    #[serde(rename = "SpecialItems", default)]
    pub special_items: Vec<OwnedItem>,
    #[serde(rename = "DrifterMelee", default)]
    pub drifter_melee: Vec<OwnedItem>,
    #[serde(rename = "CrewShipHarnesses", default)]
    pub plexus: Vec<OwnedItem>,

    // Mods
    #[serde(rename = "RawUpgrades", default)]
    pub raw_upgrades: Vec<RawUpgrade>,
    #[serde(rename = "Upgrades", default)]
    pub ranked_mods: Vec<Upgrade>,
    #[serde(rename = "FocusUpgrades", default)]
    pub focus_upgrades: Vec<Value>,

    // Resources / consumables
    #[serde(rename = "MiscItems", default)]
    pub misc_items: Vec<CountedItem>,
    #[serde(rename = "Consumables", default)]
    pub consumables: Vec<CountedItem>,
    #[serde(rename = "FusionTreasures", default)]
    pub fusion_treasures: Vec<FusionTreasure>,
    #[serde(rename = "ShipDecorations", default)]
    pub decorations: Vec<CountedItem>,
    #[serde(rename = "CrewShipRawSalvage", default)]
    pub railjack_resources: Vec<CountedItem>,
    #[serde(rename = "CrewShipAmmo", default)]
    pub railjack_ammo: Vec<CountedItem>,
    #[serde(rename = "EmailItems", default)]
    pub email_items: Vec<CountedItem>,
    #[serde(rename = "LevelKeys", default)]
    pub level_keys: Vec<CountedItem>,

    // Foundry
    #[serde(rename = "Recipes", default)]
    pub blueprints: Vec<CountedItem>,
    #[serde(rename = "PendingRecipes", default)]
    pub pending_recipes: Vec<PendingRecipe>,

    // Syndicate standing
    #[serde(rename = "Affiliations", default)]
    pub affiliations: Vec<Affiliation>,

    // Focus
    #[serde(rename = "FocusXP", default)]
    pub focus_xp: FocusXp,
    #[serde(rename = "FocusAbility")]
    pub focus_ability: Option<String>,

    // Quests
    #[serde(rename = "QuestKeys", default)]
    pub quests: Vec<QuestKey>,

    // Challenges / season
    #[serde(rename = "ChallengeProgress", default)]
    pub challenge_progress: Vec<ChallengeProgress>,
    #[serde(rename = "ChallengesFixVersion")]
    pub challenges_fix_version: Option<i32>,
    #[serde(rename = "ChallengeInstanceStates", default)]
    pub challenge_instances: Vec<Value>,
    #[serde(rename = "SeasonChallengeHistory", default)]
    pub season_challenges: Vec<Value>,

    // Missions
    #[serde(rename = "Missions", default)]
    pub missions: Vec<MissionEntry>,

    // Boosters
    #[serde(rename = "Boosters", default)]
    pub boosters: Vec<Booster>,

    // Alignment
    #[serde(rename = "Alignment", default)]
    pub alignment: AlignmentData,

    // Duviri
    #[serde(rename = "DuviriInfo")]
    pub duviri: Option<DuviriInfo>,

    // Player skills (Drifter)
    #[serde(rename = "PlayerSkills")]
    pub player_skills: Option<Value>,

    // Settings
    #[serde(rename = "Settings")]
    pub settings: Option<PlayerSettings>,

    // Nemesis (Lich / Sister / Anger)
    #[serde(rename = "Nemesis")]
    pub nemesis: Option<Value>,

    // Daily affiliation caps
    #[serde(rename = "DailyAffiliation", default)]
    pub daily_affiliation: i32,
    #[serde(rename = "DailyAffiliationPvp", default)]
    pub daily_affiliation_pvp: i32,
    #[serde(rename = "DailyAffiliationLibrary", default)]
    pub daily_affiliation_library: i32,
    #[serde(rename = "DailyAffiliationCetus", default)]
    pub daily_affiliation_cetus: i32,
    #[serde(rename = "DailyAffiliationQuills", default)]
    pub daily_affiliation_quills: i32,
    #[serde(rename = "DailyAffiliationSolaris", default)]
    pub daily_affiliation_solaris: i32,
    #[serde(rename = "DailyAffiliationVox", default)]
    pub daily_affiliation_vox: i32,
    #[serde(rename = "DailyAffiliationEntrati", default)]
    pub daily_affiliation_entrati: i32,
    #[serde(rename = "DailyAffiliationNecraloid", default)]
    pub daily_affiliation_necraloid: i32,
    #[serde(rename = "DailyAffiliationVentkids", default)]
    pub daily_affiliation_ventkids: i32,
    #[serde(rename = "DailyAffiliationKahl", default)]
    pub daily_affiliation_kahl: i32,
    #[serde(rename = "DailyAffiliationZariman", default)]
    pub daily_affiliation_zariman: i32,
    #[serde(rename = "DailyAffiliationCavia", default)]
    pub daily_affiliation_cavia: i32,
    #[serde(rename = "DailyAffiliationHex", default)]
    pub daily_affiliation_hex: i32,

    // Completed content
    #[serde(rename = "CompletedSorties", default)]
    pub completed_sorties: Vec<String>,
    #[serde(rename = "LastSortieReward")]
    pub last_sortie_reward: Option<Value>,
    #[serde(rename = "LastLiteSortieReward")]
    pub last_lite_sortie_reward: Option<Value>,
    #[serde(rename = "SortieRewardAttenuation", default)]
    pub sortie_attenuation: Vec<Value>,
    #[serde(rename = "CompletedAlerts", default)]
    pub completed_alerts: Vec<String>,
    #[serde(rename = "NodeIntrosCompleted", default)]
    pub node_intros_completed: Vec<String>,
    #[serde(rename = "ClaimedJunctionChallengeRewards", default)]
    pub claimed_junction_rewards: Vec<String>,
    #[serde(rename = "CompletedSyndicates", default)]
    pub completed_syndicates: Vec<String>,
    #[serde(rename = "OneTimePurchases", default)]
    pub one_time_purchases: Vec<String>,
    #[serde(rename = "LoginMilestoneRewards", default)]
    pub login_milestone_rewards: Vec<String>,
    #[serde(rename = "DeathMarks", default)]
    pub death_marks: Vec<String>,
    #[serde(rename = "CompletedJobs", default)]
    pub completed_jobs: Vec<Value>,
    #[serde(rename = "PeriodicMissionCompletions", default)]
    pub periodic_missions: Vec<Value>,

    // Collections / cosmetics
    #[serde(rename = "WeaponSkins", default)]
    pub weapon_skins: Vec<Value>,
    #[serde(rename = "FlavourItems", default)]
    pub cosmetics: Vec<Value>,
    #[serde(rename = "LoreFragmentScans", default)]
    pub lore_scans: Vec<Value>,
    #[serde(rename = "CollectibleSeries", default)]
    pub collectible_series: Vec<Value>,
    #[serde(rename = "EvolutionProgress", default)]
    pub evolution_progress: Vec<Value>,
    #[serde(rename = "DescentRewards", default)]
    pub incarnon_rewards: Vec<Value>,
    #[serde(rename = "EndlessXP", default)]
    pub archon_shards: Vec<Value>,
    #[serde(rename = "PersonalGoalProgress", default)]
    pub personal_goals: Vec<Value>,
    #[serde(rename = "SpecialItemRewardAttenuation", default)]
    pub special_attenuation: Vec<Value>,

    // Gear wheel / loadouts / UI
    #[serde(rename = "EquippedGear", default)]
    pub gear_wheel: Vec<String>,
    #[serde(rename = "Wishlist", default)]
    pub wishlist: Vec<String>,
    #[serde(rename = "FactionScores", default)]
    pub faction_scores: Vec<i64>,
    #[serde(rename = "CurrentLoadOutIds", default)]
    pub current_loadout_ids: Vec<MongoOid>,
    #[serde(rename = "LoadOutPresets")]
    pub loadout_presets: Option<Value>,
    #[serde(rename = "OperatorLoadOuts", default)]
    pub operator_loadouts: Vec<Value>,
    #[serde(rename = "AdultOperatorLoadOuts", default)]
    pub drifter_loadouts: Vec<Value>,
    #[serde(rename = "KahlLoadOuts", default)]
    pub kahl_loadouts: Vec<Value>,
    #[serde(rename = "OperatorSuits", default)]
    pub operator_suits: Vec<Value>,
    #[serde(rename = "HubNpcCustomizations", default)]
    pub hub_npcs: Vec<Value>,

    // Complex account state
    #[serde(rename = "InfestedFoundry")]
    pub helminth: Option<Value>,
    #[serde(rename = "PersonalTechProjects", default)]
    pub research_projects: Vec<Value>,
    #[serde(rename = "LotusCustomization")]
    pub lotus_customization: Option<Value>,
    #[serde(rename = "CalendarProgress")]
    pub calendar_progress: Option<Value>,
    #[serde(rename = "DialogueHistory")]
    pub dialogue_history: Option<Value>,
    #[serde(rename = "Ships", default)]
    pub orbiter: Vec<Value>,
    #[serde(rename = "RecentVendorPurchases", default)]
    pub vendor_purchases: Vec<Value>,
    #[serde(rename = "WeeklyGuildVaultBonusInfo", default)]
    pub guild_vault_bonus: Vec<Value>,
    #[serde(rename = "CrewShipSalvagedWeaponSkins", default)]
    pub railjack_salvage: Vec<Value>,
    #[serde(rename = "TauntHistory", default)]
    pub taunt_history: Vec<Value>,
    #[serde(rename = "DiscoveredMarkers", default)]
    pub discovered_markers: Vec<Value>,
    #[serde(rename = "LibraryPersonalProgress", default)]
    pub library_progress: Vec<Value>,
    #[serde(rename = "LibraryActiveDailyTaskInfo")]
    pub library_active_task: Option<Value>,
    #[serde(rename = "LibraryAvailableDailyTaskInfo")]
    pub library_available_task: Option<Value>,
    #[serde(rename = "Mailbox")]
    pub mailbox: Option<Value>,
    #[serde(rename = "PendingCoupon")]
    pub pending_coupon: Option<Value>,
    #[serde(rename = "WebFlags")]
    pub web_flags: Option<Value>,

    // Entrati / Hex conquest counters
    #[serde(rename = "EntratiVaultCountLastPeriod", default)]
    pub entrati_vault_count: i32,
    #[serde(rename = "EntratiVaultCountResetDate")]
    pub entrati_vault_reset: Option<MongoDate>,
    #[serde(rename = "EntratiLabConquestCacheScoreMission", default)]
    pub entrati_conquest_score: i32,
    #[serde(rename = "EntratiLabConquestUnlocked", default)]
    pub entrati_conquest_unlocked: i32,
    #[serde(rename = "EntratiLabConquestHardModeStatus", default)]
    pub entrati_conquest_hard: i32,
    #[serde(rename = "EntratiLabConquestActiveFrameVariants", default)]
    pub entrati_conquest_frames: Vec<String>,
    #[serde(rename = "EchoesHexConquestCacheScoreMission", default)]
    pub hex_conquest_score: i32,
    #[serde(rename = "EchoesHexConquestUnlocked", default)]
    pub hex_conquest_unlocked: i32,
    #[serde(rename = "EchoesHexConquestBonusTokensGiven", default)]
    pub hex_conquest_tokens: Vec<i32>,

    // Misc flags / strings
    #[serde(rename = "LastRegionPlayed")]
    pub last_region: Option<String>,
    #[serde(rename = "StoryModeChoice")]
    pub story_mode: Option<String>,
    #[serde(rename = "SupportedSyndicate")]
    pub supported_syndicate: Option<String>,
    #[serde(rename = "ActiveAvatarImageType")]
    pub avatar: Option<String>,
    #[serde(rename = "ActiveDojoColorResearch")]
    pub active_dojo_color: Option<String>,
    #[serde(rename = "ArchwingEnabled", default)]
    pub archwing_enabled: bool,
    #[serde(rename = "HWIDProtectEnabled", default)]
    pub hwid_protect: bool,
    #[serde(rename = "HasOwnedVoidProjectionsPreviously", default)]
    pub has_owned_void_projections: bool,
    #[serde(rename = "HasResetAccount", default)]
    pub has_reset_account: bool,
    #[serde(rename = "Harvestable", default)]
    pub harvestable: bool,
    #[serde(rename = "DeathSquadable", default)]
    pub death_squadable: bool,
    #[serde(rename = "PlayedParkourTutorial", default)]
    pub played_parkour_tutorial: bool,
    #[serde(rename = "ReceivedStartingGear", default)]
    pub received_starting_gear: bool,
    #[serde(rename = "SubscribedToEmails", default)]
    pub subscribed_emails: i32,
    #[serde(rename = "SubscribedToEmailsPersonalized", default)]
    pub subscribed_emails_personalized: i32,
    #[serde(rename = "HandlerPoints", default)]
    pub handler_points: i32,
}

// ── Cache wrapper ─────────────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct InventoryCache {
    pub fetched_at: u64,
    pub data: Inventory,
}

// ── Fetch / cache logic ───────────────────────────────────────────────────────

async fn fetch_inventory(account_id: &str, nonce: &str) -> Result<Inventory, String> {
    let url = format!("{API_BASE}?accountId={account_id}&nonce={nonce}");
    reqwest::get(&url)
        .await
        .map_err(|e| format!("inventory request failed: {e}"))?
        .json::<Inventory>()
        .await
        .map_err(|e| format!("inventory parse failed: {e}"))
}

pub async fn get_or_refresh_inventory(app: &AppHandle) -> Result<InventoryCache, String> {
    if let Some(cache) = load_cache(app) {
        let age = now_secs().saturating_sub(cache.fetched_at);
        if age < CACHE_TTL_SECS {
            eprintln!("[inventory] cache hit ({age}s old)");
            return Ok(cache);
        }
        eprintln!("[inventory] cache expired ({age}s old), refreshing");
    }

    let (account_id, nonce) = {
        let guard = crate::account_memory::ACCOUNT_INFO.lock().unwrap();
        let info = guard.as_ref().ok_or("no account info — is the game running?")?;
        let nonce = info.nonce.as_ref().ok_or("nonce not found in memory — is the game running?")?;
        (info.account_id.clone(), nonce.clone())
    };

    eprintln!("[inventory] fetching for account {account_id}");
    let data = fetch_inventory(&account_id, &nonce).await?;
    let cache = InventoryCache { fetched_at: now_secs(), data };
    save_cache(app, &cache);
    eprintln!("[inventory] fetched and cached");
    Ok(cache)
}

// ── View types (sent to frontend) ────────────────────────────────────────────

#[derive(Debug, Serialize)]
pub struct DisplayItem {
    #[serde(rename = "itemType")]
    pub item_type: String,
    #[serde(rename = "displayName")]
    pub display_name: String,
    #[serde(rename = "imageName")]
    pub image_name: String,
    #[serde(rename = "overlayImageName")]
    pub overlay_image_name: String,
    pub count: Option<i32>,
    pub rank: Option<i32>,
    #[serde(rename = "maxRank")]
    pub max_rank: Option<i32>,
    pub rarity: Option<String>,
    pub polarity: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct InventoryView {
    #[serde(rename = "fetchedAt")]
    pub fetched_at: u64,
    pub warframes: Vec<DisplayItem>,
    pub gear: Vec<DisplayItem>,
    pub relics: Vec<DisplayItem>,
    pub mods: Vec<DisplayItem>,
    pub resources: Vec<DisplayItem>,
    pub blueprints: Vec<DisplayItem>,
}

// Warframe inventory uses *Blueprint suffix for part recipes,
// but warframestat.us indexes them under *Component uniqueNames.
fn normalize_recipe_path(path: &str) -> std::borrow::Cow<'_, str> {
    for (from, to) in [
        ("SystemsBlueprint",   "SystemsComponent"),
        ("ChassisBlueprint",   "ChassisComponent"),
        ("HelmetBlueprint",    "HelmetComponent"),
        ("NeuropticBlueprint", "NeuropticComponent"),
    ] {
        if path.contains(from) {
            return std::borrow::Cow::Owned(path.replace(from, to));
        }
    }
    std::borrow::Cow::Borrowed(path)
}

fn parse_rank(fingerprint: Option<&str>) -> i32 {
    fingerprint
        .and_then(|s| serde_json::from_str::<serde_json::Value>(s).ok())
        .and_then(|v| v["lvl"].as_i64())
        .unwrap_or(0) as i32
}

pub fn build_view(
    cache: &InventoryCache,
    names: &std::collections::HashMap<String, String>,
    categories: &std::collections::HashMap<String, String>,
    types: &std::collections::HashMap<String, String>,
    images: &std::collections::HashMap<String, String>,
    overlay_images: &std::collections::HashMap<String, String>,
    fusion_limits: &std::collections::HashMap<String, i32>,
    rarities: &std::collections::HashMap<String, String>,
    polarities: &std::collections::HashMap<String, String>,
) -> InventoryView {
    let name_of = |item_type: &str| -> String {
        let key = normalize_recipe_path(item_type);
        names.get(key.as_ref()).cloned().unwrap_or_default()
    };

    let image_of = |item_type: &str| -> String {
        let key = normalize_recipe_path(item_type);
        images.get(key.as_ref()).cloned().unwrap_or_default()
    };

    let overlay_of = |item_type: &str| -> String {
        let key = normalize_recipe_path(item_type);
        overlay_images.get(key.as_ref()).cloned().unwrap_or_default()
    };

    let from_owned = |items: &[OwnedItem]| -> Vec<DisplayItem> {
        items.iter().map(|i| DisplayItem {
            display_name: name_of(&i.item_type),
            image_name: image_of(&i.item_type),
            overlay_image_name: overlay_of(&i.item_type),
            item_type: i.item_type.clone(),
            count: None,
            rank: None,
            max_rank: None,
            rarity: None,
            polarity: None,
        }).collect()
    };

    let from_counted = |items: &[CountedItem]| -> Vec<DisplayItem> {
        items.iter().map(|i| DisplayItem {
            display_name: name_of(&i.item_type),
            image_name: image_of(&i.item_type),
            overlay_image_name: overlay_of(&i.item_type),
            item_type: i.item_type.clone(),
            count: Some(i.item_count),
            rank: None,
            max_rank: None,
            rarity: None,
            polarity: None,
        }).collect()
    };

    let d = &cache.data;

    // Warframes: own tab
    let warframes = from_owned(&d.warframes);

    // Gear: weapons, companions, archwing, mechs — everything except warframes
    let mut gear = Vec::new();
    gear.extend(from_owned(&d.primaries));
    gear.extend(from_owned(&d.secondaries));
    gear.extend(from_owned(&d.melee));
    gear.extend(from_owned(&d.sentinels));
    gear.extend(from_owned(&d.sentinel_weapons));
    gear.extend(from_owned(&d.archwings));
    gear.extend(from_owned(&d.arch_guns));
    gear.extend(from_owned(&d.arch_melee));
    gear.extend(from_owned(&d.mechs));
    gear.extend(from_owned(&d.companions));
    gear.extend(from_owned(&d.amps));
    gear.extend(from_owned(&d.kaithe));
    gear.extend(from_owned(&d.railjacks));
    gear.extend(from_owned(&d.drifter_melee));
    gear.extend(from_owned(&d.plexus));

    // Relics vs resources: split misc_items by warframestat `type` field
    let mut relics = Vec::new();
    let mut resources = Vec::new();
    for item in &d.misc_items {
        let item_kind = types.get(&item.item_type).map(|s| s.as_str()).unwrap_or("");
        let is_relic = item_kind == "Relic"
            || categories.get(&item.item_type).map(|s| s.as_str()).unwrap_or("") == "Relics";
        let di = DisplayItem {
            display_name: name_of(&item.item_type),
            image_name: image_of(&item.item_type),
            overlay_image_name: overlay_of(&item.item_type),
            item_type: item.item_type.clone(),
            count: Some(item.item_count),
            rank: None,
            max_rank: None,
            rarity: None,
            polarity: None,
        };
        if is_relic {
            relics.push(di);
        } else {
            resources.push(di);
        }
    }
    resources.extend(from_counted(&d.consumables));
    resources.extend(from_counted(&d.railjack_resources));

    // Mods: merge unranked stacks + individual ranked copies, grouped by item_type.
    // count = total copies; rank = highest rank among ranked instances (0 if all unranked).
    let mut mod_counts: std::collections::HashMap<&str, i32> = std::collections::HashMap::new();
    let mut mod_max_rank: std::collections::HashMap<&str, i32> = std::collections::HashMap::new();
    for u in &d.raw_upgrades {
        *mod_counts.entry(u.item_type.as_str()).or_insert(0) += u.item_count;
    }
    for u in &d.ranked_mods {
        *mod_counts.entry(u.item_type.as_str()).or_insert(0) += 1;
        let lvl = parse_rank(u.fingerprint.as_deref());
        let entry = mod_max_rank.entry(u.item_type.as_str()).or_insert(0);
        if lvl > *entry { *entry = lvl; }
    }
    // Collect unique item_types preserving raw_upgrades order first, then any ranked-only
    let mut seen = std::collections::HashSet::new();
    let mut mod_types: Vec<&str> = Vec::new();
    for u in &d.raw_upgrades {
        if seen.insert(u.item_type.as_str()) { mod_types.push(u.item_type.as_str()); }
    }
    for u in &d.ranked_mods {
        if seen.insert(u.item_type.as_str()) { mod_types.push(u.item_type.as_str()); }
    }
    let mods: Vec<DisplayItem> = mod_types.iter().map(|it| {
        let count = mod_counts.get(it).copied().unwrap_or(0);
        let rank = mod_max_rank.get(it).copied();
        let max_rank = fusion_limits.get(*it).copied();
        let rarity = rarities.get(*it).cloned();
        let polarity = polarities.get(*it).cloned();
        DisplayItem {
            display_name: name_of(it),
            image_name: image_of(it),
            overlay_image_name: overlay_of(it),
            item_type: it.to_string(),
            count: Some(count),
            rank,
            max_rank,
            rarity,
            polarity,
        }
    }).collect();

    // Blueprints
    let blueprints = from_counted(&d.blueprints);

    InventoryView { fetched_at: cache.fetched_at, warframes, gear, relics, mods, resources, blueprints }
}

