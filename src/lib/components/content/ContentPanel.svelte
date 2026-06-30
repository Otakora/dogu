<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { app } from "../../stores/app.svelte.js";
  import type { EntryDto, ContentSortKey } from "../../types/index.js";
  import FileRow from "./FileRow.svelte";
  import ContextMenu from "../ui/ContextMenu.svelte";
  import type { MenuItem } from "../ui/ContextMenu.svelte";

  type Props = {
    onDelete: (paths: string[]) => void;
    onCopy: (paths: string[]) => void;
    onCut: (paths: string[]) => void;
    onPaste: () => void;
    onExtract: (paths: string[]) => void;
    onProperties: (paths: string[]) => void;
    onOpenWith: (path: string) => void;
  };

  let { onDelete, onCopy, onCut, onPaste, onExtract, onProperties, onOpenWith }: Props = $props();

  // ── Context menu ─────────────────────────────────────────
  let contextMenu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

  // ── Sorted entries ───────────────────────────────────────
  const sorted = $derived(sortEntries(app.entries, app.sortKey, app.sortDir));

  function sortEntries(entries: EntryDto[], key: ContentSortKey, dir: "asc" | "desc"): EntryDto[] {
    return [...entries].sort((a, b) => {
      // Directories always first
      if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;

      let cmp = 0;
      switch (key) {
        case "name":     cmp = a.name.localeCompare(b.name, undefined, { sensitivity: "base" }); break;
        case "type":     cmp = a.extension.localeCompare(b.extension); break;
        case "size":     cmp = a.size - b.size; break;
        case "modified": cmp = a.modifiedTs - b.modifiedTs; break;
      }
      return dir === "asc" ? cmp : -cmp;
    });
  }

  // ── Load on path change ───────────────────────────────────
  let lastLoadPath = $state<string | null>(null);

  $effect(() => {
    const path = app.currentPath;
    if (!path || path === lastLoadPath) return;
    lastLoadPath = path;

    if (app.isSearching && app.searchQuery) {
      doSearch(path, app.searchQuery);
    } else {
      loadPath(path);
    }
  });

  async function loadPath(path: string) {
    app.setLoading(true);
    app.clearSelection();
    try {
      const entries = await invoke<EntryDto[]>("list_children", { path });
      app.setEntries(entries);
    } catch (e) {
      app.notify("error", `Could not load: ${e}`);
      app.setEntries([]);
    } finally {
      app.setLoading(false);
    }
  }

  async function doSearch(path: string, query: string) {
    app.setLoading(true);
    app.clearSelection();
    app.setIsSearching(true);
    try {
      const entries = await invoke<EntryDto[]>("search_entries", { path, query, recursive: true });
      app.setEntries(entries);
    } catch (e) {
      app.notify("error", `Search failed: ${e}`);
      app.setEntries([]);
    } finally {
      app.setLoading(false);
    }
  }

  export function refresh() {
    if (app.currentPath) {
      lastLoadPath = null; // force reload
      loadPath(app.currentPath);
    }
  }

  export function search(q: string) {
    if (!app.currentPath) return;
    app.setSearch(q);
    doSearch(app.currentPath, q);
  }

  // ── Selection handling ────────────────────────────────────
  let lastClickedPath = $state<string | null>(null);

  function handleMousedown(e: MouseEvent, entry: EntryDto) {
    if (e.button !== 0 && e.button !== 2) return;

    if (e.button === 2) {
      // Right-click: select if not already
      if (!app.selectedPaths.has(entry.path)) {
        app.setSelection([entry.path]);
        lastClickedPath = entry.path;
      }
      return;
    }

    if (e.shiftKey && lastClickedPath) {
      app.rangeSelect(lastClickedPath, entry.path);
    } else if (e.ctrlKey || e.metaKey) {
      app.toggleSelection(entry.path);
      lastClickedPath = entry.path;
    } else {
      app.setSelection([entry.path]);
      lastClickedPath = entry.path;
    }
  }

  function handleActivate(entry: EntryDto) {
    if (entry.isDir) {
      app.navigate(entry.path);
    } else {
      invoke("open_path", { path: entry.path }).catch(e => app.notify("error", String(e)));
    }
  }

  // ── Keyboard shortcuts ────────────────────────────────────
  function handleKeyDown(e: KeyboardEvent) {
    const sel = [...app.selectedPaths];

    if (e.key === "Delete" || e.key === "Backspace") {
      if (sel.length > 0) { e.preventDefault(); onDelete(sel); }
    }

    if (e.key === "F2" && sel.length === 1) {
      const entry = app.entries.find(en => en.path === sel[0]);
      if (entry) app.startRename(entry.path, entry.name);
    }

    if ((e.ctrlKey || e.metaKey) && e.key === "c" && sel.length > 0) {
      e.preventDefault(); onCopy(sel);
    }

    if ((e.ctrlKey || e.metaKey) && e.key === "x" && sel.length > 0) {
      e.preventDefault(); onCut(sel);
    }

    if ((e.ctrlKey || e.metaKey) && e.key === "v") {
      e.preventDefault(); onPaste();
    }

    if (e.key === "a" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      app.setSelection(app.entries.map(en => en.path));
    }

    if (e.key === "Escape") {
      app.clearSelection();
      app.cancelRename();
      contextMenu = null;
    }

    // Arrow key navigation
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const paths = sorted.map(en => en.path);
      const lastSel = sel[sel.length - 1];
      const idx = paths.indexOf(lastSel ?? "");
      const newIdx = e.key === "ArrowDown"
        ? Math.min(idx + 1, paths.length - 1)
        : Math.max(idx - 1, 0);
      if (newIdx >= 0) {
        if (e.shiftKey) {
          app.rangeSelect(lastClickedPath ?? paths[0], paths[newIdx]);
        } else {
          app.setSelection([paths[newIdx]]);
          lastClickedPath = paths[newIdx];
        }
      }
    }

    if (e.key === "Enter" && sel.length === 1) {
      const entry = app.entries.find(en => en.path === sel[0]);
      if (entry) handleActivate(entry);
    }
  }

  // ── Context menu ─────────────────────────────────────────
  function openContextMenu(e: MouseEvent, entry: EntryDto) {
    const sel = app.selectedPaths.has(entry.path) ? [...app.selectedPaths] : [entry.path];
    if (!app.selectedPaths.has(entry.path)) app.setSelection([entry.path]);

    const isMulti = sel.length > 1;
    const allFiles = sel.every(p => !app.entries.find(en => en.path === p)?.isDir);
    const hasArchives = sel.some(p => {
      const ext = app.entries.find(en => en.path === p)?.extension ?? "";
      return [".zip", ".7z", ".rar", ".tar", ".gz", ".bz2", ".xz"].includes(ext);
    });

    const items: MenuItem[] = [
      { kind: "action", label: isMulti ? `Open (${sel.length})` : "Open", icon: openIcon, onclick: () => sel.forEach(p => invoke("open_path", { path: p })) },
    ];

    if (!isMulti) {
      items.push({ kind: "action", label: "Open with…", icon: openWithIcon, onclick: () => onOpenWith(sel[0]) });
      if (!entry.isDir) {
        items.push({ kind: "action", label: "Rename", shortcut: "F2", icon: renameIcon, onclick: () => app.startRename(entry.path, entry.name) });
      }
    }

    items.push({ kind: "separator" });
    items.push({ kind: "action", label: "Copy", shortcut: "Ctrl+C", icon: copyIcon, onclick: () => onCopy(sel) });
    items.push({ kind: "action", label: "Cut",  shortcut: "Ctrl+X", icon: cutIcon,  onclick: () => onCut(sel) });

    if (app.clipboard) {
      items.push({ kind: "action", label: "Paste", shortcut: "Ctrl+V", icon: pasteIcon, onclick: () => onPaste() });
    }

    if (hasArchives && allFiles) {
      items.push({ kind: "separator" });
      items.push({ kind: "action", label: "Extract here…", icon: extractIcon, onclick: () => onExtract(sel) });
    }

    items.push({ kind: "separator" });
    items.push({ kind: "action", label: "Properties", icon: propsIcon, onclick: () => onProperties(sel) });
    items.push({ kind: "separator" });
    items.push({ kind: "action", label: "Delete", shortcut: "Del", icon: deleteIcon, danger: true, onclick: () => onDelete(sel) });

    contextMenu = { x: e.clientX, y: e.clientY, items };
  }

  // ── Sort header click ────────────────────────────────────
  function clickSort(key: ContentSortKey) {
    if (app.sortKey === key) {
      app.setSort(key, app.sortDir === "asc" ? "desc" : "asc");
    } else {
      app.setSort(key, "asc");
    }
  }

  // ── Click on empty area ──────────────────────────────────
  function handlePanelClick(e: MouseEvent) {
    if ((e.target as HTMLElement).closest(".file-row")) return;
    app.clearSelection();
    contextMenu = null;
  }

  function handlePanelContextMenu(e: MouseEvent) {
    if ((e.target as HTMLElement).closest(".file-row")) return;
    e.preventDefault();
    const items: MenuItem[] = [];
    if (app.clipboard) items.push({ kind: "action", label: "Paste", icon: pasteIcon, onclick: () => onPaste() });
    items.push({ kind: "action", label: "New folder", icon: newFolderIcon, onclick: () => document.dispatchEvent(new CustomEvent("dogu:new-folder")) });
    items.push({ kind: "action", label: "Refresh", shortcut: "F5", icon: refreshIcon, onclick: () => refresh() });
    if (items.length > 0) contextMenu = { x: e.clientX, y: e.clientY, items };
  }

  // Inline SVG icons for context menu
  const openIcon     = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>`;
  const openWithIcon = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33"/></svg>`;
  const renameIcon   = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>`;
  const copyIcon     = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>`;
  const cutIcon      = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="6" cy="20" r="2"/><circle cx="20" cy="4" r="2"/><path d="M4 20l14.5-14.5M10.5 13.5l5.5 5.5M2 4l12 12"/><circle cx="6" cy="4" r="2"/></svg>`;
  const pasteIcon    = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/><rect x="8" y="2" width="8" height="4" rx="1" ry="1"/></svg>`;
  const extractIcon  = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="8 17 12 21 16 17"/><line x1="12" y1="12" x2="12" y2="21"/><path d="M20.88 18.09A5 5 0 0 0 18 9h-1.26A8 8 0 1 0 3 16.29"/></svg>`;
  const propsIcon    = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>`;
  const deleteIcon   = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/><path d="M10 11v6M14 11v6M9 6V4h6v2"/></svg>`;
  const newFolderIcon = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/><line x1="12" y1="11" x2="12" y2="17"/><line x1="9" y1="14" x2="15" y2="14"/></svg>`;
  const refreshIcon  = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/></svg>`;
</script>

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div
  class="content-panel"
  role="grid"
  aria-label="File list"
  tabindex="0"
  onkeydown={handleKeyDown}
  onclick={handlePanelClick}
  oncontextmenu={handlePanelContextMenu}
>
  <!-- ── Column headers ── -->
  <div class="content-header" role="row">
    <button class="col-hdr col-hdr--name" onclick={() => clickSort("name")} role="columnheader">
      Name
      {#if app.sortKey === "name"}<span class="sort-arrow">{app.sortDir === "asc" ? "↑" : "↓"}</span>{/if}
    </button>
    <button class="col-hdr col-hdr--type" onclick={() => clickSort("type")} role="columnheader">
      Type
      {#if app.sortKey === "type"}<span class="sort-arrow">{app.sortDir === "asc" ? "↑" : "↓"}</span>{/if}
    </button>
    <button class="col-hdr col-hdr--size" onclick={() => clickSort("size")} role="columnheader">
      Size
      {#if app.sortKey === "size"}<span class="sort-arrow">{app.sortDir === "asc" ? "↑" : "↓"}</span>{/if}
    </button>
    <button class="col-hdr col-hdr--modified" onclick={() => clickSort("modified")} role="columnheader">
      Modified
      {#if app.sortKey === "modified"}<span class="sort-arrow">{app.sortDir === "asc" ? "↑" : "↓"}</span>{/if}
    </button>
  </div>

  <!-- ── Content area ── -->
  <div class="content-body scrollbar-thin">
    {#if app.isLoading}
      <div class="content-state">
        <span class="loading-spinner"></span>
        <span>Loading…</span>
      </div>

    {:else if !app.currentPath}
      <div class="content-state">
        <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
        </svg>
        <span>Select a location to browse</span>
      </div>

    {:else if sorted.length === 0}
      <div class="content-state">
        <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
        </svg>
        <span>{app.isSearching ? "No results found" : "This folder is empty"}</span>
      </div>

    {:else}
      {#if app.viewMode === "list"}
        <div class="file-list" role="rowgroup">
          {#each sorted as entry (entry.path)}
            <FileRow
              {entry}
              isSelected={app.selectedPaths.has(entry.path)}
              onActivate={handleActivate}
              onContextMenu={openContextMenu}
              onMousedown={handleMousedown}
            />
          {/each}
        </div>

      {:else}
        <!-- Grid view -->
        <div class="file-grid" role="rowgroup">
          {#each sorted as entry (entry.path)}
            <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
            <div
              class="grid-item"
              class:grid-item--selected={app.selectedPaths.has(entry.path)}
              role="gridcell"
              tabindex="0"
              onmousedown={(e) => handleMousedown(e, entry)}
              ondblclick={() => handleActivate(entry)}
              oncontextmenu={(e) => { e.preventDefault(); openContextMenu(e, entry); }}
            >
              <div class="grid-icon" aria-hidden="true">
                {#if entry.isDir}
                  <svg width="32" height="32" viewBox="0 0 24 24" fill="currentColor" class="icon-dir-lg">
                    <path d="M10 4H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2h-8l-2-2z"/>
                  </svg>
                {:else}
                  <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2" class="icon-file-lg">
                    <path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/>
                    <polyline points="13 2 13 9 20 9"/>
                  </svg>
                {/if}
              </div>
              <span class="grid-name">{entry.name}</span>
            </div>
          {/each}
        </div>
      {/if}
    {/if}
  </div>

  <!-- ── Status bar ── -->
  <div class="status-bar">
    <span class="status-count">
      {#if app.selectedPaths.size > 0}
        {app.selectedPaths.size} of {sorted.length} selected
      {:else}
        {sorted.length} item{sorted.length !== 1 ? "s" : ""}
      {/if}
    </span>
    {#if app.isSearching}
      <span class="status-search">
        Search: "{app.searchQuery}"
        <button class="status-clear-search" onclick={() => { app.clearSearch(); if (app.currentPath) { lastLoadPath = null; loadPath(app.currentPath); } }}>
          Clear ✕
        </button>
      </span>
    {/if}
    {#if app.clipboard}
      <span class="status-clipboard">
        {app.clipboard.operation === "copy" ? "Copy" : "Cut"}: {app.clipboard.paths.length} item{app.clipboard.paths.length !== 1 ? "s" : ""}
      </span>
    {/if}
  </div>
</div>

<!-- ── Context menu ── -->
{#if contextMenu}
  <ContextMenu
    x={contextMenu.x}
    y={contextMenu.y}
    items={contextMenu.items}
    onclose={() => (contextMenu = null)}
  />
{/if}

<style>
  .content-panel {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    overflow: hidden;
    outline: none;
    background: var(--bg);
  }

  /* ── Column headers ── */
  .content-header {
    display: flex;
    align-items: center;
    height: 26px;
    padding: 0 8px;
    background: var(--surface);
    border-bottom: 1px solid var(--line);
    flex-shrink: 0;
  }

  .col-hdr {
    display: flex;
    align-items: center;
    gap: 4px;
    background: none;
    border: none;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-muted);
    cursor: pointer;
    padding: 0 4px;
    height: 100%;
    white-space: nowrap;

    &:hover { color: var(--text); }
  }

  .col-hdr--name     { flex: 1; min-width: 0; }
  .col-hdr--type     { width: 80px; flex-shrink: 0; }
  .col-hdr--size     { width: 90px; flex-shrink: 0; justify-content: flex-end; }
  .col-hdr--modified { width: 140px; flex-shrink: 0; padding-left: 8px; }

  .sort-arrow { color: var(--accent); font-size: 11px; }

  /* ── Content body ── */
  .content-body {
    flex: 1;
    overflow-y: auto;
    padding: 4px;
  }

  /* ── Empty / loading states ── */
  .content-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    height: 200px;
    color: var(--text-subtle);
    font-size: 13px;
  }

  .loading-spinner {
    width: 24px;
    height: 24px;
    border: 2.5px solid var(--line-strong);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }

  /* ── File list view ── */
  .file-list { display: flex; flex-direction: column; gap: 1px; }

  /* ── Grid view ── */
  .file-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(100px, 1fr));
    gap: 4px;
    padding: 4px;
  }

  .grid-item {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
    padding: 8px 6px;
    border-radius: 6px;
    cursor: pointer;
    user-select: none;
    border: 1px solid transparent;
    text-align: center;

    &:hover { background: var(--surface-hover); border-color: var(--line); }
  }

  .grid-item--selected {
    background: var(--accent-soft);
    border-color: var(--accent-border);
  }

  .grid-icon { display: flex; align-items: center; justify-content: center; }
  .icon-dir-lg { color: #e0a030; }
  .icon-file-lg { color: var(--text-muted); }

  .grid-name {
    font-size: 11px;
    color: var(--text);
    line-height: 1.3;
    word-break: break-all;
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  /* ── Status bar ── */
  .status-bar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 10px;
    height: 22px;
    font-size: 11px;
    color: var(--text-muted);
    background: var(--surface);
    border-top: 1px solid var(--line);
    flex-shrink: 0;
    overflow: hidden;
  }

  .status-count { flex-shrink: 0; }

  .status-search {
    display: flex;
    align-items: center;
    gap: 5px;
    color: var(--accent);
  }

  .status-clear-search {
    background: none;
    border: none;
    color: var(--accent);
    font: inherit;
    font-size: 11px;
    cursor: pointer;
    padding: 0;
    text-decoration: underline;
  }

  .status-clipboard {
    margin-left: auto;
    color: var(--text-subtle);
    font-style: italic;
  }
</style>
