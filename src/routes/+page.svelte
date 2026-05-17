<script lang="ts">
  import { onMount } from 'svelte';
  import { listen } from '@tauri-apps/api/event';
  import { invoke } from '@tauri-apps/api/core';

  let logPath = $state('…');
  let triggerStatus = $state('');
  let detectedCount = $state<number | null>(null);
  let overrideEnabled = $state(false);
  let playerCount = $state(4);

  // ── Keybind settings ─────────────────────────────────────────────────────────
  let shortcutInput = $state('Ctrl+Shift+I');
  let recordingShortcut = $state(false);
  let shortcutSaved = $state(false);
  let shortcutError = $state('');

  function codeToKeyName(code: string): string | null {
    if (code.startsWith('Key')) return code.slice(3);       // KeyA → A
    if (code.startsWith('Digit')) return code.slice(5);     // Digit1 → 1
    if (/^F\d+$/.test(code)) return code;                   // F1, F2, …
    const map: Record<string, string> = {
      Semicolon: ';', Equal: '=', Minus: '-', Period: '.', Comma: ',',
      Slash: '/', Backslash: '\\', BracketLeft: '[', BracketRight: ']',
      Quote: "'", Backquote: '`', Space: 'Space', Enter: 'Enter',
      Backspace: 'Backspace', Delete: 'Delete', Escape: 'Escape', Tab: 'Tab',
      ArrowUp: 'Up', ArrowDown: 'Down', ArrowLeft: 'Left', ArrowRight: 'Right',
      Home: 'Home', End: 'End', PageUp: 'PageUp', PageDown: 'PageDown',
    };
    return map[code] ?? null;
  }

  function startRecording(e: MouseEvent) {
    recordingShortcut = true;
    shortcutError = '';
    (e.currentTarget as HTMLElement).focus();
  }

  function handleKeybindKeyDown(e: KeyboardEvent) {
    if (!recordingShortcut) return;
    e.preventDefault();
    e.stopPropagation();
    if (['Control', 'Alt', 'Shift', 'Meta'].includes(e.key)) return;
    const keyName = codeToKeyName(e.code);
    if (!keyName) return;
    const parts: string[] = [];
    if (e.ctrlKey)  parts.push('Ctrl');
    if (e.altKey)   parts.push('Alt');
    if (e.shiftKey) parts.push('Shift');
    if (e.metaKey)  parts.push('Super');
    parts.push(keyName);
    shortcutInput = parts.join('+');
    recordingShortcut = false;
  }

  async function saveShortcut() {
    shortcutError = '';
    try {
      await invoke('set_interaction_shortcut', { shortcut: shortcutInput });
      shortcutSaved = true;
      setTimeout(() => (shortcutSaved = false), 2000);
    } catch (e) {
      shortcutError = String(e);
    }
  }

  onMount(() => {
    invoke<string>('ee_log_path').then(p => (logPath = p));
    invoke<string>('get_interaction_shortcut').then(s => (shortcutInput = s));

    // Mirror the overlay's trigger listener so the readout stays in sync
    // with whatever the EE.log watcher detects.
    const cleanups: Array<() => void> = [];
    listen<number>('relic-trigger', (event) => {
      detectedCount = event.payload;
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
        <p class="hint">Press this combination to toggle the overlay between click-through and interactive mode.</p>
        <div class="keybind-row">
          <!-- svelte-ignore a11y_no_noninteractive_tabindex -->
          <div
            class="keybind-input"
            class:recording={recordingShortcut}
            tabindex="0"
            role="button"
            onclick={startRecording}
            onkeydown={handleKeybindKeyDown}
            onblur={() => (recordingShortcut = false)}
          >
            {recordingShortcut ? 'Press keys…' : shortcutInput}
          </div>
          <button onclick={saveShortcut}>Save</button>
        </div>
        {#if shortcutSaved}
          <p class="status">Saved!</p>
        {:else if shortcutError}
          <p class="status error">{shortcutError}</p>
        {/if}
        <p class="hint">Click the box then press your key combination. Note: on Linux/Wayland this requires being in the <code>input</code> group (<code>sudo usermod -aG input $USER</code>).</p>
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

  .status.error {
    color: #ff6b6b;
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

  .keybind-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 6px;
  }

  .keybind-input {
    flex: 1;
    background: #0f1117;
    border: 1px solid #3a3d4a;
    border-radius: 6px;
    padding: 7px 12px;
    font-size: 13px;
    font-family: 'Consolas', 'Courier New', monospace;
    color: #ccc;
    cursor: pointer;
    user-select: none;
    outline: none;
    transition: border-color 0.15s;
  }

  .keybind-input:focus,
  .keybind-input:hover {
    border-color: #c9a227;
  }

  .keybind-input.recording {
    border-color: #c9a227;
    color: #c9a227;
    animation: pulse 1s ease-in-out infinite;
  }

  @keyframes pulse {
    0%, 100% { opacity: 1; }
    50%       { opacity: 0.6; }
  }

  code {
    font-family: 'Consolas', 'Courier New', monospace;
    font-size: 11px;
    background: #0f1117;
    border: 1px solid #2a2d3a;
    border-radius: 3px;
    padding: 1px 4px;
    color: #aaa;
  }
</style>
