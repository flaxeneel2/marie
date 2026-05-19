<script lang="ts">
  type ModItem = {
    itemType: string;
    displayName: string;
    imageName: string;
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

  let { item } = $props<{ item: ModItem }>();

  async function fetchCard(full: boolean): Promise<string> {
    const body = JSON.stringify({
      itemType:   item.itemType,
      imageName:  item.imageName,
      name:       item.displayName || item.itemType,
      rarity:     item.rarity    ?? 'Common',
      polarity:   item.polarity  ?? 'naramon',
      maxRank:    item.maxRank   ?? 0,
      rank:       item.rank      ?? 0,
      full,
      compatName:  item.compatName  ?? '',
      description: item.description ?? '',
      levelStats:  item.levelStats  ?? null,
      baseDrain:   item.baseDrain   ?? 0,
    });
    const resp = await fetch('/api/mod-card', {
      method: 'POST',
      headers: { 'Content-Type': 'application/json' },
      body,
    });
    if (!resp.ok) throw new Error(`mod-card ${resp.status}`);
    const blob = await resp.blob();
    return URL.createObjectURL(blob);
  }

  // collapsed card URL — created once on mount
  let thumbUrl = $state<string | null>(null);
  $effect(() => {
    fetchCard(false).then(u => { thumbUrl = u; }).catch(() => {});
  });

  // hover state — trigger full card render lazily
  let hovering = $state(false);
  let mouseX   = $state(0);
  let mouseY   = $state(0);
  let fullUrl  = $state<string | null>(null);

  function onMouseEnter(e: MouseEvent) {
    hovering = true;
    mouseX = e.clientX;
    mouseY = e.clientY;
    if (!fullUrl) {
      fetchCard(true).then(u => { fullUrl = u; }).catch(() => {});
    }
  }
  function onMouseMove(e: MouseEvent) {
    mouseX = e.clientX;
    mouseY = e.clientY;
  }
</script>

<div
  class="mod-wrap"
  role="img"
  aria-label={item.displayName || item.itemType}
  onmouseenter={onMouseEnter}
  onmousemove={onMouseMove}
  onmouseleave={() => (hovering = false)}
>
  {#if thumbUrl}
    <img src={thumbUrl} alt={item.displayName} class="mod-thumb" loading="lazy" />
  {/if}
  {#if item.count !== null && item.count > 1}
    <span class="mod-count">{item.count}</span>
  {/if}
</div>

{#if hovering}
  {@const above = mouseY > (typeof window !== 'undefined' ? window.innerHeight / 2 : 400)}
  {@const left  = Math.min(Math.max(mouseX - 128, 8), (typeof window !== 'undefined' ? window.innerWidth : 1280) - 264)}
  <div
    class="mod-hover"
    style="left:{left}px; top:{above ? mouseY - 392 : mouseY + 12}px;"
  >
    {#if fullUrl}
      <img src={fullUrl} alt={item.displayName} class="mod-full-img" />
    {/if}
  </div>
{/if}

<style>
  .mod-wrap {
    position: relative;
    cursor: default;
    border-radius: 4px;
  }

  .mod-thumb {
    width: 100%;
    display: block;
    border-radius: 4px;
  }

  .mod-count {
    position: absolute;
    top: 4px;
    left: 5px;
    font-size: 11px;
    font-weight: 700;
    color: #c9a227;
    background: rgba(0, 0, 0, 0.6);
    border-radius: 3px;
    padding: 1px 4px;
    pointer-events: none;
    line-height: 1.4;
  }

  :global(.mod-hover) {
    position: fixed;
    z-index: 9999;
    pointer-events: none;
    border-radius: 4px;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.9);
    animation: mod-pop 0.1s ease;
    width: 256px;
  }

  :global(.mod-full-img) {
    width: 100%;
    display: block;
    border-radius: 4px;
  }

  @keyframes mod-pop {
    from { opacity: 0; transform: scale(0.93); }
    to   { opacity: 1; transform: scale(1); }
  }
</style>
