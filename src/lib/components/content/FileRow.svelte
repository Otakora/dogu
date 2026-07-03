<script lang="ts">
  import { app } from "../../stores/app.svelte.js";
  import type { EntryDto } from "../../types/index.js";
  import { t } from "../../i18n/index.js";

  type Props = {
    entry: EntryDto;
    isSelected: boolean;
    onActivate: (entry: EntryDto) => void;
    onContextMenu: (e: MouseEvent, entry: EntryDto) => void;
    onMousedown: (e: MouseEvent, entry: EntryDto) => void;
  };

  let { entry, isSelected, onActivate, onContextMenu, onMousedown }: Props = $props();

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
      detail: { path: entry.path, newName: val }
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
  class:file-row--ghost={entry.isGhost}
  data-path={entry.path}
  role="row"
  aria-selected={isSelected}
  tabindex="0"
  title={entry.isGhost ? (entry.ghostApproximate ? t("ghost.pendingApprox") : t("ghost.pending")) : undefined}
  onmousedown={(e) => onMousedown(e, entry)}
  ondblclick={() => onActivate(entry)}
  oncontextmenu={(e) => { e.preventDefault(); onContextMenu(e, entry); }}
>
  <!-- Icon -->
  <div class="file-col file-col--name">
    <span class="file-icon" aria-hidden="true">
      {#if entry.isGhost}
        <!-- Dashed outline = a file/folder that does not exist yet -->
        {#if entry.isDir}
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-dasharray="3 2" class="icon-ghost">
            <path d="M10 4H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2h-8l-2-2z"/>
          </svg>
        {:else}
          <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" stroke-dasharray="3 2" class="icon-ghost">
            <path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/>
            <polyline points="13 2 13 9 20 9"/>
          </svg>
        {/if}
      {:else if entry.isDir}
        <svg width="15" height="15" viewBox="0 0 24 24" fill="currentColor" class="icon-dir">
          <path d="M10 4H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2h-8l-2-2z"/>
        </svg>
      {:else}
        <svg width="15" height="15" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5" class="icon-file">
          <path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/>
          <polyline points="13 2 13 9 20 9"/>
        </svg>
      {/if}
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
        aria-label={t("fileRow.rename")}
      />
    {:else}
      <span class="file-name">{entry.name}</span>
      {#if entry.isGhost}
        <span class="ghost-badge">{entry.ghostApproximate ? t("ghost.badgeApprox") : t("ghost.badge")}</span>
      {/if}
    {/if}
  </div>

  <div class="file-col file-col--type" aria-label={t("fileRow.type")}>
    {entry.isDir ? t("fileRow.folder") : (entry.extension || t("fileRow.file"))}
  </div>

  <div class="file-col file-col--size" aria-label={t("fileRow.size")}>
    {entry.isDir ? "—" : entry.sizeLabel}
  </div>

  <div class="file-col file-col--modified" aria-label={t("fileRow.modified")}>
    {entry.modifiedLabel || "—"}
  </div>
</div>

<style>
  .file-row {
    display: flex;
    align-items: center;
    height: calc(28px * var(--content-scale, 1));
    padding: 0 8px;
    border-radius: 4px;
    cursor: pointer;
    user-select: none;
    color: var(--text);

    &:hover {
      background: var(--surface-hover);
    }
  }

  .file-row--selected {
    background: var(--accent-soft);
    color: var(--accent);

    .icon-dir { color: var(--accent); }
    .file-col--type,
    .file-col--size,
    .file-col--modified { color: color-mix(in srgb, var(--accent) 60%, var(--text-muted)); }
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
  .icon-dir  { color: #e0a030; }
  .icon-file { color: var(--text-muted); }
  .icon-ghost { color: var(--accent); opacity: 0.7; }

  /* Ghost (predicted, not-yet-existing) rows */
  .file-row--ghost {
    opacity: 0.72;

    .file-name { font-style: italic; color: var(--text-muted); }
  }

  .file-row--ghost.file-row--selected { opacity: 1; }

  .ghost-badge {
    flex-shrink: 0;
    margin-left: 6px;
    padding: 0 5px;
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
