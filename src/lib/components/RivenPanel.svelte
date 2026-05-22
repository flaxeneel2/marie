<script lang="ts">
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

  let { grade, label, isNew, oldGrade } = $props<{
    grade: RivenRollGrade;
    label: string;
    isNew: boolean;
    oldGrade: RivenRollGrade | null;
  }>();

  function dispStars(disp: number): number {
    if (disp >= 1.4)  return 5;
    if (disp >= 1.15) return 4;
    if (disp >= 0.9)  return 3;
    if (disp >= 0.65) return 2;
    return 1;
  }

  function gradeColor(gradeLetter: string): string {
    return ({ S: '#e5a93b', A: '#10b981', B: '#38bdf8', C: '#94a3b8', D: '#475569', F: '#ef4444' })[gradeLetter] ?? '#475569';
  }

  function weightColor(labelVal: string): string {
    return ({ God: '#e5a93b', Great: '#10b981', Good: '#38bdf8', Filler: '#334155', Dump: '#1e293b' })[labelVal] ?? '#334155';
  }

  function weightLetter(labelVal: string): string {
    return ({ God: 'S', Great: 'A', Good: 'B', Filler: 'C', Dump: 'D' })[labelVal] ?? '?';
  }

  function findMatchingStat(slug: string, stats: ParsedStat[]): ParsedStat | undefined {
    return stats.find(s => s.slug === slug);
  }

  function statImproved(oldStat: ParsedStat | undefined, newStat: ParsedStat): boolean {
    if (!oldStat) return false;
    return newStat.weight > oldStat.weight;
  }
</script>

<div class="riven-panel" class:new-panel={isNew}>
  <div class="panel-header-row">
    <div class="panel-label" class:new-label={isNew}>{label}</div>
    <span class="roll-count-text">{grade.roll_count} roll{grade.roll_count === 1 ? '' : 's'}</span>
  </div>

  <div class="weapon-header">
    <span class="weapon-name" title={grade.weapon_name}>{grade.weapon_name || '?'}</span>
    <span class="tier-badge" style="color:{gradeColor(grade.weapon_tier)}; border-color:{gradeColor(grade.weapon_tier)}">T{grade.weapon_tier}</span>
  </div>

  <div class="disp-row">
    <div class="disp-dots">
      {#each Array(5) as _, di}
        <span class="disp-dot" class:disp-filled={di < dispStars(grade.disposition)}>⬡</span>
      {/each}
    </div>
    <span class="disp-label">DISPOSITION {grade.disposition.toFixed(2)}</span>
  </div>

  <div class="stats-section">
    {#each grade.stats as stat}
      {@const isBad = stat.is_negative || stat.effective_negative}
      {@const oldStat = oldGrade ? findMatchingStat(stat.slug, oldGrade.stats) : undefined}
      {@const improved = statImproved(oldStat, stat)}
      <div class="stat-entry" class:stat-bad={isBad} class:stat-improved={improved}>
        <div class="stat-main-row">
          <span class="weight-pip" style="color:{isBad ? '#475569' : weightColor(stat.weight_label)}">◆</span>
          <span class="stat-sign" class:neg-sign={isBad}>{isBad ? '−' : stat.is_multiplier ? '×' : '+'}</span>
          <span class="stat-val" class:neg-val={isBad}>{stat.is_multiplier ? stat.value.toFixed(2) : stat.value.toFixed(1) + '%'}</span>
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
      { label: 'Build Grade',  score: grade.build_score,  g: grade.build_grade  },
      { label: 'Market Value', score: grade.market_score, g: grade.market_grade },
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

  {#if grade.roll_count > 5}
    <div class="roll-penalty-row">
      <span class="penalty-icon">⚠</span>
      <span class="roll-penalty">Market penalty active: −{Math.min(15, grade.roll_count - 5)}% due to high rolls</span>
    </div>
  {/if}
</div>

<style>
  .riven-panel {
    background: rgba(9, 11, 16, 0.94);
    border: 1px solid rgba(56, 189, 248, 0.15);
    border-radius: 10px;
    padding: 16px;
    width: 280px;
    font-family: 'Inter', 'Segoe UI', Arial, sans-serif;
    display: flex;
    flex-direction: column;
    gap: 12px;
    box-shadow: 0 12px 36px rgba(0, 0, 0, 0.8);
    transition: all 0.2s ease;
  }

  .new-panel {
    border-color: rgba(229, 169, 59, 0.5);
    box-shadow: 0 12px 36px rgba(0, 0, 0, 0.8), 0 0 15px rgba(229, 169, 59, 0.15);
    background: linear-gradient(180deg, rgba(9, 11, 16, 0.96) 0%, rgba(229, 169, 59, 0.02) 100%);
  }

  .panel-header-row {
    display: flex;
    justify-content: space-between;
    align-items: center;
  }

  .panel-label {
    font-size: 10px;
    font-weight: 800;
    letter-spacing: 0.12em;
    color: #475569;
    text-transform: uppercase;
  }

  .panel-label.new-label {
    color: #e5a93b;
    text-shadow: 0 0 8px rgba(229, 169, 59, 0.2);
  }

  .roll-count-text {
    font-size: 11px;
    font-weight: 700;
    color: #64748b;
    font-family: 'Consolas', 'Courier New', monospace;
  }

  .weapon-header {
    display: flex;
    align-items: center;
    gap: 10px;
  }

  .weapon-name {
    font-size: 16px;
    font-weight: 800;
    color: #f1f5f9;
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    letter-spacing: 0.02em;
  }

  .tier-badge {
    font-size: 10px;
    font-weight: 800;
    letter-spacing: 0.05em;
    flex-shrink: 0;
    border: 1px solid;
    border-radius: 4px;
    padding: 1px 5px;
    background: rgba(255, 255, 255, 0.02);
  }

  .disp-row {
    display: flex;
    align-items: center;
    gap: 8px;
    background: rgba(255, 255, 255, 0.02);
    padding: 4px 8px;
    border-radius: 4px;
    border: 1px solid rgba(255, 255, 255, 0.02);
  }

  .disp-dots {
    display: flex;
    gap: 3px;
  }

  .disp-dot {
    font-size: 11px;
    color: #1e293b;
    font-weight: bold;
  }

  .disp-dot.disp-filled {
    color: #e5a93b;
    text-shadow: 0 0 6px rgba(229, 169, 59, 0.4);
  }

  .disp-label {
    font-size: 9px;
    font-weight: 700;
    color: #64748b;
    letter-spacing: 0.05em;
  }

  /* Stats Panel */
  .stats-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
    border-top: 1px solid rgba(56, 189, 248, 0.1);
    border-bottom: 1px solid rgba(56, 189, 248, 0.1);
    padding: 10px 0;
  }

  .stat-entry {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .stat-entry.stat-bad {
    opacity: 0.45;
  }

  .stat-entry.stat-improved {
    background: rgba(16, 185, 129, 0.06);
    border-radius: 4px;
    padding: 4px 6px;
    margin: -2px -6px;
    border: 1px solid rgba(16, 185, 129, 0.1);
  }

  .stat-main-row {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 12px;
  }

  .weight-pip {
    font-size: 8px;
    flex-shrink: 0;
    width: 10px;
    text-align: center;
  }

  .stat-sign {
    font-weight: 800;
    color: #10b981;
    width: 10px;
    text-align: center;
    flex-shrink: 0;
  }

  .stat-sign.neg-sign {
    color: #ef4444;
  }

  .stat-val {
    color: #f1f5f9;
    min-width: 48px;
    text-align: right;
    flex-shrink: 0;
    font-weight: 700;
    font-family: 'Consolas', 'Courier New', monospace;
  }

  .stat-val.neg-val {
    color: #cbd5e1;
  }

  .stat-name {
    color: #94a3b8;
    flex: 1;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .weight-tier {
    font-size: 9px;
    font-weight: 800;
    letter-spacing: 0.05em;
    flex-shrink: 0;
    text-transform: uppercase;
  }

  .neg-tier {
    color: #475569;
  }

  .stat-bar-track {
    height: 3px;
    background: rgba(255, 255, 255, 0.04);
    border-radius: 2px;
    overflow: hidden;
    margin-left: 16px;
  }

  .stat-bar-fill {
    height: 100%;
    border-radius: 2px;
    opacity: 0.8;
  }

  .stat-bar-unknown {
    background: repeating-linear-gradient(
      90deg,
      rgba(255, 255, 255, 0.03) 0px,
      rgba(255, 255, 255, 0.03) 4px,
      transparent 4px,
      transparent 8px
    );
  }

  /* Score Bars */
  .scores-section {
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .score-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .score-label {
    font-size: 9px;
    color: #64748b;
    font-weight: 800;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    width: 68px;
    flex-shrink: 0;
  }

  .score-bar-track {
    flex: 1;
    height: 6px;
    background: rgba(255, 255, 255, 0.04);
    border-radius: 3px;
    overflow: hidden;
  }

  .score-bar-fill {
    height: 100%;
    border-radius: 3px;
    opacity: 0.8;
  }

  .score-pct {
    font-size: 11px;
    color: #64748b;
    width: 28px;
    text-align: right;
    flex-shrink: 0;
    font-family: 'Consolas', 'Courier New', monospace;
    font-weight: bold;
  }

  .score-grade-letter {
    font-size: 16px;
    font-weight: 900;
    width: 14px;
    flex-shrink: 0;
    line-height: 1;
    text-align: center;
  }

  /* Roll Penalty */
  .roll-penalty-row {
    display: flex;
    align-items: center;
    gap: 6px;
    background: rgba(239, 68, 68, 0.04);
    border: 1px solid rgba(239, 68, 68, 0.1);
    padding: 6px 10px;
    border-radius: 4px;
  }

  .penalty-icon {
    font-size: 12px;
    color: #ef4444;
  }

  .roll-penalty {
    font-size: 10px;
    font-weight: 600;
    color: #ef4444;
  }
</style>
