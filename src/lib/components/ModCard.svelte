<script lang="ts">
  import { onMount } from 'svelte';

  type ModItem = {
    itemType: string;
    displayName: string;
    imageName: string;
    count: number | null;
    rank: number | null;
    maxRank: number | null;
    rarity: string | null;
    polarity: string | null;
  };

  let { item } = $props<{ item: ModItem }>();

  // ── Constants matching mod-generator ─────────────────────────────────────────

  const H_PAD = 8;

  const RARITY_TIER: Record<string, string> = {
    Common: 'Bronze',
    Uncommon: 'Silver',
    Rare: 'Gold',
    Legendary: 'Legendary',
  };

  const TIER_COLOR: Record<string, string> = {
    Bronze: '#CA9A87',
    Silver: '#FFFFFF',
    Gold: '#FAE7BE',
    Legendary: '#FFFFFF',
    Omega: '#AC83D5',
  };

  // ── Helpers ───────────────────────────────────────────────────────────────────

  function getTier(): string {
    if (item.itemType.includes('Randomized')) return 'Omega';
    return RARITY_TIER[item.rarity ?? ''] ?? 'Bronze';
  }

  function frame(name: string): string {
    return `/img/mod-frames/${name}`;
  }

  function loadImg(src: string): Promise<HTMLImageElement> {
    return new Promise((resolve, reject) => {
      const img = new Image();
      img.onload = () => resolve(img);
      img.onerror = () => reject(new Error(`load failed: ${src}`));
      img.src = src;
    });
  }

  function shadeCanvas(src: CanvasImageSource, w: number, h: number, brightness: number): HTMLCanvasElement {
    const c = document.createElement('canvas');
    c.width = w; c.height = h;
    const ctx = c.getContext('2d')!;
    ctx.filter = `brightness(${brightness})`;
    ctx.drawImage(src, 0, 0);
    ctx.filter = 'none';
    return c;
  }

  function flipImg(img: HTMLImageElement): HTMLCanvasElement {
    const c = document.createElement('canvas');
    c.width = img.naturalWidth; c.height = img.naturalHeight;
    const ctx = c.getContext('2d')!;
    ctx.translate(img.naturalWidth, 0);
    ctx.scale(-1, 1);
    ctx.drawImage(img, 0, 0);
    return c;
  }

  function thumbSrc(): string {
    return `/img/wf-assets/${item.imageName.replace('.png', '.avif').replace('.jpg', '.avif')}`;
  }

  // ── Bottom frame (rank slots + corner lights) — shared by both renders ────────

  async function makeBottomFrame(
    bottomImg: HTMLImageElement,
    cornerLightsImg: HTMLImageElement,
    tier: string
  ): Promise<HTMLCanvasElement> {
    const [rankEmpty, rankActive, rankComplete] = await Promise.all([
      loadImg(frame('RankSlotEmpty.png')),
      loadImg(frame('RankSlotActive.png')),
      loadImg(frame('RankCompleteLine.png')),
    ]);

    const c = document.createElement('canvas');
    c.width = bottomImg.naturalWidth; c.height = bottomImg.naturalHeight;
    const ctx = c.getContext('2d')!;
    ctx.drawImage(bottomImg, 0, 0);

    const cornerFlipped = flipImg(cornerLightsImg);
    const isRiven = tier === 'Omega';
    const isLegendary = tier === 'Legendary';

    if (isRiven) {
      const h = c.height * 0.27;
      ctx.drawImage(cornerLightsImg, c.width * 0.73, h);
      ctx.drawImage(cornerFlipped, c.width * 0.04, h);
    } else if (isLegendary) {
      const h = c.height * 0.32;
      ctx.drawImage(cornerLightsImg, c.width * 0.76, h);
      ctx.drawImage(cornerFlipped, -(c.width * 0.01), h);
    } else {
      const h = c.height * 0.24;
      ctx.drawImage(cornerLightsImg, c.width * 0.76, h);
      ctx.drawImage(cornerFlipped, -(c.width * 0.01), h);
    }

    const isRare = isRiven || isLegendary;
    const slotLineH = isRare ? c.height * 0.84 : c.height * 0.74;
    const slotH    = isRare ? c.height * 0.82 : c.height * 0.72;

    const maxRank = Math.min(item.maxRank ?? 0, 10);
    const rank    = item.rank ?? 0;

    if (rank === maxRank && maxRank > 0) ctx.drawImage(rankComplete, 0, slotLineH);

    let slotX = c.width * 0.29;
    if (maxRank <= 3) slotX = c.width * 0.425;
    else if (maxRank <= 5) slotX = c.width * 0.39;

    for (let i = 0; i < maxRank; i++) {
      ctx.drawImage(i < rank ? rankActive : rankEmpty, slotX, slotH);
      slotX += 11;
    }

    return c;
  }

  function drawCountBadge(ctx: CanvasRenderingContext2D, color: string, x: number, y: number, fontSize: number) {
    if (item.count === null || item.count <= 1) return;
    const text = String(item.count);
    ctx.font = `bold ${fontSize}px sans-serif`;
    ctx.textAlign = 'left';
    ctx.textBaseline = 'top';
    const tw = ctx.measureText(text).width + 8;
    ctx.fillStyle = 'rgba(0,0,0,0.6)';
    ctx.fillRect(x, y, tw, fontSize + 6);
    ctx.fillStyle = color;
    ctx.fillText(text, x + 4, y + 3);
  }

  // ── Collapsed card (ports generateCollapsed) ──────────────────────────────────

  let dataUrl = $state('');

  async function drawCollapsed() {
    const tier    = getTier();
    const isRiven = tier === 'Omega';
    const cardW   = isRiven ? 292 : 256;
    const cardH   = isRiven ? 180 : 150;
    const color   = TIER_COLOR[tier] ?? '#CA9A87';

    const [topImg, bottomImg, cornerLightsImg] = await Promise.all([
      loadImg(frame(`${tier}FrameTop.png`)),
      loadImg(frame(`${tier}FrameBottom.png`)),
      loadImg(frame(`${tier}CornerLights.png`)),
    ]);

    const inner = document.createElement('canvas');
    inner.width = cardW; inner.height = 256;
    const ctx = inner.getContext('2d')!;

    if (item.imageName) {
      try {
        const thumb = await loadImg(thumbSrc());
        const thumbW = inner.width - H_PAD * 2;
        const thumbH = topImg.naturalHeight / 2 + bottomImg.naturalHeight / 2;
        ctx.drawImage(
          shadeCanvas(thumb, thumb.naturalWidth, thumb.naturalHeight, 0.3),
          topImg.naturalWidth * 0.03, topImg.naturalHeight * 0.3, thumbW, thumbH
        );
      } catch { /* no image */ }
    }

    ctx.fillStyle = color;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.font = '400 22px sans-serif';
    ctx.fillText(item.displayName || item.itemType, inner.width * 0.5, topImg.naturalHeight, inner.width - H_PAD * 2);

    if (topImg.naturalWidth > inner.width) {
      ctx.drawImage(topImg, -(topImg.naturalWidth - inner.width - H_PAD * 5) / 2, 0);
    } else {
      ctx.drawImage(topImg, 0, 0);
    }

    const bottomCanvas = await makeBottomFrame(bottomImg, cornerLightsImg, tier);
    const posY = topImg.naturalHeight * 0.5;
    if (bottomImg.naturalWidth > inner.width) {
      ctx.drawImage(bottomCanvas, -(bottomImg.naturalWidth - inner.width - H_PAD * 5) / 2, posY);
    } else {
      ctx.drawImage(bottomCanvas, 0, posY);
    }

    const out = document.createElement('canvas');
    out.width = cardW; out.height = cardH;
    const outCtx = out.getContext('2d')!;
    outCtx.drawImage(inner, 0, 0);
    drawCountBadge(outCtx, color, 4, 4, 13);

    dataUrl = out.toDataURL('image/png');
  }

  // ── Full card (ports generate()) — rendered lazily on first hover ─────────────

  let fullDataUrl = $state('');
  let fullPending = false;

  async function drawFull() {
    if (fullDataUrl || fullPending) return;
    fullPending = true;

    const tier    = getTier();
    const isRiven = tier === 'Omega';
    const cardW   = isRiven ? 292 : 256;
    const color   = TIER_COLOR[tier] ?? '#CA9A87';

    const bgName     = isRiven ? 'LegendaryBackground.png' : `${tier}Background.png`;
    const backerName = isRiven ? 'RivenTopRightBacker.png' : `${tier}TopRightBacker.png`;
    const tabName    = isRiven ? 'RivenLowerTab.png'       : `${tier}LowerTab.png`;

    const [bgImg, backerImg, lowerTabImg, sideImg, topImg, bottomImg, cornerImg] = await Promise.all([
      loadImg(frame(bgName)),
      loadImg(frame(backerName)),
      loadImg(frame(tabName)),
      loadImg(frame(`${tier}SideLight.png`)),
      loadImg(frame(`${tier}FrameTop.png`)),
      loadImg(frame(`${tier}FrameBottom.png`)),
      loadImg(frame(`${tier}CornerLights.png`)),
    ]);

    const work = document.createElement('canvas');
    work.width = cardW; work.height = 512;
    const ctx = work.getContext('2d')!;

    const cx = (cardW - bgImg.naturalWidth) / 2;
    const cy = (512  - bgImg.naturalHeight) / 2;

    // Background
    ctx.drawImage(bgImg, cx, cy);

    // Shaded thumbnail
    if (item.imageName) {
      try {
        const thumb = await loadImg(thumbSrc());
        const thumbW = bgImg.naturalWidth - H_PAD * 2;
        const thumbH = thumb.naturalHeight - 80;
        ctx.drawImage(
          shadeCanvas(thumb, thumb.naturalWidth, thumb.naturalHeight, 0.3),
          cx + H_PAD, cy + bgImg.naturalHeight * 0.17, thumbW, Math.max(thumbH, 0)
        );
      } catch { /* no image */ }
    }

    // Title text
    ctx.fillStyle = color;
    ctx.textAlign = 'center';
    ctx.textBaseline = 'middle';
    ctx.font = '400 22px sans-serif';
    ctx.fillText(item.displayName || item.itemType, cardW * 0.5, cy + bgImg.naturalHeight * 0.75, bgImg.naturalWidth - H_PAD * 2);

    // Side lights
    const sideFlipped = flipImg(sideImg);
    const sideLightsY = cy + bgImg.naturalHeight * 0.21;
    ctx.drawImage(sideImg,      cardW * 0.93 - sideImg.naturalWidth, sideLightsY);
    ctx.drawImage(sideFlipped,  cx,                                   sideLightsY);

    // Backer (top-right)
    ctx.drawImage(backerImg, cx + bgImg.naturalWidth * 0.8, cy + bgImg.naturalHeight * 0.2);

    // Lower tab
    ctx.drawImage(lowerTabImg, cx + bgImg.naturalWidth * 0.09, cy + bgImg.naturalHeight - bottomImg.naturalHeight - (isRiven ? 16 : 8));

    // Top frame
    const topFrameY = cy + bgImg.naturalHeight * 0.14;
    if (topImg.naturalWidth > bgImg.naturalWidth) {
      ctx.drawImage(topImg, cx - (topImg.naturalWidth - bgImg.naturalWidth - H_PAD * 6) / 2, topFrameY);
    } else {
      ctx.drawImage(topImg, cx, topFrameY);
    }

    // Bottom frame
    const bottomCanvas = await makeBottomFrame(bottomImg, cornerImg, tier);
    const bottomY = cy + bgImg.naturalHeight * 0.65;
    if (bottomImg.naturalWidth > bgImg.naturalWidth) {
      ctx.drawImage(bottomCanvas, cx - (bottomImg.naturalWidth - bgImg.naturalWidth - H_PAD * 5) / 2, bottomY);
    } else {
      ctx.drawImage(bottomCanvas, cx, bottomY);
    }

    // Crop to 256×380
    const out = document.createElement('canvas');
    out.width = cardW; out.height = 380;
    const outCtx = out.getContext('2d')!;
    outCtx.drawImage(work, (cardW - work.width) / 2, (380 - 512) / 2);
    drawCountBadge(outCtx, color, 6, 6, 16);

    fullDataUrl = out.toDataURL('image/png');
    fullPending = false;
  }

  // ── Hover tracking for fixed-position overlay ─────────────────────────────────

  let hovering = $state(false);
  let mouseX   = $state(0);
  let mouseY   = $state(0);

  function onMouseEnter(e: MouseEvent) {
    hovering = true;
    mouseX = e.clientX;
    mouseY = e.clientY;
    drawFull();
  }

  function onMouseMove(e: MouseEvent) {
    mouseX = e.clientX;
    mouseY = e.clientY;
  }

  onMount(() => { drawCollapsed(); });
</script>

<div
  class="mod-wrap"
  role="img"
  aria-label={item.displayName || item.itemType}
  onmouseenter={onMouseEnter}
  onmousemove={onMouseMove}
  onmouseleave={() => (hovering = false)}
>
  {#if dataUrl}
    <img src={dataUrl} alt={item.displayName} class="mod-card-img" />
  {:else}
    <div class="mod-card-placeholder"></div>
  {/if}
</div>

{#if hovering}
  {@const src = fullDataUrl || dataUrl}
  {@const isRiven = getTier() === 'Omega'}
  {@const w = isRiven ? 292 : 256}
  {@const h = fullDataUrl ? 380 : (isRiven ? 180 : 150)}
  {@const above = mouseY > window.innerHeight / 2}
  <div
    class="mod-hover"
    style="
      left: {Math.min(Math.max(mouseX - w / 2, 8), window.innerWidth - w - 8)}px;
      top:  {above ? mouseY - h - 12 : mouseY + 12}px;
      width: {w}px;
    "
  >
    {#if src}
      <img {src} alt={item.displayName} class="mod-hover-img" />
    {/if}
  </div>
{/if}

<style>
  .mod-wrap {
    position: relative;
    border-radius: 4px;
    cursor: default;
  }

  .mod-card-img {
    width: 100%;
    display: block;
    border-radius: 4px;
  }

  .mod-card-placeholder {
    width: 100%;
    aspect-ratio: 256 / 150;
    background: #0d0f18;
    border: 1px solid #2a2d3a;
    border-radius: 4px;
  }

  :global(.mod-hover) {
    position: fixed;
    z-index: 9999;
    pointer-events: none;
    border-radius: 4px;
    box-shadow: 0 16px 48px rgba(0, 0, 0, 0.9);
    animation: mod-pop 0.12s ease;
  }

  :global(.mod-hover-img) {
    width: 100%;
    display: block;
    border-radius: 4px;
  }

  @keyframes mod-pop {
    from { opacity: 0; transform: scale(0.92); }
    to   { opacity: 1; transform: scale(1); }
  }
</style>
