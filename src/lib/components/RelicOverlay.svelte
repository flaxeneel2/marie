<script lang="ts">
  interface ItemPriceResult {
    ocr_text: string;
    matched_name: string;
    slug: string;
    ducats: number | null;
    plat_min_sell: number | null;
  }

  let { items, loading, error } = $props<{
    items: ItemPriceResult[];
    loading: boolean;
    error: string;
  }>();

  // Proportional bounds from screenshot.rs (measured on 2560×1440 reference).
  const TEXT_X_START = 650  / 2560;
  const TEXT_X_END   = 1920 / 2560;
  const TEXT_Y_END   = 612  / 1440;
  const CARD_Y_OFFSET_PHYS = 300;
  const CARD_GAP_FRAC = 0.006;

  function cardStyle(i: number, n: number): string {
    const dpr   = window.devicePixelRatio || 1;
    const physW = window.innerWidth  * dpr;
    const physH = window.innerHeight * dpr;

    const slotPhysW = (TEXT_X_END - TEXT_X_START) * physW / n;
    const gapPhys   = CARD_GAP_FRAC * physW;

    const leftCss  = (TEXT_X_START * physW + i * slotPhysW + gapPhys) / dpr;
    const widthCss = (slotPhysW - 2 * gapPhys) / dpr;
    const topCss   = (TEXT_Y_END * physH + CARD_Y_OFFSET_PHYS) / dpr;

    return `left:${leftCss.toFixed(1)}px; width:${widthCss.toFixed(1)}px; top:${topCss.toFixed(1)}px`;
  }

  function bestValue(item: ItemPriceResult): 'plat' | 'ducats' | 'unknown' {
    if (!item.plat_min_sell && !item.ducats) return 'unknown';
    if (item.ducats && item.plat_min_sell && item.ducats >= item.plat_min_sell * 15) return 'ducats';
    return 'plat';
  }
</script>

<div class="overlay">
  {#if loading}
    <div class="status-message">
      <span class="scanner-line"></span>
      <span class="pulse-dot"></span>
      Scanning reward data...
    </div>
  {:else if error}
    <div class="status-message error">
      <span class="err-indicator">!</span>
      {error}
    </div>
  {:else}
    {#each items as item, i}
      {@const best = bestValue(item)}
      <div
        class="card"
        class:highlight-plat={best === 'plat' && (item.plat_min_sell ?? 0) >= 30}
        class:highlight-ducats={best === 'ducats'}
        style={cardStyle(i, items.length)}
      >
        <div class="card-border-glare"></div>
        <div class="item-name" title={item.ocr_text}>
          {item.matched_name || item.ocr_text || `Item ${i + 1}`}
        </div>
        <div class="prices">
          <div class="price-badge plat" class:dim={item.plat_min_sell === null}>
            <span class="currency-symbol">⬡</span>
            <span class="val">{item.plat_min_sell != null ? `${item.plat_min_sell}p` : '—'}</span>
          </div>
          <div class="price-badge ducats" class:dim={item.ducats === null}>
            <span class="currency-symbol">◈</span>
            <span class="val">{item.ducats != null ? item.ducats : '—'}</span>
          </div>
        </div>
      </div>
    {/each}
  {/if}
</div>

<style>
  .overlay {
    position: fixed;
    inset: 0;
    font-family: 'Inter', 'Segoe UI', Arial, sans-serif;
    pointer-events: none;
  }

  .status-message {
    position: absolute;
    bottom: 4%;
    right: 4%;
    background: rgba(8, 10, 15, 0.9);
    color: #38bdf8;
    padding: 10px 20px;
    border-radius: 6px;
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.05em;
    border: 1px solid rgba(56, 189, 248, 0.3);
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.6), 0 0 10px rgba(56, 189, 248, 0.1);
    white-space: nowrap;
    display: flex;
    align-items: center;
    gap: 10px;
    overflow: hidden;
  }

  .scanner-line {
    position: absolute;
    top: 0;
    left: 0;
    right: 0;
    height: 1px;
    background: rgba(56, 189, 248, 0.6);
    box-shadow: 0 0 8px #38bdf8;
    animation: scan 1.5s linear infinite;
  }

  @keyframes scan {
    0% { transform: translateY(0); }
    100% { transform: translateY(38px); }
  }

  .pulse-dot {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #38bdf8;
    box-shadow: 0 0 8px #38bdf8;
    animation: flash 1.5s infinite;
  }

  @keyframes flash {
    0%, 100% { opacity: 0.5; }
    50% { opacity: 1; }
  }

  .status-message.error {
    color: #ef4444;
    border-color: rgba(239, 68, 68, 0.4);
    box-shadow: 0 4px 20px rgba(0, 0, 0, 0.6), 0 0 10px rgba(239, 68, 68, 0.1);
  }

  .err-indicator {
    background: rgba(239, 68, 68, 0.15);
    border: 1px solid #ef4444;
    width: 16px;
    height: 16px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 10px;
    font-weight: 800;
  }

  /* Card Design */
  .card {
    position: absolute;
    box-sizing: border-box;
    background: rgba(9, 11, 16, 0.92);
    border: 1px solid rgba(56, 189, 248, 0.15);
    border-radius: 8px;
    padding: 12px 14px;
    display: flex;
    flex-direction: column;
    gap: 8px;
    pointer-events: auto;
    user-select: text;
    cursor: default;
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.8);
    transition: all 0.2s cubic-bezier(0.16, 1, 0.3, 1);
    overflow: hidden;
    animation: card-appear 0.25s cubic-bezier(0.16, 1, 0.3, 1) forwards;
  }

  @keyframes card-appear {
    from {
      opacity: 0;
      transform: translateY(10px) scale(0.98);
    }
    to {
      opacity: 1;
      transform: translateY(0) scale(1);
    }
  }

  /* High Plat value theme */
  .card.highlight-plat {
    border-color: rgba(229, 169, 59, 0.5);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.8), 0 0 15px rgba(229, 169, 59, 0.25);
    background: linear-gradient(135deg, rgba(9, 11, 16, 0.95) 0%, rgba(229, 169, 59, 0.04) 100%);
  }

  /* High Ducat value theme */
  .card.highlight-ducats {
    border-color: rgba(56, 189, 248, 0.4);
    box-shadow: 0 8px 32px rgba(0, 0, 0, 0.8), 0 0 15px rgba(56, 189, 248, 0.2);
  }

  .card-border-glare {
    position: absolute;
    top: 0;
    left: -50%;
    width: 200%;
    height: 100%;
    background: linear-gradient(
      90deg,
      rgba(255, 255, 255, 0) 0%,
      rgba(255, 255, 255, 0.03) 50%,
      rgba(255, 255, 255, 0) 100%
    );
    transform: rotate(25deg);
    pointer-events: none;
  }

  .item-name {
    font-size: 12px;
    font-weight: 700;
    color: #e2e8f0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    letter-spacing: 0.02em;
    border-bottom: 1px solid rgba(255, 255, 255, 0.05);
    padding-bottom: 6px;
  }

  .highlight-plat .item-name {
    color: #e5a93b;
    border-color: rgba(229, 169, 59, 0.15);
  }

  .prices {
    display: flex;
    gap: 8px;
  }

  .price-badge {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
    gap: 4px;
    background: rgba(255, 255, 255, 0.03);
    border-radius: 4px;
    padding: 4px 8px;
    font-size: 13px;
    font-weight: 800;
    font-family: 'Consolas', 'Courier New', monospace;
  }

  .price-badge.dim {
    opacity: 0.3;
  }

  .price-badge.plat {
    color: #e5a93b;
    background: rgba(229, 169, 59, 0.05);
    border: 1px solid rgba(229, 169, 59, 0.1);
  }

  .price-badge.ducats {
    color: #38bdf8;
    background: rgba(56, 189, 248, 0.05);
    border: 1px solid rgba(56, 189, 248, 0.1);
  }

  .currency-symbol {
    font-size: 12px;
  }
</style>
