<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';
  import { getCurrentWindow } from '@tauri-apps/api/window';
  import { PhysicalPosition, PhysicalSize } from '@tauri-apps/api/dpi';

  interface ItemPriceResult {
    ocr_text: string;
    matched_name: string;
    slug: string;
    ducats: number | null;
    plat_min_sell: number | null;
  }

  interface WindowGeometry { x: number; y: number; width: number; height: number }

  let items = $state<ItemPriceResult[]>([]);
  let loading = $state(false);
  let error = $state('');
  let visible = $state(false);

  let dismissTimer: ReturnType<typeof setTimeout> | null = null;

  const win = getCurrentWindow();
  const log = (msg: string) => invoke('overlay_log', { message: msg });

  onMount(async () => {
    await log('onMount start');
    // Fire-and-forget — awaiting this blocks forever on Wayland/Hyprland.
    win.setIgnoreCursorEvents(true);

    const unlistenTrigger = await listen('relic-trigger', async () => {
      await log('relic-trigger received');
      await showOverlay();
    });

    const unlistenTest = await listen<ItemPriceResult[]>('relic-test-data', async (event) => {
      await log(`relic-test-data received, items=${event.payload.length}`);
      showOverlayWithItems(event.payload);
    });

    await log('listeners registered');
    return () => { unlistenTrigger(); unlistenTest(); };
  });

  async function positionOverWarframe(): Promise<boolean> {
    let geo: WindowGeometry | null;
    try {
      geo = await invoke<WindowGeometry | null>('get_warframe_geometry');
    } catch (e) {
      await log(`get_warframe_geometry threw: ${e}`);
      return false;
    }
    await log(`get_warframe_geometry = ${geo ? `(${geo.x},${geo.y}) ${geo.width}x${geo.height}` : 'null'}`);
    if (!geo) return false;
    try { await win.setPosition(new PhysicalPosition(geo.x, geo.y)); } catch (e) { await log(`setPosition failed: ${e}`); }
    try { await win.setSize(new PhysicalSize(geo.width, geo.height)); } catch (e) { await log(`setSize failed: ${e}`); }
    return true;
  }

  // Positioning already done by Rust before this event fires.
  function showOverlayWithItems(data: ItemPriceResult[]) {
    if (dismissTimer) clearTimeout(dismissTimer);
    error = '';
    loading = false;
    items = data;
    visible = true;
    log(`showOverlayWithItems: visible=true, items=${data.length}`);
    dismissTimer = setTimeout(hideOverlay, 30_000);
  }

  async function showOverlay() {
    if (dismissTimer) clearTimeout(dismissTimer);
    error = '';
    items = [];
    loading = true;
    const ok = await positionOverWarframe();
    if (!ok) { loading = false; return; }
    visible = true;
    await log('showOverlay: visible=true');

    try {
      items = await invoke<ItemPriceResult[]>('detect_relic_rewards');
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }

    dismissTimer = setTimeout(hideOverlay, 30_000);
  }

  function hideOverlay() {
    visible = false;
    log('hideOverlay: visible=false');
  }

  function bestValue(item: ItemPriceResult): 'plat' | 'ducats' | 'unknown' {
    if (!item.plat_min_sell && !item.ducats) return 'unknown';
    if (item.ducats && item.plat_min_sell && item.ducats >= item.plat_min_sell * 15) return 'ducats';
    return 'plat';
  }
</script>

{#if visible}
  <div class="overlay">
    {#if loading}
      <div class="status-message">Analysing relic rewards…</div>
    {:else if error}
      <div class="status-message error">{error}</div>
    {:else}
      <div class="cards">
        {#each items as item, i}
          <div class="card" class:highlight={bestValue(item) === 'plat'}>
            <div class="item-name" title={item.ocr_text}>
              {item.matched_name || item.ocr_text || `Item ${i + 1}`}
            </div>
            <div class="prices">
              <span class="plat">
                {#if item.plat_min_sell != null}⬡ {item.plat_min_sell}p{:else}⬡ —{/if}
              </span>
              <span class="ducats">
                {#if item.ducats != null}◈ {item.ducats}{:else}◈ —{/if}
              </span>
            </div>
          </div>
        {/each}
      </div>
    {/if}
  </div>
{/if}

<style>
  :global(html, body) {
    margin: 0;
    padding: 0;
    background: transparent;
    overflow: hidden;
  }

  /* Full-screen transparent canvas — only the card elements are opaque. */
  .overlay {
    position: fixed;
    inset: 0;
    font-family: 'Segoe UI', Arial, sans-serif;
  }

  /*
   * Cards sit below the in-game reward selection UI.
   * Warframe's reward cards occupy roughly the upper 35% of the screen;
   * we anchor our price strip just below them.
   * Horizontal padding roughly mirrors the game's card gutters (~3% each side).
   */
  .cards {
    position: absolute;
    top: 58%;
    left: 3%;
    right: 3%;
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 10px;
  }

  .status-message {
    position: absolute;
    top: 58%;
    left: 50%;
    transform: translateX(-50%);
    background: rgba(0, 0, 0, 0.82);
    color: #e0e0e0;
    padding: 10px 24px;
    border-radius: 8px;
    font-size: 15px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    white-space: nowrap;
  }

  .status-message.error {
    color: #ff6b6b;
  }

  .card {
    background: rgba(10, 12, 20, 0.88);
    border: 1px solid rgba(255, 255, 255, 0.1);
    border-radius: 8px;
    padding: 10px 14px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    transition: border-color 0.2s;
  }

  .card.highlight {
    border-color: rgba(201, 162, 39, 0.6);
    box-shadow: 0 0 8px rgba(201, 162, 39, 0.2);
  }

  .item-name {
    font-size: 13px;
    font-weight: 600;
    color: #e8e8e8;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    letter-spacing: 0.02em;
  }

  .prices {
    display: flex;
    gap: 12px;
    font-size: 14px;
    font-weight: 700;
  }

  .plat  { color: #c9a227; }
  .ducats { color: #b87333; }
</style>
