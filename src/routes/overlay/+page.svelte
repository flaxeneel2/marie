<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';

  // ── Relic types ──────────────────────────────────────────────────────────────

  interface ItemPriceResult {
    ocr_text: string;
    matched_name: string;
    slug: string;
    ducats: number | null;
    plat_min_sell: number | null;
  }

  // ── Riven types ──────────────────────────────────────────────────────────────

  interface ParsedStat {
    slug: string;
    display_name: string;
    value: number;
    is_negative: boolean;
    effective_negative: boolean;
    weight: number;
    weight_label: string;
  }

  interface RivenRollGrade {
    weapon_name: string;
    weapon_slug: string;
    disposition: number;
    weapon_tier: string;
    stats: ParsedStat[];
    roll_count: number;
    build_score: number;
    build_grade: string;
    market_score: number;
    market_grade: string;
  }

  interface RivenRerollResult {
    old: RivenRollGrade;
    new: RivenRollGrade;
  }

  // ── Relic state ──────────────────────────────────────────────────────────────

  let items = $state<ItemPriceResult[]>([]);
  let loading = $state(false);
  let error = $state('');
  let visible = $state(false);
  let playerCount = $state(4);
  let relicDismissTimer: ReturnType<typeof setTimeout> | null = null;

  // ── Riven state ──────────────────────────────────────────────────────────────

  let rivenVisible = $state(false);
  let rivenLoading = $state(false);
  let rivenRolling = $state(false); // Kuva confirmed, waiting for server result
  let rivenError = $state('');
  let rivenData = $state<RivenRerollResult | null>(null);
  let rivenDismissTimer: ReturnType<typeof setTimeout> | null = null;

  onMount(() => {
    const cleanups: Array<() => void> = [];

    Promise.all([
      // Relic listeners
      listen<number>('relic-trigger', async (event) => {
        playerCount = event.payload;
        await showOverlay();
      }),
      listen<ItemPriceResult[]>('relic-test-data', (event) => {
        showOverlayWithItems(event.payload);
      }),
      // Dismiss timer for test overlay is driven from Rust (Tokio) rather than setTimeout —
      // WebKitGTK throttles JS timers on windows with KeyboardMode::None (layer shell).
      listen('hide-overlay', () => hideOverlay()),

      // Riven listeners
      // Screen opened: capture current stats as baseline — no overlay shown yet.
      listen('riven-screen-open', async () => {
        rivenRolling = false;
        try { await invoke('capture_current_riven'); } catch { /* game not running, ignore */ }
      }),
      // Kuva confirmed — roll is in flight on the server.
      listen('riven-rolling', () => {
        rivenRolling = true;
        rivenVisible = true;
        rivenData = null;
        rivenError = '';
        rivenLoading = false;
      }),
      listen('riven-reroll', async () => {
        rivenRolling = false;
        await showRivenOverlay();
      }),
      listen<RivenRerollResult>('riven-test-data', (event) => {
        showRivenWithData(event.payload);
      }),
      listen('hide-riven-overlay', () => hideRivenOverlay()),
    ]).then(fns => cleanups.push(...fns));

    return () => cleanups.forEach(fn => fn());
  });

  // ── Relic overlay logic ──────────────────────────────────────────────────────

  function showOverlayWithItems(data: ItemPriceResult[]) {
    if (relicDismissTimer) clearTimeout(relicDismissTimer);
    error = '';
    loading = false;
    items = data;
    visible = true;
  }

  async function showOverlay() {
    if (relicDismissTimer) clearTimeout(relicDismissTimer);
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

    relicDismissTimer = setTimeout(hideOverlay, 30_000);
  }

  function hideOverlay() {
    visible = false;
  }

  // ── Riven overlay logic ──────────────────────────────────────────────────────

  function showRivenWithData(data: RivenRerollResult) {
    if (rivenDismissTimer) clearTimeout(rivenDismissTimer);
    rivenError = '';
    rivenLoading = false;
    rivenData = data;
    rivenVisible = true;
    rivenDismissTimer = setTimeout(hideRivenOverlay, 30_000);
  }

  async function showRivenOverlay() {
    if (rivenDismissTimer) clearTimeout(rivenDismissTimer);
    rivenError = '';
    rivenData = null;
    rivenLoading = true;
    rivenVisible = true;

    try {
      rivenData = await invoke<RivenRerollResult>('grade_riven_reroll');
    } catch (e) {
      rivenError = String(e);
    } finally {
      rivenLoading = false;
    }

    rivenDismissTimer = setTimeout(hideRivenOverlay, 30_000);
  }

  function hideRivenOverlay() {
    rivenVisible = false;
  }

  // ── Riven display helpers ────────────────────────────────────────────────────

  function dispStars(disp: number): number {
    if (disp >= 1.4)  return 5;
    if (disp >= 1.15) return 4;
    if (disp >= 0.9)  return 3;
    if (disp >= 0.65) return 2;
    return 1;
  }

  function gradeColor(grade: string): string {
    return ({ S: '#c9a227', A: '#7ec8a0', B: '#5bc0be', C: '#e0e0e0', D: '#888', F: '#ff6b6b' })[grade] ?? '#888';
  }

  function weightColor(label: string): string {
    return ({ God: '#c9a227', Great: '#7ec8a0', Good: '#5bc0be', Filler: '#666', Dump: '#444' })[label] ?? '#666';
  }

  function statImproved(oldStat: ParsedStat | undefined, newStat: ParsedStat): boolean {
    if (!oldStat) return false;
    return newStat.weight > oldStat.weight;
  }

  function findMatchingStat(slug: string, stats: ParsedStat[]): ParsedStat | undefined {
    return stats.find(s => s.slug === slug);
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

{#if rivenVisible}
  <div class="riven-overlay">
    {#if rivenRolling}
      <div class="riven-status riven-rolling">Rolling…</div>
    {:else if rivenLoading}
      <div class="riven-status">Grading riven…</div>
    {:else if rivenError}
      <div class="riven-status riven-error">{rivenError}</div>
    {:else if rivenData}
      <div class="riven-compare">
        {#each [{ roll: rivenData.old, label: 'CURRENT' }, { roll: rivenData.new, label: 'NEW' }] as side, si}
          <div class="riven-panel" class:new-panel={si === 1}>
            <div class="panel-label">{side.label}</div>

            <div class="weapon-row">
              <span class="weapon-name">{side.roll.weapon_name || '?'}</span>
              <span class="tier-badge" style="color:{gradeColor(side.roll.weapon_tier)}">
                T{side.roll.weapon_tier}
              </span>
            </div>

            <div class="disp-row">
              {#each { length: 5 } as _, di}
                <span class="disp-dot" class:disp-filled={di < dispStars(side.roll.disposition)}>●</span>
              {/each}
              <span class="disp-label">{side.roll.disposition.toFixed(2)}</span>
            </div>

            <div class="stats-list">
              {#each side.roll.stats as stat}
                {@const isBad = stat.is_negative || stat.effective_negative}
                {@const matchInOther = si === 1 ? findMatchingStat(stat.slug, rivenData!.old.stats) : undefined}
                {@const improved = si === 1 && matchInOther ? stat.weight > matchInOther.weight : false}
                <div class="stat-row" class:stat-bad={isBad} class:stat-improved={improved}>
                  {#if !isBad}
                    <span class="weight-dot" style="color:{weightColor(stat.weight_label)}" title={stat.weight_label}>◆</span>
                  {:else}
                    <span class="weight-dot neg-dot">◆</span>
                  {/if}
                  <span class="stat-sign" class:neg-sign={isBad}>{isBad ? '−' : '+'}</span>
                  <span class="stat-val">{stat.value.toFixed(1)}%</span>
                  <span class="stat-name">{stat.display_name}</span>
                </div>
              {/each}
            </div>

            <div class="roll-count">Rolls: {side.roll.roll_count}</div>

            <div class="grade-row">
              <div class="grade-block">
                <span class="grade-label">Build</span>
                <span class="grade-letter" style="color:{gradeColor(side.roll.build_grade)}">{side.roll.build_grade}</span>
              </div>
              <div class="grade-block">
                <span class="grade-label">Market</span>
                <span class="grade-letter" style="color:{gradeColor(side.roll.market_grade)}">{side.roll.market_grade}</span>
              </div>
            </div>
          </div>

          {#if si === 0}
            <div class="compare-arrow">→</div>
          {/if}
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

  /* ── Riven overlay ── */

  .riven-overlay {
    position: fixed;
    top: 18px;
    left: 50%;
    transform: translateX(-50%);
    z-index: 10;
    pointer-events: none;
  }

  .riven-status {
    background: rgba(10, 12, 20, 0.88);
    color: #e0e0e0;
    padding: 8px 18px;
    border-radius: 8px;
    font-family: 'Segoe UI', Arial, sans-serif;
    font-size: 14px;
    border: 1px solid rgba(255,255,255,0.1);
  }

  .riven-error   { color: #ff6b6b; }
  .riven-rolling { color: #c9a227; }

  .riven-compare {
    display: flex;
    align-items: flex-start;
    gap: 0;
  }

  .riven-panel {
    background: rgba(10, 12, 20, 0.92);
    border: 1px solid rgba(255,255,255,0.12);
    border-radius: 8px;
    padding: 12px 16px;
    min-width: 220px;
    max-width: 260px;
    font-family: 'Segoe UI', Arial, sans-serif;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .new-panel {
    border-color: rgba(201, 162, 39, 0.35);
  }

  .compare-arrow {
    align-self: center;
    color: #555;
    font-size: 20px;
    padding: 0 8px;
    user-select: none;
  }

  .panel-label {
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.12em;
    color: #666;
    text-transform: uppercase;
  }

  .weapon-row {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }

  .weapon-name {
    font-size: 14px;
    font-weight: 700;
    color: #e8e8e8;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .tier-badge {
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.05em;
    flex-shrink: 0;
  }

  .disp-row {
    display: flex;
    align-items: center;
    gap: 2px;
  }

  .disp-dot {
    font-size: 10px;
    color: #333;
  }

  .disp-dot.disp-filled {
    color: #c9a227;
  }

  .disp-label {
    font-size: 10px;
    color: #555;
    margin-left: 4px;
  }

  .stats-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .stat-row {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
  }

  .stat-row.stat-bad {
    opacity: 0.65;
  }

  .stat-row.stat-improved {
    background: rgba(126, 200, 160, 0.08);
    border-radius: 3px;
    padding: 1px 3px;
    margin: -1px -3px;
  }

  .weight-dot {
    font-size: 8px;
    flex-shrink: 0;
    width: 10px;
    text-align: center;
  }

  .neg-dot { color: #333; }

  .stat-sign {
    font-weight: 700;
    color: #7ec8a0;
    width: 10px;
    text-align: center;
    flex-shrink: 0;
  }

  .neg-sign { color: #ff6b6b; }

  .stat-val {
    color: #ccc;
    min-width: 42px;
    text-align: right;
    flex-shrink: 0;
  }

  .stat-name {
    color: #aaa;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .roll-count {
    font-size: 11px;
    color: #555;
  }

  .grade-row {
    display: flex;
    gap: 12px;
    padding-top: 4px;
    border-top: 1px solid rgba(255,255,255,0.06);
  }

  .grade-block {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 2px;
  }

  .grade-label {
    font-size: 9px;
    font-weight: 600;
    letter-spacing: 0.1em;
    color: #555;
    text-transform: uppercase;
  }

  .grade-letter {
    font-size: 22px;
    font-weight: 900;
    line-height: 1;
  }
</style>
