<script lang="ts">
  import { onMount } from 'svelte';
  import { invoke } from '@tauri-apps/api/core';

  let logPath = $state('…');
  let cacheReady = $state(false);
  let triggerStatus = $state('');

  onMount(async () => {
    logPath = await invoke<string>('ee_log_path');
  });

  async function testTrigger() {
    triggerStatus = 'Trigger sent — check the overlay window.';
    await invoke('test_trigger');
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
      <dt>Overlay</dt>
      <dd>Transparent window – appears automatically when a relic reward screen is detected.</dd>
    </dl>
  </section>

  <section>
    <h2>Development</h2>
    <p>Send a test trigger to open the overlay and exercise the OCR + price pipeline:</p>
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
