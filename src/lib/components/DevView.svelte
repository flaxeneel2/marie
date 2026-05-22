<script lang="ts">
  let {
    logPath,
    detectedCount,
    overlayInteractive,
    initialBind,
    onSaveBind,
    onTestTrigger,
    onTestOverlay,
    onTestRivenTrigger,
    onTestRivenOverlay
  } = $props<{
    logPath: string;
    detectedCount: number | null;
    overlayInteractive: boolean;
    initialBind: string;
    onSaveBind: (bind: string) => Promise<string>;
    onTestTrigger: (count: number) => Promise<void>;
    onTestOverlay: () => Promise<void>;
    onTestRivenTrigger: () => Promise<void>;
    onTestRivenOverlay: () => Promise<void>;
  }>();

  let currentBind = $state('');
  let recordingBind = $state(false);
  let bindStatus = $state('');
  let overrideEnabled = $state(false);
  let playerCount = $state(4);
  let triggerStatus = $state('');

  $effect(() => {
    currentBind = initialBind;
  });

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

  async function handleSaveBind() {
    try {
      const res = await onSaveBind(currentBind);
      bindStatus = res || 'Saved & applied.';
    } catch (err) {
      bindStatus = `Error: ${err}`;
    }
    setTimeout(() => (bindStatus = ''), 4000);
  }

  function effectiveCount(): number {
    return overrideEnabled ? playerCount : (detectedCount ?? 4);
  }

  async function handleTestTrigger() {
    const count = effectiveCount();
    triggerStatus = `Trigger sent (${count} player${count === 1 ? '' : 's'}) — check overlay.`;
    await onTestTrigger(count);
    setTimeout(() => (triggerStatus = ''), 4000);
  }

  async function handleTestOverlay() {
    triggerStatus = 'Overlay shown with fake data.';
    await onTestOverlay();
    setTimeout(() => (triggerStatus = ''), 4000);
  }

  async function handleTestRivenTrigger() {
    triggerStatus = 'Riven trigger sent — check overlay.';
    await onTestRivenTrigger();
    setTimeout(() => (triggerStatus = ''), 4000);
  }

  async function handleTestRivenOverlay() {
    triggerStatus = 'Riven overlay shown with fake data.';
    await onTestRivenOverlay();
    setTimeout(() => (triggerStatus = ''), 4000);
  }
</script>

<div class="dev-container">
  <div class="page-header">
    <h1>Dev Tools</h1>
    <p class="subtitle">Development tools, diagnostics, and companion settings</p>
  </div>

  <div class="dev-sections-grid">
    <!-- Status Section -->
    <section class="dev-section">
      <div class="section-header">
        <span class="header-icon">📊</span>
        <h2>System Status</h2>
      </div>
      <div class="diagnostic-list">
        <div class="diag-item">
          <span class="diag-label">EE.log path</span>
          <span class="diag-value mono" title={logPath}>{logPath}</span>
        </div>
        <div class="diag-item">
          <span class="diag-label">Current lobby</span>
          <span class="diag-value">
            {#if detectedCount !== null}
              <span class="badge active">{detectedCount} player{detectedCount === 1 ? '' : 's'}</span>
            {:else}
              <span class="muted">— (no relic screen detected)</span>
            {/if}
          </span>
        </div>
        <div class="diag-item">
          <span class="diag-label">Interactive mode</span>
          <span class="diag-value">
            <span class="status-indicator" class:active={overlayInteractive}>
              <span class="indicator-pip"></span>
              {overlayInteractive ? 'ACTIVE' : 'INACTIVE'}
            </span>
          </span>
        </div>
      </div>
    </section>

    <!-- Hotkey Settings Section -->
    <section class="dev-section">
      <div class="section-header">
        <span class="header-icon">⌨</span>
        <h2>Interaction Shortcut</h2>
      </div>
      <div class="settings-content">
        <p class="section-desc">Toggle the overlay mouse capture on Wayland by pressing this hotkey.</p>

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
            <span class="bind-hint">Listening... press key combo</span>
          {:else}
            <span class="bind-value">{currentBind || '—'}</span>
          {/if}
        </div>
        <p class="hint">Click the field, enter hotkey combination, then click Apply.</p>

        <div class="action-row">
          <button class="action-btn-primary" onclick={handleSaveBind}>Apply Shortcut</button>
          {#if bindStatus}
            <span class="status-msg-inline" class:error={bindStatus.includes('Error')}>{bindStatus}</span>
          {/if}
        </div>
      </div>
    </section>

    <!-- Relics Testing Section -->
    <section class="dev-section">
      <div class="section-header">
        <span class="header-icon">◈</span>
        <h2>Relic Scanner Simulation</h2>
      </div>
      <div class="settings-content">
        <p class="section-desc">Simulate a relic reward screen OCR trigger with custom squad count.</p>

        <div class="player-count-row">
          <span class="row-label">Squad Size</span>
          <div class="count-selector">
            {#each [1, 2, 3, 4] as n}
              <button
                class="count-btn"
                class:active={overrideEnabled && playerCount === n}
                class:disabled={!overrideEnabled}
                onclick={() => { overrideEnabled = true; playerCount = n; }}
              >{n}</button>
            {/each}
          </div>
          <button
            class="override-toggle-btn"
            class:active={overrideEnabled}
            onclick={() => (overrideEnabled = !overrideEnabled)}
          >
            {overrideEnabled ? 'Override: On' : 'Override: Off'}
          </button>
        </div>

        <div class="btn-group">
          <button class="action-btn-primary" onclick={handleTestTrigger}>Trigger Real OCR</button>
          <button class="action-btn-secondary" onclick={handleTestOverlay}>Mock Price HUD</button>
        </div>
      </div>
    </section>

    <!-- Riven Testing Section -->
    <section class="dev-section">
      <div class="section-header">
        <span class="header-icon">⚡</span>
        <h2>Riven Grading Simulation</h2>
      </div>
      <div class="settings-content">
        <p class="section-desc">Simulate a riven reroll screen to test grading metrics and visual charts.</p>

        <div class="btn-group">
          <button class="action-btn-primary" onclick={handleTestRivenTrigger}>Trigger Real OCR</button>
          <button class="action-btn-secondary" onclick={handleTestRivenOverlay}>Mock Grading HUD</button>
        </div>
      </div>
    </section>
  </div>

  {#if triggerStatus}
    <div class="floating-toast">
      <span class="toast-icon">ℹ</span>
      <span class="toast-text">{triggerStatus}</span>
    </div>
  {/if}
</div>

<style>
  .dev-container {
    display: flex;
    flex-direction: column;
    height: 100%;
    min-height: 0;
  }

  .dev-sections-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(320px, 1fr));
    gap: 16px;
    padding-bottom: 24px;
    overflow-y: auto;
    flex: 1;
    padding-right: 4px;
  }

  /* Custom Slim Scrollbar */
  .dev-sections-grid::-webkit-scrollbar {
    width: 6px;
  }
  .dev-sections-grid::-webkit-scrollbar-track {
    background: rgba(255, 255, 255, 0.02);
    border-radius: 3px;
  }
  .dev-sections-grid::-webkit-scrollbar-thumb {
    background: rgba(56, 189, 248, 0.2);
    border-radius: 3px;
  }

  .dev-section {
    background: rgba(14, 18, 28, 0.7);
    border: 1px solid rgba(56, 189, 248, 0.1);
    border-radius: 8px;
    padding: 18px;
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .section-header {
    display: flex;
    align-items: center;
    gap: 10px;
    border-bottom: 1px solid rgba(56, 189, 248, 0.06);
    padding-bottom: 8px;
  }

  .header-icon {
    font-size: 16px;
  }

  h2 {
    font-size: 13px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.1em;
    color: #e2e8f0;
    margin: 0;
  }

  .section-desc {
    font-size: 11px;
    color: #64748b;
    margin: 0 0 12px 0;
    line-height: 1.4;
  }

  /* Diagnostics Styling */
  .diagnostic-list {
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .diag-item {
    display: flex;
    flex-direction: column;
    gap: 4px;
    padding-bottom: 8px;
    border-bottom: 1px dashed rgba(255, 255, 255, 0.03);
  }

  .diag-item:last-child {
    border-bottom: none;
    padding-bottom: 0;
  }

  .diag-label {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: #64748b;
  }

  .diag-value {
    font-size: 12px;
    color: #cbd5e1;
    font-weight: 600;
  }

  .diag-value.mono {
    font-family: 'Consolas', 'Courier New', monospace;
    font-size: 11px;
    word-break: break-all;
    background: rgba(0, 0, 0, 0.2);
    padding: 4px 8px;
    border-radius: 4px;
    border: 1px solid rgba(255, 255, 255, 0.04);
  }

  .badge {
    font-size: 10px;
    font-weight: 700;
    background: rgba(56, 189, 248, 0.08);
    border: 1px solid rgba(56, 189, 248, 0.2);
    color: #38bdf8;
    padding: 2px 6px;
    border-radius: 4px;
  }

  /* Status indicator */
  .status-indicator {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    font-size: 11px;
    font-weight: 700;
    letter-spacing: 0.05em;
    color: #64748b;
  }

  .status-indicator.active {
    color: #10b981;
  }

  .indicator-pip {
    width: 6px;
    height: 6px;
    border-radius: 50%;
    background: #475569;
  }

  .status-indicator.active .indicator-pip {
    background: #10b981;
    box-shadow: 0 0 8px #10b981;
    animation: pulse 1.5s infinite;
  }

  @keyframes pulse {
    0% { opacity: 0.6; }
    50% { opacity: 1; }
    100% { opacity: 0.6; }
  }

  /* Keybind Recorder */
  .bind-recorder {
    background: rgba(8, 10, 15, 0.8);
    border: 1px solid rgba(56, 189, 248, 0.15);
    border-radius: 6px;
    padding: 10px 14px;
    cursor: pointer;
    text-align: center;
    margin-bottom: 8px;
    transition: all 0.2s ease;
    display: flex;
    align-items: center;
    justify-content: center;
    min-height: 40px;
  }

  .bind-recorder:hover {
    border-color: #38bdf8;
    background: rgba(8, 10, 15, 0.9);
  }

  .bind-recorder.recording {
    border-color: #e5a93b;
    box-shadow: 0 0 10px rgba(229, 169, 59, 0.15);
    background: rgba(229, 169, 59, 0.02);
  }

  .bind-value {
    font-family: 'Consolas', 'Courier New', monospace;
    font-size: 13px;
    color: #e5a93b;
    font-weight: 700;
    text-shadow: 0 0 4px rgba(229, 169, 59, 0.2);
  }

  .bind-hint {
    font-size: 11px;
    color: #38bdf8;
    font-style: italic;
    animation: flash 1s ease-in-out infinite alternate;
  }

  @keyframes flash {
    from { opacity: 0.5; }
    to { opacity: 1; }
  }

  .hint {
    font-size: 10px;
    color: #475569;
    margin: 4px 0 12px 0;
    line-height: 1.4;
  }

  /* Controls and buttons */
  .action-row {
    display: flex;
    align-items: center;
    gap: 12px;
  }

  .action-btn-primary {
    background: #e5a93b;
    color: #090a0f;
    border: none;
    border-radius: 6px;
    padding: 8px 16px;
    font-size: 11px;
    font-weight: 700;
    cursor: pointer;
    transition: all 0.2s ease;
    box-shadow: 0 4px 10px rgba(229, 169, 59, 0.15);
  }

  .action-btn-primary:hover {
    background: #f1b74c;
    box-shadow: 0 4px 14px rgba(229, 169, 59, 0.3);
  }

  .action-btn-secondary {
    background: rgba(30, 41, 59, 0.6);
    border: 1px solid rgba(56, 189, 248, 0.15);
    border-radius: 6px;
    color: #cbd5e1;
    padding: 8px 16px;
    font-size: 11px;
    font-weight: 700;
    cursor: pointer;
    transition: all 0.2s ease;
  }

  .action-btn-secondary:hover {
    background: rgba(56, 189, 248, 0.08);
    border-color: #38bdf8;
    color: #ffffff;
  }

  .status-msg-inline {
    font-size: 11px;
    font-weight: 600;
    color: #10b981;
  }

  .status-msg-inline.error {
    color: #ef4444;
  }

  .btn-group {
    display: flex;
    gap: 8px;
    margin-top: 8px;
  }

  /* Player override count styling */
  .player-count-row {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-bottom: 12px;
    background: rgba(0, 0, 0, 0.15);
    padding: 6px 10px;
    border-radius: 6px;
    border: 1px solid rgba(255, 255, 255, 0.02);
  }

  .row-label {
    font-size: 10px;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: #64748b;
    margin-right: 4px;
  }

  .count-selector {
    display: flex;
    gap: 4px;
  }

  .count-btn {
    background: rgba(30, 41, 59, 0.4);
    border: 1px solid rgba(56, 189, 248, 0.1);
    color: #64748b;
    border-radius: 4px;
    width: 28px;
    height: 28px;
    font-size: 11px;
    font-weight: 700;
    cursor: pointer;
    transition: all 0.15s ease;
    padding: 0;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .count-btn:hover:not(.disabled) {
    background: rgba(56, 189, 248, 0.05);
    color: #e2e8f0;
  }

  .count-btn.active {
    background: #e5a93b;
    color: #090a0f;
    border-color: #e5a93b;
    box-shadow: 0 0 6px rgba(229, 169, 59, 0.3);
  }

  .count-btn.disabled {
    opacity: 0.25;
    cursor: not-allowed;
  }

  .override-toggle-btn {
    background: transparent;
    border: 1px dashed rgba(64, 74, 89, 0.4);
    color: #64748b;
    border-radius: 4px;
    padding: 4px 10px;
    font-size: 10px;
    font-weight: 700;
    cursor: pointer;
    transition: all 0.15s ease;
    margin-left: auto;
  }

  .override-toggle-btn:hover {
    border-color: rgba(56, 189, 248, 0.2);
    color: #cbd5e1;
  }

  .override-toggle-btn.active {
    background: rgba(16, 185, 129, 0.08);
    border-color: #10b981;
    color: #10b981;
    border-style: solid;
  }

  /* Floating Toast notification */
  .floating-toast {
    position: fixed;
    bottom: 20px;
    right: 20px;
    background: rgba(9, 10, 15, 0.9);
    border: 1px solid #38bdf8;
    border-radius: 8px;
    padding: 10px 16px;
    display: flex;
    align-items: center;
    gap: 10px;
    box-shadow: 0 8px 24px rgba(0, 0, 0, 0.6);
    z-index: 999;
    animation: slideIn 0.2s cubic-bezier(0.16, 1, 0.3, 1) forwards;
  }

  .toast-icon {
    font-size: 14px;
    color: #38bdf8;
    background: rgba(56, 189, 248, 0.1);
    width: 20px;
    height: 20px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-weight: 700;
  }

  .toast-text {
    font-size: 12px;
    color: #f1f5f9;
    font-weight: 600;
  }

  @keyframes slideIn {
    from {
      transform: translateY(20px);
      opacity: 0;
    }
    to {
      transform: translateY(0);
      opacity: 1;
    }
  }
</style>
