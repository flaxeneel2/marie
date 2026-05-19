// Browser-native mod card renderer, ported from @wfcd/mod-generator.
// Uses HTMLCanvasElement; no server or Node.js required.
import '@fontsource-variable/roboto';

// ── Public types ──────────────────────────────────────────────────────────────

export interface ModCardParams {
  name:        string;
  imageName:   string;
  rarity:      string;        // 'Common' | 'Uncommon' | 'Rare' | 'Legendary'
  polarity:    string;        // 'naramon' | 'madurai' | …
  fusionLimit: number;
  rank:        number;
  baseDrain:   number;
  compatName:  string;
  description: string;
  levelStats:  unknown[] | null;
  full:        boolean;
}

// ── Styling ───────────────────────────────────────────────────────────────────

const TITLE_FONT  = '400 22px "Roboto"';
const DESC_FONT   = '14px "Roboto"';
const COMPAT_FONT = '500 16px "Roboto"';
const H_PAD       = 8;

const RARITY_TIER: Record<string, string> = {
  common: 'Bronze', uncommon: 'Silver', rare: 'Gold', legendary: 'Legendary', riven: 'Omega',
};
const TIER_COLOR: Record<string, string> = {
  Bronze: '#CA9A87', Silver: '#FFFFFF', Gold: '#FAE7BE', Legendary: '#FFFFFF', Omega: '#AC83D5',
};

function getTier(rarity: string, name: string): string {
  if (name.includes('Archon')) return RARITY_TIER.rare;
  return RARITY_TIER[rarity.toLowerCase()] ?? RARITY_TIER.common;
}
function textColor(tier: string): string {
  return tier === 'Legendary' ? TIER_COLOR.Silver : (TIER_COLOR[tier] ?? TIER_COLOR.Bronze);
}

// ── Font ──────────────────────────────────────────────────────────────────────

let _font: Promise<void> | null = null;
function ensureFont(): Promise<void> {
  if (!_font) _font = document.fonts.load('22px "Roboto"').then(() => {});
  return _font;
}

// ── Image cache ───────────────────────────────────────────────────────────────

const _imgCache = new Map<string, HTMLImageElement>();

function loadImg(url: string): Promise<HTMLImageElement> {
  const hit = _imgCache.get(url);
  if (hit) return Promise.resolve(hit);
  return new Promise((resolve, reject) => {
    const img = new Image();
    img.onload = () => { _imgCache.set(url, img); resolve(img); };
    img.onerror = reject;
    img.src = url;
  });
}

async function tryLoadImg(url: string): Promise<HTMLImageElement | null> {
  try { return await loadImg(url); } catch { return null; }
}

function frameImg(name: string): Promise<HTMLImageElement> {
  return loadImg(`/img/mod-frames/${name}`);
}
function polarityImg(name: string): Promise<HTMLImageElement> {
  return loadImg(`/img/mod-frames/polarities/${name}.png`);
}

// ── Canvas helpers ────────────────────────────────────────────────────────────

type Drawable = HTMLImageElement | HTMLCanvasElement;
const W = (d: Drawable) => d instanceof HTMLCanvasElement ? d.width  : d.naturalWidth;
const H = (d: Drawable) => d instanceof HTMLCanvasElement ? d.height : d.naturalHeight;

function mkCanvas(w: number, h: number): [HTMLCanvasElement, CanvasRenderingContext2D] {
  const c = document.createElement('canvas');
  c.width = w; c.height = h;
  return [c, c.getContext('2d')!];
}

function canvasToBlob(c: HTMLCanvasElement): Promise<Blob> {
  return new Promise((ok, fail) => c.toBlob(b => b ? ok(b) : fail(new Error('toBlob failed')), 'image/png'));
}

function flipH(src: Drawable): HTMLCanvasElement {
  const [c, ctx] = mkCanvas(W(src), H(src));
  ctx.translate(W(src), 0); ctx.scale(-1, 1);
  ctx.drawImage(src, 0, 0);
  return c;
}

function pixelOp(src: Drawable, fn: (data: Uint8ClampedArray) => void): HTMLCanvasElement {
  const w = W(src), h = H(src);
  const [c, ctx] = mkCanvas(w, h);
  ctx.drawImage(src, 0, 0);
  const id = ctx.getImageData(0, 0, w, h);
  fn(id.data);
  ctx.putImageData(id, 0, 0);
  return c;
}

// ── Pixel effects ─────────────────────────────────────────────────────────────

function shadeSrc(src: Drawable): HTMLCanvasElement {
  return pixelOp(src, data => {
    for (let i = 0; i < data.length; i += 4) {
      if (!data[i + 3]) continue;
      data[i]     = Math.round(Math.max(0, data[i]     * 0.3));
      data[i + 1] = Math.round(Math.max(0, data[i + 1] * 0.3));
      data[i + 2] = Math.round(Math.max(0, data[i + 2] * 0.3));
    }
  });
}

function tintSrc(src: Drawable, tier: string): HTMLCanvasElement {
  const hex = textColor(tier);
  const tr = parseInt(hex.slice(1, 3), 16);
  const tg = parseInt(hex.slice(3, 5), 16);
  const tb = parseInt(hex.slice(5, 7), 16);
  return pixelOp(src, data => {
    for (let i = 0; i < data.length; i += 4) {
      if (!data[i + 3]) continue;
      const brightness = (data[i] + data[i + 1] + data[i + 2]) / 3 / 255;
      const ow = 0.2, pct = 0.8;
      data[i]     = Math.round(Math.min(255, data[i]     * ow + tr * brightness * pct));
      data[i + 1] = Math.round(Math.min(255, data[i + 1] * ow + tg * brightness * pct));
      data[i + 2] = Math.round(Math.min(255, data[i + 2] * ow + tb * brightness * pct));
    }
  });
}

// ── Text utils ────────────────────────────────────────────────────────────────

function wrapText(ctx: CanvasRenderingContext2D, text: string, maxW: number): string[] {
  const words = text.split(' ');
  let cur = '';
  const lines: string[] = [];
  for (const w of words) {
    const next = `${cur} ${w}`;
    if (ctx.measureText(next).width > maxW) { lines.push(cur); cur = w; }
    else cur = next;
  }
  lines.push(cur);
  return lines;
}

function calcTextHeight(ctx: CanvasRenderingContext2D, maxW: number, title: string | undefined, lines: string[] | undefined): number {
  const prev = ctx.font;
  let h = 0;
  if (title) {
    ctx.font = TITLE_FONT;
    const m = ctx.measureText(title);
    h = m.actualBoundingBoxAscent + m.actualBoundingBoxDescent;
  }
  ctx.font = DESC_FONT;
  lines?.forEach(line => wrapText(ctx, line, maxW).forEach(t => {
    const m = ctx.measureText(t);
    h += m.actualBoundingBoxAscent + m.actualBoundingBoxDescent;
  }));
  ctx.font = prev;
  return h + 15;
}

// eslint-disable-next-line @typescript-eslint/no-explicit-any
function getDesc(rank: number, description: string, levelStats: any[] | null): string | undefined {
  if (description?.length) return description;
  const entry = levelStats?.[rank];
  if (entry?.stats?.length) return (entry.stats as string[]).join(' \n');
  return undefined;
}

// ── Asset loaders ─────────────────────────────────────────────────────────────

async function getFrame(tier: string) {
  const [cornerLights, bottom, top, sideLights] = await Promise.all([
    frameImg(`${tier}CornerLights.png`), frameImg(`${tier}FrameBottom.png`),
    frameImg(`${tier}FrameTop.png`),     frameImg(`${tier}SideLight.png`),
  ]);
  return { cornerLights, bottom, top, sideLights };
}

async function getBackground(tier: string) {
  const isRiven = tier === 'Omega';
  const [background, backer, lowerTab] = await Promise.all([
    frameImg(isRiven ? 'LegendaryBackground.png' : `${tier}Background.png`),
    frameImg(isRiven ? 'RivenTopRightBacker.png' : `${tier}TopRightBacker.png`),
    frameImg(isRiven ? 'RivenLowerTab.png'       : `${tier}LowerTab.png`),
  ]);
  return { background, backer, lowerTab };
}

// ── Drawers ───────────────────────────────────────────────────────────────────

async function drawPolarity(tier: string, polarity: string): Promise<HTMLCanvasElement> {
  const img = await polarityImg(polarity);
  const [c, ctx] = mkCanvas(32, 32);
  ctx.drawImage(img, 0, 0);
  ctx.globalCompositeOperation = 'source-in';
  ctx.fillStyle = textColor(tier);
  ctx.fillRect(0, 0, 32, 32);
  return c;
}

async function drawBacker(
  backer: HTMLImageElement, tier: string, base: number, polarity: string, rank: number,
): Promise<HTMLCanvasElement> {
  const [c, ctx] = mkCanvas(backer.naturalWidth, backer.naturalHeight);
  ctx.drawImage(backer, 0, 0);
  ctx.font = 'bold 14px "Roboto"'; ctx.fillStyle = textColor(tier);
  const dy = c.height * 0.7;
  if (tier === 'Omega') { ctx.fillText('???', c.width * 0.4, dy); return c; }
  if (polarity === 'universal') ctx.fillText('??', c.width * 0.6, dy);
  else ctx.drawImage(await drawPolarity(tier, polarity), c.width * 0.6, c.height * 0.2, 16, 16);
  const drain = `${base < 0 ? '+' : ''}${Math.abs(base) + rank}`;
  ctx.fillText(drain, drain.length >= 2 ? c.width * 0.2 : c.width * 0.35, dy);
  return c;
}

function drawLowerTab(lowerTab: HTMLImageElement, tier: string, compatName: string): HTMLCanvasElement {
  const [c, ctx] = mkCanvas(lowerTab.naturalWidth, lowerTab.naturalHeight);
  ctx.drawImage(lowerTab, 0, 0);
  if (compatName) {
    ctx.font = COMPAT_FONT; ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
    ctx.fillStyle = textColor(tier);
    ctx.fillText(compatName.toUpperCase(), c.width * 0.5, c.height * 0.5);
  }
  return c;
}

async function drawBottom(
  bottom: HTMLImageElement, cornerLights: HTMLImageElement, tier: string, max: number, rank: number,
): Promise<HTMLCanvasElement> {
  const [slotEmpty, rankLine, slotActive] = await Promise.all([
    frameImg('RankSlotEmpty.png'), frameImg('RankCompleteLine.png'), frameImg('RankSlotActive.png'),
  ]);
  const [c, ctx] = mkCanvas(bottom.naturalWidth, bottom.naturalHeight);
  ctx.drawImage(bottom, 0, 0);
  const clLeft = flipH(cornerLights);
  const isRiven = tier === 'Omega', isLeg = tier === 'Legendary';
  if (isRiven)    { ctx.drawImage(cornerLights, c.width*.73, c.height*.27); ctx.drawImage(clLeft, c.width*.04,      c.height*.27); }
  else if (isLeg) { ctx.drawImage(cornerLights, c.width*.76, c.height*.32); ctx.drawImage(clLeft, -(c.width*.01), c.height*.32); }
  else            { ctx.drawImage(cornerLights, c.width*.76, c.height*.24); ctx.drawImage(clLeft, -(c.width*.01), c.height*.24); }
  const isRare  = isRiven || isLeg;
  const lineY   = isRare ? c.height * 0.84 : c.height * 0.74;
  const slotY   = isRare ? c.height * 0.82 : c.height * 0.72;
  const maxRank = Math.min(max, 10);
  if (rank === maxRank) ctx.drawImage(rankLine, 0, lineY);
  let slotX = maxRank <= 3 ? c.width * 0.425 : maxRank <= 5 ? c.width * 0.39 : c.width * 0.29;
  for (let i = 0; i < maxRank; i++) { ctx.drawImage(i < rank ? slotActive : slotEmpty, slotX, slotY); slotX += 11; }
  return c;
}

async function drawBg(
  background: HTMLImageElement, sideLights: HTMLImageElement,
  backer: HTMLImageElement, lowerTab: HTMLImageElement, bottomH: number,
  tier: string, name: string, description: string,
  // eslint-disable-next-line @typescript-eslint/no-explicit-any
  levelStats: any[] | null, compatName: string, baseDrain: number,
  polarity: string, rank: number, modImage: HTMLImageElement | null,
): Promise<HTMLCanvasElement> {
  const [c, ctx] = mkCanvas(background.naturalWidth, background.naturalHeight);
  ctx.drawImage(background, 0, 0);
  const maxW  = c.width * 0.85;
  const desc  = getDesc(rank, description, levelStats);
  const lines = desc?.split('\n');
  const modTextH = calcTextHeight(ctx, maxW, name, lines);
  const lineSpacing = 15;
  let pos = c.height * 0.17;

  if (modImage) {
    const thumbH = Math.max(0, modImage.naturalHeight - modTextH);
    ctx.drawImage(modImage, H_PAD, pos, c.width - H_PAD * 2, thumbH);
    pos += thumbH;
  }

  ctx.fillStyle = textColor(tier); ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
  ctx.font = TITLE_FONT;
  ctx.fillText(name, c.width * 0.5, pos + H_PAD * 2);
  pos += H_PAD + lineSpacing;

  if (desc?.length) {
    ctx.font = DESC_FONT;
    let y = pos + H_PAD * 2;
    lines?.forEach(line => wrapText(ctx, line, maxW).forEach(t => {
      ctx.fillText(t, c.width * 0.5, y, maxW); y += lineSpacing;
    }));
  }

  const slY = c.height * 0.21;
  ctx.drawImage(sideLights, c.width * 0.93, slY);
  ctx.drawImage(flipH(sideLights), 0, slY);
  ctx.drawImage(await drawBacker(backer, tier, baseDrain, polarity, rank), c.width * 0.8, c.height * 0.2);
  ctx.drawImage(drawLowerTab(lowerTab, tier, compatName), c.width * 0.09, c.height - bottomH - (tier === 'Omega' ? 16 : 8));
  return c;
}

// ── Public API ────────────────────────────────────────────────────────────────

export async function generateModCard(params: ModCardParams): Promise<Blob> {
  await ensureFont();
  const { name, imageName, rarity, polarity, fusionLimit, rank, baseDrain, compatName, description, levelStats, full } = params;
  const tier    = getTier(rarity, name);
  const isRiven = tier === 'Omega';
  const cw      = isRiven ? 292 : 256;

  const modImageUrl = imageName
    ? `/img/wf-assets/${imageName.replace(/\.(png|jpg|jpeg|webp)$/i, '.avif')}`
    : null;
  const modImage = modImageUrl ? await tryLoadImg(modImageUrl) : null;

  if (full) {
    const [bgAssets, frameAssets] = await Promise.all([getBackground(tier), getFrame(tier)]);
    const { background, backer, lowerTab } = bgAssets;
    const { cornerLights, bottom, top, sideLights } = frameAssets;
    const [c, ctx] = mkCanvas(cw, 512);
    const cx = (c.width - background.naturalWidth) / 2;
    const cy = (c.height - background.naturalHeight) / 2;
    ctx.drawImage(
      await drawBg(background, sideLights, backer, lowerTab, bottom.naturalHeight,
        tier, name, description, levelStats as never, compatName, baseDrain, polarity, rank, modImage),
      cx, cy,
    );
    const topY = background.naturalHeight * 0.14;
    if (top.naturalWidth > background.naturalWidth) {
      ctx.drawImage(top, -(top.naturalWidth - background.naturalWidth - H_PAD * 6) / 2, topY);
    } else ctx.drawImage(top, cx, topY);
    const bot  = await drawBottom(bottom, cornerLights, tier, fusionLimit, rank);
    const botY = background.naturalHeight * 0.65;
    if (bottom.naturalWidth > background.naturalWidth) {
      ctx.drawImage(bot, -(bottom.naturalWidth - background.naturalWidth - H_PAD * 5) / 2, botY);
    } else ctx.drawImage(bot, cx, botY);
    const [out, octx] = mkCanvas(cw, 380);
    octx.drawImage(c, (out.width - c.width) / 2, (out.height - c.height) / 2);
    return canvasToBlob(out);
  } else {
    const { cornerLights, bottom, top } = await getFrame(tier);
    const [c, ctx] = mkCanvas(cw, 256);
    if (modImage) {
      const thumbH = top.naturalHeight / 2 + bottom.naturalHeight / 2;
      ctx.drawImage(shadeSrc(modImage), top.naturalWidth * 0.03, top.naturalHeight * 0.3, c.width - H_PAD * 2, thumbH);
    }
    ctx.fillStyle = textColor(tier); ctx.textAlign = 'center'; ctx.textBaseline = 'middle';
    ctx.font = TITLE_FONT;
    ctx.fillText(name, c.width * 0.5, top.naturalHeight);
    ctx.drawImage(top, 0, 0);
    const bot  = await drawBottom(bottom, cornerLights, tier, fusionLimit, rank);
    const posY = top.naturalHeight * 0.5;
    if (bottom.naturalWidth > c.width) {
      ctx.drawImage(bot, -(bottom.naturalWidth - c.width - H_PAD * 5) / 2, posY);
    } else ctx.drawImage(bot, 0, posY);
    const [out, octx] = mkCanvas(cw, isRiven ? 180 : 150);
    octx.drawImage(c, 0, 0);
    return canvasToBlob(out);
  }
}
