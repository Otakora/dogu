<script lang="ts">
  import { onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { app } from "../../stores/app.svelte.js";
  import { t, tn } from "../../i18n/index.js";
  import type { QueueConflict, QueuedOp, QueuedOpKind } from "../../types/index.js";

  async function resumeJob(jobId: string, decision: "retry" | "skip" | "abort") {
    app.clearJobPause(jobId);
    await invoke("resume_job", { jobId, decision });
  }

  // ── Log expansion ─────────────────────────────────────────
  let expandedLogs = $state<Set<string>>(new Set());
  function toggleLogs(jobId: string) {
    const next = new Set(expandedLogs);
    if (next.has(jobId)) next.delete(jobId); else next.add(jobId);
    expandedLogs = next;
  }

  // ── Derived ───────────────────────────────────────────────
  const runningCount        = $derived(app.jobs.filter(j => !j.done).length);
  const hasQueue            = $derived(app.opQueue.length > 0 || !!app.queueRunStats);
  const hasJobs             = $derived(app.jobs.length > 0);
  const panelVisible        = $derived(hasQueue || hasJobs);
  const blockingConflicts   = $derived(app.queueConflicts.filter(c => c.severity === 'blocking'));
  const parallelConflicts   = $derived(app.queueConflicts.filter(c => c.severity === 'parallel-only'));
  const hasConflicts        = $derived(app.queueConflicts.length > 0);
  const hasBlockingConflicts= $derived(blockingConflicts.length > 0);
  // Parallel is only safe with NO conflicts AND no chained dependencies
  // (a consumer must wait for the producer that creates its input).
  const canRunParallel      = $derived(hasQueue && !hasConflicts && !app.queueHasDependencies && !app.queueRunning);
  // Sequential is safe even with parallel-only conflicts (queue order protects it)
  const canRunSequential    = $derived(hasQueue && !hasBlockingConflicts && !app.queueRunning);

  // ── Kind metadata ─────────────────────────────────────────
  const KIND_COLOR: Record<QueuedOpKind, string> = {
    copy:         "#3b82f6",
    move:         "#f59e0b",
    delete:       "#ef4444",
    extract:      "#8b5cf6",
    compress:     "#10b981",
    "chd-convert":"#0d9488",
    "chd-restore":"#0d9488",
    m3u:          "#d946ef",
  };

  function kindLabel(k: QueuedOpKind): string {
    const map: Record<QueuedOpKind, Parameters<typeof t>[0]> = {
      copy: "queue.kind.copy", move: "queue.kind.move", delete: "queue.kind.delete",
      extract: "queue.kind.extract", compress: "queue.kind.compress",
      "chd-convert": "queue.kind.chdConvert", "chd-restore": "queue.kind.chdRestore",
      m3u: "queue.kind.m3u",
    };
    return t(map[k]);
  }

  function opDisplaySources(op: QueuedOp): string[] {
    return (op.kind === "delete" ? op.deletes : op.sources);
  }

  /** 1-based step numbers this op depends on, e.g. "1" or "1, 2" (null if none). */
  function dependencyLabel(op: QueuedOp): string | null {
    if (op.dependsOn.length === 0) return null;
    const steps = op.dependsOn
      .map(id => app.opQueue.findIndex(o => o.id === id) + 1)
      .filter(n => n > 0)
      .sort((a, b) => a - b);
    return steps.length ? steps.join(", ") : null;
  }

  function basename(p: string): string {
    return p.split(/[/\\]/).filter(Boolean).pop() ?? p;
  }

  // ── Conflict descriptions ─────────────────────────────────
  type ConflictParts = {
    pre: string; labelA: string; colorA: string;
    mid: string; labelB: string; colorB: string;
    suf: string;
  };

  function conflictParts(c: QueueConflict): ConflictParts {
    const a   = app.opQueue.find(o => o.id === c.opAId);
    const b   = app.opQueue.find(o => o.id === c.opBId);
    const idxA = app.opQueue.findIndex(o => o.id === c.opAId) + 1;
    const idxB = app.opQueue.findIndex(o => o.id === c.opBId) + 1;
    if (!a || !b) return { pre:"", labelA:"", colorA:"", mid:"", labelB:"", colorB:"", suf:"" };

    const path = basename(c.pathA);
    const key  = c.kind === "source-deleted" ? "queue.conflict.sourceDeleted"
               : c.kind === "dest-deleted"   ? "queue.conflict.destDeleted"
               :                               "queue.conflict.destCollision";
    // Template uses %%A%% / %%B%% as split sentinels; {path} is interpolated normally
    const raw   = t(key as Parameters<typeof t>[0], { path });
    const parts = raw.split("%%");
    // parts: [before-A, "A", between-A-B, "B", after-B]
    return {
      pre:    parts[0] ?? "",
      labelA: `«${idxA}. ${a.title}»`,
      colorA: KIND_COLOR[a.kind],
      mid:    parts[2] ?? "",
      labelB: `«${idxB}. ${b.title}»`,
      colorB: KIND_COLOR[b.kind],
      suf:    parts[4] ?? "",
    };
  }

  // ── Drag-to-reorder (vertical list, mouse events) ─────────
  type DragState = { fromIdx: number; startY: number; active: boolean };
  let drag: DragState | null = null;
  let draggedIdx    = $state<number | null>(null);
  let dropInsertIdx = $state<number | null>(null);
  let queueListEl   = $state<HTMLElement | undefined>();

  function onEntryMouseDown(e: MouseEvent, idx: number) {
    if (e.button !== 0) return;
    drag = { fromIdx: idx, startY: e.clientY, active: false };
    window.addEventListener("mousemove", onWindowMouseMove);
    window.addEventListener("mouseup",  onWindowMouseUp);
  }

  function onWindowMouseMove(e: MouseEvent) {
    if (!drag) return;
    if (!drag.active) {
      if (Math.abs(e.clientY - drag.startY) < 5) return;
      drag.active = true;
      draggedIdx = drag.fromIdx;
      document.body.style.cursor = "grabbing";
    }
    const els = queueListEl?.querySelectorAll<HTMLElement>(".queue-card");
    if (!els) return;
    let insert = els.length;
    for (let i = 0; i < els.length; i++) {
      const r = els[i].getBoundingClientRect();
      if (e.clientY < r.top + r.height / 2) { insert = i; break; }
    }
    dropInsertIdx = insert;
  }

  function onWindowMouseUp() {
    if (drag?.active && dropInsertIdx !== null) {
      app.reorderOpQueue(drag.fromIdx, dropInsertIdx);
    }
    drag = null;
    draggedIdx = null;
    dropInsertIdx = null;
    document.body.style.cursor = "";
    window.removeEventListener("mousemove", onWindowMouseMove);
    window.removeEventListener("mouseup",  onWindowMouseUp);
  }

  onDestroy(() => {
    window.removeEventListener("mousemove", onWindowMouseMove);
    window.removeEventListener("mouseup",  onWindowMouseUp);
    document.body.style.cursor = "";
  });
</script>

{#if panelVisible}
  <div class="jobs-panel" class:open={app.jobsPanelOpen}>

    <!-- ── Header strip (always visible) ── -->
    <div class="jobs-header" role="toolbar">
      <button class="jobs-toggle" onclick={() => app.toggleJobsPanel()} aria-expanded={app.jobsPanelOpen}>
        <svg class="chevron" width="11" height="11" viewBox="0 0 11 11" fill="none" aria-hidden="true">
          <path d="M2 7.5L5.5 4L9 7.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
        <span class="jobs-title">{t("jobsPanel.title")}</span>
        {#if app.queueRunStats}
          <span class="badge badge--queue" aria-label="queue progress">{app.queueRunStats.completed}/{app.queueRunStats.total}</span>
        {:else}
          {#if hasQueue}
            <span class="badge badge--queue" aria-label="{app.opQueue.length} queued">{app.opQueue.length}</span>
          {/if}
          {#if runningCount > 0}
            <span class="badge" aria-label="{runningCount} running">{runningCount}</span>
          {/if}
        {/if}
        {#if hasConflicts}
          <span class="badge badge--warn" aria-label="conflicts">!</span>
        {/if}
      </button>

      {#if app.jobsPanelOpen && app.opQueue.length > 0}
        <button
          class="header-btn header-btn--accent"
          onclick={() => app.executeQueue("parallel")}
          disabled={!canRunParallel}
          title={app.queueHasDependencies ? t("queue.parallelDisabledDeps") : hasConflicts ? t("queue.conflicts.blocking") : t("queue.runParallel")}
        >{t("queue.runParallel")}</button>
        <button
          class="header-btn header-btn--accent"
          onclick={() => app.executeQueue("sequential")}
          disabled={!canRunSequential}
          title={hasBlockingConflicts ? t("queue.conflicts.blocking") : t("queue.runSequential")}
        >{t("queue.runSequential")}</button>
        <button class="header-btn" onclick={() => app.clearQueue()}>{t("queue.clearQueue")}</button>
      {/if}
      {#if app.jobsPanelOpen && hasJobs}
        <button class="header-btn" onclick={() => app.clearDoneJobs()}>{t("jobsPanel.clearDone")}</button>
      {/if}
    </div>

    <!-- ── Expandable body ── -->
    {#if app.jobsPanelOpen}
      <div class="jobs-body">

        <!-- ── QUEUE SECTION ── -->
        {#if hasQueue}
          <div class="section">
            <div class="section-label">{t("queue.title")} ({app.queueRunStats ? app.queueRunStats.total : app.opQueue.length})</div>

            {#if app.queueRunStats}
              <div class="queue-stats-view">
                <div class="queue-progress-track">
                  <div class="queue-progress-fill" style:width="{Math.round(app.queueRunStats.completed / app.queueRunStats.total * 100)}%"></div>
                </div>
                <div class="queue-stats-row">
                  {#if app.queueRunning}
                    <svg class="spin" width="11" height="11" viewBox="0 0 14 14" fill="none" aria-hidden="true">
                      <circle cx="7" cy="7" r="5.5" stroke="var(--line-strong)" stroke-width="1.5"/>
                      <path d="M7 1.5A5.5 5.5 0 0 1 12.5 7" stroke="var(--accent)" stroke-width="1.5" stroke-linecap="round"/>
                    </svg>
                  {/if}
                  <span class="queue-stat-text">{t("queue.stats.completed", { completed: app.queueRunStats.completed, total: app.queueRunStats.total })}</span>
                  {#if app.queueRunStats.succeeded > 0}
                    <span class="stat-ok">✓{app.queueRunStats.succeeded}</span>
                  {/if}
                  {#if app.queueRunStats.failed > 0}
                    <span class="stat-fail">✗{app.queueRunStats.failed}</span>
                  {/if}
                </div>
              </div>
            {:else}
              <!-- Queue cards list -->
              <div class="queue-list" bind:this={queueListEl}>
                {#each app.opQueue as op, i (op.id)}
                  <!-- svelte-ignore a11y_no_static_element_interactions -->
                  <div
                    class="queue-card"
                    class:queue-card--dragging={draggedIdx === i}
                    class:drop-before={draggedIdx !== null && dropInsertIdx === i}
                    class:drop-after={draggedIdx !== null && dropInsertIdx === i + 1}
                    style:--kind-color={KIND_COLOR[op.kind]}
                    data-kind={op.kind}
                  >
                    <!-- Drag handle -->
                    <!-- svelte-ignore a11y_no_static_element_interactions -->
                    <div
                      class="drag-handle"
                      onmousedown={(e) => onEntryMouseDown(e, i)}
                      title="Drag to reorder"
                      aria-hidden="true"
                    >
                      <svg width="10" height="14" viewBox="0 0 10 14" fill="currentColor">
                        <circle cx="3" cy="3"  r="1.3"/>
                        <circle cx="7" cy="3"  r="1.3"/>
                        <circle cx="3" cy="7"  r="1.3"/>
                        <circle cx="7" cy="7"  r="1.3"/>
                        <circle cx="3" cy="11" r="1.3"/>
                        <circle cx="7" cy="11" r="1.3"/>
                      </svg>
                    </div>

                    <!-- Step block: big number + action -->
                    <div class="op-step" class:op-step--chained={op.dependsOn.length > 0}>
                      <span class="op-step-num">{i + 1}</span>
                      <span class="op-step-kind">{kindLabel(op.kind)}</span>
                    </div>

                    <!-- Content -->
                    <div class="op-content">
                      <div class="op-title">{op.title}</div>
                      {#if dependencyLabel(op)}
                        <div class="op-chain" title={t("queue.parallelDisabledDeps")}>
                          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                            <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/>
                            <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>
                          </svg>
                          <span>{t("queue.chainedAfter", { n: dependencyLabel(op) ?? "" })}</span>
                        </div>
                      {/if}
                      {#if opDisplaySources(op).length > 0}
                        <div class="op-paths">
                          {#each opDisplaySources(op).slice(0, 2) as src}
                            <span class="op-path" title={src}>{basename(src)}</span>
                          {/each}
                          {#if opDisplaySources(op).length > 2}
                            <span class="op-path op-path--more">+{opDisplaySources(op).length - 2} more</span>
                          {/if}
                        </div>
                      {/if}
                      {#if op.destinations.length > 0 && op.kind !== "delete"}
                        <div class="op-dest" title={op.destinations[0]}>→ {basename(op.destinations[0])}</div>
                      {/if}
                    </div>

                    <!-- Controls -->
                    <div class="op-controls">
                      <button
                        class="icon-btn"
                        onclick={() => app.moveOpQueueItem(i, -1)}
                        disabled={i === 0}
                        title="Move up"
                        aria-label="Move up"
                      >
                        <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
                          <path d="M2 7L5 3L8 7" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
                        </svg>
                      </button>
                      <button
                        class="icon-btn"
                        onclick={() => app.moveOpQueueItem(i, 1)}
                        disabled={i === app.opQueue.length - 1}
                        title="Move down"
                        aria-label="Move down"
                      >
                        <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
                          <path d="M2 3L5 7L8 3" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
                        </svg>
                      </button>
                      <button
                        class="icon-btn icon-btn--remove"
                        onclick={() => { const n = app.removeFromQueue(op.id); if (n > 1) app.notify("info", t("queue.removedWithDependents", { count: n })); }}
                        title={t("queue.remove")}
                        aria-label={t("queue.remove")}
                      >
                        <svg width="10" height="10" viewBox="0 0 10 10" fill="none">
                          <path d="M2 2L8 8M8 2L2 8" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
                        </svg>
                      </button>
                    </div>
                  </div>
                {/each}
              </div>

              <!-- Conflicts: blocking first, then parallel-only -->
              {#if hasBlockingConflicts}
                <div class="conflicts-box conflicts-box--blocking">
                  <div class="conflicts-header">
                    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
                      <circle cx="12" cy="12" r="10"/>
                      <line x1="4.93" y1="4.93" x2="19.07" y2="19.07"/>
                    </svg>
                    <strong>{t("queue.conflicts.blocking")}</strong>
                  </div>
                  {#each blockingConflicts as c}
                    {@const p = conflictParts(c)}
                    <div class="conflict-row">
                      <span class="conflict-severity-badge conflict-severity-badge--blocking">{t("queue.conflict.badge.blocking")}</span>
                      <span class="conflict-text">{p.pre}<span class="conflict-op" style:color={p.colorA}>{p.labelA}</span>{p.mid}<span class="conflict-op" style:color={p.colorB}>{p.labelB}</span>{p.suf}</span>
                    </div>
                  {/each}
                </div>
              {/if}

              {#if parallelConflicts.length > 0}
                <div class="conflicts-box conflicts-box--parallel">
                  <div class="conflicts-header">
                    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
                      <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
                      <line x1="12" y1="9" x2="12" y2="13"/>
                      <line x1="12" y1="17" x2="12.01" y2="17"/>
                    </svg>
                    <strong>{t("queue.conflicts.parallelOnly.warn")}</strong>
                    <span class="conflicts-sep" aria-hidden="true">—</span>
                    <span class="conflicts-safe-label">
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" aria-hidden="true">
                        <polyline points="20 6 9 17 4 12"/>
                      </svg>
                      {t("queue.conflicts.parallelOnly.safe")}
                    </span>
                  </div>
                  {#each parallelConflicts as c}
                    {@const p = conflictParts(c)}
                    <div class="conflict-row">
                      <span class="conflict-severity-badge conflict-severity-badge--parallel">{t("queue.conflict.badge.parallelOnly")}</span>
                      <span class="conflict-text">{p.pre}<span class="conflict-op" style:color={p.colorA}>{p.labelA}</span>{p.mid}<span class="conflict-op" style:color={p.colorB}>{p.labelB}</span>{p.suf}</span>
                    </div>
                  {/each}
                </div>
              {/if}
            {/if}
          </div>
        {/if}

        <!-- ── ACTIVE / COMPLETED JOBS ── -->
        {#if hasJobs}
          {#if hasQueue}
            <div class="section-label" style="margin-top: 4px">{t("jobsPanel.title")}</div>
          {/if}

          {#each app.jobs as job (job.id)}
            <div class="job-entry" class:done={job.done} class:failed={job.done && !job.success}>

              <!-- ── Title row ── -->
              <div class="job-row">
                <div class="job-icon" aria-hidden="true">
                  {#if !job.done}
                    <svg class="spin" width="15" height="15" viewBox="0 0 14 14" fill="none">
                      <circle cx="7" cy="7" r="5.5" stroke="var(--line-strong)" stroke-width="1.5"/>
                      <path d="M7 1.5A5.5 5.5 0 0 1 12.5 7" stroke="var(--accent)" stroke-width="1.5" stroke-linecap="round"/>
                    </svg>
                  {:else if job.success}
                    <svg width="15" height="15" viewBox="0 0 14 14" fill="none">
                      <circle cx="7" cy="7" r="5.5" stroke="var(--success, #22c55e)" stroke-width="1.5"/>
                      <path d="M4.5 7L6.2 8.7L9.5 5.5" stroke="var(--success, #22c55e)" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/>
                    </svg>
                  {:else}
                    <svg width="15" height="15" viewBox="0 0 14 14" fill="none">
                      <circle cx="7" cy="7" r="5.5" stroke="var(--error, #ef4444)" stroke-width="1.5"/>
                      <path d="M5 5L9 9M9 5L5 9" stroke="var(--error, #ef4444)" stroke-width="1.5" stroke-linecap="round"/>
                    </svg>
                  {/if}
                </div>
                <span class="job-title">{job.title}</span>
                <span class="job-status-label" class:job-status-label--success={job.done && job.success} class:job-status-label--error={job.done && !job.success}>
                  {#if !job.done}{t("jobsPanel.running")}
                  {:else if job.success}{t("jobsPanel.done")}
                  {:else}{t("jobsPanel.failed")}
                  {/if}
                </span>
                {#if job.done}
                  <button class="job-btn" onclick={() => app.dismissJob(job.id)}>{t("jobsPanel.dismiss")}</button>
                {/if}
              </div>

              {#if !job.done}
                <!-- ── Progress bar (full-width) + percentage ── -->
                <div class="job-progress-bar-row">
                  <div class="progress-track">
                    <div
                      class="progress-fill"
                      class:progress-fill--paused={job.paused}
                      style:width="{job.progress}%"
                    ></div>
                  </div>
                  <span class="job-progress-pct">{Math.round(job.progress)}%</span>
                </div>

                <!-- ── Current item label ── -->
                {#if job.message && !job.paused}
                  <div class="job-current-item">{job.message}</div>
                {/if}

                <!-- ── Pause banner ── -->
                {#if job.paused}
                  <div class="pause-banner" class:pause-banner--unrecoverable={!job.pauseIsRecoverable}>
                    <div class="pause-banner-text">
                      <strong>{t("jobPause.title")}</strong>
                      {#if job.pauseFileName}<span class="pause-file">{job.pauseFileName}</span>{/if}
                      {#if job.pauseError}<span class="pause-error">{job.pauseError}</span>{/if}
                    </div>
                    <div class="pause-actions">
                      {#if job.pauseIsRecoverable}
                        <button class="pause-btn pause-btn--retry" onclick={() => resumeJob(job.id, "retry")}>
                          {t("jobPause.retry")}
                        </button>
                        <button class="pause-btn pause-btn--skip" onclick={() => resumeJob(job.id, "skip")}>
                          {t("jobPause.skip")}
                        </button>
                      {/if}
                      <button class="pause-btn pause-btn--abort" onclick={() => resumeJob(job.id, "abort")}>
                        {t("jobPause.abort")}
                      </button>
                    </div>
                  </div>
                {/if}
              {:else if job.resultMessage}
                <div class="job-result-row"><span class="job-message">{job.resultMessage}</span></div>
              {/if}

              <!-- ── Output log toggle ── -->
              {#if job.logs.length > 0}
                <div class="job-logs-toggle">
                  <button class="job-btn" onclick={() => toggleLogs(job.id)}>
                    {expandedLogs.has(job.id) ? t("jobsPanel.hideOutput") : t("jobsPanel.showOutput")}
                    — {tn("jobsPanel.outputLines", "jobsPanel.outputLines", job.logs.length, { count: String(job.logs.length) })}
                  </button>
                </div>
                {#if expandedLogs.has(job.id)}
                  <div class="job-logs">
                    {#each job.logs as line}
                      <div class="log-line">{line}</div>
                    {/each}
                  </div>
                {/if}
              {/if}

            </div>
          {/each}
        {/if}

      </div>
    {/if}
  </div>
{/if}

<style>
  /* ── Panel shell ── */
  .jobs-panel {
    flex-shrink: 0;
    border-top: 1px solid var(--line);
    background: var(--surface);
    max-height: 340px;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    transition: max-height 0.2s ease;

    &:not(.open) { max-height: 32px; }
  }

  .jobs-header {
    display: flex;
    align-items: center;
    height: 32px;
    flex-shrink: 0;
    padding: 0 8px 0 0;
    gap: 4px;
    overflow: hidden;
  }

  .jobs-toggle {
    display: flex;
    align-items: center;
    gap: 6px;
    flex: 1;
    min-width: 0;
    height: 100%;
    padding: 0 8px;
    background: none;
    border: none;
    cursor: pointer;
    color: var(--text-muted);
    font-size: 12px;
    text-align: left;
    transition: color 0.1s;
    overflow: hidden;

    &:hover { color: var(--text); }
  }

  .chevron {
    flex-shrink: 0;
    transition: transform 0.18s ease;
    .open & { transform: rotate(180deg); }
  }

  .jobs-title {
    font-size: 11px;
    font-weight: 600;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--text-muted);
    white-space: nowrap;
  }

  .badge {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    min-width: 16px;
    height: 16px;
    padding: 0 4px;
    border-radius: 8px;
    background: var(--accent);
    color: #fff;
    font-size: 10px;
    font-weight: 700;
    line-height: 1;
    flex-shrink: 0;
  }

  .badge--queue { background: #8b5cf6; }
  .badge--warn  { background: #f59e0b; }

  .header-btn {
    flex-shrink: 0;
    padding: 2px 7px;
    background: none;
    border: 1px solid var(--line-strong);
    border-radius: 4px;
    cursor: pointer;
    font-size: 11px;
    color: var(--text-muted);
    transition: background 0.1s, color 0.1s;
    white-space: nowrap;

    &:hover:not(:disabled) { background: var(--surface-hover); color: var(--text); }
    &:disabled { opacity: 0.4; cursor: not-allowed; }
  }

  .header-btn--accent {
    border-color: var(--accent);
    color: var(--accent);
    &:hover:not(:disabled) { background: var(--accent-soft); }
  }

  /* ── Body ── */
  .jobs-body {
    flex: 1;
    overflow-y: auto;
    padding: 6px 8px 10px;
    display: flex;
    flex-direction: column;
    gap: 6px;
    scrollbar-width: thin;
    scrollbar-color: var(--line-strong) transparent;
  }

  .section {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .section-label {
    font-size: 10px;
    font-weight: 600;
    letter-spacing: 0.07em;
    text-transform: uppercase;
    color: var(--text-muted);
    padding: 2px 0;
  }

  /* ── Queue list + cards ── */
  .queue-list {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .queue-card {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 7px 8px 7px 0;
    background: var(--surface-alt);
    border: 1px solid var(--line);
    border-left: 3px solid var(--kind-color, var(--accent));
    border-radius: 6px;
    min-width: 0;
    position: relative;
    transition: opacity 0.15s, box-shadow 0.15s;

    &:hover { box-shadow: 0 1px 4px color-mix(in srgb, var(--kind-color) 15%, transparent); }
    &.queue-card--dragging { opacity: 0.35; }

    /* Vertical drop indicators */
    &.drop-before::before,
    &.drop-after::after {
      content: '';
      position: absolute;
      left: -3px;
      right: 0;
      height: 2px;
      background: var(--accent);
      border-radius: 1px;
      pointer-events: none;
    }
    &.drop-before::before { top:    -3px; }
    &.drop-after::after   { bottom: -3px; }
  }

  .drag-handle {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 20px;
    height: 100%;
    padding-left: 6px;
    color: var(--text-muted);
    cursor: grab;
    opacity: 0.4;
    transition: opacity 0.1s;

    &:hover { opacity: 0.8; }
    &:active { cursor: grabbing; }
  }

  /* Step block: prominent number + action on the left of each card */
  .op-step {
    flex-shrink: 0;
    width: 50px;
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 2px;
    padding: 4px 2px;
    border-radius: 5px;
    background: color-mix(in srgb, var(--kind-color) 14%, var(--surface));
    border: 1px solid color-mix(in srgb, var(--kind-color) 28%, transparent);
  }

  .op-step-num {
    font-size: 19px;
    font-weight: 800;
    line-height: 1;
    color: var(--kind-color);
    font-variant-numeric: tabular-nums;
  }

  .op-step-kind {
    font-size: 8.5px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--kind-color);
    white-space: nowrap;
  }

  /* Chained cards get a link glyph on their step block */
  .op-step--chained {
    position: relative;

    &::before {
      content: '';
      position: absolute;
      top: -7px;
      left: 50%;
      transform: translateX(-50%);
      width: 2px;
      height: 7px;
      background: var(--accent);
      border-radius: 1px;
    }
  }

  .op-content {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .op-title {
    font-size: 12px;
    font-weight: 500;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* Chain indicator: clearly ties this op to an earlier step */
  .op-chain {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    align-self: flex-start;
    padding: 2px 8px;
    border-radius: 10px;
    font-size: 10.5px;
    font-weight: 600;
    color: var(--accent);
    background: var(--accent-soft);
    border: 1px solid color-mix(in srgb, var(--accent) 25%, transparent);
    white-space: nowrap;

    svg { flex-shrink: 0; }
  }

  .op-paths {
    display: flex;
    gap: 5px;
    overflow: hidden;
    flex-wrap: nowrap;
    /* Takes full width of .op-content, paths fill as much as they need */
  }

  .op-path {
    font-size: 10.5px;
    color: var(--text-muted);
    /* Shrinks when space is tight, but never grows beyond content width */
    flex: 0 1 auto;
    min-width: 30px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;

    &.op-path--more {
      /* "+N more" badge: fixed size, never shrinks */
      flex: 0 0 auto;
      opacity: 0.7;
    }
  }

  .op-dest {
    font-size: 10.5px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    opacity: 0.8;
  }

  /* Up/Down/Remove controls */
  .op-controls {
    display: flex;
    flex-direction: column;
    gap: 2px;
    flex-shrink: 0;
    padding-right: 2px;
  }

  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    border-radius: 3px;
    background: none;
    border: none;
    cursor: pointer;
    color: var(--text-muted);
    padding: 0;
    transition: background 0.1s, color 0.1s;

    &:hover:not(:disabled) { background: var(--surface-hover); color: var(--text); }
    &:disabled { opacity: 0.25; cursor: default; }
    &.icon-btn--remove:hover:not(:disabled) { color: var(--error, #ef4444); background: color-mix(in srgb, var(--error, #ef4444) 10%, transparent); }
  }

  /* ── Conflicts ── */
  .conflicts-box {
    padding: 8px 10px;
    border-radius: 6px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .conflicts-box--blocking {
    background: color-mix(in srgb, #ef4444 8%, var(--surface-alt));
    border: 1px solid color-mix(in srgb, #ef4444 30%, var(--line));

    .conflicts-header { color: #991b1b; svg { stroke: #dc2626; } }
    .conflict-row     { color: var(--text, #111); }
  }

  .conflicts-box--parallel {
    background: color-mix(in srgb, #f59e0b 8%, var(--surface-alt));
    border: 1px solid color-mix(in srgb, #f59e0b 35%, var(--line));

    .conflicts-header { color: #92400e; svg { stroke: #b45309; } }
    .conflict-row     { color: var(--text, #111); }
  }

  /* Override header's svg rule for the safe-label checkmark */
  .conflicts-header .conflicts-safe-label svg { stroke: var(--success, #16a34a); }

  .conflicts-header {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11.5px;
    font-weight: 600;
    flex-wrap: wrap;
    row-gap: 3px;

    svg { flex-shrink: 0; }
  }

  .conflicts-sep {
    color: inherit;
    opacity: 0.6;
    font-weight: 400;
    margin: 0 2px;
    flex-shrink: 0;
  }

  .conflicts-safe-label {
    display: inline-flex;
    align-items: center;
    gap: 4px;
    font-size: 11px;
    font-weight: 600;
    color: var(--success, #16a34a);
    white-space: nowrap;
    flex-shrink: 0;

    svg { stroke: var(--success, #16a34a); flex-shrink: 0; }
  }

  .conflict-row {
    display: flex;
    gap: 7px;
    font-size: 11.5px;
    font-weight: 600;
    line-height: 1.55;
    padding-left: 2px;
    align-items: baseline;
    flex-wrap: wrap;
  }

  .conflict-severity-badge {
    flex-shrink: 0;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    padding: 1px 5px;
    border-radius: 3px;
    white-space: nowrap;
    line-height: 1.6;
  }

  .conflict-severity-badge--blocking {
    background: color-mix(in srgb, #ef4444 15%, transparent);
    color: #dc2626;
    border: 1px solid color-mix(in srgb, #ef4444 30%, transparent);
  }

  .conflict-severity-badge--parallel {
    background: color-mix(in srgb, #f59e0b 15%, transparent);
    color: #b45309;
    border: 1px solid color-mix(in srgb, #f59e0b 30%, transparent);
  }

  .conflict-text {
    flex: 1;
    min-width: 0;
    word-break: break-word;
  }

  .conflict-op {
    font-weight: 800;
    /* color set via inline style:color binding */
  }

  /* ── Active / completed jobs ── */
  .job-entry {
    background: var(--surface-alt);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 9px 11px;
    display: flex;
    flex-direction: column;
    gap: 6px;

    &.failed { border-color: color-mix(in srgb, var(--error, #ef4444) 40%, var(--line)); }
  }

  .job-row {
    display: flex;
    align-items: center;
    gap: 8px;
    min-width: 0;
  }

  .job-icon {
    flex-shrink: 0;
    display: flex;
    align-items: center;
  }

  .spin { animation: spin 1.2s linear infinite; }

  @keyframes spin { to { transform: rotate(360deg); } }

  .job-title {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    font-weight: 600;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .job-status-label {
    flex-shrink: 0;
    font-size: 11px;
    font-weight: 500;
    color: var(--text-muted);

    &.job-status-label--success { color: var(--success, #16a34a); }
    &.job-status-label--error   { color: var(--error, #ef4444); }
  }

  .job-btn {
    flex-shrink: 0;
    padding: 2px 7px;
    background: none;
    border: 1px solid var(--line-strong);
    border-radius: 3px;
    cursor: pointer;
    font-size: 11px;
    color: var(--text-muted);
    white-space: nowrap;
    transition: background 0.1s, color 0.1s;

    &:hover { background: var(--surface-hover); color: var(--text); }
  }

  /* Full-width progress bar with percentage label */
  .job-progress-bar-row {
    display: flex;
    align-items: center;
    gap: 8px;
  }

  .progress-track {
    flex: 1;
    height: 6px;
    background: var(--line-strong);
    border-radius: 3px;
    overflow: hidden;
    min-width: 60px;
  }

  .progress-fill {
    height: 100%;
    background: var(--accent);
    border-radius: 3px;
    transition: width 0.3s ease;
  }

  .job-progress-pct {
    flex-shrink: 0;
    min-width: 36px;
    text-align: right;
    font-size: 12px;
    font-weight: 600;
    color: var(--text-muted);
  }

  /* Current item label — separate row below the bar */
  .job-current-item {
    font-size: 11.5px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    padding-left: 1px;
  }

  .job-result-row { display: flex; align-items: center; }

  .job-message {
    flex: 1;
    font-size: 11.5px;
    color: var(--text-muted);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .job-logs-toggle { display: flex; }

  .job-logs {
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: 4px;
    padding: 6px 8px;
    max-height: 100px;
    overflow-y: auto;
    scrollbar-width: thin;
  }

  .log-line {
    font-family: var(--font-mono);
    font-size: 10.5px;
    color: var(--text-muted);
    line-height: 1.6;
    white-space: pre-wrap;
    word-break: break-all;
  }

  .progress-fill--paused {
    background: #f59e0b;
    animation: none;
  }

  .pause-banner {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 8px 10px;
    border-radius: 5px;
    background: color-mix(in srgb, #f59e0b 10%, var(--surface-alt));
    border: 1px solid color-mix(in srgb, #f59e0b 40%, var(--line));

    &.pause-banner--unrecoverable {
      background: color-mix(in srgb, #ef4444 10%, var(--surface-alt));
      border-color: color-mix(in srgb, #ef4444 40%, var(--line));
    }
  }

  .pause-banner-text {
    display: flex;
    flex-direction: column;
    gap: 2px;
    font-size: 11px;

    strong { color: var(--text); font-size: 11.5px; }
  }

  .pause-file {
    color: var(--text-muted);
    font-family: var(--font-mono);
    font-size: 10.5px;
    word-break: break-all;
  }

  .pause-error {
    color: var(--text-muted);
    font-size: 10.5px;
    white-space: pre-wrap;
    word-break: break-all;
  }

  .pause-actions {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .pause-btn {
    padding: 3px 10px;
    border-radius: 4px;
    font-size: 11px;
    font-weight: 600;
    cursor: pointer;
    border: 1px solid transparent;
    transition: background 0.1s, color 0.1s;
  }

  .pause-btn--retry {
    background: var(--accent-soft, color-mix(in srgb, var(--accent) 15%, transparent));
    color: var(--accent);
    border-color: var(--accent);
    &:hover { background: var(--accent); color: #fff; }
  }

  .pause-btn--skip {
    background: color-mix(in srgb, #f59e0b 12%, transparent);
    color: #b45309;
    border-color: color-mix(in srgb, #f59e0b 40%, transparent);
    &:hover { background: #f59e0b; color: #fff; }
  }

  .pause-btn--abort {
    background: color-mix(in srgb, #ef4444 10%, transparent);
    color: #dc2626;
    border-color: color-mix(in srgb, #ef4444 30%, transparent);
    &:hover { background: #ef4444; color: #fff; }
  }

  /* ── Queue run stats ── */
  .queue-stats-view {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 4px 0;
  }

  .queue-progress-track {
    height: 6px;
    background: var(--line-strong);
    border-radius: 3px;
    overflow: hidden;
  }

  .queue-progress-fill {
    height: 100%;
    background: var(--accent);
    border-radius: 3px;
    transition: width 0.4s ease;
  }

  .queue-stats-row {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 12px;
    color: var(--text-muted);
  }

  .queue-stat-text { flex: 1; min-width: 0; }

  .stat-ok   { font-size: 11px; font-weight: 700; color: var(--success, #16a34a); }
  .stat-fail { font-size: 11px; font-weight: 700; color: var(--error, #ef4444); }
</style>
