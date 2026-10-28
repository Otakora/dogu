<script lang="ts">
  import { app } from "../../stores/app.svelte.js";
  import { t } from "../../i18n/index.js";

  const visibleJobs = $derived(app.jobs.filter(j => !j.retried));
  const job = $derived(visibleJobs.find(j => j.paused) ?? visibleJobs.find(j => j.done && !j.success) ?? visibleJobs.find(j => !j.done) ?? visibleJobs[0]);
  const running = $derived(app.jobs.filter(j => !j.done).length);
</script>

<div class="global-status" role="toolbar" aria-label={t("status.openOperations")}>
  <button class="activity" class:activity--error={!!job && (job.paused || (job.done && !job.success))}
    onclick={() => app.openJobsPanel()} title={job?.resultMessage || job?.pauseError || t("status.openOperations")}>
    {#if job && !job.done && !job.paused}<span class="activity-spinner" aria-hidden="true"></span>
    {:else if job && (job.paused || !job.success)}<span class="activity-error" aria-hidden="true">!</span>
    {:else}<span class="activity-dot" aria-hidden="true"></span>{/if}
    <span class="activity-text" aria-live="polite">{job ? (job.paused ? (job.pauseError || t("jobPause.title")) : job.done ? (job.resultMessage || job.title) : (job.message || job.title)) : t("status.ready")}</span>
    {#if job && !job.done && !job.paused}<span class="activity-percent">{Math.round(job.progress)}%</span>{/if}
    {#if running > 1}<span class="activity-extra">{t("status.multiple", { count: running - 1 })}</span>{/if}
    {#if app.opQueue.length}<span class="activity-extra">{app.opQueue.length}</span>{/if}
  </button>
  <div class="global-controls">
    <button class="global-queue" class:global-queue--active={app.queueMode} onclick={() => app.toggleQueueMode()}
      aria-pressed={app.queueMode} title={t("toolbar.queueMode")}>
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><path d="M3 6h12M3 12h12M3 18h12M18 9l3 3-3 3"/></svg>
      <span class="global-queue-label">
        <span>{app.queueMode ? t("status.queueOn") : t("status.queueOff")}</span>
        <span class="global-queue-measure" aria-hidden="true">{t("status.queueOn")}</span>
        <span class="global-queue-measure" aria-hidden="true">{t("status.queueOff")}</span>
      </span>
    </button>
    <span class="global-separator"></span>
    <button class="global-icon" class:global-icon--active={app.isSplit} onclick={() => app.isSplit ? app.closeSecondPane() : app.addPane()}
      title={app.isSplit ? t("toolbar.closeSplit") : t("toolbar.splitView")}
      aria-label={app.isSplit ? t("toolbar.closeSplit") : t("toolbar.splitView")} aria-pressed={app.isSplit}>
      <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M12 3v18"/></svg>
    </button>
    {#if app.isSplit}
      <button class="global-icon" onclick={() => app.toggleSplitOrientation()}
        title={app.splitOrientation === "stacked" ? t("toolbar.arrangeSideBySide") : t("toolbar.arrangeStacked")}
        aria-label={app.splitOrientation === "stacked" ? t("toolbar.arrangeSideBySide") : t("toolbar.arrangeStacked")}>
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true"><rect x="3" y="3" width="18" height="18" rx="2"/>{#if app.splitOrientation === "stacked"}<path d="M12 3v18"/>{:else}<path d="M3 12h18"/>{/if}</svg>
      </button>
    {/if}
  </div>
</div>

<style>
  .global-status { min-height: 32px; display: flex; align-items: center; gap: 8px; padding: 3px 8px; border-top: 1px solid var(--line); background: var(--surface); flex-shrink: 0; }
  .activity { min-width: 0; flex: 1; display: flex; align-items: center; gap: 7px; border: 0; border-radius: 4px; padding: 4px; background: none; color: var(--text-muted); text-align: left; font: inherit; font-size: 11px; cursor: pointer; }
  .activity:hover:not(:disabled) { background: var(--surface-hover); color: var(--text); }
  .activity:disabled { cursor: default; }
  .activity--error { color: var(--danger, #e5484d); }
  .activity-text { overflow: hidden; white-space: nowrap; text-overflow: ellipsis; }
  .activity-percent, .activity-extra { flex-shrink: 0; font-size: 10px; }
  .activity-dot, .activity-error { width: 12px; text-align: center; flex-shrink: 0; }
  .activity-dot::after { content: ""; display: inline-block; width: 6px; height: 6px; border-radius: 50%; background: var(--text-subtle); }
  .activity-error { font-weight: 800; }
  .activity-spinner { width: 12px; height: 12px; border: 2px solid var(--line-strong); border-top-color: var(--accent); border-radius: 50%; animation: spin .8s linear infinite; flex-shrink: 0; }
  .global-controls { display: flex; align-items: center; gap: 4px; flex-shrink: 0; }
  .global-queue, .global-icon { display: flex; align-items: center; justify-content: center; gap: 5px; height: 26px; border: 1px solid var(--line-strong); border-radius: 5px; background: transparent; color: var(--text-muted); cursor: pointer; font: inherit; font-size: 10px; padding: 0 7px; }
  .global-queue { font-weight: 700; }
  .global-queue-label { display: grid; white-space: nowrap; }
  .global-queue-label > span { grid-area: 1 / 1; }
  .global-queue-measure { visibility: hidden; }
  .global-queue:hover, .global-icon:hover { background: var(--surface-hover); color: var(--text); }
  .global-queue--active { background: var(--accent); color: var(--accent-contrast); border-color: var(--accent); font-weight: 700; }
  .global-queue--active:hover { background: var(--accent); color: var(--accent-contrast); }
  .global-icon--active { color: var(--accent); background: var(--accent-soft); }
  .global-separator { height: 16px; border-left: 1px solid var(--line); margin: 0 3px; }
  @media (prefers-reduced-motion: reduce) { .activity-spinner { animation: none; } }
</style>
