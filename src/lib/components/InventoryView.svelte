<script lang="ts">
  import VirtualModGrid from '$lib/components/VirtualModGrid.svelte';

  type DisplayItem = {
    itemType: string;
    displayName: string;
    imageName: string;
    overlayImageName: string;
    count: number | null;
    rank: number | null;
    maxRank: number | null;
    rarity: string | null;
    polarity: string | null;
    compatName: string | null;
    description: string | null;
    levelStats: string | null;
    baseDrain: number | null;
  };

  type InventoryViewData = {
    fetchedAt: number;
    warframes: DisplayItem[];
    gear: DisplayItem[];
    relics: DisplayItem[];
    mods: DisplayItem[];
    resources: DisplayItem[];
    blueprints: DisplayItem[];
  };

  type InventoryTab = 'warframes' | 'gear' | 'relics' | 'mods' | 'resources' | 'blueprints';
  type RelicTier = 'all' | 'lith' | 'meso' | 'neo' | 'axi' | 'requiem';

  let {
    inventoryData,
    inventoryLoading,
    inventoryError,
    loadInventory,
    onRefresh
  } = $props<{
    inventoryData: InventoryViewData | null;
    inventoryLoading: boolean;
    inventoryError: string;
    loadInventory: () => Promise<void>;
    onRefresh: () => void;
  }>();

  let inventoryTab = $state<InventoryTab>('warframes');
  let relicTier = $state<RelicTier>('all');
  let modPolarity = $state('all');
  let modRarity = $state('all');
  let modSearch = $state('');
  let modSort = $state<'name' | 'owned' | 'cost'>('name');
  let modSortDir = $state<'asc' | 'desc'>('asc');

  const RARITY_ORDER = ['Common', 'Uncommon', 'Rare', 'Legendary'];

  const modPolarities = $derived<string[]>(
    inventoryData
      ? ['all', ...[...new Set<string>(inventoryData.mods.map((m: DisplayItem) => m.polarity).filter((p: string | null): p is string => !!p))].sort()]
      : ['all']
  );
  const modRarities = $derived<string[]>(
    inventoryData
      ? ['all', ...[...new Set<string>(inventoryData.mods.map((m: DisplayItem) => m.rarity).filter((r: string | null): r is string => !!r))]
          .sort((a, b) => RARITY_ORDER.indexOf(a) - RARITY_ORDER.indexOf(b))]
      : ['all']
  );

  function switchTab(id: InventoryTab) {
    inventoryTab = id;
  }

  function currentItems(): DisplayItem[] {
    if (!inventoryData) return [];
    let items = [...(inventoryData[inventoryTab] ?? [])];
    if (inventoryTab === 'relics') {
      if (relicTier !== 'all') {
        const prefix = relicTier.charAt(0).toUpperCase() + relicTier.slice(1);
        items = items.filter(i => i.displayName.startsWith(prefix));
      }
      items.sort((a, b) => a.displayName.localeCompare(b.displayName));
    } else if (inventoryTab === 'mods') {
      if (modPolarity !== 'all') items = items.filter(i => i.polarity === modPolarity);
      if (modRarity   !== 'all') items = items.filter(i => i.rarity   === modRarity);
      if (modSearch.trim()) {
        const q = modSearch.trim().toLowerCase();
        items = items.filter(i => i.displayName.toLowerCase().includes(q));
      }
      items.sort((a, b) => {
        let cmp = 0;
        if (modSort === 'name')  cmp = a.displayName.localeCompare(b.displayName);
        if (modSort === 'owned') cmp = (a.count ?? 0) - (b.count ?? 0);
        if (modSort === 'cost')  cmp = (Math.abs(a.baseDrain ?? 0) + (a.rank ?? 0)) - (Math.abs(b.baseDrain ?? 0) + (b.rank ?? 0));
        return modSortDir === 'asc' ? cmp : -cmp;
      });
    }
    return items;
  }
</script>

<div class="inventory-container">
  <div class="page-header">
    <div class="title-row">
      <h1>Inventory</h1>
      {#if inventoryData}
        <span class="update-time">Cached: {new Date(inventoryData.fetchedAt * 1000).toLocaleTimeString()}</span>
      {/if}
    </div>
    <p class="subtitle">View and filter your synced Warframe account inventory.</p>
  </div>

  <div class="tabs-control-row">
    <div class="subtab-bar">
      {#each [
        { id: 'warframes',  label: 'Warframes', icon: '⬡' },
        { id: 'gear',       label: 'Gear', icon: '⚔' },
        { id: 'relics',     label: 'Relics', icon: '◈' },
        { id: 'mods',       label: 'Mods', icon: '▲' },
        { id: 'resources',  label: 'Resources', icon: '♦' },
        { id: 'blueprints', label: 'Blueprints', icon: '⚒' },
      ] as tab}
        <button
          class="subtab"
          class:active={inventoryTab === tab.id}
          onclick={() => switchTab(tab.id as InventoryTab)}
        >
          <span class="subtab-icon">{tab.icon}</span>
          {tab.label}
        </button>
      {/each}
    </div>

    <button
      class="refresh-btn"
      onclick={onRefresh}
      disabled={inventoryLoading}
      title="Refresh inventory"
    >
      <span class="refresh-icon" class:spinning={inventoryLoading}>↻</span>
      Refresh
    </button>
  </div>

  <!-- Relics sub-filters -->
  {#if inventoryTab === 'relics'}
    <div class="filter-panel relic-filters">
      <span class="filter-label">Tiers:</span>
      <div class="subtab-bar-sm">
        {#each [
          { id: 'all',     label: 'All' },
          { id: 'lith',    label: 'Lith' },
          { id: 'meso',    label: 'Meso' },
          { id: 'neo',     label: 'Neo' },
          { id: 'axi',     label: 'Axi' },
          { id: 'requiem', label: 'Requiem' },
        ] as tier}
          <button
            class="subtab-sm"
            class:active={relicTier === tier.id}
            onclick={() => (relicTier = tier.id as RelicTier)}
          >{tier.label}</button>
        {/each}
      </div>
    </div>
  {/if}

  <!-- Mods sub-filters -->
  {#if inventoryTab === 'mods'}
    <div class="filter-panel mod-filters-panel">
      <div class="search-sort-row">
        <div class="search-input-wrapper">
          <span class="search-icon">🔍</span>
          <input
            class="mod-search"
            type="search"
            placeholder="Search mods..."
            bind:value={modSearch}
          />
        </div>

        <div class="mod-sort-group">
          {#each ([['name','Name'],['owned','Owned'],['cost','Cost']] as const) as [id, label]}
            <button
              class="subtab-sm sort-btn"
              class:active={modSort === id}
              onclick={() => {
                if (modSort === id) modSortDir = modSortDir === 'asc' ? 'desc' : 'asc';
                else { modSort = id; modSortDir = 'asc'; }
              }}
            >
              {label}
              {#if modSort === id}
                <span class="sort-dir">{modSortDir === 'asc' ? '▲' : '▼'}</span>
              {/if}
            </button>
          {/each}
        </div>
      </div>

      <div class="filter-row">
        <span class="filter-label">Polarity:</span>
        <div class="subtab-bar-sm scrollable-filters">
          {#each modPolarities as p}
            <button
              class="subtab-sm"
              class:active={modPolarity === p}
              onclick={() => (modPolarity = p)}
            >{p === 'all' ? 'All' : p.charAt(0).toUpperCase() + p.slice(1)}</button>
          {/each}
        </div>
      </div>

      <div class="filter-row">
        <span class="filter-label">Rarity:</span>
        <div class="subtab-bar-sm">
          {#each modRarities as r}
            <button
              class="subtab-sm"
              class:active={modRarity === r}
              onclick={() => (modRarity = r)}
            >{r === 'all' ? 'All' : r}</button>
          {/each}
        </div>
      </div>
    </div>
  {/if}

  <div class="grid-viewport-wrapper">
    {#if inventoryLoading}
      <div class="state-msg">
        <span class="loader"></span>
        <p>Syncing neural networks with Warframe inventory...</p>
      </div>
    {:else if inventoryError}
      <div class="state-msg error">
        <span class="error-icon">⚠️</span>
        <p class="error-title">Account Sync Unsuccessful</p>
        <p class="error-body">{inventoryError}</p>
        <p class="error-hint">Make sure Warframe is running and the memory feature is enabled (requires root / --features memory).</p>
      </div>
    {:else if inventoryData}
      {@const items = currentItems()}
      {#if items.length === 0}
        <div class="state-msg muted">
          <span class="empty-icon">📁</span>
          <p>No items found in this segment.</p>
        </div>
      {:else if inventoryTab === 'mods'}
        <div class="mod-virtual-wrap">
          <VirtualModGrid {items} />
        </div>
      {:else}
        <div class="item-grid-scroll">
          <div class="item-grid">
            {#each items as item}
              <div class="item-card">
                <div class="card-visual-container">
                  {#if item.overlayImageName}
                    <div class="item-img-stack">
                      <img class="item-img base" src="/img/wf-assets/{item.imageName.replace('.png', '.avif')}" alt="" />
                      <img class="item-img-overlay" src="/img/wf-assets/{item.overlayImageName.replace('.png', '.avif')}" alt={item.displayName} />
                    </div>
                  {:else if item.imageName}
                    <img
                      class="item-img"
                      src="/img/wf-assets/{item.imageName.replace('.png', '.avif')}"
                      alt={item.displayName}
                      loading="lazy"
                    />
                  {:else}
                    <div class="item-img-placeholder">
                      <span class="placeholder-symbol">⬡</span>
                    </div>
                  {/if}

                  {#if item.count !== null}
                    <span class="item-badge-count">×{item.count}</span>
                  {/if}
                </div>

                <div class="item-info">
                  <span class="item-name" title={item.displayName || item.itemType}>{item.displayName || item.itemType}</span>
                </div>
              </div>
            {/each}
          </div>
        </div>
      {/if}
    {:else}
      <div class="state-msg muted">
        <span class="empty-icon">🔌</span>
        <p>No inventory data loaded yet.</p>
        <button class="action-btn" onclick={loadInventory}>Initialize Sync</button>
      </div>
    {/if}
  </div>
</div>

<style>
  .inventory-container {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .title-row {
    display: flex;
    align-items: baseline;
    justify-content: space-between;
    gap: 16px;
  }

  .update-time {
    font-size: 11px;
    font-weight: 600;
    color: #e5a93b;
    letter-spacing: 0.05em;
    background: rgba(229, 169, 59, 0.08);
    border: 1px solid rgba(229, 169, 59, 0.2);
    padding: 3px 8px;
    border-radius: 4px;
    font-family: 'Consolas', 'Courier New', monospace;
  }

  .tabs-control-row {
    display: flex;
    align-items: center;
    gap: 16px;
    border-bottom: 1px solid rgba(56, 189, 248, 0.12);
    padding-bottom: 12px;
    margin-bottom: 12px;
  }

  .subtab-bar {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
    flex: 1;
  }

  .subtab {
    background: rgba(14, 17, 26, 0.6);
    border: 1px solid rgba(56, 189, 248, 0.15);
    border-radius: 6px;
    color: #8f9cae;
    font-size: 12px;
    font-weight: 600;
    padding: 8px 14px;
    cursor: pointer;
    transition: all 0.2s ease;
    display: flex;
    align-items: center;
    gap: 6px;
  }

  .subtab:hover {
    background: rgba(56, 189, 248, 0.08);
    border-color: rgba(56, 189, 248, 0.3);
    color: #f1f5f9;
  }

  .subtab.active {
    background: rgba(229, 169, 59, 0.1);
    color: #e5a93b;
    border-color: #e5a93b;
    box-shadow: 0 0 12px rgba(229, 169, 59, 0.15);
  }

  .subtab-icon {
    font-size: 13px;
  }

  .refresh-btn {
    background: rgba(56, 189, 248, 0.06);
    border: 1px solid rgba(56, 189, 248, 0.25);
    border-radius: 6px;
    color: #38bdf8;
    font-size: 12px;
    font-weight: 600;
    padding: 8px 16px;
    cursor: pointer;
    transition: all 0.2s ease;
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .refresh-btn:hover:not(:disabled) {
    background: rgba(56, 189, 248, 0.15);
    border-color: #38bdf8;
    color: #ffffff;
    box-shadow: 0 0 10px rgba(56, 189, 248, 0.2);
  }

  .refresh-btn:disabled {
    opacity: 0.5;
    cursor: not-allowed;
  }

  .refresh-icon {
    display: inline-block;
    font-size: 14px;
    transition: transform 0.25s ease;
  }

  .spinning {
    animation: spin 1s linear infinite;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }

  /* Filters styling */
  .filter-panel {
    background: rgba(14, 18, 28, 0.8);
    border: 1px solid rgba(56, 189, 248, 0.08);
    border-radius: 8px;
    padding: 10px 14px;
    margin-bottom: 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .relic-filters {
    flex-direction: row;
    align-items: center;
    gap: 12px;
  }

  .filter-label {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: #64748b;
    min-width: 60px;
  }

  .subtab-bar-sm {
    display: flex;
    gap: 4px;
    flex-wrap: wrap;
  }

  .subtab-sm {
    background: rgba(30, 41, 59, 0.4);
    border: 1px solid rgba(56, 189, 248, 0.1);
    border-radius: 4px;
    color: #64748b;
    font-size: 11px;
    font-weight: 600;
    padding: 4px 10px;
    cursor: pointer;
    transition: all 0.15s ease;
  }

  .subtab-sm:hover {
    background: rgba(56, 189, 248, 0.05);
    border-color: rgba(56, 189, 248, 0.2);
    color: #e2e8f0;
  }

  .subtab-sm.active {
    background: rgba(56, 189, 248, 0.12);
    color: #38bdf8;
    border-color: #38bdf8;
  }

  /* Mod filters specifics */
  .search-sort-row {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 16px;
  }

  .search-input-wrapper {
    position: relative;
    flex: 1;
    max-width: 280px;
  }

  .search-icon {
    position: absolute;
    left: 10px;
    top: 50%;
    transform: translateY(-50%);
    font-size: 12px;
    color: #64748b;
  }

  .mod-search {
    width: 100%;
    background: rgba(8, 10, 15, 0.6);
    border: 1px solid rgba(56, 189, 248, 0.15);
    border-radius: 6px;
    padding: 6px 10px 6px 28px;
    color: #f1f5f9;
    font-size: 12px;
    outline: none;
    transition: all 0.2s ease;
  }

  .mod-search:focus {
    border-color: #38bdf8;
    box-shadow: 0 0 10px rgba(56, 189, 248, 0.1);
    background: rgba(8, 10, 15, 0.9);
  }

  .mod-sort-group {
    display: flex;
    gap: 4px;
  }

  .sort-btn {
    display: flex;
    align-items: center;
    gap: 4px;
  }

  .sort-dir {
    font-size: 9px;
  }

  .filter-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .scrollable-filters {
    max-height: 80px;
    overflow-y: auto;
  }

  /* Grid Layouts */
  .grid-viewport-wrapper {
    flex: 1;
    min-height: 0;
    position: relative;
  }

  .mod-virtual-wrap {
    height: 100%;
    min-height: 0;
    overflow: hidden;
  }

  .item-grid-scroll {
    height: 100%;
    overflow-y: auto;
    padding-right: 4px;
  }

  /* Custom Slim Scrollbar */
  .item-grid-scroll::-webkit-scrollbar,
  .scrollable-filters::-webkit-scrollbar {
    width: 6px;
    height: 6px;
  }
  .item-grid-scroll::-webkit-scrollbar-track,
  .scrollable-filters::-webkit-scrollbar-track {
    background: rgba(255, 255, 255, 0.02);
    border-radius: 3px;
  }
  .item-grid-scroll::-webkit-scrollbar-thumb,
  .scrollable-filters::-webkit-scrollbar-thumb {
    background: rgba(56, 189, 248, 0.2);
    border-radius: 3px;
  }
  .item-grid-scroll::-webkit-scrollbar-thumb:hover,
  .scrollable-filters::-webkit-scrollbar-thumb:hover {
    background: rgba(56, 189, 248, 0.4);
  }

  .item-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(130px, 1fr));
    gap: 12px;
    padding-bottom: 24px;
  }

  /* Item Card Design */
  .item-card {
    background: rgba(14, 18, 28, 0.55);
    border: 1px solid rgba(56, 189, 248, 0.08);
    border-radius: 8px;
    padding: 10px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    transition: all 0.2s cubic-bezier(0.4, 0, 0.2, 1);
    position: relative;
  }

  .item-card:hover {
    transform: translateY(-2px);
    border-color: rgba(229, 169, 59, 0.3);
    background: rgba(18, 23, 36, 0.85);
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.5), 0 0 10px rgba(229, 169, 59, 0.1);
  }

  .card-visual-container {
    position: relative;
    width: 100%;
    aspect-ratio: 1;
    border-radius: 6px;
    background: rgba(255, 255, 255, 0.02);
    border: 1px solid rgba(255, 255, 255, 0.04);
    display: flex;
    align-items: center;
    justify-content: center;
    overflow: hidden;
  }

  .item-img {
    width: 90%;
    height: 90%;
    object-fit: contain;
    transition: transform 0.2s ease;
  }

  .item-card:hover .item-img {
    transform: scale(1.06);
  }

  .item-img-stack {
    position: relative;
    width: 90%;
    height: 90%;
  }

  .item-img-stack .item-img.base {
    position: absolute;
    inset: 0;
    width: 100%;
    height: 100%;
    opacity: 0.35;
  }

  .item-img-overlay {
    position: absolute;
    inset: 12%;
    width: 76%;
    height: 76%;
    object-fit: contain;
    transition: transform 0.2s ease;
  }

  .item-card:hover .item-img-overlay {
    transform: scale(1.06);
  }

  .item-img-placeholder {
    width: 100%;
    height: 100%;
    display: flex;
    align-items: center;
    justify-content: center;
    background: rgba(255, 255, 255, 0.01);
  }

  .placeholder-symbol {
    font-size: 24px;
    color: #1e293b;
  }

  /* Badge counts in game style */
  .item-badge-count {
    position: absolute;
    top: 6px;
    left: 6px;
    font-size: 11px;
    font-weight: 800;
    color: #090a0f;
    background: #e5a93b;
    border-radius: 4px;
    padding: 1px 5px;
    box-shadow: 0 2px 6px rgba(0, 0, 0, 0.4);
    font-family: 'Consolas', 'Courier New', monospace;
  }

  .item-info {
    display: flex;
    flex-direction: column;
    justify-content: space-between;
    min-height: 32px;
  }

  .item-name {
    font-size: 11px;
    font-weight: 600;
    color: #94a3b8;
    line-height: 1.3;
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
    text-overflow: ellipsis;
    transition: color 0.2s ease;
  }

  .item-card:hover .item-name {
    color: #f1f5f9;
  }

  /* Loading & State Messages styling */
  .state-msg {
    position: absolute;
    inset: 0;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    text-align: center;
    padding: 32px;
    gap: 16px;
  }

  .state-msg p {
    font-size: 14px;
    color: #64748b;
    max-width: 320px;
    line-height: 1.5;
  }

  .loader {
    width: 40px;
    height: 40px;
    border: 3px solid rgba(56, 189, 248, 0.1);
    border-radius: 50%;
    border-top-color: #38bdf8;
    animation: spin 1s ease-in-out infinite;
  }

  .state-msg.error {
    background: rgba(239, 68, 68, 0.02);
    border: 1px dashed rgba(239, 68, 68, 0.15);
    border-radius: 8px;
  }

  .error-icon {
    font-size: 32px;
    color: #ef4444;
  }

  .error-title {
    font-weight: 700;
    color: #ef4444 !important;
  }

  .error-body {
    color: #94a3b8 !important;
    font-family: 'Consolas', 'Courier New', monospace;
    font-size: 12px !important;
    background: rgba(0, 0, 0, 0.2);
    padding: 6px 12px;
    border-radius: 4px;
    border: 1px solid rgba(239, 68, 68, 0.1);
  }

  .error-hint {
    font-size: 11px !important;
    color: #64748b !important;
  }

  .empty-icon {
    font-size: 36px;
    color: #334155;
  }

  .action-btn {
    background: #e5a93b;
    color: #090a0f;
    border: none;
    border-radius: 6px;
    padding: 8px 20px;
    font-size: 12px;
    font-weight: 700;
    cursor: pointer;
    transition: all 0.2s ease;
    box-shadow: 0 4px 12px rgba(229, 169, 59, 0.2);
  }

  .action-btn:hover {
    background: #f1b74c;
    box-shadow: 0 4px 16px rgba(229, 169, 59, 0.35);
  }
</style>
