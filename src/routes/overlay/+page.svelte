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
    is_multiplier: boolean;
    weight: number;
    weight_label: string;
    roll_quality: number | null;
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

  // ── Interactive mode ─────────────────────────────────────────────────────────

  let overlayInteractive = $state(false);

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
  let rivenCurrentGrade = $state<RivenRollGrade | null>(null); // Single-panel view on screen open
  let rivenDismissTimer: ReturnType<typeof setTimeout> | null = null;

  onMount(() => {
    const cleanups: Array<() => void> = [];

    Promise.all([
      // Interactive mode listener
      listen<boolean>('overlay-interactive-changed', (event) => {
        overlayInteractive = event.payload;
      }),

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
      // Screen opened: capture current stats and show single-panel overlay.
      listen('riven-screen-open', async () => {
        rivenRolling = false;
        rivenData = null;
        rivenCurrentGrade = null;
        try {
          rivenCurrentGrade = await invoke<RivenRollGrade>('capture_current_riven');
          rivenVisible = true;
          if (rivenDismissTimer) clearTimeout(rivenDismissTimer);
          rivenDismissTimer = setTimeout(hideRivenOverlay, 30_000);
        } catch { /* game not running, ignore */ }
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
    if (overlayInteractive) invoke('disable_overlay_interaction');
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
    rivenCurrentGrade = null;
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
    rivenCurrentGrade = null;
    if (overlayInteractive) invoke('disable_overlay_interaction');
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

  function weightLetter(label: string): string {
    return ({ God: 'S', Great: 'A', Good: 'B', Filler: 'C', Dump: 'D' })[label] ?? '?';
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

{#if overlayInteractive}
  <div class="interactive-badge">
    <span class="interactive-label">INTERACTIVE</span>
    <button class="interactive-close" onclick={() => invoke('disable_overlay_interaction')}>×</button>
  </div>
{/if}

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

{#snippet rivenPanel(grade: RivenRollGrade, label: string, isNew: boolean, oldGrade: RivenRollGrade | null)}
  <div class="riven-panel" class:new-panel={isNew}>
    <div class="panel-label">{label}</div>

    <div class="weapon-header">
      <span class="weapon-name">{grade.weapon_name || '?'}</span>
      <span class="tier-badge" style="color:{gradeColor(grade.weapon_tier)}">T{grade.weapon_tier}</span>
    </div>

    <div class="disp-row">
      {#each { length: 5 } as _, di}
        <span class="disp-dot" class:disp-filled={di < dispStars(grade.disposition)}>●</span>
      {/each}
      <span class="disp-label">Disp {grade.disposition.toFixed(2)}</span>
    </div>

    <div class="stats-section">
      {#each grade.stats as stat}
        {@const isBad = stat.is_negative || stat.effective_negative}
        {@const oldStat = oldGrade ? findMatchingStat(stat.slug, oldGrade.stats) : undefined}
        {@const improved = oldStat != null && stat.weight > oldStat.weight}
        <div class="stat-entry" class:stat-bad={isBad} class:stat-improved={improved}>
          <div class="stat-main-row">
            <span class="weight-pip" style="color:{isBad ? '#3a3a3a' : weightColor(stat.weight_label)}">◆</span>
            <span class="stat-sign" class:neg-sign={isBad}>{isBad ? '−' : stat.is_multiplier ? 'x' : '+'}</span>
            <span class="stat-val">{stat.is_multiplier ? stat.value.toFixed(2) : stat.value.toFixed(1) + '%'}</span>
            <span class="stat-name">{stat.display_name}</span>
            {#if !isBad}
              <span class="weight-tier" style="color:{weightColor(stat.weight_label)}">{weightLetter(stat.weight_label)}</span>
            {:else}
              <span class="weight-tier neg-tier">−</span>
            {/if}
          </div>
          {#if !isBad && stat.roll_quality != null}
            <div class="stat-bar-track">
              <div
                class="stat-bar-fill"
                style="width:{Math.round(stat.roll_quality * 100)}%; background:{weightColor(stat.weight_label)}"
              ></div>
            </div>
          {:else if !isBad}
            <div class="stat-bar-track stat-bar-unknown"></div>
          {/if}
        </div>
      {/each}
    </div>

    <div class="scores-section">
      {#each [
        { label: 'Build',  score: grade.build_score,  g: grade.build_grade  },
        { label: 'Market', score: grade.market_score, g: grade.market_grade },
      ] as row}
        <div class="score-row">
          <span class="score-label">{row.label}</span>
          <div class="score-bar-track">
            <div class="score-bar-fill" style="width:{Math.round(row.score * 100)}%; background:{gradeColor(row.g)}"></div>
          </div>
          <span class="score-pct">{Math.round(row.score * 100)}%</span>
          <span class="score-grade-letter" style="color:{gradeColor(row.g)}">{row.g}</span>
        </div>
      {/each}
    </div>

    <div class="roll-footer">
      <span class="roll-count-text">{grade.roll_count} roll{grade.roll_count === 1 ? '' : 's'}</span>
      {#if grade.roll_count > 5}
        <span class="roll-penalty">−{Math.min(15, grade.roll_count - 5)}% mkt</span>
      {/if}
    </div>
  </div>
{/snippet}

{#if rivenVisible}
  <div class="riven-overlay">
    {#if rivenRolling}
      <div class="riven-status riven-rolling">Rolling riven…</div>
    {:else if rivenLoading}
      <div class="riven-status">Grading riven…</div>
    {:else if rivenError}
      <div class="riven-status riven-error">{rivenError}</div>
    {:else if rivenCurrentGrade}
      <div class="riven-side riven-side-left">
        {@render rivenPanel(rivenCurrentGrade, 'CURRENT', false, null)}
      </div>
    {:else if rivenData}
      <div class="riven-side riven-side-left">
        {@render rivenPanel(rivenData.old, 'CURRENT', false, null)}
      </div>
      <div class="riven-side riven-side-right">
        {@render rivenPanel(rivenData.new, 'NEW', true, rivenData.old)}
      </div>
    {/if}
  </div>
{/if}

<style>
  .interactive-badge {
    position: fixed;
    top: 16px;
    right: 16px;
    z-index: 9999;
    display: flex;
    align-items: center;
    gap: 8px;
    background: rgba(15, 17, 23, 0.92);
    border: 1px solid #c9a227;
    border-radius: 20px;
    padding: 6px 12px 6px 14px;
    pointer-events: auto;
    box-shadow: 0 2px 12px rgba(0, 0, 0, 0.6);
  }

  .interactive-label {
    font-family: 'Segoe UI', Arial, sans-serif;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.1em;
    color: #c9a227;
    text-transform: uppercase;
  }

  .interactive-close {
    background: none;
    border: 1px solid #3a3d4a;
    border-radius: 50%;
    color: #888;
    cursor: pointer;
    font-size: 14px;
    line-height: 1;
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    padding: 0;
    transition: border-color 0.15s, color 0.15s;
  }

  .interactive-close:hover {
    border-color: #c9a227;
    color: #c9a227;
  }

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
    pointer-events: none; /* transparent container; cards opt back in */
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
    pointer-events: auto;
    user-select: text;
    cursor: default;
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
    inset: 0;
    z-index: 10;
    pointer-events: none; /* transparent container; children opt back in below */
  }

  /* Allow content panels to receive pointer events when GTK lets them through. */
  .riven-side {
    pointer-events: auto;
  }

  .riven-panel {
    user-select: text;
    cursor: default;
  }

  .riven-status {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
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

  .riven-side {
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
  }

  .riven-side-left  { left: 20px; }
  .riven-side-right { right: 20px; }

  .riven-panel {
    background: rgba(10, 12, 20, 0.94);
    border: 1px solid rgba(255,255,255,0.12);
    border-radius: 10px;
    padding: 14px 16px;
    width: 268px;
    font-family: 'Segoe UI', Arial, sans-serif;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .new-panel {
    border-color: rgba(201, 162, 39, 0.4);
    box-shadow: 0 0 14px rgba(201, 162, 39, 0.07);
  }

  .panel-label {
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.14em;
    color: #484848;
    text-transform: uppercase;
  }

  .weapon-header {
    display: flex;
    align-items: baseline;
    gap: 8px;
  }

  .weapon-name {
    font-size: 15px;
    font-weight: 700;
    color: #eee;
    flex: 1;
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
    color: #252525;
  }

  .disp-dot.disp-filled {
    color: #c9a227;
  }

  .disp-label {
    font-size: 10px;
    color: #4a4a4a;
    margin-left: 5px;
  }

  /* ── Stats ── */

  .stats-section {
    display: flex;
    flex-direction: column;
    gap: 7px;
    border-top: 1px solid rgba(255,255,255,0.06);
    border-bottom: 1px solid rgba(255,255,255,0.06);
    padding: 8px 0;
  }

  .stat-entry {
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .stat-entry.stat-bad { opacity: 0.55; }

  .stat-entry.stat-improved {
    background: rgba(126, 200, 160, 0.07);
    border-radius: 4px;
    padding: 3px 5px;
    margin: -3px -5px;
  }

  .stat-main-row {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 12px;
  }

  .weight-pip {
    font-size: 8px;
    flex-shrink: 0;
    width: 10px;
    text-align: center;
  }

  .stat-sign {
    font-weight: 700;
    color: #7ec8a0;
    width: 10px;
    text-align: center;
    flex-shrink: 0;
  }

  .neg-sign { color: #ff6b6b; }

  .stat-val {
    color: #ddd;
    min-width: 46px;
    text-align: right;
    flex-shrink: 0;
    font-weight: 600;
  }

  .stat-name {
    color: #999;
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .weight-tier {
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.07em;
    flex-shrink: 0;
    text-transform: uppercase;
  }

  .neg-tier { color: #383838; }

  .stat-bar-track {
    height: 3px;
    background: rgba(255,255,255,0.05);
    border-radius: 2px;
    overflow: hidden;
    margin-left: 15px;
  }

  .stat-bar-fill {
    height: 100%;
    border-radius: 2px;
    opacity: 0.7;
    transition: width 0.25s ease;
  }

  .stat-bar-unknown {
    background: repeating-linear-gradient(
      90deg,
      rgba(255,255,255,0.04) 0px,
      rgba(255,255,255,0.04) 4px,
      transparent 4px,
      transparent 8px
    );
  }

  /* ── Scores ── */

  .scores-section {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .score-row {
    display: flex;
    align-items: center;
    gap: 7px;
  }

  .score-label {
    font-size: 9px;
    color: #484848;
    font-weight: 700;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    width: 38px;
    flex-shrink: 0;
  }

  .score-bar-track {
    flex: 1;
    height: 5px;
    background: rgba(255,255,255,0.05);
    border-radius: 3px;
    overflow: hidden;
  }

  .score-bar-fill {
    height: 100%;
    border-radius: 3px;
    opacity: 0.75;
    transition: width 0.25s ease;
  }

  .score-pct {
    font-size: 11px;
    color: #666;
    width: 32px;
    text-align: right;
    flex-shrink: 0;
  }

  .score-grade-letter {
    font-size: 18px;
    font-weight: 900;
    width: 14px;
    flex-shrink: 0;
    line-height: 1;
  }

  /* ── Footer ── */

  .roll-footer {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .roll-count-text {
    font-size: 10px;
    color: #404040;
  }

  .roll-penalty {
    font-size: 10px;
    color: #ff6b6b;
    opacity: 0.65;
  }
</style>
