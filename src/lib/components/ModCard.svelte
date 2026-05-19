<script lang="ts">
  import { generateModCard } from '$lib/mod-card';

  // module-level: survives component unmount/remount (virtual scroll)
  const _urlCache = new Map<string, string>();

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

  async function renderCard(full: boolean): Promise<string> {
    const cacheKey = `${item.itemType}:${item.rank ?? 0}:${full ? 'f' : 't'}`;
    const hit = _urlCache.get(cacheKey);
    if (hit) return hit;

    let levelStats: unknown[] | null = null;
    if (item.levelStats) {
      try { levelStats = JSON.parse(item.levelStats); } catch { /* ignore */ }
    }
    const blob = await generateModCard({
      name:        item.displayName || item.itemType,
      imageName:   item.imageName,
      rarity:      item.rarity    ?? 'Common',
      polarity:    item.polarity  ?? 'naramon',
      fusionLimit: item.maxRank   ?? 0,
      rank:        item.rank      ?? 0,
      baseDrain:   item.baseDrain ?? 0,
      compatName:  item.compatName  ?? '',
      description: item.description ?? '',
      levelStats,
      full,
      count: full ? null : item.count,
    });
    const url = URL.createObjectURL(blob);
    _urlCache.set(cacheKey, url);
    return url;
  }

  let thumbUrl = $state<string | null>(
    _urlCache.get(`${item.itemType}:${item.rank ?? 0}:t`) ?? null
  );

  $effect(() => {
    if (!wrapEl || thumbUrl) return;
    let live = true;

    const render = () => {
      if (live && !thumbUrl)
        renderCard(false).then(u => { if (live) thumbUrl = u; }).catch(() => {});
    };

    const ric = (window as Window & { requestIdleCallback?: (cb: () => void) => number }).requestIdleCallback;

    // prioritise visible cards: IO fires early, but still defers to idle so the
    // initial paint isn't blocked by 36 concurrent canvas renders
    const io = new IntersectionObserver(([e]) => {
      if (e.isIntersecting) { io.disconnect(); ric ? ric(render) : setTimeout(render, 0); }
    }, { rootMargin: '200px' });
    io.observe(wrapEl);

    // off-screen cards also render during idle time
    const id = ric ? ric(render) : setTimeout(render, 100);

    return () => {
      live = false;
      io.disconnect();
      const cic = (window as Window & { cancelIdleCallback?: (id: number) => void }).cancelIdleCallback;
      ric ? cic?.(id as number) : clearTimeout(id as ReturnType<typeof setTimeout>);
    };
  });

  let wrapEl    = $state<HTMLDivElement | null>(null);
  let hovering  = $state(false);
  let mouseX    = $state(0);
  let mouseY    = $state(0);
  let fullUrl   = $state<string | null>(null);
  let hoverLeft = $state(0);
  let hoverTop  = $state(0);

  const FULL_W = 256;
  const FULL_H = 380;

  function calcHoverPos() {
    if (!wrapEl) return;
    const rect = wrapEl.getBoundingClientRect();
    const cx = rect.left + rect.width / 2;
    const cy = rect.top  + rect.height / 2;
    hoverLeft = Math.min(Math.max(cx - FULL_W / 2, 8), window.innerWidth  - FULL_W - 8);
    hoverTop  = Math.min(Math.max(cy - FULL_H / 2, 8), window.innerHeight - FULL_H - 8);
  }

  function onMouseEnter(e: MouseEvent) {
    hovering = true;
    mouseX = e.clientX; mouseY = e.clientY;
    if (!fullUrl) renderCard(true).then(u => { fullUrl = u; }).catch(() => {});
    calcHoverPos();
  }

  function onMouseMove(e: MouseEvent) {
    mouseX = e.clientX; mouseY = e.clientY;
  }
</script>

<div
  class="mod-wrap"
  bind:this={wrapEl}
  role="img"
  aria-label={item.displayName || item.itemType}
  onmouseenter={onMouseEnter}
  onmousemove={onMouseMove}
  onmouseleave={() => (hovering = false)}
>
  {#if thumbUrl}
    <img src={thumbUrl} alt={item.displayName} class="mod-thumb" loading="lazy" />
  {/if}
</div>

{#if hovering}
  <div class="mod-hover" style="left:{hoverLeft}px; top:{hoverTop}px;">
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
  :global(.mod-hover) {
    position: fixed;
    z-index: 9999;
    pointer-events: none;
    width: 256px;
    border-radius: 4px;
    overflow: hidden;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.9);
    animation: mod-expand 0.2s ease-out forwards;
  }
  :global(.mod-full-img) {
    width: 100%;
    display: block;
    border-radius: 4px;
  }
  @keyframes mod-expand {
    from {
      opacity: 0.2;
      clip-path: inset(30% 0% 30% 0% round 4px);
    }
    to {
      opacity: 1;
      clip-path: inset(0% 0% 0% 0% round 4px);
    }
  }
</style>
