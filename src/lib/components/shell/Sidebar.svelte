<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open } from "@tauri-apps/plugin-dialog";
  import { app } from "../../stores/app.svelte.js";
  import type { EntryDto, ActiveConnectionDto } from "../../types/index.js";
  import TreeNode from "./TreeNode.svelte";

  // ── State ────────────────────────────────────────────────
  let locationRoots = $state<Map<string, EntryDto>>(new Map());
  let disconnectingId = $state<string | null>(null);

  // ── Load entry info for local locations ──────────────────
  $effect(() => {
    const locs = app.settings.localLocations;
    locs.forEach(async (loc) => {
      if (locationRoots.has(loc)) return;
      try {
        const entry = await invoke<EntryDto>("inspect_path", { path: loc });
        locationRoots = new Map([...locationRoots, [loc, entry]]);
      } catch {
        // path may no longer exist — remove it silently
      }
    });
  });

  // ── Add local folder ─────────────────────────────────────
  async function addLocation() {
    try {
      const selected = await open({ directory: true, multiple: false, title: "Add location" });
      if (typeof selected === "string") app.addLocalLocation(selected);
    } catch {
      // dialog cancelled
    }
  }

  // ── Disconnect remote session ─────────────────────────────
  async function disconnect(conn: ActiveConnectionDto) {
    disconnectingId = conn.sessionId;
    try {
      await invoke("disconnect_connection", { sessionId: conn.sessionId });
      app.removeActiveConnection(conn.sessionId);
      // If we were browsing this session, go home
      if (app.currentPath?.startsWith(`remote://${conn.sessionId}`)) {
        app.navigate(app.settings.localLocations[0] ?? "");
      }
    } catch (e) {
      app.notify("error", String(e));
    } finally {
      disconnectingId = null;
    }
  }

  const PROTO_COLORS: Record<string, string> = {
    ssh: "#2a7db5", smb: "#7a5fb5", ftp: "#3f9a6a", ftps: "#2a8060",
  };

  function protoLabel(p: string) { return p.toUpperCase(); }
</script>

<aside class="sidebar" aria-label="Locations">
  <!-- ── Header ── -->
  <div class="sb-header">
    <span class="sb-title">Locations</span>
    <div class="sb-header-btns">
      <button class="sb-icon-btn" onclick={addLocation} title="Add local folder" aria-label="Add local folder">
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
          <line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
        </svg>
      </button>
      <button
        class="sb-icon-btn"
        class:sb-icon-btn--active={app.connectionManagerOpen}
        onclick={() => app.openConnectionManager()}
        title="Manage connections"
        aria-label="Manage connections"
      >
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
          <rect x="2" y="3" width="20" height="14" rx="2"/>
          <path d="M8 21h8M12 17v4"/>
        </svg>
      </button>
    </div>
  </div>

  <div class="sb-body scrollbar-thin">

    <!-- ── Local locations ── -->
    {#if app.settings.localLocations.length > 0}
      <div class="sb-section">
        <div class="sb-section-label">Local</div>
        {#each app.settings.localLocations as loc (loc)}
          {@const entry = locationRoots.get(loc)}
          {#if entry}
            <div class="sb-location-row">
              <div class="sb-location-main">
                <TreeNode {entry} depth={0} />
              </div>
              <button
                class="sb-remove-btn"
                onclick={() => app.removeLocalLocation(loc)}
                title="Remove from sidebar"
                aria-label="Remove {loc} from sidebar"
              >
                <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                  <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
                </svg>
              </button>
            </div>
          {:else}
            <div class="sb-loc-loading">
              <span class="spinner-xs"></span>
              <span class="sb-loc-path">{loc}</span>
            </div>
          {/if}
        {/each}
      </div>
    {:else}
      <div class="sb-empty">
        <svg width="24" height="24" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
        </svg>
        <p>No locations added.</p>
        <button class="sb-add-link" onclick={addLocation}>Add a folder</button>
      </div>
    {/if}

    <!-- ── Remote sessions ── -->
    {#if app.activeConnections.length > 0}
      <div class="sb-section">
        <div class="sb-section-label">Remote</div>
        {#each app.activeConnections as conn (conn.sessionId)}
          <div class="sb-remote-item" class:sb-remote-item--active={app.currentPath?.startsWith(`remote://${conn.sessionId}`)}>
            <button
              class="sb-remote-main"
              onclick={() => app.navigate(conn.rootPath)}
              title="{conn.protocol.toUpperCase()} · {conn.host}"
            >
              <span class="sb-remote-dot" style="background:{PROTO_COLORS[conn.protocol] ?? 'var(--accent)'}"></span>
              <span class="sb-remote-label">{conn.label || conn.host}</span>
              <span class="sb-proto-badge" style="background:{PROTO_COLORS[conn.protocol]}20;color:{PROTO_COLORS[conn.protocol]}">
                {protoLabel(conn.protocol)}
              </span>
            </button>
            <button
              class="sb-disconnect-btn"
              onclick={() => disconnect(conn)}
              disabled={disconnectingId === conn.sessionId}
              title="Disconnect"
              aria-label="Disconnect {conn.label}"
            >
              {#if disconnectingId === conn.sessionId}
                <span class="spinner-xs"></span>
              {:else}
                <svg width="11" height="11" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                  <path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/>
                  <polyline points="16 17 21 12 16 7"/>
                  <line x1="21" y1="12" x2="9" y2="12"/>
                </svg>
              {/if}
            </button>
          </div>
        {/each}
      </div>
    {/if}
  </div>
</aside>

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    width: 220px;
    min-width: 160px;
    max-width: 320px;
    height: 100%;
    background: var(--surface-alt);
    border-right: 1px solid var(--line);
    overflow: hidden;
    flex-shrink: 0;
  }

  .sb-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    padding: 8px 8px 6px;
    flex-shrink: 0;
    border-bottom: 1px solid var(--line);
    gap: 4px;
  }

  .sb-title {
    font-size: 11px;
    font-weight: 600;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-subtle);
  }

  .sb-header-btns {
    display: flex;
    gap: 2px;
  }

  .sb-icon-btn {
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    color: var(--text-muted);
    border-radius: 4px;
    cursor: pointer;

    &:hover { background: var(--surface-hover); color: var(--text); }
  }

  .sb-icon-btn--active {
    background: var(--accent-soft);
    color: var(--accent);
  }

  .sb-body {
    flex: 1;
    overflow-y: auto;
    padding: 4px;
  }

  .sb-section {
    margin-bottom: 8px;
  }

  .sb-section-label {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-subtle);
    padding: 4px 6px 2px;
  }

  /* Local location row with remove button */
  .sb-location-row {
    display: flex;
    align-items: center;
    gap: 2px;

    &:hover .sb-remove-btn { opacity: 1; }
  }

  .sb-location-main {
    flex: 1;
    min-width: 0;
  }

  .sb-remove-btn {
    flex-shrink: 0;
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    color: var(--text-subtle);
    border-radius: 3px;
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.1s;

    &:hover { background: var(--danger-soft); color: var(--danger); opacity: 1; }
  }

  /* Remote session item */
  .sb-remote-item {
    display: flex;
    align-items: center;
    border-radius: 5px;
    overflow: hidden;

    &:hover { background: var(--surface-hover); }
    &:hover .sb-disconnect-btn { opacity: 1; }
  }

  .sb-remote-item--active {
    background: var(--accent-soft);
    .sb-remote-label { color: var(--accent); }
  }

  .sb-remote-main {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 6px;
    background: none;
    border: none;
    text-align: left;
    cursor: pointer;
    font: inherit;
    color: var(--text);
  }

  .sb-remote-dot {
    width: 7px;
    height: 7px;
    border-radius: 50%;
    flex-shrink: 0;
  }

  .sb-remote-label {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .sb-proto-badge {
    font-size: 9px;
    font-weight: 700;
    padding: 1px 4px;
    border-radius: 3px;
    letter-spacing: 0.04em;
    flex-shrink: 0;
  }

  .sb-disconnect-btn {
    flex-shrink: 0;
    width: 24px;
    height: 100%;
    min-height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    opacity: 0;
    transition: opacity 0.1s;

    &:hover { background: var(--danger-soft); color: var(--danger); opacity: 1; }
    &:disabled { opacity: 0.4; cursor: default; }
  }

  /* Empty state */
  .sb-empty {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 6px;
    padding: 24px 12px;
    color: var(--text-subtle);
    text-align: center;
    font-size: 12px;

    p { margin: 0; }
  }

  .sb-add-link {
    background: none;
    border: none;
    color: var(--accent);
    font: inherit;
    font-size: 12px;
    text-decoration: underline;
    cursor: pointer;
    padding: 0;
  }

  .sb-loc-loading {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 8px;
    font-size: 12px;
    color: var(--text-subtle);
  }

  .sb-loc-path {
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    flex: 1;
    min-width: 0;
  }

  .spinner-xs {
    display: inline-block;
    width: 10px;
    height: 10px;
    border: 1.5px solid transparent;
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
    flex-shrink: 0;
  }
</style>
