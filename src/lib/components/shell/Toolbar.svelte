<script lang="ts">
  import { app } from "../../stores/app.svelte.js";

  type Props = {
    onRefresh: () => void;
    onNewFolder: () => void;
    onSearch: (q: string) => void;
  };

  let { onRefresh, onNewFolder, onSearch }: Props = $props();

  let searchInput = $state("");
  let searchFocused = $state(false);

  // ── Breadcrumb ───────────────────────────────────────────
  const breadcrumbs = $derived(buildBreadcrumbs(app.currentPath));

  function buildBreadcrumbs(path: string | null): { label: string; path: string }[] {
    if (!path) return [];

    // Remote path: remote://session-abc123/a/b/c
    const remoteMatch = path.match(/^remote:\/\/([^/]+)(\/.*)?$/);
    if (remoteMatch) {
      const sessionId = remoteMatch[1];
      const rest = (remoteMatch[2] ?? "").split("/").filter(Boolean);
      const conn = app.activeConnections.find(c => c.sessionId === sessionId);
      const rootLabel = conn?.label ?? conn?.host ?? sessionId;

      const crumbs: { label: string; path: string }[] = [
        { label: rootLabel, path: `remote://${sessionId}` },
      ];
      let acc = `remote://${sessionId}`;
      for (const seg of rest) {
        acc += `/${seg}`;
        crumbs.push({ label: seg, path: acc });
      }
      return crumbs;
    }

    // Windows path: C:\foo\bar
    const winMatch = path.match(/^([A-Za-z]:\\)(.*)/);
    if (winMatch) {
      const drive = winMatch[1];
      const rest = winMatch[2].split(/[\\/]/).filter(Boolean);
      const crumbs: { label: string; path: string }[] = [
        { label: drive, path: drive },
      ];
      let acc = drive;
      for (const seg of rest) {
        acc += seg + "\\";
        crumbs.push({ label: seg, path: acc.replace(/\\$/, "") });
      }
      return crumbs;
    }

    // Unix path: /a/b/c
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

  function submitSearch() {
    if (searchInput.trim()) onSearch(searchInput.trim());
  }

  function clearSearch() {
    searchInput = "";
    app.clearSearch();
  }

  function handleSearchKey(e: KeyboardEvent) {
    if (e.key === "Enter") submitSearch();
    if (e.key === "Escape") clearSearch();
  }
</script>

<div class="toolbar">
  <!-- ── Navigation buttons ── -->
  <div class="tb-nav">
    <button
      class="tb-btn"
      onclick={() => app.navigateBack()}
      disabled={!app.canGoBack}
      title="Back"
      aria-label="Back"
    >
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2">
        <polyline points="15 18 9 12 15 6"/>
      </svg>
    </button>
    <button
      class="tb-btn"
      onclick={() => app.navigateForward()}
      disabled={!app.canGoForward}
      title="Forward"
      aria-label="Forward"
    >
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2">
        <polyline points="9 18 15 12 9 6"/>
      </svg>
    </button>
    <button
      class="tb-btn"
      onclick={() => app.navigateUp()}
      disabled={!app.currentPath}
      title="Up"
      aria-label="Up"
    >
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2">
        <polyline points="18 15 12 9 6 15"/>
      </svg>
    </button>
    <button
      class="tb-btn"
      onclick={onRefresh}
      disabled={!app.currentPath}
      title="Refresh (F5)"
      aria-label="Refresh"
    >
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <polyline points="23 4 23 10 17 10"/>
        <path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/>
      </svg>
    </button>
  </div>

  <!-- ── Breadcrumb ── -->
  <div class="tb-breadcrumb" aria-label="Current path">
    {#each breadcrumbs as crumb, i}
      {#if i > 0}
        <span class="tb-sep" aria-hidden="true">›</span>
      {/if}
      <button
        class="tb-crumb"
        class:tb-crumb--last={i === breadcrumbs.length - 1}
        onclick={() => app.navigate(crumb.path)}
      >
        {crumb.label}
      </button>
    {/each}
    {#if !app.currentPath}
      <span class="tb-crumb-placeholder">No location selected</span>
    {/if}
  </div>

  <!-- ── Action buttons ── -->
  <div class="tb-actions">
    <button
      class="tb-btn"
      onclick={onNewFolder}
      disabled={!app.currentPath}
      title="New folder"
      aria-label="New folder"
    >
      <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
        <line x1="12" y1="11" x2="12" y2="17"/>
        <line x1="9" y1="14" x2="15" y2="14"/>
      </svg>
    </button>

    <!-- View mode toggle -->
    <div class="tb-view-toggle" role="group" aria-label="View mode">
      <button
        class="tb-btn tb-btn--toggle"
        class:tb-btn--toggle-active={app.viewMode === "list"}
        onclick={() => app.setViewMode("list")}
        title="List view"
        aria-label="List view"
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
        title="Grid view"
        aria-label="Grid view"
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
      placeholder="Search…"
      bind:value={searchInput}
      onfocus={() => (searchFocused = true)}
      onblur={() => (searchFocused = false)}
      onkeydown={handleSearchKey}
      aria-label="Search files"
    />
    {#if searchInput}
      <button class="tb-search-clear" onclick={clearSearch} aria-label="Clear search">✕</button>
    {/if}
  </div>

  <!-- ── Settings ── -->
  <button class="tb-btn" onclick={() => app.openSettings()} title="Settings" aria-label="Settings">
    <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
      <circle cx="12" cy="12" r="3"/>
      <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
    </svg>
  </button>
</div>

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
  }

  .tb-sep {
    color: var(--text-subtle);
    font-size: 12px;
    padding: 0 1px;
    flex-shrink: 0;
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
