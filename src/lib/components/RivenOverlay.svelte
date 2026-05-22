<script lang="ts">
  import RivenPanel from '$lib/components/RivenPanel.svelte';

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

  let {
    rivenVisible,
    rivenRolling,
    rivenLoading,
    rivenError,
    rivenCurrentGrade,
    rivenData
  } = $props<{
    rivenVisible: boolean;
    rivenRolling: boolean;
    rivenLoading: boolean;
    rivenError: string;
    rivenCurrentGrade: RivenRollGrade | null;
    rivenData: RivenRerollResult | null;
  }>();
</script>

{#if rivenVisible}
  <div class="riven-overlay">
    {#if rivenRolling}
      <div class="riven-status riven-rolling">
        <span class="rolling-spinner"></span>
        <span>ROLLING RIVEN...</span>
      </div>
    {:else if rivenLoading}
      <div class="riven-status riven-grading">
        <span class="grading-spinner"></span>
        <span>GRADING STATS...</span>
      </div>
    {:else if rivenError}
      <div class="riven-status riven-error">
        <span class="error-badge">!</span>
        <span>{rivenError}</span>
      </div>
    {:else}
      <div class="panels-container">
        {#if rivenCurrentGrade}
          <div class="riven-side riven-side-left">
            <RivenPanel grade={rivenCurrentGrade} label="CURRENT" isNew={false} oldGrade={null} />
          </div>
        {:else if rivenData}
          <div class="riven-side riven-side-left">
            <RivenPanel grade={rivenData.old} label="CURRENT" isNew={false} oldGrade={null} />
          </div>
          <div class="riven-side riven-side-right">
            <RivenPanel grade={rivenData.new} label="NEW" isNew={true} oldGrade={rivenData.old} />
          </div>
        {/if}
      </div>
    {/if}
  </div>
{/if}

<style>
  .riven-overlay {
    position: fixed;
    inset: 0;
    z-index: 10;
    pointer-events: none;
    font-family: 'Inter', 'Segoe UI', Arial, sans-serif;
  }

  .panels-container {
    position: relative;
    width: 100%;
    height: 100%;
  }

  .riven-side {
    position: absolute;
    top: 50%;
    transform: translateY(-50%);
    pointer-events: auto;
    animation: slide-panel-in 0.25s cubic-bezier(0.16, 1, 0.3, 1) forwards;
  }

  .riven-side-left {
    left: 40px;
  }

  .riven-side-right {
    right: 40px;
  }

  @keyframes slide-panel-in {
    from {
      opacity: 0;
      transform: translateY(-50%) scale(0.95);
    }
    to {
      opacity: 1;
      transform: translateY(-50%) scale(1);
    }
  }

  /* Status messages styles */
  .riven-status {
    position: absolute;
    top: 50%;
    left: 50%;
    transform: translate(-50%, -50%);
    background: rgba(9, 11, 16, 0.95);
    color: #e2e8f0;
    padding: 12px 24px;
    border-radius: 8px;
    font-size: 13px;
    font-weight: 700;
    letter-spacing: 0.08em;
    border: 1px solid rgba(56, 189, 248, 0.25);
    display: flex;
    align-items: center;
    gap: 12px;
    box-shadow: 0 12px 36px rgba(0, 0, 0, 0.8);
    pointer-events: auto;
  }

  .riven-rolling {
    color: #e5a93b;
    border-color: rgba(229, 169, 59, 0.4);
  }

  .riven-error {
    color: #ef4444;
    border-color: rgba(239, 68, 68, 0.4);
  }

  /* Spinners */
  .rolling-spinner, .grading-spinner {
    width: 16px;
    height: 16px;
    border: 2px solid rgba(255, 255, 255, 0.1);
    border-radius: 50%;
    display: inline-block;
  }

  .rolling-spinner {
    border-top-color: #e5a93b;
    animation: spin 0.8s linear infinite;
  }

  .grading-spinner {
    border-top-color: #38bdf8;
    animation: spin 0.8s linear infinite;
  }

  .error-badge {
    background: rgba(239, 68, 68, 0.15);
    border: 1px solid #ef4444;
    color: #ef4444;
    width: 18px;
    height: 18px;
    border-radius: 50%;
    display: flex;
    align-items: center;
    justify-content: center;
    font-size: 10px;
    font-weight: 900;
  }

  @keyframes spin {
    from { transform: rotate(0deg); }
    to { transform: rotate(360deg); }
  }
</style>
