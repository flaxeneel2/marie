<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { invoke, type InvokeArgs } from '@tauri-apps/api/core';
  import Sidebar from '$lib/components/Sidebar.svelte';
  import InventoryView from '$lib/components/InventoryView.svelte';
  import DevView from '$lib/components/DevView.svelte';

  // ── Navigation State ──────────────────────────────────────────────────────────
  type NavItem = 'inventory' | 'foundry' | 'dev';
  let activeNav = $state<NavItem>('inventory');

  // ── Diagnostics & Hotkey States ───────────────────────────────────────────────
  let logPath = $state('…');
  let detectedCount = $state<number | null>(null);
  let overlayInteractive = $state(false);
  let initialBind = $state('');

  // ── Inventory States ──────────────────────────────────────────────────────────
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

  let inventoryData = $state<InventoryViewData | null>(null);
  let inventoryError = $state('');
  let inventoryLoading = $state(false);

  async function loadInventory() {
    if (inventoryData) return;
    inventoryLoading = true;
    inventoryError = '';
    try {
      inventoryData = await invoke<InventoryViewData>('get_inventory');
    } catch (e) {
      inventoryError = String(e);
    } finally {
      inventoryLoading = false;
    }
  }

  function handleRefreshInventory() {
    inventoryData = null;
    loadInventory();
  }

  $effect(() => {
    if (activeNav === 'inventory') loadInventory();
  });

  onMount(() => {
    invoke<string>('ee_log_path').then(p => (logPath = p));
    invoke<string>('get_interaction_shortcut').then(b => (initialBind = b));

    const cleanups: Array<() => void> = [];
    listen<number>('relic-trigger', (event) => {
      detectedCount = event.payload;
    }).then(fn => cleanups.push(fn));
    listen<boolean>('overlay-interactive-changed', (event) => {
      overlayInteractive = event.payload;
    }).then(fn => cleanups.push(fn));

    return () => cleanups.forEach(fn => fn());
  });

  // ── Developer Actions & IPC Handlers ──────────────────────────────────────────
  async function handleSaveBind(modsKey: string): Promise<string> {
    await invoke('set_interaction_shortcut', { modsKey });
    initialBind = modsKey;
    return 'Saved & applied.';
  }

  async function handleTestTrigger(playerCount: number) {
    await invoke('test_trigger', { playerCount });
  }

  async function handleTestOverlay() {
    await invoke('show_test_overlay');
  }

  async function handleTestRivenTrigger() {
    await invoke('test_riven_trigger');
  }

  async function handleTestRivenOverlay() {
    await invoke('show_test_riven_overlay');
  }

  // Intercept invokes for debugging / performance profiling
  window.core = window.core || {} as Window["core"];
  window.core.invoke = async (fn_to_invoke: string, args: InvokeArgs | undefined) => {
    const start = performance.now();
    try {
      const res = await invoke(fn_to_invoke, args);
      console.log(`Fetch [${fn_to_invoke}] took ${(performance.now() - start).toFixed(1)}ms.`, res);
      return res;
    } catch (error) {
      console.error(`Command [${fn_to_invoke}] failed:`, error);
      throw error;
    }
  };
</script>

<div class="app-shell">
  <Sidebar bind:activeNav />

  <main class="content">
    {#if activeNav === 'inventory'}
      <InventoryView
        {inventoryData}
        {inventoryLoading}
        {inventoryError}
        {loadInventory}
        onRefresh={handleRefreshInventory}
      />
    {:else if activeNav === 'foundry'}
      <div class="foundry-placeholder">
        <div class="foundry-icon">⚒</div>
        <h1>Foundry Segments</h1>
        <p>Foundry manufacturing module is offline. Standby for orbital link initialization.</p>
      </div>
    {:else if activeNav === 'dev'}
      <DevView
        {logPath}
        {detectedCount}
        {overlayInteractive}
        {initialBind}
        onSaveBind={handleSaveBind}
        onTestTrigger={handleTestTrigger}
        onTestOverlay={handleTestOverlay}
        onTestRivenTrigger={handleTestRivenTrigger}
        onTestRivenOverlay={handleTestRivenOverlay}
      />
    {/if}
  </main>
</div>

<style>
  :global(body) {
    background: #06070a;
    color: #cbd5e1;
    font-family: 'Inter', sans-serif;
    margin: 0;
    padding: 0;
    user-select: none;
  }

  :global(h1) {
    font-size: 22px;
    font-weight: 800;
    color: #ffffff;
    letter-spacing: -0.02em;
    margin: 0 0 6px 0;
  }

  :global(.page-header) {
    margin-bottom: 24px;
    flex-shrink: 0;
  }

  :global(.subtitle) {
    font-size: 13px;
    color: #64748b;
    margin: 0;
  }

  .app-shell {
    display: flex;
    height: 100vh;
    overflow: hidden;
  }

  .content {
    flex: 1;
    overflow: hidden;
    padding: 28px 32px;
    background: radial-gradient(circle at top right, #0e111a 0%, #06070a 100%);
    display: flex;
    flex-direction: column;
    min-width: 0;
  }

  /* Foundry Placeholder Styling */
  .foundry-placeholder {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    height: 100%;
    text-align: center;
    color: #475569;
    gap: 16px;
  }

  .foundry-icon {
    font-size: 48px;
    color: #1e293b;
    animation: breathe 3s infinite ease-in-out;
  }

  .foundry-placeholder h1 {
    font-size: 18px;
    font-weight: 700;
    color: #94a3b8;
    margin: 0;
  }

  .foundry-placeholder p {
    font-size: 13px;
    max-width: 320px;
    line-height: 1.5;
    margin: 0;
  }

  @keyframes breathe {
    0%, 100% { opacity: 0.4; transform: scale(0.96); }
    50% { opacity: 1; transform: scale(1.04); }
  }
</style>
