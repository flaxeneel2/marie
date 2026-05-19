<script lang="ts">
  import ModCard from '$lib/components/ModCard.svelte';

  const COLS    = 4;
  const GAP     = 10;
  const ASPECT  = 150 / 256; // thumb h/w ratio
  const OVERSCAN = 2;

  let { items } = $props<{ items: unknown[] }>();

  let viewport = $state<HTMLDivElement | null>(null);
  let vpW      = $state(0);
  let vpH      = $state(0);
  let scrollTop = $state(0);

  const cellW    = $derived(vpW > 0 ? (vpW - (COLS - 1) * GAP) / COLS : 0);
  const rowH     = $derived(cellW > 0 ? cellW * ASPECT + GAP : 0);
  const rowCount = $derived(Math.ceil(items.length / COLS));
  const totalH   = $derived(rowCount > 0 && rowH > 0 ? rowCount * rowH - GAP : 0);

  const firstRow = $derived(rowH > 0 ? Math.max(0, Math.floor(scrollTop / rowH) - OVERSCAN) : 0);
  const lastRow  = $derived(rowH > 0 ? Math.min(rowCount - 1, Math.ceil((scrollTop + vpH) / rowH) + OVERSCAN) : Math.max(0, rowCount - 1));

  const slice   = $derived(items.slice(firstRow * COLS, (lastRow + 1) * COLS));
  const offsetY = $derived(firstRow * rowH);

  let _prevVpW = 0;
  $effect(() => {
    if (vpW > 0 && _prevVpW === 0) {
      _prevVpW = vpW;
      console.log(`[VirtualModGrid] vpW resolved: ${vpW}px → rowH=${rowH.toFixed(1)}px slice=${slice.length}/${items.length} items`);
    }
  });
</script>

<div
  class="virtual-viewport"
  bind:this={viewport}
  bind:clientWidth={vpW}
  bind:clientHeight={vpH}
  onscroll={(e) => (scrollTop = (e.currentTarget as HTMLElement).scrollTop)}
>
  <div style="height:{totalH}px; position:relative;">
    <div class="mod-grid" style="position:absolute; top:{offsetY}px; left:0; right:0;">
      {#each slice as item (item.itemType)}
        <ModCard {item} />
      {/each}
    </div>
  </div>
</div>

<style>
  .virtual-viewport {
    height: 100%;
    overflow-y: auto;
  }
  .mod-grid {
    display: grid;
    grid-template-columns: repeat(4, 1fr);
    gap: 10px;
  }
</style>
