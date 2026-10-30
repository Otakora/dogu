<script lang="ts">
  import { app } from "../../stores/app.svelte.js";
  import type { EntryDto } from "../../types/index.js";
  import { t } from "../../i18n/index.js";
  import FileIcon from "./FileIcon.svelte";
  import HighlightedName from "./HighlightedName.svelte";

  type Props = {
    entry: EntryDto;
    isSelected: boolean;
    isDropTarget: boolean;
    isDragSource: boolean;
    highlight?: string;
    discLabel?: string | null;
    onClick: (entry: EntryDto) => void;
    onActivate: (entry: EntryDto) => void;
    onContextMenu: (e: MouseEvent, entry: EntryDto) => void;
    onMousedown: (e: MouseEvent, entry: EntryDto) => void;
    onShowQueueStory: (entry: EntryDto) => void;
  };

  let { entry, isSelected, isDropTarget, isDragSource, highlight = "", discLabel = null, onClick, onActivate, onContextMenu, onMousedown, onShowQueueStory }: Props = $props();

  // ── Inline rename ────────────────────────────────────────
  const isRenaming = $derived(app.renaming?.path === entry.path);

  // Svelte action — runs exactly once when the input mounts.
  // No reactive reads, no $effect, no bind:value to the store.
  // The input is uncontrolled: the DOM owns the value while editing.
  function initRenameInput(node: HTMLInputElement) {
    const initial = app.renaming?.value ?? entry.name;
    node.value = initial;
    node.focus();
    const dot = initial.lastIndexOf(".");
    if (dot > 0 && !entry.isDir) node.setSelectionRange(0, dot);
    else node.select();
  }

  function commitRename(e: Event) {
    // Guard: cancelRename() may have already been called (e.g. Escape triggers
    // DOM removal which fires blur — we must not re-commit in that case).
    if (!app.renaming) return;
    const val = (e.currentTarget as HTMLInputElement).value.trim();
    if (!val || val === entry.name) { app.cancelRename(); return; }
    document.dispatchEvent(new CustomEvent("dogu:rename-commit", {
      detail: { path: entry.path, newName: val, immediate: app.renaming?.immediate }
    }));
  }

  function handleRenameKey(e: KeyboardEvent) {
    if (e.key === "Enter") { e.preventDefault(); commitRename(e); }
    if (e.key === "Escape") { e.preventDefault(); app.cancelRename(); }
    e.stopPropagation();
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  class="file-row"
  class:file-row--selected={isSelected}
  class:file-row--dir={entry.isDir}
  class:file-row--ghost={entry.isGhost || entry.willBeRecreated}
  class:file-row--removed={entry.willBeRemoved && !entry.willBeRecreated}
  class:file-row--replaced={entry.willBeReplaced}
  class:file-row--conflict={entry.willConflict}
  class:file-row--drop-target={isDropTarget}
  class:file-row--drag-source={isDragSource}
  data-path={entry.path}
  data-is-dir={entry.isDir}
  role="row"
  aria-selected={isSelected}
  tabindex="0"
  draggable={false}
  title={entry.willBeRecreated ? t("ghost.willBeRecreated")
    : entry.willBeRemoved ? t("ghost.willBeRemoved")
    : entry.willBeReplaced ? t("ghost.willBeReplaced")
    : entry.willConflict ? t("ghost.willConflict")
    : entry.isGhost ? (entry.ghostApproximate ? t("ghost.pendingApprox") : t("ghost.pending"))
    : undefined}
  onclick={() => { if (!isRenaming) onClick(entry); }}
  onmousedown={(e) => onMousedown(e, entry)}
  ondblclick={() => { if (!isRenaming) onActivate(entry); }}
  oncontextmenu={(e) => { e.preventDefault(); onContextMenu(e, entry); }}
>
  <!-- Icon -->
  <div class="file-col file-col--name">
    <span class="file-icon" aria-hidden="true">
      <FileIcon {entry} size={16} />
    </span>

    {#if isRenaming}
      <input
        class="file-rename-input"
        type="text"
        use:initRenameInput
        onblur={commitRename}
        onkeydown={handleRenameKey}
        onclick={(e) => e.stopPropagation()}
        onmousedown={(e) => e.stopPropagation()}
        ondblclick={(e) => e.stopPropagation()}
        ondragstart={(e) => { e.preventDefault(); e.stopPropagation(); }}
        aria-label={t("fileRow.rename")}
      />
    {:else}
      <span class="file-name"><HighlightedName text={entry.name} query={highlight} /></span>
      {#if discLabel && !entry.willBeRecreated}<span class="disc-kind-badge" title={t(entry.isGhost ? "discKind.predictedHint" : "discKind.detectedHint")}>{discLabel}</span>{/if}
      {#if entry.willBeRecreated || entry.queueRenamedTo}
        <button class="status-badge status-badge--story" title={t("ghost.storyOpen")} aria-label={t("ghost.storyOpen")} onclick={(event) => { event.stopPropagation(); onShowQueueStory(entry); }} onmousedown={(event) => event.stopPropagation()}>{t(entry.willBeRecreated ? "ghost.badgeFinalNew" : "ghost.badgeRenamed")}<svg aria-hidden="true" width="9" height="9" viewBox="0 0 12 12" fill="none"><path d="M3 4.5 6 7.5 9 4.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round" stroke-linejoin="round"/></svg></button>
      {:else if entry.willBeRemoved}
        <span class="status-badge status-badge--removed">{t("ghost.badgeRemoved")}</span>
      {:else if entry.willBeReplaced}
        <span class="status-badge status-badge--replaced">{t("ghost.badgeReplaced")}</span>
      {:else if entry.willConflict}
        <span class="status-badge status-badge--conflict">{t("ghost.badgeConflict")}</span>
      {:else if entry.isGhost}
        <span class="ghost-badge">{entry.ghostApproximate ? t("ghost.badgeApprox") : t("ghost.badge")}</span>
      {/if}
    {/if}
  </div>

  <div class="file-col file-col--type" aria-label={t("fileRow.type")}>
    {entry.isDir ? t("fileRow.folder") : (entry.extension || t("fileRow.file"))}
  </div>

  <div class="file-col file-col--size" aria-label={t("fileRow.size")}>
    {entry.isDir || entry.willBeRecreated ? "—" : entry.sizeLabel}
  </div>

  <div class="file-col file-col--modified" aria-label={t("fileRow.modified")}>
    {entry.willBeRecreated ? "—" : (entry.modifiedLabel || "—")}
  </div>
</div>

<style>
  .disc-kind-badge {
    flex: none;
    margin-left: 6px;
    padding: 1px 5px;
    border: 1px solid #47865d;
    border-radius: 4px;
    color: #8dcc9d;
    font-size: 10px;
    line-height: 1.4;
  }
  .file-row {
    display: flex;
    align-items: center;
    height: calc(28px * var(--content-scale, 1));
    /* Keep the row's geometry available while the browser skips off-screen
       contents. This avoids laying out every file on each pane resize. */
    width: calc(var(--col-name, 280px) + var(--col-type, 80px) + var(--col-size, 90px) + var(--col-modified, 140px) + 16px);
    content-visibility: auto;
    padding: 0 8px;
    border-radius: 4px;
    user-select: none;
    color: var(--text);

    &:hover {
      background: var(--surface-hover);
    }
  }

  .file-row--selected {
    background: var(--accent-soft);
    color: var(--accent);

    .file-col--type,
    .file-col--size,
    .file-col--modified { color: color-mix(in srgb, var(--accent) 60%, var(--text-muted)); }
  }

  .file-row--drop-target {
    background: color-mix(in srgb, var(--accent) 10%, var(--surface-hover));
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 26%, transparent);
  }

  .file-row--drag-source {
    background: color-mix(in srgb, var(--accent) 7%, transparent);
    box-shadow: inset 0 0 0 1px color-mix(in srgb, var(--accent) 18%, transparent);
  }

  /* Columns */
  .file-col {
    display: flex;
    align-items: center;
    overflow: hidden;
    white-space: nowrap;
    font-size: calc(13px * var(--content-scale, 1));
  }

  .file-col--name {
    width: var(--col-name, 280px);
    flex-shrink: 0;
    gap: 7px;
    min-width: 0;
  }

  .file-col--type  { width: var(--col-type, 80px); flex-shrink: 0; color: var(--text-muted); font-size: calc(12px * var(--content-scale, 1)); }
  .file-col--size  { width: var(--col-size, 90px); flex-shrink: 0; color: var(--text-muted); font-size: calc(12px * var(--content-scale, 1)); justify-content: flex-end; }
  .file-col--modified { width: var(--col-modified, 140px); flex-shrink: 0; color: var(--text-muted); font-size: calc(12px * var(--content-scale, 1)); padding-left: 8px; }

  .file-name {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
    min-width: 0;
  }

  /* Icons */
  .file-icon { flex-shrink: 0; display: flex; }

  /* Ghost (predicted, not-yet-existing) rows */
  .file-row--ghost {
    opacity: 0.78;

    .file-name { font-style: italic; color: var(--text-muted); }
  }

  .file-row--ghost.file-row--selected { opacity: 1; }

  .ghost-badge,
  .status-badge {
    flex-shrink: 0;
    margin-left: 8px;
    margin-right: 10px;
    padding: 0 6px;
    height: 15px;
    display: inline-flex;
    align-items: center;
    border-radius: 7px;
    font-size: 9px;
    font-weight: 700;
    letter-spacing: 0.04em;
    text-transform: uppercase;
    color: var(--accent);
    background: var(--accent-soft);
    border: 1px solid color-mix(in srgb, var(--accent) 25%, transparent);
  }

  .status-badge--story { cursor: pointer; gap: 4px; font-family: inherit; }
  .status-badge--story:hover, .status-badge--story:focus-visible { border-color: var(--accent); outline-offset: 2px; }

  /* ── Real entries affected by queued operations ── */
  .status-badge--removed {
    color: var(--danger, #e5484d);
    background: color-mix(in srgb, var(--danger, #e5484d) 12%, transparent);
    border-color: color-mix(in srgb, var(--danger, #e5484d) 30%, transparent);
  }
  .status-badge--replaced {
    color: #d9820b;
    background: color-mix(in srgb, #d9820b 12%, transparent);
    border-color: color-mix(in srgb, #d9820b 30%, transparent);
  }
  .status-badge--conflict {
    color: var(--danger, #e5484d);
    background: color-mix(in srgb, var(--danger, #e5484d) 12%, transparent);
    border-color: color-mix(in srgb, var(--danger, #e5484d) 45%, transparent);
  }

  .file-row--removed {
    .file-name { text-decoration: line-through; color: var(--text-subtle); }
    .file-icon { opacity: 0.5; }
  }
  .file-row--replaced {
    .file-name { color: #d9820b; text-decoration: line-through; }
  }
  .file-row--conflict {
    background: color-mix(in srgb, var(--danger, #e5484d) 7%, transparent);
  }

  /* Rename input */
  .file-rename-input {
    flex: 1;
    min-width: 0;
    height: 22px;
    padding: 0 6px;
    border: 1.5px solid var(--accent);
    border-radius: 4px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 13px;
    outline: none;
    box-shadow: 0 0 0 2px var(--accent-soft);
  }
</style>
