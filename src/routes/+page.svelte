<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';

  let logPath = $state('…');
  let triggerStatus = $state('');
  let detectedCount = $state<number | null>(null);
  let overrideEnabled = $state(false);
  let playerCount = $state(4);

  onMount(async () => {
    logPath = await invoke<string>('ee_log_path');

    // Mirror the overlay's trigger listener so the readout stays in sync
    // with whatever the EE.log watcher detects.
    const unlisten = await listen<number>('relic-trigger', (event) => {
      detectedCount = event.payload;
    });
    return unlisten;
  });

  function effectiveCount(): number {
    return overrideEnabled ? playerCount : (detectedCount ?? 4);
  }

  async function testTrigger() {
    const count = effectiveCount();
    triggerStatus = `Trigger sent (${count} player${count === 1 ? '' : 's'}) — check the overlay window.`;
    await invoke('test_trigger', { playerCount: count });
    setTimeout(() => (triggerStatus = ''), 4000);
  }

  async function testOverlay() {
    triggerStatus = 'Overlay shown with fake data.';
    await invoke('show_test_overlay');
    setTimeout(() => (triggerStatus = ''), 4000);
  }
</script>

<main>
  <h1>marie</h1>
  <p class="subtitle">Warframe relic reward price overlay</p>

  <section>
    <h2>Status</h2>
    <dl>
      <dt>EE.log path</dt>
      <dd class="mono">{logPath}</dd>
      <dt>Current lobby</dt>
      <dd>
        {#if detectedCount !== null}
          {detectedCount} player{detectedCount === 1 ? '' : 's'}
        {:else}
          <span class="muted">— (no relic screen seen yet)</span>
        {/if}
      </dd>
      <dt>Overlay</dt>
      <dd>Transparent window – appears automatically when a relic reward screen is detected.</dd>
    </dl>
  </section>

  <section>
    <h2>Development</h2>
    <p>Send a test trigger to open the overlay and exercise the OCR + price pipeline:</p>
    <div class="player-count-row">
      <span class="player-count-label">Players</span>
      {#each [1, 2, 3, 4] as n}
        <button
          class="count-btn"
          class:active={overrideEnabled && playerCount === n}
          class:disabled={!overrideEnabled}
          onclick={() => { overrideEnabled = true; playerCount = n; }}
        >{n}</button>
      {/each}
      <button
        class="override-btn"
        class:active={overrideEnabled}
        onclick={() => (overrideEnabled = !overrideEnabled)}
        title={overrideEnabled ? 'Using manual player count' : 'Using auto-detected player count'}
      >{overrideEnabled ? 'Override ON' : 'Override OFF'}</button>
    </div>
    <button onclick={testTrigger}>Send test trigger</button>
    <button class="secondary" onclick={testOverlay}>Test overlay (fake data)</button>
    {#if triggerStatus}
      <p class="status">{triggerStatus}</p>
    {/if}
  </section>
</main>

<style>
  :root {
    font-family: 'Segoe UI', Arial, sans-serif;
    font-size: 15px;
    line-height: 1.6;
    color: #e0e0e0;
    background: #0f1117;
  }

  main {
    max-width: 640px;
    margin: 0 auto;
    padding: 32px 24px;
  }

  h1 {
    font-size: 2rem;
    font-weight: 700;
    margin: 0 0 4px;
    color: #c9a227;
    letter-spacing: 0.05em;
  }

  .subtitle {
    margin: 0 0 28px;
    color: #888;
    font-size: 13px;
  }

  h2 {
    font-size: 1rem;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: #aaa;
    margin: 0 0 10px;
  }

  section {
    background: #161922;
    border: 1px solid #2a2d3a;
    border-radius: 8px;
    padding: 16px 20px;
    margin-bottom: 16px;
  }

  dl {
    margin: 0;
    display: grid;
    grid-template-columns: max-content 1fr;
    gap: 6px 16px;
  }

  dt {
    color: #888;
    font-size: 13px;
    align-self: start;
    padding-top: 1px;
  }

  dd {
    margin: 0;
    color: #ccc;
    font-size: 13px;
  }

  .mono {
    font-family: 'Consolas', 'Courier New', monospace;
    font-size: 12px;
    word-break: break-all;
  }

  button {
    background: #c9a227;
    color: #0f1117;
    border: none;
    border-radius: 6px;
    padding: 8px 18px;
    font-size: 13px;
    font-weight: 700;
    cursor: pointer;
    transition: opacity 0.15s;
    margin-right: 8px;
  }

  button:hover {
    opacity: 0.85;
  }

  button.secondary {
    background: #2a2d3a;
    color: #ccc;
    border: 1px solid #3a3d4a;
  }

  .player-count-row {
    display: flex;
    align-items: center;
    gap: 6px;
    margin-bottom: 12px;
  }

  .player-count-label {
    font-size: 13px;
    color: #888;
    margin-right: 2px;
  }

  .count-btn {
    background: #2a2d3a;
    color: #ccc;
    border: 1px solid #3a3d4a;
    border-radius: 6px;
    padding: 5px 12px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    margin: 0;
    transition: background 0.15s, color 0.15s, border-color 0.15s;
  }

  .count-btn:hover {
    background: #353849;
    opacity: 1;
  }

  .count-btn.active {
    background: #c9a227;
    color: #0f1117;
    border-color: #c9a227;
  }

  .count-btn.disabled {
    opacity: 0.35;
    cursor: default;
  }

  .override-btn {
    margin-left: 6px;
    background: #2a2d3a;
    color: #888;
    border: 1px solid #3a3d4a;
    border-radius: 6px;
    padding: 5px 12px;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    margin-right: 0;
    transition: background 0.15s, color 0.15s, border-color 0.15s;
  }

  .override-btn:hover {
    background: #353849;
    opacity: 1;
  }

  .override-btn.active {
    background: #1e3a2a;
    color: #7ec8a0;
    border-color: #3a6a50;
  }

  .muted {
    color: #555;
  }

  .status {
    margin: 8px 0 0;
    font-size: 13px;
    color: #7ec8a0;
  }

  p {
    margin: 0 0 12px;
    font-size: 13px;
    color: #bbb;
  }
</style>
