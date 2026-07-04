<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { untrack } from "svelte";
  import type {
    M3uGroupDto,
    M3uScanOptions,
    M3uGeneratePayload,
    M3uGenerateResultDto,
    M3uWarningDto,
  } from "../../types/index.js";
  import Modal from "../ui/Modal.svelte";
  import Button from "../ui/Button.svelte";
  import DestinationField from "./DestinationField.svelte";
  import { t } from "../../i18n/index.js";
  import { app } from "../../stores/app.svelte.js";

  let customDestLabel = $state<string | null>(null);

  type Props = {
    dirs: string[];
    /** The folder currently open in the app — used for the "currentDir" output option. */
    currentDir?: string;
    onclose: () => void;
    /**
     * Called instead of running immediately when queue mode is on. Receives the
     * generation payload and the .m3u output paths (for ghost prediction).
     */
    onEnqueue?: (payload: M3uGeneratePayload, outputPaths: string[], sources: string[]) => void;
  };

  let { dirs, currentDir = "", onclose, onEnqueue }: Props = $props();

  // ── Options ───────────────────────────────────────────────
  let opts = $state<M3uScanOptions>({
    multiDiscOnly: true,
    recursive: true,
    // Default to currentDir when we know where we are; scanRoot otherwise.
    outputLocation: untrack(() => currentDir ? "currentDir" : "scanRoot"),
    customOutputPath: untrack(() => currentDir || null),
    useRelativePaths: true,
  });
  let overwrite = $state(false);

  // Keep customOutputPath in sync when the user picks "currentDir"
  $effect(() => {
    if (opts.outputLocation === "currentDir") {
      opts.customOutputPath = currentDir || null;
    }
  });

  // ── State machine ─────────────────────────────────────────
  type Phase = "config" | "scanning" | "preview" | "generating" | "done";
  let phase = $state<Phase>("config");
  let groups = $state<M3uGroupDto[]>([]);
  let selectedIds = $state<Set<string>>(new Set());
  let expandedIds = $state<Set<string>>(new Set());
  let result = $state<M3uGenerateResultDto | null>(null);
  let scanError = $state<string | null>(null);

  // Start scanning immediately on mount
  $effect(() => {
    scan();
  });

  async function scan() {
    phase = "scanning";
    scanError = null;
    groups = [];
    try {
      const found = await invoke<M3uGroupDto[]>("scan_for_m3u_groups", {
        dirs,
        options: opts,
      });
      groups = found;
      selectedIds = new Set(found.map((g) => g.id));
      phase = "preview";
    } catch (e) {
      scanError = String(e);
      phase = "config";
    }
  }

  function buildPayload(toGenerate: M3uGroupDto[]): M3uGeneratePayload {
    return {
      overwrite,
      groups: toGenerate.map((g) => ({
        outputPath: g.outputPath,
        baseName: g.baseName,
        entries: g.entries.map((e) => e.m3uPath),
      })),
    };
  }

  async function generate() {
    const toGenerate = groups.filter((g) => selectedIds.has(g.id));
    if (toGenerate.length === 0) return;

    const payload = buildPayload(toGenerate);

    // Queue mode: hand the op to the caller (which shows a .m3u ghost) instead
    // of generating right away.
    if (app.queueMode && onEnqueue) {
      const outputPaths = toGenerate.map((g) => g.outputPath);
      const sources = toGenerate.flatMap((g) => g.entries.map((e) => e.absolutePath));
      onEnqueue(payload, outputPaths, sources);
      onclose();
      return;
    }

    phase = "generating";
    try {
      result = await invoke<M3uGenerateResultDto>("generate_m3u_files", { payload });
      phase = "done";
    } catch (e) {
      scanError = String(e);
      phase = "preview";
    }
  }

  function handleCustomDestChange(path: string, label: string) {
    opts = { ...opts, customOutputPath: path };
    customDestLabel = label;
  }

  function toggleGroup(id: string) {
    const next = new Set(selectedIds);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    selectedIds = next;
  }

  function toggleExpand(id: string) {
    const next = new Set(expandedIds);
    if (next.has(id)) next.delete(id);
    else next.add(id);
    expandedIds = next;
  }

  function selectAll() { selectedIds = new Set(groups.map((g) => g.id)); }
  function selectNone() { selectedIds = new Set(); }

  function warnLabel(w: M3uWarningDto): string {
    const detail = w.detail ?? "";
    switch (w.kind) {
      case "missingDisc":   return t("m3u.warn.missingDisc",   { detail });
      case "duplicateDisc": return t("m3u.warn.duplicateDisc", { detail });
      case "mixedFormats":  return t("m3u.warn.mixedFormats",  { detail });
      case "m3uExists":     return overwrite ? t("m3u.warn.m3uExists") : t("m3u.warn.m3uExistsSkip");
    }
  }

  const selectedCount = $derived(selectedIds.size);
  const totalWarnings = $derived(groups.filter((g) => g.warnings.length > 0).length);
  const canGenerate   = $derived(selectedCount > 0 && phase === "preview");

  function leafName(path: string): string {
    return path.replace(/\\/g, "/").split("/").pop() ?? path;
  }

  /** Show last 2 path segments so the user can see both folder and filename. */
  function shortPath(path: string): string {
    const parts = path.replace(/\\/g, "/").split("/").filter(Boolean);
    if (parts.length <= 2) return parts.join("/");
    return `…/${parts.slice(-2).join("/")}`;
  }
</script>

<Modal title={t("m3u.dialog.title")} width="640px" {onclose}>
  {#snippet children()}
    <!-- ── Config panel ─────────────────────────────────── -->
    <div class="config-panel">
      <div class="config-row">
        <label class="check-label">
          <input type="checkbox" bind:checked={opts.multiDiscOnly} />
          {t("m3u.options.multiDiscOnly")}
          <span class="hint">{t("m3u.options.multiDiscOnlyHint")}</span>
        </label>
      </div>

      <div class="config-row">
        <label class="check-label">
          <input type="checkbox" bind:checked={opts.recursive} />
          {t("m3u.options.recursive")}
        </label>
      </div>

      <div class="config-row config-row--output">
        <span class="config-label">{t("m3u.options.outputLocation")}</span>
        <div class="radio-group">
          <label class="radio-label" class:radio-label--disabled={!currentDir}>
            <input type="radio" bind:group={opts.outputLocation} value="currentDir" disabled={!currentDir} />
            {t("m3u.options.currentDir")}
          </label>
          {#if opts.outputLocation === "currentDir"}
            <p class="output-hint">
              {t("m3u.options.currentDirHint")}
              {#if currentDir}<code class="path-code">{currentDir}</code>{/if}
            </p>
          {/if}

          <label class="radio-label">
            <input type="radio" bind:group={opts.outputLocation} value="scanRoot" />
            {t("m3u.options.scanRoot")}
          </label>
          {#if opts.outputLocation === "scanRoot"}
            <p class="output-hint">{t("m3u.options.scanRootHint")}</p>
          {/if}

          <label class="radio-label">
            <input type="radio" bind:group={opts.outputLocation} value="sameFolder" />
            {t("m3u.options.sameFolder")}
          </label>
          {#if opts.outputLocation === "sameFolder"}
            <p class="output-hint">{t("m3u.options.sameFolderHint")}</p>
          {/if}

          <label class="radio-label">
            <input type="radio" bind:group={opts.outputLocation} value="custom" />
            {t("m3u.options.custom")}
          </label>
          {#if opts.outputLocation === "custom"}
            <DestinationField
              path={opts.customOutputPath}
              label={customDestLabel}
              onchange={handleCustomDestChange}
            />
          {/if}
        </div>
      </div>

      <div class="config-row config-row--inline">
        <label class="check-label">
          <input type="checkbox" bind:checked={opts.useRelativePaths} />
          {t("m3u.options.useRelativePaths")}
        </label>
        <label class="check-label">
          <input type="checkbox" bind:checked={overwrite} />
          {t("m3u.options.overwrite")}
        </label>
      </div>

      <div class="config-actions">
        <Button onclick={scan} disabled={phase === "scanning"}>
          {phase === "scanning" ? t("m3u.scanning") : t("m3u.scan")}
        </Button>
        {#if scanError}
          <span class="scan-error">{scanError}</span>
        {/if}
      </div>
    </div>

    <!-- ── Results ─────────────────────────────────────────── -->
    {#if phase === "scanning"}
      <div class="loading-row">
        <div class="spinner" aria-hidden="true"></div>
        {t("m3u.scanning")}
      </div>

    {:else if phase === "preview" || phase === "generating"}
      {#if groups.length === 0}
        <p class="empty-msg">{t("m3u.noGroups")}</p>
      {:else}
        <!-- Summary bar -->
        <div class="preview-bar">
          <span class="preview-count">{t("m3u.groupCount", { n: String(groups.length) })}</span>
          {#if totalWarnings > 0}
            <span class="preview-warnings">⚠ {t("m3u.warningCount", { n: String(totalWarnings) })}</span>
          {/if}
          <div class="preview-actions">
            <button class="link-btn" onclick={selectAll}>All</button>
            <button class="link-btn" onclick={selectNone}>None</button>
          </div>
        </div>

        <!-- Group list -->
        <div class="groups-list" aria-label="Detected groups">
          {#each groups as group (group.id)}
            {@const isSelected = selectedIds.has(group.id)}
            {@const isExpanded = expandedIds.has(group.id)}
            {@const hasWarnings = group.warnings.length > 0}
            <div class="group-card" class:group-card--unchecked={!isSelected} class:group-card--warn={hasWarnings}>
              <!-- Row: checkbox + name + toggle -->
              <div class="group-header">
                <input
                  type="checkbox"
                  class="group-check"
                  checked={isSelected}
                  onchange={() => toggleGroup(group.id)}
                  aria-label={group.baseName}
                />
                <button class="group-name" onclick={() => toggleExpand(group.id)}>
                  <span class="group-title">{group.baseName}</span>
                  <span class="group-meta">
                    {group.entries.length} disc{group.entries.length !== 1 ? "s" : ""}
                    {#if hasWarnings}
                      <span class="warn-badge">⚠ {group.warnings.length}</span>
                    {/if}
                  </span>
                  <span class="group-expand" aria-hidden="true">{isExpanded ? "▾" : "▸"}</span>
                </button>
              </div>

              <!-- Output path -->
              <div class="group-output">
                <span class="output-label">→</span>
                <span class="output-path" title={group.outputPath}>{shortPath(group.outputPath)}</span>
              </div>

              <!-- Warnings -->
              {#if hasWarnings}
                <div class="group-warns">
                  {#each group.warnings as w}
                    <span class="warn-item">⚠ {warnLabel(w)}</span>
                  {/each}
                </div>
              {/if}

              <!-- Expanded entries: show m3uPath (= exact content written to the file) -->
              {#if isExpanded}
                <ol class="entries-list">
                  {#each group.entries as entry}
                    <li class="entry-row">
                      <span class="entry-fmt">{entry.format}</span>
                      <span class="entry-path" title={entry.absolutePath}>{entry.m3uPath}</span>
                      {#if entry.discNumber > 0}
                        <span class="entry-disc">#{entry.discNumber}</span>
                      {/if}
                    </li>
                  {/each}
                </ol>
              {/if}
            </div>
          {/each}
        </div>
      {/if}

    {:else if phase === "done" && result}
      <div class="result-box">
        <div class="result-row result-row--created">
          <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
            <polyline points="20 6 9 17 4 12"/>
          </svg>
          {t("m3u.result.created", { n: String(result.created.length) })}
        </div>
        {#if result.skipped.length > 0}
          <div class="result-row result-row--skipped">
            — {t("m3u.result.skipped", { n: String(result.skipped.length) })}
          </div>
        {/if}
        {#if result.failed.length > 0}
          <div class="result-row result-row--failed">
            ✕ {t("m3u.result.failed", { n: String(result.failed.length) })}
          </div>
          <ul class="fail-list">
            {#each result.failed as f}
              <li class="fail-item"><span class="fail-name">{leafName(f.path)}</span> — {f.error}</li>
            {/each}
          </ul>
        {/if}
      </div>
    {/if}
  {/snippet}

  {#snippet footer()}
    {#if phase === "done"}
      <Button onclick={onclose}>{t("common.close")}</Button>
    {:else}
      <Button onclick={onclose} variant="ghost">{t("common.cancel")}</Button>
      <Button onclick={generate} disabled={!canGenerate || phase === "generating"}>
        {phase === "generating" ? t("m3u.generating") : `${t("m3u.generate")} (${selectedCount})`}
      </Button>
    {/if}
  {/snippet}
</Modal>

<style>
  /* ── Config panel ────────────────────────────────────────── */
  .config-panel {
    display: flex;
    flex-direction: column;
    gap: 10px;
    padding-bottom: 14px;
    border-bottom: 1px solid var(--line);
    margin-bottom: 14px;
  }

  .config-row {
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .config-row--inline {
    flex-direction: row;
    gap: 20px;
    flex-wrap: wrap;
  }

  .config-row--output {
    gap: 6px;
  }

  .config-label {
    font-size: 11.5px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .check-label {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 13px;
    cursor: pointer;
    user-select: none;
  }

  .hint {
    font-size: 11px;
    color: var(--text-muted);
    font-style: italic;
  }

  .radio-group {
    display: flex;
    flex-direction: column;
    gap: 5px;
    padding-left: 4px;
  }

  .radio-label {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 13px;
    cursor: pointer;
    user-select: none;
  }

  .output-hint {
    font-size: 11px;
    color: var(--text-muted);
    margin: 0 0 0 22px;
    line-height: 1.4;
  }

  .path-code {
    display: block;
    margin-top: 3px;
    font-size: 10.5px;
    font-family: monospace;
    color: var(--text);
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: 3px;
    padding: 2px 5px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .radio-label--disabled {
    opacity: 0.4;
    cursor: not-allowed;
  }

  .config-actions {
    display: flex;
    align-items: center;
    gap: 10px;
    padding-top: 2px;
  }

  .scan-error {
    font-size: 12px;
    color: var(--error, #dc2626);
  }

  /* ── Loading ─────────────────────────────────────────────── */
  .loading-row {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 20px 0;
    font-size: 13px;
    color: var(--text-muted);
    justify-content: center;
  }

  .spinner {
    width: 16px;
    height: 16px;
    border: 2px solid var(--line-strong);
    border-top-color: var(--accent, #3b82f6);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }

  @keyframes spin { to { transform: rotate(360deg); } }

  /* ── Empty ───────────────────────────────────────────────── */
  .empty-msg {
    text-align: center;
    color: var(--text-muted);
    font-size: 13px;
    padding: 20px 0;
    margin: 0;
  }

  /* ── Preview bar ─────────────────────────────────────────── */
  .preview-bar {
    display: flex;
    align-items: center;
    gap: 10px;
    margin-bottom: 8px;
    font-size: 12px;
  }

  .preview-count {
    font-weight: 600;
    color: var(--text);
  }

  .preview-warnings {
    color: #b45309;
    font-weight: 600;
  }

  .preview-actions {
    margin-left: auto;
    display: flex;
    gap: 6px;
  }

  .link-btn {
    background: none;
    border: none;
    color: var(--accent, #3b82f6);
    font-size: 12px;
    cursor: pointer;
    padding: 0;
    text-decoration: underline;

    &:hover { opacity: 0.75; }
  }

  /* ── Groups list ─────────────────────────────────────────── */
  .groups-list {
    display: flex;
    flex-direction: column;
    gap: 5px;
    max-height: 340px;
    overflow-y: auto;
    scrollbar-width: thin;
    scrollbar-color: var(--line-strong) transparent;
  }

  .group-card {
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 7px 10px;
    background: var(--surface-alt);
    display: flex;
    flex-direction: column;
    gap: 3px;

    &.group-card--warn {
      border-color: color-mix(in srgb, #f59e0b 40%, var(--line));
    }

    &.group-card--unchecked {
      opacity: 0.45;
    }
  }

  .group-header {
    display: flex;
    align-items: center;
    gap: 7px;
  }

  .group-check {
    flex-shrink: 0;
    cursor: pointer;
  }

  .group-name {
    display: flex;
    align-items: center;
    gap: 8px;
    flex: 1;
    background: none;
    border: none;
    padding: 0;
    cursor: pointer;
    text-align: left;
    min-width: 0;
  }

  .group-title {
    font-size: 13px;
    font-weight: 500;
    color: var(--text);
    flex: 1;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .group-meta {
    font-size: 11px;
    color: var(--text-muted);
    white-space: nowrap;
    flex-shrink: 0;
  }

  .warn-badge {
    color: #b45309;
    font-weight: 600;
  }

  .group-expand {
    font-size: 10px;
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .group-output {
    display: flex;
    align-items: center;
    gap: 6px;
    padding-left: 26px;
  }

  .output-label {
    font-size: 11px;
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .output-path {
    font-size: 11px;
    color: var(--text-muted);
    font-family: monospace;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .group-warns {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding-left: 26px;
  }

  .warn-item {
    font-size: 11px;
    color: #b45309;
  }

  /* ── Entries list ────────────────────────────────────────── */
  .entries-list {
    list-style: none;
    margin: 4px 0 0;
    padding: 0 0 0 26px;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .entry-row {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11.5px;
  }

  .entry-fmt {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    color: var(--text-muted);
    background: var(--surface);
    border: 1px solid var(--line);
    border-radius: 3px;
    padding: 0 4px;
    flex-shrink: 0;
  }

  .entry-path {
    flex: 1;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .entry-disc {
    font-size: 10px;
    color: var(--text-muted);
    flex-shrink: 0;
  }

  /* ── Result ──────────────────────────────────────────────── */
  .result-box {
    display: flex;
    flex-direction: column;
    gap: 6px;
    padding: 12px;
    background: var(--surface-alt);
    border: 1px solid var(--line);
    border-radius: 6px;
  }

  .result-row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 13px;
    font-weight: 500;

    &.result-row--created { color: var(--success, #16a34a); svg { stroke: var(--success, #16a34a); } }
    &.result-row--skipped { color: var(--text-muted); }
    &.result-row--failed  { color: var(--error, #dc2626); }
  }

  .fail-list {
    list-style: none;
    padding: 0;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .fail-item {
    font-size: 11px;
    color: var(--error, #dc2626);
    padding-left: 12px;
  }

  .fail-name {
    font-weight: 600;
  }
</style>
