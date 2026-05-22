<script lang="ts">
  import { onMount, tick } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';
  import InteractiveBadge from '$lib/components/InteractiveBadge.svelte';
  import RelicOverlay from '$lib/components/RelicOverlay.svelte';
  import RivenOverlay from '$lib/components/RivenOverlay.svelte';

  // ── Types ────────────────────────────────────────────────────────────────────
  interface ItemPriceResult {
    ocr_text: string;
    matched_name: string;
    slug: string;
    ducats: number | null;
    plat_min_sell: number | null;
  }

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

  // ── Interactivity & Pointer shape states ──────────────────────────────────────
  let overlayInteractive = $state(false);

  // ── Relic scan states ─────────────────────────────────────────────────────────
  let items = $state<ItemPriceResult[]>([]);
  let loading = $state(false);
  let error = $state('');
  let visible = $state(false);
  let playerCount = $state(4);
  let relicDismissTimer: ReturnType<typeof setTimeout> | null = null;

  // ── Riven scanner states ──────────────────────────────────────────────────────
  let rivenVisible = $state(false);
  let rivenLoading = $state(false);
  let rivenRolling = $state(false);
  let rivenError = $state('');
  let rivenData = $state<RivenRerollResult | null>(null);
  let rivenCurrentGrade = $state<RivenRollGrade | null>(null);
  let rivenDismissTimer: ReturnType<typeof setTimeout> | null = null;

  // ── Wayland Pointer Interaction Rects Sync ────────────────────────────────────
  async function updateInteractiveRegion() {
    await tick(); // Allow Svelte's DOM updates to complete before querying boundaries
    const scale = window.devicePixelRatio || 1;
    const rects: { x: number; y: number; width: number; height: number }[] = [];

    // Gather bounding boxes of all clickable/selectable elements on screen
    for (const sel of ['.card', '.riven-panel', '.interactive-badge']) {
      for (const el of document.querySelectorAll(sel)) {
        const r = el.getBoundingClientRect();
        if (r.width > 0 && r.height > 0) {
          rects.push({
            x:      Math.floor(r.left   * scale),
            y:      Math.floor(r.top    * scale),
            width:  Math.ceil(r.width   * scale),
            height: Math.ceil(r.height  * scale),
          });
        }
      }
    }
    await invoke('set_interactive_region', { rects });
  }

  // Automatically recalculate Wayland cursor regions whenever visible UI changes
  $effect(() => {
    // Declare dependencies to trigger effect reactively
    const _1 = overlayInteractive;
    const _2 = visible;
    const _3 = rivenVisible;
    const _4 = items;
    const _5 = rivenCurrentGrade;
    const _6 = rivenData;
    const _7 = loading;
    const _8 = rivenLoading;
    const _9 = rivenRolling;

    if (overlayInteractive) {
      updateInteractiveRegion();
    }
  });

  // ── Relic scan handlers ───────────────────────────────────────────────────────
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

  // ── Riven grading handlers ────────────────────────────────────────────────────
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

  // ── Lifecycle & Event Listeners ───────────────────────────────────────────────
  onMount(() => {
    const cleanups: Array<() => void> = [];

    Promise.all([
      // Interactive mode toggling
      listen<boolean>('overlay-interactive-changed', async (event) => {
        overlayInteractive = event.payload;
        if (event.payload) await updateInteractiveRegion();
      }),

      // Relic trigger event handlers
      listen<number>('relic-trigger', async (event) => {
        playerCount = event.payload;
        await showOverlay();
      }),
      listen<ItemPriceResult[]>('relic-test-data', (event) => {
        showOverlayWithItems(event.payload);
      }),
      listen('hide-overlay', () => hideOverlay()),

      // Riven screen state change handlers
      listen('riven-screen-open', async () => {
        rivenRolling = false;
        rivenData = null;
        rivenCurrentGrade = null;
        try {
          rivenCurrentGrade = await invoke<RivenRollGrade>('capture_current_riven');
          rivenVisible = true;
          if (rivenDismissTimer) clearTimeout(rivenDismissTimer);
          rivenDismissTimer = setTimeout(hideRivenOverlay, 30_000);
        } catch { /* Suppress error if game not running or window captured failed */ }
      }),
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
</script>

{#if overlayInteractive}
  <InteractiveBadge onClose={() => invoke('disable_overlay_interaction')} />
{/if}

{#if visible}
  <RelicOverlay {items} {loading} {error} />
{/if}

<RivenOverlay
  {rivenVisible}
  {rivenRolling}
  {rivenLoading}
  {rivenError}
  {rivenCurrentGrade}
  {rivenData}
/>

<style>
  :global(html, body) {
    margin: 0;
    padding: 0;
    background: transparent;
    overflow: hidden;
    height: 100vh;
    width: 100vw;
  }
</style>
