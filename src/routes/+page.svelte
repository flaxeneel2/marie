<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';

  let logPath = $state('…');
  let triggerStatus = $state('');
  let detectedCount = $state<number | null>(null);
  let overrideEnabled = $state(false);
  let playerCount = $state(4);
  let overlayInteractive = $state(false);

  onMount(() => {
    invoke<string>('ee_log_path').then(p => (logPath = p));
    invoke<string>('get_interaction_shortcut').then(b => (currentBind = b));

    const cleanups: Array<() => void> = [];
    listen<number>('relic-trigger', (event) => {
      detectedCount = event.payload;
    }).then(fn => cleanups.push(fn));
    listen<boolean>('overlay-interactive-changed', (event) => {
      overlayInteractive = event.payload;
    }).then(fn => cleanups.push(fn));

    return () => cleanups.forEach(fn => fn());
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

  async function testRivenTrigger() {
    triggerStatus = 'Riven trigger sent — check the overlay window.';
    await invoke('test_riven_trigger');
    setTimeout(() => (triggerStatus = ''), 4000);
  }

  async function testRivenOverlay() {
    triggerStatus = 'Riven overlay shown with fake data.';
    await invoke('show_test_riven_overlay');
    setTimeout(() => (triggerStatus = ''), 4000);
  }

  // ── Keybind recorder ──────────────────────────────────────────────────────
  let currentBind = $state('');
  let recordingBind = $state(false);
  let bindStatus = $state('');

  // Window-level listener active only while recording — div-level onkeydown
  // is unreliable in WebKitGTK because clicking a div doesn't always give it
  // keyboard focus.
  $effect(() => {
    if (!recordingBind) return;
    window.addEventListener('keydown', onBindKeydown);
    return () => window.removeEventListener('keydown', onBindKeydown);
  });

  function hyprlandKey(e: KeyboardEvent): string {
    const map: Record<string, string> = {
      ' ': 'SPACE', Enter: 'Return', Escape: 'Escape', Tab: 'Tab',
      Backspace: 'BackSpace', Delete: 'Delete',
      ArrowLeft: 'Left', ArrowRight: 'Right', ArrowUp: 'Up', ArrowDown: 'Down',
      Home: 'Home', End: 'End', PageUp: 'Prior', PageDown: 'Next',
      ';': 'semicolon', ':': 'colon', "'": 'apostrophe', '"': 'quotedbl',
      ',': 'comma', '.': 'period', '/': 'slash', '\\': 'backslash',
      '[': 'bracketleft', ']': 'bracketright', '`': 'grave',
      '-': 'minus', '=': 'equal',
    };
    if (e.key.startsWith('F') && /^F\d+$/.test(e.key)) return e.key;
    if (map[e.key]) return map[e.key];
    if (e.key.length === 1) return e.key.toLowerCase();
    return '';
  }

  function onBindKeydown(e: KeyboardEvent) {
    e.preventDefault();
    const MODS = ['Control', 'Alt', 'Shift', 'Meta'];
    if (MODS.includes(e.key)) return;

    const mods: string[] = [];
    if (e.ctrlKey)  mods.push('CTRL');
    if (e.altKey)   mods.push('ALT');
    if (e.shiftKey) mods.push('SHIFT');
    if (e.metaKey)  mods.push('SUPER');

    const key = hyprlandKey(e);
    if (!key) return;

    currentBind = mods.length ? `${mods.join(' ')}, ${key}` : key;
    recordingBind = false;
  }

  // Also re-injects the hyprctl bind — needed after Hyprland config reload
  // wipes keyword-injected binds.
  async function saveBind() {
    try {
      await invoke('set_interaction_shortcut', { modsKey: currentBind });
      bindStatus = 'Saved & applied.';
    } catch (err) {
      bindStatus = `Error: ${err}`;
    }
    setTimeout(() => (bindStatus = ''), 4000);
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
      <dt>Interactive mode</dt>
      <dd class:interactive-on={overlayInteractive} class:interactive-off={!overlayInteractive}>
        {overlayInteractive ? '● ON' : '○ OFF'}
      </dd>
    </dl>
  </section>

  <section>
    <h2>Relics — Development</h2>
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

  <section>
    <h2>Rivens — Development</h2>
    <p>Send a test trigger to exercise the OCR + grading pipeline on the real game screen:</p>
    <button onclick={testRivenTrigger}>Send test trigger</button>
    <button class="secondary" onclick={testRivenOverlay}>Test overlay (fake data)</button>
  </section>

  <section>
    <h2>Settings</h2>
    <dl>
      <dt>Overlay hotkey</dt>
      <dd>
        <!-- svelte-ignore a11y_no_static_element_interactions -->
        <div
          class="bind-recorder"
          class:recording={recordingBind}
          tabindex="0"
          role="button"
          onclick={() => (recordingBind = true)}
          onblur={() => (recordingBind = false)}
          onkeydown={() => {}}
        >
          {#if recordingBind}
            <span class="bind-hint">Press a key combo…</span>
          {:else}
            <span class="bind-value">{currentBind || '—'}</span>
          {/if}
        </div>
        <p class="hint">Click the box, then press your desired key combination.</p>
        <button onclick={saveBind}>Apply</button>
        <p class="hint">Re-click Apply if the shortcut stops working after a Hyprland config reload — it re-injects the bind.</p>
        {#if bindStatus}
          <p class="status">{bindStatus}</p>
        {/if}
      </dd>
    </dl>
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

  .interactive-on  { color: #7ec8a0; font-weight: 600; }
  .interactive-off { color: #555; }

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

  .hint {
    color: #666;
    font-size: 12px;
    margin: 4px 0 8px;
  }

  .bind-recorder {
    display: inline-flex;
    align-items: center;
    min-width: 180px;
    background: #0f1117;
    border: 1px solid #2a2d3a;
    border-radius: 6px;
    padding: 8px 14px;
    margin-bottom: 8px;
    cursor: pointer;
    outline: none;
    transition: border-color 0.15s;
  }

  .bind-recorder:hover,
  .bind-recorder:focus {
    border-color: #c9a227;
  }

  .bind-recorder.recording {
    border-color: #7ec8a0;
  }

  .bind-value {
    font-family: 'Consolas', 'Courier New', monospace;
    font-size: 12px;
    color: #c9a227;
  }

  .bind-hint {
    font-size: 12px;
    color: #7ec8a0;
    font-style: italic;
  }
</style>
