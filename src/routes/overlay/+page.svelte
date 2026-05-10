<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';

  interface ItemPriceResult {
    ocr_text: string;
    matched_name: string;
    slug: string;
    ducats: number | null;
    plat_min_sell: number | null;
  }

  let items = $state<ItemPriceResult[]>([]);
  let loading = $state(false);
  let error = $state('');
  let visible = $state(false);
  let playerCount = $state(4);

  let dismissTimer: ReturnType<typeof setTimeout> | null = null;

  onMount(async () => {
    const unlistenTrigger = await listen<number>('relic-trigger', async (event) => {
      playerCount = event.payload;
      await showOverlay();
    });

    const unlistenTest = await listen<ItemPriceResult[]>('relic-test-data', (event) => {
      showOverlayWithItems(event.payload);
    });

    // Dismiss timer for test overlay is driven from Rust (Tokio) rather than setTimeout —
    // WebKitGTK throttles JS timers on windows with KeyboardMode::None (layer shell).
    const unlistenHide = await listen('hide-overlay', () => {
      hideOverlay();
    });

    return () => { unlistenTrigger(); unlistenTest(); unlistenHide(); };
  });

  function showOverlayWithItems(data: ItemPriceResult[]) {
    if (dismissTimer) clearTimeout(dismissTimer);
    error = '';
    loading = false;
    items = data;
    visible = true;
  }

  async function showOverlay() {
    if (dismissTimer) clearTimeout(dismissTimer);
    error = '';
    items = [];
    loading = true;
    visible = true;

    try {
      items = await invoke<ItemPriceResult[]>('detect_relic_rewards', { playerCount });
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }

    dismissTimer = setTimeout(hideOverlay, 30_000);
  }

  function hideOverlay() {
    visible = false;
  }

  function bestValue(item: ItemPriceResult): 'plat' | 'ducats' | 'unknown' {
    if (!item.plat_min_sell && !item.ducats) return 'unknown';
    if (item.ducats && item.plat_min_sell && item.ducats >= item.plat_min_sell * 15) return 'ducats';
    return 'plat';
  }

  // Proportional bounds from screenshot.rs (measured on 2560×1440 reference).
  const TEXT_X_START = 650  / 2560;
  const TEXT_X_END   = 1920 / 2560;
  const TEXT_Y_END   = 612  / 1440;
  // Offset below the item text strip, in physical pixels.
  const CARD_Y_OFFSET_PHYS = 300;
  // Gap on each side within a slot, as a fraction of window width.
  const CARD_GAP_FRAC = 0.006;

  // Work in physical pixels then divide by devicePixelRatio for CSS px.
  // This avoids the mismatch between the logical CSS viewport and physical
  // monitor pixels that occurs with fractional Wayland scaling / GTK zoom.
  function cardStyle(i: number, n: number): string {
    const dpr   = window.devicePixelRatio;
    const physW = window.innerWidth  * dpr;
    const physH = window.innerHeight * dpr;

    const slotPhysW = (TEXT_X_END - TEXT_X_START) * physW / n;
    const gapPhys   = CARD_GAP_FRAC * physW;

    const leftCss  = (TEXT_X_START * physW + i * slotPhysW + gapPhys) / dpr;
    const widthCss = (slotPhysW - 2 * gapPhys) / dpr;
    const topCss   = (TEXT_Y_END * physH + CARD_Y_OFFSET_PHYS) / dpr;

    return `left:${leftCss.toFixed(1)}px; width:${widthCss.toFixed(1)}px; top:${topCss.toFixed(1)}px`;
  }
</script>

{#if visible}
  <div class="overlay">
    {#if loading}
      <div class="status-message">Analysing relic rewards…</div>
    {:else if error}
      <div class="status-message error">{error}</div>
    {:else}
      {#each items as item, i}
        <div class="card" class:highlight={bestValue(item) === 'plat'} style={cardStyle(i, items.length)}>
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
    /*background-color: rgba(255,255,255,0.05);*/
    font-family: 'Segoe UI', Arial, sans-serif;
  }

  .status-message {
    position: absolute;
    bottom: 2%;
    right: 2%;
    background: rgba(0, 0, 0, 0.82);
    color: #e0e0e0;
    padding: 8px 18px;
    border-radius: 8px;
    font-size: 14px;
    border: 1px solid rgba(255, 255, 255, 0.12);
    white-space: nowrap;
  }

  .status-message.error { color: #ff6b6b; }

  .card {
    position: absolute;
    box-sizing: border-box;
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

  .plat   { color: #c9a227; }
  .ducats { color: #b87333; }
</style>
