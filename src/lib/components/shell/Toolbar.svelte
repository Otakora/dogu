<script lang="ts">
  import { app } from "../../stores/app.svelte.js";
  import { t } from "../../i18n/index.js";
  import { openTerminalAt } from "../../utils/terminal.js";
  import ContextMenu from "../ui/ContextMenu.svelte";
  import type { MenuItem } from "../ui/ContextMenu.svelte";

  type Props = {
    onRefresh: () => void;
    onNewFolder: () => void;
    onNewFile: () => void;
    onSearch: (q: string) => void;
    onClearSearch?: () => void;
  };

  let { onRefresh, onNewFolder, onNewFile, onSearch, onClearSearch }: Props = $props();

  let searchInput = $state("");
  let searchFocused = $state(false);

  // ── Editable path state ──────────────────────────────────
  let editingPath = $state(false);
  let editPathValue = $state("");

  function enterPathEdit() {
    if (!app.currentPath) return;
    editPathValue = app.currentPath;
    editingPath = true;
  }

  function commitPathEdit() {
    const val = editPathValue.trim();
    editingPath = false;
    if (val && val !== app.currentPath) app.navigate(val);
  }

  function cancelPathEdit() {
    editingPath = false;
  }

  function handlePathInputKey(e: KeyboardEvent) {
    if (e.key === "Enter") { e.preventDefault(); commitPathEdit(); }
    if (e.key === "Escape") { e.preventDefault(); cancelPathEdit(); }
  }

  function autofocusSelect(node: HTMLInputElement) {
    node.focus();
    node.select();
  }

  function handleBreadcrumbAreaClick(e: MouseEvent) {
    if ((e.target as HTMLElement).closest(".tb-crumb, .tb-sep")) return;
    if (!editingPath && app.currentPath) enterPathEdit();
  }

  // ── Context menu ─────────────────────────────────────────
  let contextMenu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

  function openCrumbMenu(e: MouseEvent, crumb: { label: string; path: string }) {
    e.preventDefault();
    e.stopPropagation();
    const isRemote = crumb.path.startsWith("remote://");
    const items: MenuItem[] = [
      {
        kind: "action",
        label: t("sidebar.open"),
        icon: ICON_FOLDER,
        onclick: () => app.navigate(crumb.path),
      },
      {
        kind: "action",
        label: t("tabs.openInNewTab"),
        icon: ICON_NEW_TAB,
        onclick: () => app.addTab(crumb.path),
      },
      { kind: "separator" },
      {
        kind: "action",
        label: t("tabs.copyPath"),
        icon: ICON_COPY,
        onclick: () => {
          navigator.clipboard.writeText(crumb.path);
          app.notify("info", t("sidebar.pathCopied"));
        },
      },
    ];

    if (!isRemote) {
      items.push({ kind: "separator" });
      if (app.isFavorite(crumb.path)) {
        items.push({
          kind: "action",
          label: t("menu.removeFromFavorites"),
          icon: ICON_STAR_FILLED,
          onclick: () => app.removeFavorite(crumb.path),
        });
      } else {
        items.push({
          kind: "action",
          label: t("menu.addToFavorites"),
          icon: ICON_STAR,
          onclick: () => app.addFavorite(crumb.path),
        });
      }
    }

    contextMenu = { x: e.clientX, y: e.clientY, items };
  }

  // ── Breadcrumb ───────────────────────────────────────────
  const breadcrumbs = $derived(buildBreadcrumbs(app.currentPath));

  function buildBreadcrumbs(path: string | null): { label: string; path: string }[] {
    if (!path) return [];

    const remoteMatch = path.match(/^remote:\/\/([^/]+)(\/.*)?$/);
    if (remoteMatch) {
      const sessionId = remoteMatch[1];
      const rest = (remoteMatch[2] ?? "").split("/").filter(Boolean);
      const conn = app.activeConnections.find(c => c.sessionId === sessionId);
      const rootLabel = conn?.label ?? conn?.host ?? sessionId;
      const crumbs: { label: string; path: string }[] = [
        { label: rootLabel, path: `remote://${sessionId}/` },
      ];
      let acc = `remote://${sessionId}`;
      for (const seg of rest) {
        acc += `/${seg}`;
        crumbs.push({ label: seg, path: acc });
      }
      return crumbs;
    }

    const winMatch = path.match(/^([A-Za-z]:\\)(.*)/);
    if (winMatch) {
      const drive = winMatch[1];
      const rest = winMatch[2].split(/[\\/]/).filter(Boolean);
      const crumbs: { label: string; path: string }[] = [{ label: drive, path: drive }];
      let acc = drive;
      for (const seg of rest) {
        acc += seg + "\\";
        crumbs.push({ label: seg, path: acc.replace(/\\$/, "") });
      }
      return crumbs;
    }

    if (path.startsWith("/")) {
      const parts = path.split("/").filter(Boolean);
      const crumbs: { label: string; path: string }[] = [{ label: "/", path: "/" }];
      let acc = "";
      for (const seg of parts) {
        acc += "/" + seg;
        crumbs.push({ label: seg, path: acc });
      }
      return crumbs;
    }

    return [{ label: path, path }];
  }

  // ── Search ────────────────────────────────────────────────
  // Sync toolbar input when search is cleared from ContentPanel
  $effect(() => {
    if (!app.isSearching) searchInput = "";
    else if (app.searchQuery && searchInput !== app.searchQuery) searchInput = app.searchQuery;
  });

  function submitSearch() {
    if (searchInput.trim()) onSearch(searchInput.trim());
  }

  function clearSearch() {
    searchInput = "";
    app.clearSearch();
    onClearSearch?.();
  }

  function handleSearchKey(e: KeyboardEvent) {
    if (e.key === "Enter") submitSearch();
    if (e.key === "Escape") clearSearch();
  }

  // ── Favorite state for current path ──────────────────────
  const isFavorited = $derived(
    !!app.currentPath &&
    !app.currentPath.startsWith("remote://") &&
    app.isFavorite(app.currentPath)
  );

  const canFavorite = $derived(
    !!app.currentPath && !app.currentPath.startsWith("remote://")
  );

  // ── Icon strings ─────────────────────────────────────────
  const ICON_FOLDER    = `<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><path d="M10 4H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2h-8l-2-2z"/></svg>`;
  const ICON_NEW_TAB   = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M9 3v18M3 9h6"/></svg>`;
  const ICON_COPY      = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>`;
  const ICON_STAR      = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>`;
  const ICON_STAR_FILLED = `<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>`;
</script>

<div class="toolbar">
  <!-- ── Navigation buttons ── -->
  <div class="tb-nav">
    <button
      class="tb-btn"
      onclick={() => app.navigateBack()}
      disabled={!app.canGoBack}
      title={t("toolbar.back")}
      aria-label={t("toolbar.back")}
    >
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2">
        <polyline points="15 18 9 12 15 6"/>
      </svg>
    </button>
    <button
      class="tb-btn"
      onclick={() => app.navigateForward()}
      disabled={!app.canGoForward}
      title={t("toolbar.forward")}
      aria-label={t("toolbar.forward")}
    >
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2">
        <polyline points="9 18 15 12 9 6"/>
      </svg>
    </button>
    <button
      class="tb-btn"
      onclick={() => app.navigateUp()}
      disabled={!app.currentPath}
      title={t("toolbar.up")}
      aria-label={t("toolbar.up")}
    >
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2">
        <polyline points="18 15 12 9 6 15"/>
      </svg>
    </button>
    <button
      class="tb-btn"
      onclick={onRefresh}
      disabled={!app.currentPath}
      title={t("toolbar.refresh")}
      aria-label={t("menu.refresh")}
    >
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <polyline points="23 4 23 10 17 10"/>
        <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/>
      </svg>
    </button>
  </div>

  <!-- ── Breadcrumb / editable path ── -->
  <div
    class="tb-breadcrumb"
    class:tb-breadcrumb--editing={editingPath}
    aria-label={t("toolbar.currentPath")}
    role="textbox"
    tabindex={editingPath ? -1 : 0}
    aria-readonly={!editingPath}
    onclick={handleBreadcrumbAreaClick}
    onkeydown={(e) => { if (e.key === "Enter" && !editingPath && app.currentPath) enterPathEdit(); }}
  >
    {#if editingPath}
      <input
        use:autofocusSelect
        class="tb-path-input"
        type="text"
        bind:value={editPathValue}
        onkeydown={handlePathInputKey}
        onblur={cancelPathEdit}
        placeholder={t("toolbar.editPath")}
        aria-label={t("toolbar.editPath")}
        spellcheck="false"
        autocomplete="off"
      />
    {:else}
      {#each breadcrumbs as crumb, i}
        {#if i > 0}
          <span class="tb-sep" aria-hidden="true">›</span>
        {/if}
        <button
          class="tb-crumb"
          class:tb-crumb--last={i === breadcrumbs.length - 1}
          onclick={(e) => { e.stopPropagation(); app.navigate(crumb.path); }}
          oncontextmenu={(e) => openCrumbMenu(e, crumb)}
        >
          {crumb.label}
        </button>
      {/each}
      {#if !app.currentPath}
        <span class="tb-crumb-placeholder">{t("toolbar.noLocationSelected")}</span>
      {/if}
    {/if}
  </div>

  <!-- ── Favorite button ── -->
  {#if canFavorite}
    <button
      class="tb-btn"
      class:tb-btn--starred={isFavorited}
      onclick={() => app.currentPath && app.toggleFavorite(app.currentPath)}
      title={isFavorited ? t("toolbar.removeFromFavorites") : t("toolbar.addToFavorites")}
      aria-label={isFavorited ? t("toolbar.removeFromFavorites") : t("toolbar.addToFavorites")}
      aria-pressed={isFavorited}
    >
      {#if isFavorited}
        <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor">
          <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/>
        </svg>
      {:else}
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/>
        </svg>
      {/if}
    </button>
  {:else}
    <div class="tb-btn-placeholder"></div>
  {/if}

  <!-- ── Action buttons ── -->
  <div class="tb-actions">
    <button
      class="tb-btn"
      onclick={onNewFolder}
      disabled={!app.currentPath}
      title={t("toolbar.newFolder")}
      aria-label={t("toolbar.newFolder")}
    >
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
        <line x1="12" y1="11" x2="12" y2="17"/>
        <line x1="9" y1="14" x2="15" y2="14"/>
      </svg>
    </button>
    <button
      class="tb-btn"
      onclick={onNewFile}
      disabled={!app.currentPath}
      title={t("toolbar.newFile")}
      aria-label={t("toolbar.newFile")}
    >
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/>
        <polyline points="13 2 13 9 20 9"/>
        <line x1="12" y1="13" x2="12" y2="19"/>
        <line x1="9" y1="16" x2="15" y2="16"/>
      </svg>
    </button>

    <!-- View mode toggle -->
    <div class="tb-view-toggle" role="group" aria-label={t("toolbar.viewMode")}>
      <button
        class="tb-btn tb-btn--toggle"
        class:tb-btn--toggle-active={app.viewMode === "list"}
        onclick={() => app.setViewMode("list")}
        title={t("toolbar.listView")}
        aria-label={t("toolbar.listView")}
        aria-pressed={app.viewMode === "list"}
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <line x1="8" y1="6" x2="21" y2="6"/>
          <line x1="8" y1="12" x2="21" y2="12"/>
          <line x1="8" y1="18" x2="21" y2="18"/>
          <line x1="3" y1="6" x2="3.01" y2="6"/>
          <line x1="3" y1="12" x2="3.01" y2="12"/>
          <line x1="3" y1="18" x2="3.01" y2="18"/>
        </svg>
      </button>
      <button
        class="tb-btn tb-btn--toggle"
        class:tb-btn--toggle-active={app.viewMode === "grid"}
        onclick={() => app.setViewMode("grid")}
        title={t("toolbar.gridView")}
        aria-label={t("toolbar.gridView")}
        aria-pressed={app.viewMode === "grid"}
      >
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <rect x="3" y="3" width="7" height="7"/><rect x="14" y="3" width="7" height="7"/>
          <rect x="14" y="14" width="7" height="7"/><rect x="3" y="14" width="7" height="7"/>
        </svg>
      </button>
    </div>
  </div>

  <!-- ── Search ── -->
  <div class="tb-search" class:tb-search--focused={searchFocused}>
    <svg class="tb-search-icon" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>
    </svg>
    <input
      class="tb-search-input"
      type="search"
      placeholder={t("toolbar.searchPlaceholder")}
      bind:value={searchInput}
      onfocus={() => (searchFocused = true)}
      onblur={() => (searchFocused = false)}
      onkeydown={handleSearchKey}
      aria-label={t("toolbar.searchFiles")}
    />
    {#if searchInput}
      <button class="tb-search-clear" onclick={clearSearch} aria-label={t("toolbar.clearSearch")}>✕</button>
    {/if}
  </div>

  <!-- ── Queue mode toggle ── -->
  <button
    class="tb-btn"
    class:tb-btn--active={app.queueMode}
    onclick={() => app.toggleQueueMode()}
    title={t("toolbar.queueMode")}
    aria-label={t("toolbar.queueMode")}
    aria-pressed={app.queueMode}
  >
    <!-- Stack of lines with play arrow -->
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <line x1="3" y1="6" x2="15" y2="6"/>
      <line x1="3" y1="12" x2="15" y2="12"/>
      <line x1="3" y1="18" x2="15" y2="18"/>
      <polyline points="18 9 21 12 18 15"/>
    </svg>
  </button>

  <!-- ── Terminal toggle ── -->
  <button
    class="tb-btn"
    class:tb-btn--active={app.terminalPanelOpen}
    onclick={() => {
      if (!app.terminalPanelOpen && app.terminalTabs.length === 0 && app.currentPath) {
        openTerminalAt(app.currentPath);
      } else {
        app.toggleTerminalPanel();
      }
    }}
    title={t("terminal.openTerminal") + " (Ctrl+`)"}
    aria-label={t("terminal.openTerminal")}
    aria-pressed={app.terminalPanelOpen}
  >
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/>
    </svg>
  </button>

  <!-- ── Settings ── -->
  <button class="tb-btn" onclick={() => app.openSettings()} title={t("toolbar.settings")} aria-label={t("toolbar.settings")}>
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <circle cx="12" cy="12" r="3"/>
      <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
    </svg>
  </button>
</div>

{#if contextMenu}
  <ContextMenu
    x={contextMenu.x}
    y={contextMenu.y}
    items={contextMenu.items}
    onclose={() => (contextMenu = null)}
  />
{/if}

<style>
  .toolbar {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 4px 8px;
    background: var(--surface);
    border-bottom: 1px solid var(--line);
    height: 38px;
    flex-shrink: 0;
    overflow: hidden;
  }

  /* ── Nav buttons ── */
  .tb-nav {
    display: flex;
    gap: 1px;
    flex-shrink: 0;
  }

  .tb-btn {
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    color: var(--text-muted);
    border-radius: 5px;
    cursor: pointer;
    flex-shrink: 0;
    transition: background 0.1s, color 0.1s;

    &:hover:not(:disabled) {
      background: var(--surface-hover);
      color: var(--text);
    }

    &:disabled { opacity: 0.35; cursor: default; }
  }

  .tb-btn--starred {
    color: #f59e0b;

    &:hover:not(:disabled) { color: #d97706; }
  }

  .tb-btn--active {
    background: var(--accent-soft);
    color: var(--accent);

    &:hover:not(:disabled) {
      background: color-mix(in srgb, var(--accent) 20%, transparent);
      color: var(--accent);
    }
  }

  .tb-btn-placeholder {
    width: 28px;
    height: 28px;
    flex-shrink: 0;
  }

  .tb-btn--toggle {
    border-radius: 0;
    &:first-child { border-radius: 5px 0 0 5px; }
    &:last-child { border-radius: 0 5px 5px 0; }
  }

  .tb-view-toggle {
    display: flex;
    border: 1px solid var(--line-strong);
    border-radius: 5px;
    overflow: hidden;
  }

  .tb-btn--toggle-active {
    background: var(--accent-soft);
    color: var(--accent);

    &:hover:not(:disabled) {
      background: var(--accent-soft);
      color: var(--accent);
    }
  }

  /* ── Breadcrumb ── */
  .tb-breadcrumb {
    flex: 1;
    display: flex;
    align-items: center;
    gap: 1px;
    overflow: hidden;
    min-width: 0;
    background: var(--surface-alt);
    border: 1px solid var(--line-strong);
    border-radius: 6px;
    padding: 0 6px;
    height: 26px;
    cursor: text;
    transition: border-color 0.15s;

    &:hover { border-color: var(--accent); }
    &.tb-breadcrumb--editing {
      border-color: var(--accent);
      box-shadow: 0 0 0 2px var(--accent-soft);
      cursor: default;
    }
  }

  .tb-sep {
    color: var(--text-subtle);
    font-size: 12px;
    padding: 0 1px;
    flex-shrink: 0;
    cursor: default;
  }

  .tb-crumb {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 12px;
    padding: 2px 4px;
    border-radius: 4px;
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 200px;
    flex-shrink: 1;

    &:hover { background: var(--surface-hover); color: var(--text); }
  }

  .tb-crumb--last {
    color: var(--text);
    font-weight: 500;
  }

  .tb-crumb-placeholder {
    font-size: 12px;
    color: var(--text-subtle);
    padding: 0 4px;
    cursor: text;
  }

  /* ── Path edit input ── */
  .tb-path-input {
    flex: 1;
    background: none;
    border: none;
    outline: none;
    font-size: 12px;
    font-family: var(--font-mono, monospace);
    color: var(--text);
    padding: 0 2px;
    min-width: 0;

    &::placeholder { color: var(--text-subtle); }
  }

  /* ── Action buttons ── */
  .tb-actions {
    display: flex;
    gap: 2px;
    flex-shrink: 0;
  }

  /* ── Search ── */
  .tb-search {
    display: flex;
    align-items: center;
    gap: 5px;
    background: var(--surface-alt);
    border: 1px solid var(--line-strong);
    border-radius: 6px;
    padding: 0 8px;
    height: 26px;
    width: 180px;
    flex-shrink: 0;
    transition: border-color 0.15s, box-shadow 0.15s;
  }

  .tb-search--focused {
    border-color: var(--accent);
    box-shadow: 0 0 0 2px var(--accent-soft);
  }

  .tb-search-icon {
    color: var(--text-subtle);
    flex-shrink: 0;
  }

  .tb-search-input {
    flex: 1;
    background: none;
    border: none;
    font-size: 12px;
    color: var(--text);
    outline: none;
    min-width: 0;

    &::placeholder { color: var(--text-subtle); }
    &::-webkit-search-cancel-button { display: none; }
  }

  .tb-search-clear {
    background: none;
    border: none;
    color: var(--text-subtle);
    font-size: 11px;
    cursor: pointer;
    padding: 0;
    line-height: 1;
    flex-shrink: 0;

    &:hover { color: var(--text); }
  }
</style>
