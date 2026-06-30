<script lang="ts">
  import { app } from "../../stores/app.svelte.js";
  import type { EntryDto } from "../../types/index.js";

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
  let renameInput = $state<HTMLInputElement | undefined>(undefined);

  $effect(() => {
    if (isRenaming && renameInput) {
      renameInput.focus();
      // Select name without extension
      const dot = app.renaming!.value.lastIndexOf(".");
      if (dot > 0 && !entry.isDir) {
        renameInput.setSelectionRange(0, dot);
      } else {
        renameInput.select();
      }
    }
  });

  function commitRename(e?: Event) {
    e?.stopPropagation();
    const val = app.renaming?.value?.trim();
    if (!val || val === entry.name) { app.cancelRename(); return; }
    // Dispatch event for Shell to handle
    document.dispatchEvent(new CustomEvent("dogu:rename-commit", {
      detail: { path: entry.path, newName: val }
    }));
  }

  function handleRenameKey(e: KeyboardEvent) {
    if (e.key === "Enter") { e.preventDefault(); commitRename(); }
    if (e.key === "Escape") { e.preventDefault(); app.cancelRename(); }
    e.stopPropagation();
  }
</script>

<!-- svelte-ignore a11y_click_events_have_key_events -->
<div
  class="file-row"
  class:file-row--selected={isSelected}
  class:file-row--dir={entry.isDir}
  role="row"
  aria-selected={isSelected}
  tabindex="0"
  onmousedown={(e) => onMousedown(e, entry)}
  ondblclick={() => onActivate(entry)}
  oncontextmenu={(e) => { e.preventDefault(); onContextMenu(e, entry); }}
>
  <!-- Icon -->
  <div class="file-col file-col--name">
    <span class="file-icon" aria-hidden="true">
      {#if entry.isDir}
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
      <!-- Inline rename input -->
      <input
        class="file-rename-input"
        type="text"
        bind:value={app.renaming!.value}
        bind:this={renameInput}
        onblur={commitRename}
        onkeydown={handleRenameKey}
        onclick={(e) => e.stopPropagation()}
        onmousedown={(e) => e.stopPropagation()}
        aria-label="Rename"
      />
    {:else}
      <span class="file-name">{entry.name}</span>
    {/if}
  </div>

  <div class="file-col file-col--type" aria-label="Type">
    {entry.isDir ? "Folder" : (entry.extension || "File")}
  </div>

  <div class="file-col file-col--size" aria-label="Size">
    {entry.isDir ? "—" : entry.sizeLabel}
  </div>

  <div class="file-col file-col--modified" aria-label="Modified">
    {entry.modifiedLabel || "—"}
  </div>
</div>

<style>
  .file-row {
    display: flex;
    align-items: center;
    height: 28px;
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
    font-size: 13px;
  }

  .file-col--name {
    flex: 1;
    gap: 7px;
    min-width: 0;
  }

  .file-col--type  { width: 80px; flex-shrink: 0; color: var(--text-muted); font-size: 12px; }
  .file-col--size  { width: 90px; flex-shrink: 0; color: var(--text-muted); font-size: 12px; justify-content: flex-end; }
  .file-col--modified { width: 140px; flex-shrink: 0; color: var(--text-muted); font-size: 12px; padding-left: 8px; }

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
