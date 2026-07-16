<script lang="ts">
  import { onDestroy } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { app } from "../../stores/app.svelte.js";
  import { t, tn } from "../../i18n/index.js";
  import type { QueueConflict, QueuedOp, QueuedOpKind } from "../../types/index.js";
  import { collectAccentIssues, isAccentUnsafeKind } from "../../utils/ascii.js";
  import { normalizePath } from "../../utils/ghosts.js";

  let { onDeaccentOp }: { onDeaccentOp?: (op: QueuedOp) => void } = $props();

  const MAX_VISIBLE_WAVES = 4;
  const MAX_VISIBLE_WAVE_STEPS = 6;

  /** True when this op is blocked by accented file names its tool can't process. */
  function opAccentBlocked(op: QueuedOp): boolean {
    return isAccentUnsafeKind(op.kind) && collectAccentIssues(op.sources).length > 0;
  }

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
  const hasQueueWarnings    = $derived(hasConflicts || app.queueHasDependencies);
  const retryableFailedJobs = $derived(app.jobs.filter(j => j.done && !j.success && !!j.retryQueuedOp && !j.retried));
  const canRetryAllFailed   = $derived(retryableFailedJobs.length > 0 && !app.queueRunning && !app.failedQueueRetryRunning);
  const hasAccentBlocks     = $derived(app.queueAccentBlocked.size > 0);
  const canRunSmart         = $derived(hasQueue && !hasBlockingConflicts && !hasAccentBlocks && !app.queueRunning);
  // Sequential is safe even with parallel-only conflicts (queue order protects it)
  const canRunSequential    = $derived(hasQueue && !hasBlockingConflicts && !hasAccentBlocks && !app.queueRunning);

  // ── Kind metadata ─────────────────────────────────────────
  const KIND_COLOR: Record<QueuedOpKind, string> = {
    copy:         "#3b82f6",
    move:         "#f59e0b",
    delete:       "#ef4444",
    extract:      "#8b5cf6",
    compress:     "#10b981",
    "chd-convert":"#0d9488",
    "chd-restore":"#0d9488",
    "cso-convert":"#2563eb",
    "cso-restore":"#2563eb",
    "xiso-pack":"#f97316",
    "xiso-unpack":"#f97316",
    "rvz-convert":"#7c3aed",
    "rvz-restore":"#7c3aed",
    m3u:          "#d946ef",
  };

  function kindLabel(k: QueuedOpKind): string {
    const map: Record<QueuedOpKind, Parameters<typeof t>[0]> = {
      copy: "queue.kind.copy", move: "queue.kind.move", delete: "queue.kind.delete",
      extract: "queue.kind.extract", compress: "queue.kind.compress",
      "chd-convert": "queue.kind.chdConvert", "chd-restore": "queue.kind.chdRestore",
      "cso-convert": "queue.kind.csoConvert", "cso-restore": "queue.kind.csoRestore",
      "xiso-pack": "queue.kind.xisoPack", "xiso-unpack": "queue.kind.xisoUnpack",
      "rvz-convert": "queue.kind.rvzConvert", "rvz-restore": "queue.kind.rvzRestore",
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

  function batchLabel(op: QueuedOp): string | null {
    if (!op.batchId || !op.batchTitle || !op.batchIndex || !op.batchTotal) return null;
    return t("queue.batch.item", {
      title: op.batchTitle,
      index: op.batchIndex,
      total: op.batchTotal,
    });
  }

  function opById(id: string): QueuedOp | undefined {
    return app.opQueue.find(o => o.id === id);
  }

  function stepNumberForId(id: string): number {
    return app.opQueue.findIndex(o => o.id === id) + 1;
  }

  function waveStepTitle(id: string): string {
    const op = opById(id);
    const step = stepNumberForId(id);
    return op && step > 0 ? `${step}. ${op.title}` : "";
  }

  function waveStepColor(id: string): string {
    const op = opById(id);
    return op ? KIND_COLOR[op.kind] : "var(--accent)";
  }

  function visiblePlanLevels(): string[][] {
    return app.queuePlan.levels.slice(0, MAX_VISIBLE_WAVES);
  }

  function basename(p: string): string {
    return p.split(/[/\\]/).filter(Boolean).pop() ?? p;
  }

  function pathContainsOrEquals(parent: string, child: string): boolean {
    const p = normalizePath(parent);
    const c = normalizePath(child);
    return c === p || (p.length > 0 && c.startsWith(`${p}/`));
  }

  function opRemovesPath(op: QueuedOp, path: string): boolean {
    return op.deletes.some((deletePath) => pathContainsOrEquals(deletePath, path));
  }

  type OperationFlow = {
    source: string | null;
    target: string | null;
    extraTargets: number;
  };

  function opFlowRows(op: QueuedOp): OperationFlow[] {
    const sources = opDisplaySources(op);
    const targets = app.resolvedOutputsFor(op.id).map((g) => g.path);
    if (sources.length === 0 && targets.length === 0) return [];
    if (sources.length === 0) {
      return targets.slice(0, 2).map((target) => ({ source: null, target, extraTargets: 0 }));
    }
    if (targets.length === 0) {
      return sources.slice(0, 2).map((source) => ({ source, target: null, extraTargets: 0 }));
    }
    if (sources.length === 1) {
      return [{
        source: sources[0],
        target: targets[0],
        extraTargets: Math.max(0, targets.length - 1),
      }];
    }
    return sources.slice(0, 2).map((source, index) => ({
      source,
      target: targets[index] ?? null,
      extraTargets: 0,
    }));
  }

  function opHiddenSourceCount(op: QueuedOp): number {
    return Math.max(0, opDisplaySources(op).length - 2);
  }

  function flowTitle(flow: OperationFlow): string {
    const source = flow.source ?? "";
    const target = flow.target ?? "";
    if (source && target) return flow.extraTargets > 0 ? `${source} -> ${target} +${flow.extraTargets}` : `${source} -> ${target}`;
    return source || target;
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
        {#if hasQueueWarnings}
          <span class="badge badge--warn" aria-label="queue warnings">!</span>
        {/if}
      </button>

      {#if app.jobsPanelOpen && (app.opQueue.length > 0 || hasJobs)}
        <div class="jobs-actions" aria-label="Queue actions">
          {#if app.opQueue.length > 0}
            <div class="jobs-run-actions">
              <button
                class="header-btn header-btn--primary"
                onclick={() => app.executeQueue("smart")}
                disabled={!canRunSmart}
                title={hasBlockingConflicts ? t("queue.conflicts.blocking") : t("queue.runSmart")}
              >
                <svg width="10" height="10" viewBox="0 0 10 10" fill="currentColor" aria-hidden="true">
                  <path d="M3 1.8v6.4L8 5 3 1.8z"/>
                </svg>
                {t("queue.runSmart")}
              </button>
              <button
                class="header-btn header-btn--secondary"
                onclick={() => app.executeQueue("sequential")}
                disabled={!canRunSequential}
                title={hasBlockingConflicts ? t("queue.conflicts.blocking") : t("queue.runSequential")}
              >{t("queue.runSequential")}</button>
            </div>
            <div class="jobs-action-sep" aria-hidden="true"></div>
            <button class="header-btn header-btn--danger-ghost" onclick={() => app.clearQueue()}>{t("queue.clearQueue")}</button>
          {/if}
          {#if retryableFailedJobs.length > 0}
            <button
              class="header-btn header-btn--ghost"
              onclick={() => void app.retryAllFailedJobs()}
              disabled={!canRetryAllFailed}
            >{t("jobsPanel.retryAllFailed", { count: retryableFailedJobs.length })}</button>
          {/if}
          {#if hasJobs}
            <button class="header-btn header-btn--ghost" onclick={() => app.clearDoneJobs()}>{t("jobsPanel.clearDone")}</button>
          {/if}
        </div>
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
                  {#if app.queueRunStats.running > 0}
                    <span class="stat-running">{t("queue.stats.running", { count: app.queueRunStats.running })}</span>
                  {/if}
                  {#if app.queueRunStats.succeeded > 0}
                    <span class="stat-ok">✓{app.queueRunStats.succeeded}</span>
                  {/if}
                  {#if app.queueRunStats.failed > 0}
                    <span class="stat-fail">✗{app.queueRunStats.failed}</span>
                  {/if}
                  {#if app.queueRunStats.skipped > 0}
                    <span class="stat-skip">↷{app.queueRunStats.skipped}</span>
                  {/if}
                </div>
              </div>
            {:else}
              {#if app.queuePlan.total > 1 && !hasBlockingConflicts}
                <div class="queue-plan-summary">
                  <div class="queue-plan-copy">
                    <strong>{t("queue.smartPlanTitle")}</strong>
                    <span>{t("queue.smartPlan", {
                      first: app.queuePlan.firstWave,
                      total: app.queuePlan.total,
                      lanes: app.queuePlan.maxConcurrent,
                    })}</span>
                  </div>
                  <div class="queue-wave-map" aria-label={t("queue.wavePlanLabel")}>
                    {#each visiblePlanLevels() as wave, waveIdx}
                      <div class="queue-wave">
                        <span class="queue-wave-label">{t("queue.wave", { n: waveIdx + 1 })}</span>
                        <div class="queue-wave-steps">
                          {#each wave.slice(0, MAX_VISIBLE_WAVE_STEPS) as opId}
                            {#if stepNumberForId(opId) > 0}
                              <span
                                class="queue-wave-step"
                                style:--wave-color={waveStepColor(opId)}
                                title={waveStepTitle(opId)}
                              >{stepNumberForId(opId)}</span>
                            {/if}
                          {/each}
                          {#if wave.length > MAX_VISIBLE_WAVE_STEPS}
                            <span class="queue-wave-more">{t("queue.moreSteps", { count: wave.length - MAX_VISIBLE_WAVE_STEPS })}</span>
                          {/if}
                        </div>
                      </div>
                      {#if waveIdx < visiblePlanLevels().length - 1}
                        <span class="queue-wave-arrow" aria-hidden="true">→</span>
                      {/if}
                    {/each}
                    {#if app.queuePlan.levels.length > MAX_VISIBLE_WAVES}
                      <span class="queue-wave-more queue-wave-more--levels">
                        {t("queue.moreWaves", { count: app.queuePlan.levels.length - MAX_VISIBLE_WAVES })}
                      </span>
                    {/if}
                  </div>
                </div>
              {/if}

              <!-- Queue cards list -->
              <div class="queue-list" bind:this={queueListEl}>
                {#each app.opQueue as op, i (op.id)}
                  <!-- svelte-ignore a11y_no_static_element_interactions -->
                  <div
                    class="queue-card"
                    class:queue-card--chained={op.dependsOn.length > 0}
                    class:queue-card--blocked={opAccentBlocked(op)}
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
                    <div class="op-step" class:op-step--chained={op.dependsOn.length > 0} class:op-step--batched={!!op.batchId}>
                      <span class="op-step-num">{i + 1}</span>
                      <span class="op-step-kind">{kindLabel(op.kind)}</span>
                    </div>

                    <!-- Content -->
                    <div class="op-content">
                      <div class="op-header">
                        <div class="op-title" title={op.title}>{op.title}</div>
                        <div class="op-meta">
                          {#if batchLabel(op)}
                            <div class="op-batch" title={op.batchTitle}>
                              <span class="op-batch-mark" aria-hidden="true"></span>
                              <span>{batchLabel(op)}</span>
                            </div>
                          {/if}
                          {#if dependencyLabel(op)}
                            <div class="op-chain" title={t("queue.parallelDisabledDeps")} aria-label={t("queue.parallelDisabledDeps")}>
                              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" stroke-linecap="round" stroke-linejoin="round" aria-hidden="true">
                                <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/>
                                <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>
                              </svg>
                              <span>{t("queue.chainedAfter", { n: dependencyLabel(op) ?? "" })}</span>
                            </div>
                          {/if}
                        </div>
                      </div>
                      {#if opFlowRows(op).length > 0}
                        <div class="op-flows">
                          {#each opFlowRows(op) as flow}
                            <div class="op-flow" title={flowTitle(flow)}>
                              {#if flow.source}
                                <span
                                  class="op-flow-name op-flow-source"
                                  class:op-flow-name--removed={opRemovesPath(op, flow.source)}
                                >{basename(flow.source)}</span>
                              {/if}
                              {#if flow.target}
                                {#if flow.source}
                                  <span class="op-flow-arrow" aria-hidden="true">-&gt;</span>
                                {/if}
                                <span class="op-flow-name op-flow-target">{basename(flow.target)}</span>
                                {#if flow.extraTargets > 0}
                                  <span class="op-flow-more">+{flow.extraTargets}</span>
                                {/if}
                              {/if}
                            </div>
                          {/each}
                          {#if opHiddenSourceCount(op) > 0}
                            <span class="op-flow-more">+{opHiddenSourceCount(op)}</span>
                          {/if}
                        </div>
                      {/if}
                      {#if opFlowRows(op).length === 0 && op.destinations.length > 0 && op.kind !== "delete"}
                        <div class="op-flows">
                          <div class="op-flow" title={op.destinations[0]}>
                            <span class="op-flow-arrow" aria-hidden="true">-&gt;</span>
                            <span class="op-flow-name op-flow-target">{basename(op.destinations[0])}</span>
                          </div>
                        </div>
                      {/if}
                      {#if opAccentBlocked(op)}
                        <div class="op-accent-block">
                          <span class="op-accent-msg">⚠ {t("accents.queueBlocked")}</span>
                          <button class="op-deaccent-btn" onclick={() => onDeaccentOp?.(op)}>
                            {t("accents.deaccentAction")}
                          </button>
                        </div>
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

              {#if app.queueHasDependencies || app.queuePlan.hasOrderingConstraints}
                <div class="conflicts-box conflicts-box--dependencies">
                  <div class="conflicts-header">
                    <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
                      <path d="M10 13a5 5 0 0 0 7.54.54l3-3a5 5 0 0 0-7.07-7.07l-1.72 1.71"/>
                      <path d="M14 11a5 5 0 0 0-7.54-.54l-3 3a5 5 0 0 0 7.07 7.07l1.71-1.71"/>
                    </svg>
                    <strong>{t("queue.dependencies.sequentialOnly")}</strong>
                    <span class="conflicts-sep" aria-hidden="true">—</span>
                    <span class="conflicts-safe-label">
                      <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5" aria-hidden="true">
                        <polyline points="20 6 9 17 4 12"/>
                      </svg>
                      {t("queue.dependencies.sequentialSafe")}
                    </span>
                  </div>
                  <div class="conflict-row">
                    <span class="conflict-severity-badge conflict-severity-badge--dependency">{t("queue.conflict.badge.dependency")}</span>
                    <span class="conflict-text">{t("queue.dependencies.description", {
                      dependencies: app.queuePlan.dependencyEdges,
                      ordered: app.queuePlan.orderingEdges,
                    })}</span>
                  </div>
                </div>
              {/if}

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
                  {#if !job.success && job.retryQueuedOp && !job.retried}
                    <button
                      class="job-btn"
                      onclick={() => app.retryFailedJob(job.id)}
                      disabled={app.queueRunning || app.failedQueueRetryRunning}
                    >{t("jobsPanel.retry")}</button>
                  {/if}
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
    padding: 0 7px 0 0;
    gap: 6px;
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

  .jobs-actions {
    display: flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
    flex-shrink: 0;
  }

  .jobs-run-actions {
    display: flex;
    align-items: center;
    gap: 4px;
    flex-shrink: 0;
    padding: 2px;
    border-radius: 7px;
    background: color-mix(in srgb, var(--surface-alt) 82%, transparent);
    border: 1px solid color-mix(in srgb, var(--line) 78%, transparent);
  }

  .jobs-action-sep {
    width: 1px;
    height: 18px;
    background: var(--line);
    margin: 0 1px;
    flex-shrink: 0;
  }

  .header-btn {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    gap: 5px;
    flex-shrink: 0;
    min-height: 22px;
    padding: 3px 8px;
    background: none;
    border: 1px solid var(--line-strong);
    border-radius: 5px;
    cursor: pointer;
    font-size: 11px;
    font-weight: 650;
    color: var(--text-muted);
    transition: background 0.1s, color 0.1s, border-color 0.1s, box-shadow 0.1s;
    white-space: nowrap;

    &:hover:not(:disabled) { background: var(--surface-hover); color: var(--text); }
    &:disabled { opacity: 0.4; cursor: not-allowed; }
  }

  .header-btn--primary {
    background: var(--accent);
    border-color: color-mix(in srgb, var(--accent) 82%, #000);
    color: #fff;
    box-shadow: 0 1px 5px color-mix(in srgb, var(--accent) 26%, transparent);

    &:hover:not(:disabled) {
      background: var(--accent-light);
      border-color: var(--accent-light);
      color: #fff;
    }
  }

  .header-btn--secondary {
    background: color-mix(in srgb, var(--accent) 9%, transparent);
    border-color: color-mix(in srgb, var(--accent) 44%, var(--line));
    color: var(--accent);
    &:hover:not(:disabled) { background: var(--accent-soft); }
  }

  .header-btn--ghost {
    border-color: transparent;
    background: transparent;
  }

  .header-btn--danger-ghost {
    border-color: color-mix(in srgb, var(--error, #ef4444) 24%, transparent);
    color: color-mix(in srgb, var(--error, #ef4444) 70%, var(--text-muted));
    background: color-mix(in srgb, var(--error, #ef4444) 6%, transparent);

    &:hover:not(:disabled) {
      background: color-mix(in srgb, var(--error, #ef4444) 12%, transparent);
      border-color: color-mix(in srgb, var(--error, #ef4444) 38%, transparent);
      color: var(--error, #ef4444);
    }
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

  .queue-plan-summary {
    display: flex;
    flex-direction: column;
    gap: 8px;
    padding: 9px 10px;
    border-radius: 8px;
    background:
      radial-gradient(circle at 12% 0%, color-mix(in srgb, var(--accent) 16%, transparent), transparent 36%),
      linear-gradient(135deg, color-mix(in srgb, var(--accent) 9%, var(--surface-alt)), var(--surface-alt));
    border: 1px solid color-mix(in srgb, var(--accent) 28%, var(--line));
    color: var(--text-muted);
    font-size: 11.5px;
    font-weight: 600;
  }

  .queue-plan-copy {
    display: flex;
    align-items: baseline;
    gap: 7px;
    flex-wrap: wrap;

    strong {
      color: var(--text);
      font-size: 11px;
      letter-spacing: 0.06em;
      text-transform: uppercase;
    }
  }

  .queue-wave-map {
    display: flex;
    align-items: center;
    gap: 7px;
    overflow-x: auto;
    padding-bottom: 1px;
    scrollbar-width: thin;
  }

  .queue-wave {
    display: flex;
    align-items: center;
    gap: 6px;
    flex-shrink: 0;
    padding: 5px 7px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--surface) 82%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent) 16%, var(--line));
  }

  .queue-wave-label {
    font-size: 9px;
    font-weight: 800;
    letter-spacing: 0.06em;
    text-transform: uppercase;
    color: var(--text-muted);
    white-space: nowrap;
  }

  .queue-wave-steps {
    display: flex;
    align-items: center;
    gap: 3px;
  }

  .queue-wave-step {
    display: inline-flex;
    align-items: center;
    justify-content: center;
    width: 19px;
    height: 19px;
    border-radius: 999px;
    color: color-mix(in srgb, var(--wave-color) 82%, var(--text));
    background: color-mix(in srgb, var(--wave-color) 13%, var(--surface));
    border: 1px solid color-mix(in srgb, var(--wave-color) 38%, transparent);
    font-size: 10px;
    font-weight: 850;
    font-variant-numeric: tabular-nums;
  }

  .queue-wave-arrow {
    flex-shrink: 0;
    color: color-mix(in srgb, var(--accent) 58%, var(--text-muted));
    font-size: 13px;
    font-weight: 800;
    opacity: 0.82;
  }

  .queue-wave-more {
    flex-shrink: 0;
    padding: 2px 6px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--text-muted) 9%, transparent);
    color: var(--text-muted);
    font-size: 9.5px;
    font-weight: 750;
    white-space: nowrap;
  }

  .queue-wave-more--levels {
    border: 1px dashed color-mix(in srgb, var(--accent) 32%, var(--line));
    background: color-mix(in srgb, var(--accent) 6%, transparent);
  }

  .queue-card {
    display: flex;
    align-items: center;
    gap: 7px;
    padding: 8px 8px 8px 0;
    background: var(--surface-alt);
    border: 1px solid var(--line);
    border-left: 3px solid var(--kind-color, var(--accent));
    border-radius: 6px;
    min-width: 0;
    position: relative;
    transition: opacity 0.15s, box-shadow 0.15s, border-color 0.15s, background 0.15s;

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

  .queue-card--chained {
    border-color: color-mix(in srgb, var(--accent) 28%, var(--line));
    border-left-color: color-mix(in srgb, var(--accent) 88%, var(--kind-color));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 6%, transparent);

    &:hover {
      box-shadow:
        inset 0 0 0 1px color-mix(in srgb, var(--accent) 8%, transparent),
        0 1px 5px color-mix(in srgb, var(--accent) 12%, transparent);
    }
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

  .op-step--chained {
    border-color: color-mix(in srgb, var(--accent) 42%, transparent);
    box-shadow: inset 0 -2px 0 color-mix(in srgb, var(--accent) 44%, transparent);
  }

  .op-step--batched {
    box-shadow: inset 0 -2px 0 color-mix(in srgb, var(--kind-color) 42%, transparent);
  }

  .op-content {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .op-header {
    display: flex;
    align-items: center;
    gap: 7px;
    min-width: 0;
    flex-wrap: wrap;
  }

  .op-title {
    flex: 0 1 auto;
    min-width: 130px;
    max-width: 100%;
    font-size: 12.5px;
    font-weight: 650;
    color: var(--text);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .op-meta {
    display: flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
    flex: 999 1 260px;
    flex-wrap: wrap;
  }

  .op-batch {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    align-self: flex-start;
    max-width: 100%;
    padding: 2px 7px;
    border-radius: 999px;
    font-size: 10px;
    font-weight: 700;
    letter-spacing: 0.01em;
    color: color-mix(in srgb, var(--kind-color) 72%, var(--text));
    background: color-mix(in srgb, var(--kind-color) 11%, transparent);
    border: 1px solid color-mix(in srgb, var(--kind-color) 24%, transparent);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .op-batch-mark {
    width: 6px;
    height: 6px;
    border-radius: 999px;
    background: var(--kind-color);
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--kind-color) 14%, transparent);
    flex-shrink: 0;
  }

  /* Chain indicator: clearly ties this op to an earlier step */
  .op-chain {
    display: inline-flex;
    align-items: center;
    gap: 6px;
    align-self: flex-start;
    padding: 2px 8px;
    border-radius: 999px;
    font-size: 10.5px;
    font-weight: 750;
    color: color-mix(in srgb, var(--accent) 82%, var(--text));
    background: color-mix(in srgb, var(--accent) 10%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent) 30%, transparent);
    white-space: nowrap;
    max-width: 100%;
    min-width: 0;

    svg {
      flex-shrink: 0;
      width: 12px;
      height: 12px;
    }

    span {
      min-width: 0;
      overflow: hidden;
      text-overflow: ellipsis;
    }
  }

  .op-flows {
    display: flex;
    align-items: center;
    gap: 6px;
    overflow: hidden;
    flex-wrap: wrap;
    min-width: 0;
  }

  .op-flow {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    min-width: 0;
    max-width: 100%;
    flex: 0 1 auto;
    font-size: 10.5px;
    color: var(--text-muted);
  }

  .op-flow-name {
    flex: 0 1 auto;
    min-width: 30px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .op-flow-source {
    color: var(--text-muted);
  }

  .op-flow-target {
    color: color-mix(in srgb, var(--text-muted) 82%, var(--text));
  }

  .op-flow-arrow {
    flex: 0 0 auto;
    color: color-mix(in srgb, var(--kind-color) 70%, var(--text-muted));
    font-weight: 700;
  }

  .op-flow-more {
    flex: 0 0 auto;
    color: var(--text-subtle);
    opacity: 0.78;
    font-size: 10.5px;
    white-space: nowrap;
  }

  .op-flow-name--removed {
    color: color-mix(in srgb, var(--danger, #e5484d) 72%, var(--text-muted));
    text-decoration: line-through;
    text-decoration-thickness: 1.5px;
  }

  .queue-card--blocked {
    border-color: color-mix(in srgb, var(--danger) 45%, var(--line-strong));
    background: color-mix(in srgb, var(--danger) 7%, transparent);
  }

  .op-accent-block {
    display: flex;
    align-items: center;
    gap: 8px;
    margin-top: 4px;
    flex-wrap: wrap;
  }

  .op-accent-msg {
    font-size: 10.5px;
    color: var(--danger);
    font-weight: 600;
  }

  .op-deaccent-btn {
    padding: 2px 8px;
    background: var(--danger);
    border: none;
    border-radius: 4px;
    color: #fff;
    font-size: 10.5px;
    font-weight: 600;
    cursor: pointer;

    &:hover { filter: brightness(1.08); }
  }

  /* Up/Down/Remove controls */
  .op-controls {
    display: flex;
    flex-direction: column;
    gap: 3px;
    flex-shrink: 0;
    padding-right: 1px;
  }

  .icon-btn {
    display: flex;
    align-items: center;
    justify-content: center;
    width: 22px;
    height: 20px;
    border-radius: 5px;
    background: color-mix(in srgb, var(--surface) 72%, transparent);
    border: 1px solid transparent;
    cursor: pointer;
    color: var(--text-muted);
    padding: 0;
    transition: background 0.1s, color 0.1s, border-color 0.1s;

    &:hover:not(:disabled) {
      background: var(--surface-hover);
      border-color: var(--line);
      color: var(--text);
    }
    &:disabled { opacity: 0.25; cursor: default; }
    &.icon-btn--remove {
      color: color-mix(in srgb, var(--error, #ef4444) 68%, var(--text-muted));
      background: color-mix(in srgb, var(--error, #ef4444) 5%, transparent);
    }
    &.icon-btn--remove:hover:not(:disabled) {
      color: var(--error, #ef4444);
      border-color: color-mix(in srgb, var(--error, #ef4444) 25%, transparent);
      background: color-mix(in srgb, var(--error, #ef4444) 11%, transparent);
    }
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

  .conflicts-box--dependencies {
    background: color-mix(in srgb, var(--accent) 8%, var(--surface-alt));
    border: 1px solid color-mix(in srgb, var(--accent) 30%, var(--line));

    .conflicts-header { color: var(--accent); svg { stroke: var(--accent); } }
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

  .conflict-severity-badge--dependency {
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    color: var(--accent);
    border: 1px solid color-mix(in srgb, var(--accent) 30%, transparent);
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
    gap: 7px;
    padding: 8px 9px;
    border-radius: 8px;
    background:
      radial-gradient(circle at 15% -20%, color-mix(in srgb, var(--accent) 16%, transparent), transparent 48%),
      var(--surface-alt);
    border: 1px solid color-mix(in srgb, var(--accent) 18%, var(--line));
  }

  .queue-progress-track {
    height: 7px;
    background: color-mix(in srgb, var(--line-strong) 78%, transparent);
    border-radius: 999px;
    overflow: hidden;
  }

  .queue-progress-fill {
    height: 100%;
    background: linear-gradient(90deg, var(--accent), color-mix(in srgb, var(--accent) 74%, #fff));
    border-radius: 999px;
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
  .stat-skip { font-size: 11px; font-weight: 700; color: #b45309; }
  .stat-running {
    padding: 1px 6px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--accent) 10%, transparent);
    color: var(--accent);
    font-size: 10px;
    font-weight: 750;
    white-space: nowrap;
  }
</style>
