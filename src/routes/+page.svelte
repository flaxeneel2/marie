<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { invoke, type InvokeArgs } from '@tauri-apps/api/core';
  import ModCard from '$lib/components/ModCard.svelte';

  // ── Nav ───────────────────────────────────────────────────────────────────────
  type NavItem = 'inventory' | 'foundry' | 'dev';
  let activeNav = $state<NavItem>('inventory');

  // ── Dev tab state ─────────────────────────────────────────────────────────────
  let logPath = $state('…');
  let triggerStatus = $state('');
  let detectedCount = $state<number | null>(null);
  let overrideEnabled = $state(false);
  let playerCount = $state(4);
  let overlayInteractive = $state(false);

  // ── Inventory state ───────────────────────────────────────────────────────────
  type DisplayItem = { itemType: string; displayName: string; imageName: string; overlayImageName: string; count: number | null; rank: number | null; maxRank: number | null; rarity: string | null; polarity: string | null };
  type InventoryView = {
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
  let inventoryTab = $state<InventoryTab>('warframes');
  let relicTier = $state<RelicTier>('all');
  let inventoryData = $state<InventoryView | null>(null);
  let inventoryError = $state('');
  let inventoryLoading = $state(false);

  async function loadInventory() {
    if (inventoryData) return;
    inventoryLoading = true;
    inventoryError = '';
    try {
      inventoryData = await invoke<InventoryView>('get_inventory');
    } catch (e) {
      inventoryError = String(e);
    } finally {
      inventoryLoading = false;
    }
  }

  $effect(() => {
    if (activeNav === 'inventory') loadInventory();
  });

  function currentItems(): DisplayItem[] {
    if (!inventoryData) return [];
    let items = [...(inventoryData[inventoryTab] ?? [])];
    if (inventoryTab === 'relics') {
      if (relicTier !== 'all') {
        const prefix = relicTier.charAt(0).toUpperCase() + relicTier.slice(1);
        items = items.filter(i => i.displayName.startsWith(prefix));
      }
      items.sort((a, b) => a.displayName.localeCompare(b.displayName));
    }
    return items;
  }

  // ── Dev tab helpers ───────────────────────────────────────────────────────────
  onMount(() => {
    invoke<string>('ee_log_path').then(p => (logPath = p));
    invoke<string>('get_interaction_shortcut').then(b => (currentBind = b));

    const cleanups: Array<() => void> = [];
    listen<number>('relic-trigger', (event) => {
      detectedCount = event.payload;
    }).then(fn => cleanups.push(fn));
    listen<boolean>('overlay-interactive-changed', (event) => {
      overlayInteractive = event.payload;
    }).then(fn => cleanups.push(fn));

    return () => cleanups.forEach(fn => fn());
  });

  function effectiveCount(): number {
    return overrideEnabled ? playerCount : (detectedCount ?? 4);
  }

  async function testTrigger() {
    const count = effectiveCount();
    triggerStatus = `Trigger sent (${count} player${count === 1 ? '' : 's'}) — check the overlay window.`;
    await invoke('test_trigger', { playerCount: count });
    setTimeout(() => (triggerStatus = ''), 4000);
  }

  async function testOverlay() {
    triggerStatus = 'Overlay shown with fake data.';
    await invoke('show_test_overlay');
    setTimeout(() => (triggerStatus = ''), 4000);
  }

  async function testRivenTrigger() {
    triggerStatus = 'Riven trigger sent — check the overlay window.';
    await invoke('test_riven_trigger');
    setTimeout(() => (triggerStatus = ''), 4000);
  }

  async function testRivenOverlay() {
    triggerStatus = 'Riven overlay shown with fake data.';
    await invoke('show_test_riven_overlay');
    setTimeout(() => (triggerStatus = ''), 4000);
  }

  // ── Keybind recorder ──────────────────────────────────────────────────────────
  let currentBind = $state('');
  let recordingBind = $state(false);
  let bindStatus = $state('');

  $effect(() => {
    if (!recordingBind) return;
    window.addEventListener('keydown', onBindKeydown);
    return () => window.removeEventListener('keydown', onBindKeydown);
  });

  window.core = window.core || {} as Window["core"];

  window.core.invoke = async (fn_to_invoke: string, args: InvokeArgs | undefined) => {
    const start = performance.now();
    try {
      const res = await invoke(fn_to_invoke, args);
      console.log(`Fetch [${fn_to_invoke}] took ${performance.now() - start}ms.`, res);
      return res;
    } catch (error) {
      console.error(`Command [${fn_to_invoke}] failed:`, error);
      throw error;
    }
  };

  function hyprlandKey(e: KeyboardEvent): string {
    const map: Record<string, string> = {
      ' ': 'SPACE', Enter: 'Return', Escape: 'Escape', Tab: 'Tab',
      Backspace: 'BackSpace', Delete: 'Delete',
      ArrowLeft: 'Left', ArrowRight: 'Right', ArrowUp: 'Up', ArrowDown: 'Down',
      Home: 'Home', End: 'End', PageUp: 'Prior', PageDown: 'Next',
      ';': 'semicolon', ':': 'colon', "'": 'apostrophe', '"': 'quotedbl',
      ',': 'comma', '.': 'period', '/': 'slash', '\\': 'backslash',
      '[': 'bracketleft', ']': 'bracketright', '`': 'grave',
      '-': 'minus', '=': 'equal',
    };
    if (e.key.startsWith('F') && /^F\d+$/.test(e.key)) return e.key;
    if (map[e.key]) return map[e.key];
    if (e.key.length === 1) return e.key.toLowerCase();
    return '';
  }

  function onBindKeydown(e: KeyboardEvent) {
    e.preventDefault();
    const MODS = ['Control', 'Alt', 'Shift', 'Meta'];
    if (MODS.includes(e.key)) return;

    const mods: string[] = [];
    if (e.ctrlKey)  mods.push('CTRL');
    if (e.altKey)   mods.push('ALT');
    if (e.shiftKey) mods.push('SHIFT');
    if (e.metaKey)  mods.push('SUPER');

    const key = hyprlandKey(e);
    if (!key) return;

    currentBind = mods.length ? `${mods.join(' ')}, ${key}` : key;
    recordingBind = false;
  }

  async function saveBind() {
    try {
      await invoke('set_interaction_shortcut', { modsKey: currentBind });
      bindStatus = 'Saved & applied.';
    } catch (err) {
      bindStatus = `Error: ${err}`;
    }
    setTimeout(() => (bindStatus = ''), 4000);
  }
</script>

<div class="app-shell">
  <!-- ── Sidebar ── -->
  <nav class="sidebar">
    <div class="sidebar-logo">
      <span class="logo-text">marie</span>
    </div>

    <div class="nav-group">
      <button
        class="nav-item"
        class:active={activeNav === 'inventory'}
        onclick={() => (activeNav = 'inventory')}
      >
        <span class="nav-icon">⬡</span>
        Inventory
      </button>
      <button
        class="nav-item"
        class:active={activeNav === 'foundry'}
        onclick={() => (activeNav = 'foundry')}
      >
        <span class="nav-icon">⚒</span>
        Foundry
      </button>
    </div>

    <div class="nav-group nav-bottom">
      <button
        class="nav-item nav-dev"
        class:active={activeNav === 'dev'}
        onclick={() => (activeNav = 'dev')}
      >
        <span class="nav-icon">⚙</span>
        Dev
      </button>
    </div>
  </nav>

  <!-- ── Content ── -->
  <main class="content">
    {#if activeNav === 'inventory'}
      <div class="page-header">
        <h1>Inventory</h1>
      </div>

      <div class="subtab-bar">
        {#each [
          { id: 'warframes',  label: 'Warframes' },
          { id: 'gear',       label: 'Gear' },
          { id: 'relics',     label: 'Relics' },
          { id: 'mods',       label: 'Mods' },
          { id: 'resources',  label: 'Resources' },
          { id: 'blueprints', label: 'Blueprints' },
        ] as tab}
          <button
            class="subtab"
            class:active={inventoryTab === tab.id}
            onclick={() => (inventoryTab = tab.id as InventoryTab)}
          >{tab.label}</button>
        {/each}

        <button
          class="subtab refresh-btn"
          onclick={() => { inventoryData = null; loadInventory(); }}
          title="Refresh inventory"
        >↻</button>
      </div>

      {#if inventoryTab === 'relics'}
        <div class="subtab-bar tier-bar">
          {#each [
            { id: 'all',     label: 'All' },
            { id: 'lith',    label: 'Lith' },
            { id: 'meso',    label: 'Meso' },
            { id: 'neo',     label: 'Neo' },
            { id: 'axi',     label: 'Axi' },
            { id: 'requiem', label: 'Requiem' },
          ] as tier}
            <button
              class="subtab subtab-sm"
              class:active={relicTier === tier.id}
              onclick={() => (relicTier = tier.id as RelicTier)}
            >{tier.label}</button>
          {/each}
        </div>
      {/if}

      {#if inventoryLoading}
        <div class="state-msg">Loading inventory…</div>
      {:else if inventoryError}
        <div class="state-msg error">{inventoryError}</div>
      {:else if inventoryData}
        {@const items = currentItems()}
        {#if items.length === 0}
          <div class="state-msg muted">No items in this category.</div>
        {:else if inventoryTab === 'mods'}
          <div class="mod-grid">
            {#each items as item}
              <ModCard {item} />
            {/each}
          </div>
        {:else}
          <div class="item-grid">
            {#each items as item}
              <div class="item-card">
                {#if item.overlayImageName}
                  <div class="item-img-stack">
                    <img class="item-img" src="/img/wf-assets/{item.imageName.replace('.png', '.avif')}" alt="" />
                    <img class="item-img-overlay" src="/img/wf-assets/{item.overlayImageName.replace('.png', '.avif')}" alt={item.displayName || item.itemType} />
                  </div>
                {:else if item.imageName}
                  <img
                    class="item-img"
                    src="/img/wf-assets/{item.imageName.replace('.png', '.avif')}"
                    alt={item.displayName || item.itemType}
                  />
                {:else}
                  <div class="item-img-placeholder"></div>
                {/if}
                <div class="item-info">
                  <span class="item-name" title={item.itemType}>{item.displayName || item.itemType}</span>
                  {#if item.count !== null}
                    <span class="item-count">×{item.count}</span>
                  {/if}
                </div>
              </div>
            {/each}
          </div>
        {/if}
      {:else}
        <div class="state-msg muted">No data.</div>
      {/if}

    {:else if activeNav === 'foundry'}
      <div class="page-header">
        <h1>Foundry</h1>
      </div>
      <div class="state-msg muted">Coming soon.</div>

    {:else if activeNav === 'dev'}
      <div class="page-header">
        <h1>Dev</h1>
        <p class="subtitle">Development tools & settings</p>
      </div>

      <section>
        <h2>Status</h2>
        <dl>
          <dt>EE.log path</dt>
          <dd class="mono">{logPath}</dd>
          <dt>Current lobby</dt>
          <dd>
            {#if detectedCount !== null}
              {detectedCount} player{detectedCount === 1 ? '' : 's'}
            {:else}
              <span class="muted">— (no relic screen seen yet)</span>
            {/if}
          </dd>
          <dt>Overlay</dt>
          <dd>Transparent window – appears automatically when a relic reward screen is detected.</dd>
          <dt>Interactive mode</dt>
          <dd class:interactive-on={overlayInteractive} class:interactive-off={!overlayInteractive}>
            {overlayInteractive ? '● ON' : '○ OFF'}
          </dd>
        </dl>
      </section>

      <section>
        <h2>Relics — Development</h2>
        <p>Send a test trigger to open the overlay and exercise the OCR + price pipeline:</p>
        <div class="player-count-row">
          <span class="player-count-label">Players</span>
          {#each [1, 2, 3, 4] as n}
            <button
              class="count-btn"
              class:active={overrideEnabled && playerCount === n}
              class:disabled={!overrideEnabled}
              onclick={() => { overrideEnabled = true; playerCount = n; }}
            >{n}</button>
          {/each}
          <button
            class="override-btn"
            class:active={overrideEnabled}
            onclick={() => (overrideEnabled = !overrideEnabled)}
            title={overrideEnabled ? 'Using manual player count' : 'Using auto-detected player count'}
          >{overrideEnabled ? 'Override ON' : 'Override OFF'}</button>
        </div>
        <button onclick={testTrigger}>Send test trigger</button>
        <button class="secondary" onclick={testOverlay}>Test overlay (fake data)</button>
        {#if triggerStatus}
          <p class="status">{triggerStatus}</p>
        {/if}
      </section>

      <section>
        <h2>Rivens — Development</h2>
        <p>Send a test trigger to exercise the OCR + grading pipeline on the real game screen:</p>
        <button onclick={testRivenTrigger}>Send test trigger</button>
        <button class="secondary" onclick={testRivenOverlay}>Test overlay (fake data)</button>
      </section>

      <section>
        <h2>Settings</h2>
        <dl>
          <dt>Overlay hotkey</dt>
          <dd>
            <!-- svelte-ignore a11y_no_static_element_interactions -->
            <div
              class="bind-recorder"
              class:recording={recordingBind}
              tabindex="0"
              role="button"
              onclick={() => (recordingBind = true)}
              onblur={() => (recordingBind = false)}
              onkeydown={() => {}}
            >
              {#if recordingBind}
                <span class="bind-hint">Press a key combo…</span>
              {:else}
                <span class="bind-value">{currentBind || '—'}</span>
              {/if}
            </div>
            <p class="hint">Click the box, then press your desired key combination.</p>
            <button onclick={saveBind}>Apply</button>
            <p class="hint">Re-click Apply if the shortcut stops working after a Hyprland config reload — it re-injects the bind.</p>
            {#if bindStatus}
              <p class="status">{bindStatus}</p>
            {/if}
          </dd>
        </dl>
      </section>
    {/if}
  </main>
</div>

<style>
  :root {
    font-family: 'Segoe UI', Arial, sans-serif;
    font-size: 15px;
    line-height: 1.6;
    color: #e0e0e0;
    background: #0f1117;
  }

  /* ── Layout ── */
  .app-shell {
    display: flex;
    height: 100vh;
    overflow: hidden;
  }

  /* ── Sidebar ── */
  .sidebar {
    width: 180px;
    min-width: 180px;
    background: #0b0d13;
    border-right: 1px solid #1e2130;
    display: flex;
    flex-direction: column;
    padding: 0;
  }

  .sidebar-logo {
    padding: 20px 16px 16px;
    border-bottom: 1px solid #1e2130;
  }

  .logo-text {
    font-size: 1.3rem;
    font-weight: 700;
    color: #c9a227;
    letter-spacing: 0.08em;
  }

  .nav-group {
    display: flex;
    flex-direction: column;
    padding: 8px 0;
  }

  .nav-bottom {
    margin-top: auto;
    border-top: 1px solid #1e2130;
  }

  .nav-item {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 10px 16px;
    background: none;
    border: none;
    border-radius: 0;
    color: #888;
    font-size: 13px;
    font-weight: 500;
    cursor: pointer;
    text-align: left;
    transition: background 0.12s, color 0.12s;
    margin: 0;
  }

  .nav-item:hover {
    background: #161922;
    color: #ccc;
    opacity: 1;
  }

  .nav-item.active {
    background: #1a1f2e;
    color: #c9a227;
    border-left: 2px solid #c9a227;
    padding-left: 14px;
  }

  .nav-dev {
    color: #666;
  }

  .nav-icon {
    font-size: 14px;
    width: 16px;
    text-align: center;
  }

  /* ── Content area ── */
  .content {
    flex: 1;
    overflow-y: auto;
    padding: 24px 28px;
  }

  .page-header {
    margin-bottom: 20px;
  }

  h1 {
    font-size: 1.5rem;
    font-weight: 700;
    margin: 0 0 4px;
    color: #e0e0e0;
  }

  .subtitle {
    margin: 0 0 20px;
    color: #888;
    font-size: 13px;
  }

  h2 {
    font-size: 0.85rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: #aaa;
    margin: 0 0 10px;
  }

  section {
    background: #161922;
    border: 1px solid #2a2d3a;
    border-radius: 8px;
    padding: 16px 20px;
    margin-bottom: 16px;
    max-width: 640px;
  }

  /* ── Subtabs ── */
  .subtab-bar {
    display: flex;
    gap: 4px;
    margin-bottom: 20px;
    flex-wrap: wrap;
    align-items: center;
  }

  .subtab {
    background: #161922;
    border: 1px solid #2a2d3a;
    border-radius: 6px;
    color: #888;
    font-size: 12px;
    font-weight: 600;
    padding: 6px 14px;
    cursor: pointer;
    transition: background 0.12s, color 0.12s, border-color 0.12s;
    margin: 0;
  }

  .subtab:hover {
    background: #1e2236;
    color: #ccc;
    opacity: 1;
  }

  .subtab.active {
    background: #1a1f2e;
    color: #c9a227;
    border-color: #c9a227;
  }

  .refresh-btn {
    margin-left: auto;
    font-size: 15px;
    padding: 4px 10px;
  }

  .tier-bar {
    margin-top: -12px;
    margin-bottom: 12px;
  }

  .subtab-sm {
    font-size: 11px;
    padding: 4px 10px;
  }

  /* ── Mod grid ── */
  .mod-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 10px;
    overflow: visible;
  }

  /* ── Item grid ── */
  .item-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(160px, 1fr));
    gap: 12px;
  }

  .item-card {
    background: #161922;
    border: 1px solid #2a2d3a;
    border-radius: 8px;
    padding: 12px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .item-img {
    width: 100%;
    aspect-ratio: 1;
    object-fit: contain;
    border-radius: 6px;
    background: #ffffff08;
  }

  .item-img-stack {
    position: relative;
    width: 100%;
    aspect-ratio: 1;
    border-radius: 6px;
    background: #ffffff08;
  }

  .item-img-stack .item-img {
    position: absolute;
    inset: 0;
    border-radius: 6px;
    opacity: 0.5;
    background: transparent;
  }

  .item-img-overlay {
    position: absolute;
    inset: 10%;
    width: 80%;
    height: 80%;
    object-fit: contain;
  }

  .item-img-placeholder {
    width: 100%;
    aspect-ratio: 1;
    background: #ffffff14;
    border: 1px solid #2a2d3a;
    border-radius: 6px;
  }

  .item-info {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .item-name {
    font-size: 10px;
    color: #bbb;
    word-break: break-all;
    line-height: 1.4;
  }

  .item-count {
    font-size: 12px;
    font-weight: 700;
    color: #c9a227;
  }

  /* ── Mod grid ── */
  .mod-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(80px, 1fr));
    gap: 8px;
  }

  /* ── State messages ── */
  .state-msg {
    padding: 32px;
    text-align: center;
    font-size: 13px;
    color: #bbb;
  }

  .state-msg.error {
    color: #ff6b6b;
  }

  .state-msg.muted {
    color: #555;
  }

  /* ── Dev tab shared ── */
  dl {
    margin: 0;
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px 16px;
  }

  dt {
    color: #888;
    font-size: 13px;
    align-self: start;
    padding-top: 1px;
  }

  dd {
    margin: 0;
    color: #ccc;
    font-size: 13px;
  }

  .mono {
    font-family: 'Consolas', 'Courier New', monospace;
    font-size: 12px;
    word-break: break-all;
  }

  .interactive-on  { color: #7ec8a0; font-weight: 600; }
  .interactive-off { color: #555; }

  button {
    background: #c9a227;
    color: #0f1117;
    border: none;
    border-radius: 6px;
    padding: 8px 18px;
    font-size: 13px;
    font-weight: 700;
    cursor: pointer;
    transition: opacity 0.15s;
    margin-right: 8px;
  }

  button:hover {
    opacity: 0.85;
  }

  button.secondary {
    background: #2a2d3a;
    color: #ccc;
    border: 1px solid #3a3d4a;
  }

  .player-count-row {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 12px;
  }

  .player-count-label {
    font-size: 13px;
    color: #888;
    margin-right: 2px;
  }

  .count-btn {
    background: #2a2d3a;
    color: #ccc;
    border: 1px solid #3a3d4a;
    border-radius: 6px;
    padding: 5px 12px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    margin: 0;
    transition: background 0.15s, color 0.15s, border-color 0.15s;
  }

  .count-btn:hover {
    background: #353849;
    opacity: 1;
  }

  .count-btn.active {
    background: #c9a227;
    color: #0f1117;
    border-color: #c9a227;
  }

  .count-btn.disabled {
    opacity: 0.35;
    cursor: default;
  }

  .override-btn {
    margin-left: 6px;
    background: #2a2d3a;
    color: #888;
    border: 1px solid #3a3d4a;
    border-radius: 6px;
    padding: 5px 12px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    margin-right: 0;
    transition: background 0.15s, color 0.15s, border-color 0.15s;
  }

  .override-btn:hover {
    background: #353849;
    opacity: 1;
  }

  .override-btn.active {
    background: #1e3a2a;
    color: #7ec8a0;
    border-color: #3a6a50;
  }

  .muted {
    color: #555;
  }

  .status {
    margin: 8px 0 0;
    font-size: 13px;
    color: #7ec8a0;
  }

  p {
    margin: 0 0 12px;
    font-size: 13px;
    color: #bbb;
  }

  .hint {
    color: #666;
    font-size: 12px;
    margin: 4px 0 8px;
  }

  .bind-recorder {
    display: inline-flex;
    align-items: center;
    min-width: 180px;
    background: #0f1117;
    border: 1px solid #2a2d3a;
    border-radius: 6px;
    padding: 8px 14px;
    margin-bottom: 8px;
    cursor: pointer;
    outline: none;
    transition: border-color 0.15s;
  }

  .bind-recorder:hover,
  .bind-recorder:focus {
    border-color: #c9a227;
  }

  .bind-recorder.recording {
    border-color: #7ec8a0;
  }

  .bind-value {
    font-family: 'Consolas', 'Courier New', monospace;
    font-size: 12px;
    color: #c9a227;
  }

  .bind-hint {
    font-size: 12px;
    color: #7ec8a0;
    font-style: italic;
  }
</style>
