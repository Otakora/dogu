<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { app } from "../../stores/app.svelte.js";
  import type { EntryDto, ActiveConnectionDto, VolumeDto } from "../../types/index.js";
  import TreeNode, { type TabHighlight } from "./TreeNode.svelte";
  import ContextMenu from "../ui/ContextMenu.svelte";
  import type { MenuItem } from "../ui/ContextMenu.svelte";
  import { t, type TranslationKey } from "../../i18n/index.js";
  import { openTerminalAt } from "../../utils/terminal.js";

  // ── State ────────────────────────────────────────────────
  let disconnectingId = $state<string | null>(null);
  let contextMenu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

  // Section collapse
  let collapsed = $state({ favorites: false, computer: false, connections: false });

  // ── Tab highlights (for TreeNode coloring) ───────────────
  // Uses allTabHighlights to aggregate highlights from ALL panes (split-pane aware)
  const tabHighlights = $derived<TabHighlight[]>(app.allTabHighlights);

  // ── Flat item highlight (for favorites & drives) ─────────
  function computeHighlight(path: string): string | null {
    if (!path) return null;
    let bestColor: string | null = null;
    let bestScore = -1;
    for (const th of app.allTabHighlights) {
      if (!th.path) continue;
      const tp = th.path;
      const isExact = tp === path;
      const isAncestor = !isExact && (
        tp.startsWith(path + "/") ||
        tp.startsWith(path + "\\") ||
        ((path.endsWith("/") || path.endsWith("\\")) && tp.startsWith(path))
      );
      if (!isExact && !isAncestor) continue;
      const score = (isExact ? 3 : 0) + (th.isActive ? 1 : 0);
      if (score > bestScore) { bestScore = score; bestColor = th.color; }
    }
    return bestColor;
  }

  // ── Known folder items ───────────────────────────────────
  type KnownFolderItem = { key: string; labelKey: TranslationKey; path: string; icon: string };

  const knownFolderItems = $derived.by<KnownFolderItem[]>(() => {
    const kf = app.knownFolders;
    if (!kf) return [];
    const result: KnownFolderItem[] = [];
    const add = (key: string, labelKey: TranslationKey, path: string | null, icon: string) => {
      if (path) result.push({ key, labelKey, path, icon });
    };
    add("home",      "sidebar.home",      kf.home,      ICON_HOME);
    add("desktop",   "sidebar.desktop",   kf.desktop,   ICON_DESKTOP);
    add("documents", "sidebar.documents", kf.documents, ICON_DOCS);
    add("downloads", "sidebar.downloads", kf.downloads, ICON_DOWNLOADS);
    add("pictures",  "sidebar.pictures",  kf.pictures,  ICON_PICTURES);
    add("music",     "sidebar.music",     kf.music,     ICON_MUSIC);
    add("videos",    "sidebar.videos",    kf.videos,    ICON_VIDEOS);
    return result;
  });

  // ── Bytes formatter ──────────────────────────────────────
  function formatBytes(bytes: number): string {
    if (bytes < 1024) return `${bytes} B`;
    if (bytes < 1024 * 1024) return `${(bytes / 1024).toFixed(0)} KB`;
    if (bytes < 1024 * 1024 * 1024) return `${(bytes / 1024 / 1024).toFixed(1)} MB`;
    if (bytes < 1024 ** 4) return `${(bytes / 1024 / 1024 / 1024).toFixed(1)} GB`;
    return `${(bytes / 1024 ** 4).toFixed(1)} TB`;
  }

  // ── Remote connection fake root entry ────────────────────
  function makeRemoteRootEntry(conn: ActiveConnectionDto): EntryDto {
    return {
      path: conn.rootPath,
      name: conn.label || conn.host,
      isDir: true,
      size: 0,
      sizeLabel: "",
      modifiedTs: 0,
      modifiedLabel: "",
      extension: "",
      hasChildren: true,
      hasDirectoryChildren: true,
      locationKind: "remote",
      displayPath: conn.displayPath,
      rootLabel: null,
    };
  }

  // ── Context menus ────────────────────────────────────────
  function openFlatItemMenu(e: MouseEvent, path: string, canRemove: boolean) {
    e.preventDefault();
    const items: MenuItem[] = [
      {
        kind: "action",
        label: t("sidebar.open"),
        icon: ICON_OPEN,
        onclick: () => app.navigate(path),
      },
      {
        kind: "action",
        label: t("tabs.openInNewTab"),
        icon: ICON_NEW_TAB,
        onclick: () => app.addTab(path),
      },
    ];

    if (app.isSplit) {
      const otherPane = app.getPaneView(app.focusedPaneIdx === 0 ? 1 : 0);
      items.push({
        kind: "action",
        label: t("sidebar.openInOtherPane"),
        icon: ICON_SPLIT,
        onclick: () => otherPane.navigate(path),
      });
    }

    items.push({ kind: "separator" });
    items.push({
      kind: "action",
      label: t("sidebar.copyPath"),
      icon: ICON_COPY,
      onclick: () => {
        navigator.clipboard.writeText(path);
        app.notify("info", t("sidebar.pathCopied"));
      },
    });

    items.push({ kind: "separator" });
    items.push({
      kind: "action",
      label: t("terminal.openTerminal"),
      icon: ICON_TERMINAL,
      onclick: () => openTerminalAt(path),
    });

    if (canRemove) {
      items.push({ kind: "separator" });
      items.push({
        kind: "action",
        label: t("sidebar.removeFavorite"),
        icon: ICON_REMOVE,
        danger: true,
        onclick: () => app.removeFavorite(path),
      });
    }

    contextMenu = { x: e.clientX, y: e.clientY, items };
  }

  function openVolumeMenu(e: MouseEvent, vol: VolumeDto) {
    e.preventDefault();
    const volItems: MenuItem[] = [
      { kind: "action", label: t("sidebar.open"), icon: ICON_OPEN, onclick: () => app.navigate(vol.path) },
      { kind: "action", label: t("tabs.openInNewTab"), icon: ICON_NEW_TAB, onclick: () => app.addTab(vol.path) },
    ];

    if (app.isSplit) {
      const otherPane = app.getPaneView(app.focusedPaneIdx === 0 ? 1 : 0);
      volItems.push({
        kind: "action",
        label: t("sidebar.openInOtherPane"),
        icon: ICON_SPLIT,
        onclick: () => otherPane.navigate(vol.path),
      });
    }

    volItems.push(
      { kind: "separator" },
      { kind: "action", label: t("terminal.openTerminal"), icon: ICON_TERMINAL, onclick: () => openTerminalAt(vol.path) },
      { kind: "separator" },
      {
        kind: "action",
        label: t("sidebar.copyPath"),
        icon: ICON_COPY,
        onclick: () => {
          navigator.clipboard.writeText(vol.path);
          app.notify("info", t("sidebar.pathCopied"));
        },
      },
    );

    contextMenu = { x: e.clientX, y: e.clientY, items: volItems };
  }

  function handleRemoteTreeContextMenu(e: MouseEvent, entry: EntryDto, conn: ActiveConnectionDto) {
    e.preventDefault();
    const isRoot = entry.path === conn.rootPath;
    const items: MenuItem[] = [
      { kind: "action", label: t("sidebar.open"), icon: ICON_OPEN, onclick: () => app.navigate(entry.path) },
      { kind: "action", label: t("tabs.openInNewTab"), icon: ICON_NEW_TAB, onclick: () => app.addTab(entry.path) },
    ];

    if (app.isSplit) {
      const otherPane = app.getPaneView(app.focusedPaneIdx === 0 ? 1 : 0);
      items.push({
        kind: "action",
        label: t("sidebar.openInOtherPane"),
        icon: ICON_SPLIT,
        onclick: () => otherPane.navigate(entry.path),
      });
    }

    items.push({ kind: "separator" });
    items.push({
      kind: "action",
      label: t("sidebar.copyPath"),
      icon: ICON_COPY,
      onclick: () => {
        navigator.clipboard.writeText(entry.path);
        app.notify("info", t("sidebar.pathCopied"));
      },
    });
    if (isRoot) {
      items.push({ kind: "separator" });
      items.push({
        kind: "action",
        label: t("sidebar.disconnect"),
        icon: ICON_DISCONNECT,
        danger: true,
        onclick: () => disconnect(conn),
      });
    }
    contextMenu = { x: e.clientX, y: e.clientY, items };
  }

  // ── Pin current folder ────────────────────────────────────
  function pinCurrentFolder() {
    const path = app.currentPath;
    if (!path || path.startsWith("remote://")) return;
    app.toggleFavorite(path);
  }

  // ── Disconnect remote session ─────────────────────────────
  async function disconnect(conn: ActiveConnectionDto) {
    disconnectingId = conn.sessionId;
    try {
      await invoke("disconnect_connection", { sessionId: conn.sessionId });
      app.removeActiveConnection(conn.sessionId);
      if (app.currentPath?.startsWith(`remote://${conn.sessionId}`)) {
        const fallback = app.knownFolders?.home ?? app.favoriteLocations[0] ?? null;
        if (fallback) app.navigate(fallback);
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

  // ── SVG icon strings ─────────────────────────────────────
  const ICON_HOME       = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M3 9l9-7 9 7v11a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><polyline points="9 22 9 12 15 12 15 22"/></svg>`;
  const ICON_DESKTOP    = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="2" y="3" width="20" height="14" rx="2"/><line x1="8" y1="21" x2="16" y2="21"/><line x1="12" y1="17" x2="12" y2="21"/></svg>`;
  const ICON_DOCS       = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M14 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V8z"/><polyline points="14 2 14 8 20 8"/></svg>`;
  const ICON_DOWNLOADS  = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>`;
  const ICON_PICTURES   = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2"/><circle cx="8.5" cy="8.5" r="1.5"/><polyline points="21 15 16 10 5 21"/></svg>`;
  const ICON_MUSIC      = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg>`;
  const ICON_VIDEOS     = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="23 7 16 12 23 17 23 7"/><rect x="1" y="5" width="15" height="14" rx="2"/></svg>`;
  const ICON_STAR       = `<svg width="13" height="13" viewBox="0 0 24 24" fill="currentColor"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>`;
  const ICON_DRIVE      = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><ellipse cx="12" cy="5" rx="9" ry="3"/><path d="M21 12c0 1.66-4 3-9 3s-9-1.34-9-3"/><path d="M3 5v14c0 1.66 4 3 9 3s9-1.34 9-3V5"/></svg>`;
  const ICON_DRIVE_USB  = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M12 2v8M5 9l7 7 7-7M5 22h14"/></svg>`;
  const ICON_OPEN       = `<svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor"><path d="M10 4H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2h-8l-2-2z"/></svg>`;
  const ICON_NEW_TAB    = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M9 3v18M3 9h6"/></svg>`;
  const ICON_COPY       = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>`;
  const ICON_REMOVE     = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>`;
  const ICON_DISCONNECT = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M9 21H5a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h4"/><polyline points="16 17 21 12 16 7"/><line x1="21" y1="12" x2="9" y2="12"/></svg>`;
  const ICON_TERMINAL   = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>`;
  const ICON_SPLIT      = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2"/><line x1="12" y1="3" x2="12" y2="21"/></svg>`;
</script>

<aside
  class="sidebar"
  aria-label={t("sidebar.locations")}
  onwheel={(e) => {
    if (!e.ctrlKey) return;
    e.preventDefault();
    const delta = e.deltaY < 0 ? 0.1 : -0.1;
    const next = Math.max(0.6, Math.min(2.0, Math.round((app.treeZoom + delta) * 10) / 10));
    app.setTreeZoom(next);
  }}
>
  <div class="sb-body scrollbar-thin">

    <!-- ══════════════════════════════════════════════════════ -->
    <!-- FAVORITOS                                              -->
    <!-- ══════════════════════════════════════════════════════ -->
    <div class="sb-section">
      <div class="sb-section-header" role="button" tabindex="0"
        onclick={() => (collapsed.favorites = !collapsed.favorites)}
        onkeydown={(e) => e.key === "Enter" && (collapsed.favorites = !collapsed.favorites)}
      >
        <svg class="sb-section-chevron" class:sb-section-chevron--collapsed={collapsed.favorites}
          width="10" height="10" viewBox="0 0 24 24" fill="currentColor">
          <path d="M8 5l8 7-8 7V5z"/>
        </svg>
        <span class="sb-section-label">{t("sidebar.favorites")}</span>
        {#if app.currentPath && !app.currentPath.startsWith("remote://")}
          <button
            class="sb-section-action"
            title={app.isFavorite(app.currentPath ?? "") ? t("sidebar.removeFavorite") : t("sidebar.pinCurrent")}
            onclick={(e) => { e.stopPropagation(); pinCurrentFolder(); }}
          >
            {#if app.isFavorite(app.currentPath ?? "")}
              <svg width="12" height="12" viewBox="0 0 24 24" fill="currentColor" opacity="0.7">
                <line x1="18" y1="6" x2="6" y2="18" stroke="currentColor" stroke-width="2"/>
                <line x1="6" y1="6" x2="18" y2="18" stroke="currentColor" stroke-width="2"/>
              </svg>
            {:else}
              <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <line x1="12" y1="5" x2="12" y2="19"/><line x1="5" y1="12" x2="19" y2="12"/>
              </svg>
            {/if}
          </button>
        {/if}
      </div>

      {#if !collapsed.favorites}
        <!-- Known folders -->
        {#each knownFolderItems as item (item.key)}
          {@const hColor = computeHighlight(item.path)}
          <button
            class="sb-flat-item"
            class:sb-flat-item--highlighted={!!hColor}
            style:--item-color={hColor ?? "transparent"}
            onclick={() => app.navigate(item.path)}
            oncontextmenu={(e) => openFlatItemMenu(e, item.path, false)}
            title={item.path}
          >
            <span class="sb-flat-icon" aria-hidden="true">{@html item.icon}</span>
            <span class="sb-flat-label">{t(item.labelKey)}</span>
          </button>
        {/each}

        <!-- User-pinned favorites -->
        {#if app.favoriteLocations.length > 0}
          {#if knownFolderItems.length > 0}
            <div class="sb-micro-divider"></div>
          {/if}
          {#each app.favoriteLocations as loc (loc)}
            {@const hColor = computeHighlight(loc)}
            {@const name = loc.split(/[/\\]/).filter(Boolean).pop() ?? loc}
            <div class="sb-flat-item-row">
              <button
                class="sb-flat-item sb-flat-item--pinned"
                class:sb-flat-item--highlighted={!!hColor}
                style:--item-color={hColor ?? "transparent"}
                onclick={() => app.navigate(loc)}
                oncontextmenu={(e) => openFlatItemMenu(e, loc, true)}
                title={loc}
              >
                <span class="sb-flat-icon sb-flat-icon--star" aria-hidden="true">{@html ICON_STAR}</span>
                <span class="sb-flat-label">{name}</span>
              </button>
              <button
                class="sb-remove-btn"
                onclick={() => app.removeFavorite(loc)}
                title={t("sidebar.removeFavorite")}
                aria-label="{t('sidebar.removeFavorite')}: {name}"
              >
                <svg width="10" height="10" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                  <line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/>
                </svg>
              </button>
            </div>
          {/each}
        {/if}

        {#if knownFolderItems.length === 0 && app.favoriteLocations.length === 0}
          <div class="sb-empty-hint">{t("sidebar.noLocations")}</div>
        {/if}
      {/if}
    </div>

    <!-- ══════════════════════════════════════════════════════ -->
    <!-- ESTE EQUIPO                                            -->
    <!-- ══════════════════════════════════════════════════════ -->
    {#if app.volumes.length > 0}
      <div class="sb-section">
        <div class="sb-section-header" role="button" tabindex="0"
          onclick={() => (collapsed.computer = !collapsed.computer)}
          onkeydown={(e) => e.key === "Enter" && (collapsed.computer = !collapsed.computer)}
        >
          <svg class="sb-section-chevron" class:sb-section-chevron--collapsed={collapsed.computer}
            width="10" height="10" viewBox="0 0 24 24" fill="currentColor">
            <path d="M8 5l8 7-8 7V5z"/>
          </svg>
          <span class="sb-section-label">{t("sidebar.thisComputer")}</span>
        </div>

        {#if !collapsed.computer}
          {#each app.volumes as vol (vol.path)}
            {@const hColor = computeHighlight(vol.path)}
            {@const usedBytes = vol.totalBytes - vol.freeBytes}
            {@const usedPct = vol.totalBytes > 0 ? Math.min(100, (usedBytes / vol.totalBytes) * 100) : 0}
            <button
              class="sb-drive-item"
              class:sb-drive-item--highlighted={!!hColor}
              style:--item-color={hColor ?? "transparent"}
              onclick={() => app.navigate(vol.path)}
              oncontextmenu={(e) => openVolumeMenu(e, vol)}
              title={vol.path}
            >
              <span class="sb-drive-icon" aria-hidden="true">
                {@html vol.isRemovable ? ICON_DRIVE_USB : ICON_DRIVE}
              </span>
              <span class="sb-drive-body">
                <span class="sb-drive-name">{vol.name}</span>
                {#if vol.totalBytes > 0}
                  <span class="sb-drive-bar-track">
                    <span
                      class="sb-drive-bar-fill"
                      class:sb-drive-bar-fill--warn={usedPct > 85}
                      style="width:{usedPct}%"
                    ></span>
                  </span>
                  <span class="sb-drive-info">{formatBytes(vol.freeBytes)} libre</span>
                {/if}
              </span>
            </button>
          {/each}
        {/if}
      </div>
    {/if}

    <!-- ══════════════════════════════════════════════════════ -->
    <!-- CONEXIONES REMOTAS                                     -->
    <!-- ══════════════════════════════════════════════════════ -->
    <div class="sb-section">
      <div class="sb-section-header" role="button" tabindex="0"
        onclick={() => (collapsed.connections = !collapsed.connections)}
        onkeydown={(e) => e.key === "Enter" && (collapsed.connections = !collapsed.connections)}
      >
        <svg class="sb-section-chevron" class:sb-section-chevron--collapsed={collapsed.connections}
          width="10" height="10" viewBox="0 0 24 24" fill="currentColor">
          <path d="M8 5l8 7-8 7V5z"/>
        </svg>
        <span class="sb-section-label">{t("sidebar.connections")}</span>
        <button
          class="sb-section-action"
          class:sb-section-action--active={app.connectionManagerOpen}
          title={t("sidebar.manageConnections")}
          onclick={(e) => { e.stopPropagation(); app.openConnectionManager(); }}
        >
          <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
            <circle cx="12" cy="12" r="3"/>
            <path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33 1.65 1.65 0 0 0-1 1.51V21a2 2 0 0 1-4 0v-.09A1.65 1.65 0 0 0 9 19.4a1.65 1.65 0 0 0-1.82.33l-.06.06a2 2 0 0 1-2.83-2.83l.06-.06A1.65 1.65 0 0 0 4.68 15a1.65 1.65 0 0 0-1.51-1H3a2 2 0 0 1 0-4h.09A1.65 1.65 0 0 0 4.6 9a1.65 1.65 0 0 0-.33-1.82l-.06-.06a2 2 0 0 1 2.83-2.83l.06.06A1.65 1.65 0 0 0 9 4.68a1.65 1.65 0 0 0 1-1.51V3a2 2 0 0 1 4 0v.09a1.65 1.65 0 0 0 1 1.51 1.65 1.65 0 0 0 1.82-.33l.06-.06a2 2 0 0 1 2.83 2.83l-.06.06A1.65 1.65 0 0 0 19.4 9a1.65 1.65 0 0 0 1.51 1H21a2 2 0 0 1 0 4h-.09a1.65 1.65 0 0 0-1.51 1z"/>
          </svg>
        </button>
      </div>

      {#if !collapsed.connections}
        {#if app.activeConnections.length === 0}
          <div class="sb-empty-hint">
            {t("sidebar.noConnections")}
            <button class="sb-link" onclick={() => app.openConnectionManager()}>
              {t("sidebar.manageConnections")}
            </button>
          </div>
        {:else}
          {#each app.activeConnections as conn (conn.sessionId)}
            <div class="sb-remote-tree-row">
              <div class="sb-remote-tree-main">
                <TreeNode
                  entry={makeRemoteRootEntry(conn)}
                  depth={0}
                  iconVariant="remote"
                  {tabHighlights}
                  onNavigate={(p) => app.navigate(p)}
                  onContextMenu={(e, entry) => handleRemoteTreeContextMenu(e, entry, conn)}
                />
              </div>
              <div class="sb-remote-actions">
                <span
                  class="sb-proto-badge"
                  style="background:{PROTO_COLORS[conn.protocol] ?? 'var(--accent)'}20;color:{PROTO_COLORS[conn.protocol] ?? 'var(--accent)'}"
                >
                  {conn.protocol.toUpperCase()}
                </span>
                <button
                  class="sb-disconnect-btn"
                  onclick={() => disconnect(conn)}
                  disabled={disconnectingId === conn.sessionId}
                  title={t("sidebar.disconnect")}
                  aria-label="{t('sidebar.disconnect')}: {conn.label}"
                >
                  {#if disconnectingId === conn.sessionId}
                    <span class="spinner-xs"></span>
                  {:else}
                    {@html ICON_DISCONNECT}
                  {/if}
                </button>
              </div>
            </div>
          {/each}
        {/if}
      {/if}
    </div>

  </div>
</aside>

{#if contextMenu}
  <ContextMenu
    x={contextMenu.x}
    y={contextMenu.y}
    items={contextMenu.items}
    onclose={() => (contextMenu = null)}
  />
{/if}

<style>
  .sidebar {
    display: flex;
    flex-direction: column;
    width: var(--sidebar-width, 220px);
    height: 100%;
    background: var(--surface-alt);
    border-right: 1px solid var(--line);
    overflow: hidden;
    flex-shrink: 0;
  }

  .sb-body {
    flex: 1;
    overflow-y: auto;
    padding: 4px 4px 12px;
  }

  /* ── Section ─────────────────────────────── */
  .sb-section {
    margin-bottom: 4px;
  }

  .sb-section-header {
    display: flex;
    align-items: center;
    gap: 4px;
    padding: 5px 6px 4px;
    border-radius: 4px;
    cursor: pointer;
    user-select: none;

    &:hover { background: var(--surface-hover); }
    &:hover .sb-section-action { opacity: 1; }
  }

  .sb-section-chevron {
    flex-shrink: 0;
    color: var(--text-subtle);
    transition: transform 0.15s;
    transform: rotate(90deg);

    &.sb-section-chevron--collapsed { transform: rotate(0deg); }
  }

  .sb-section-label {
    flex: 1;
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.08em;
    color: var(--text-subtle);
    min-width: 0;
  }

  .sb-section-action {
    flex-shrink: 0;
    width: 20px;
    height: 20px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    color: var(--text-muted);
    border-radius: 3px;
    cursor: pointer;
    opacity: 0;
    padding: 0;
    transition: opacity 0.1s, background 0.1s, color 0.1s;

    &:hover { background: var(--surface-active); color: var(--text); opacity: 1; }
    &.sb-section-action--active { color: var(--accent); opacity: 1; }
  }

  /* ── Flat items (favorites, known folders) ─ */
  .sb-flat-item-row {
    display: flex;
    align-items: center;

    &:hover .sb-remove-btn { opacity: 1; }
  }

  .sb-flat-item {
    flex: 1;
    min-width: 0;
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 4px 6px;
    border: none;
    border-left: 2px solid transparent;
    border-radius: 4px;
    background: none;
    color: var(--text);
    font: inherit;
    font-size: calc(13px * var(--tree-scale, 1));
    text-align: left;
    cursor: pointer;
    transition: background 0.1s, color 0.1s, border-left-color 0.1s;

    &:hover { background: var(--surface-hover); }

    &.sb-flat-item--highlighted {
      background: color-mix(in srgb, var(--item-color) 13%, transparent);
      color: var(--item-color);
      border-left-color: var(--item-color);

      &:hover { background: color-mix(in srgb, var(--item-color) 20%, transparent); }
    }
  }

  .sb-flat-icon {
    flex-shrink: 0;
    display: flex;
    color: #e0a030;
    width: 14px;
    height: 14px;

    .sb-flat-item--highlighted & { color: var(--item-color); }
  }

  .sb-flat-icon--star {
    color: var(--text-subtle);
    .sb-flat-item--highlighted & { color: var(--item-color); }
  }

  .sb-flat-label {
    flex: 1;
    min-width: 0;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .sb-micro-divider {
    height: 1px;
    margin: 2px 6px;
    background: var(--line);
    opacity: 0.5;
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

  /* ── Drive items ─────────────────────────── */
  .sb-drive-item {
    display: flex;
    align-items: flex-start;
    gap: 7px;
    padding: 5px 8px;
    border: none;
    border-left: 2px solid transparent;
    border-radius: 4px;
    background: none;
    color: var(--text);
    font: inherit;
    text-align: left;
    cursor: pointer;
    width: 100%;
    transition: background 0.1s, color 0.1s, border-left-color 0.1s;

    &:hover { background: var(--surface-hover); }

    &.sb-drive-item--highlighted {
      background: color-mix(in srgb, var(--item-color) 10%, transparent);
      border-left-color: var(--item-color);

      .sb-drive-name { color: var(--item-color); }
      &:hover { background: color-mix(in srgb, var(--item-color) 17%, transparent); }
    }
  }

  .sb-drive-icon {
    flex-shrink: 0;
    display: flex;
    color: var(--text-muted);
    margin-top: 1px;
    width: 14px;
    height: 14px;

    .sb-drive-item--highlighted & { color: var(--item-color); }
  }

  .sb-drive-body {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 3px;
  }

  .sb-drive-name {
    font-size: calc(12px * var(--tree-scale, 1));
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    color: var(--text);
  }

  .sb-drive-bar-track {
    display: block;
    height: 3px;
    border-radius: 2px;
    background: var(--surface-active);
    overflow: hidden;
  }

  .sb-drive-bar-fill {
    display: block;
    height: 100%;
    border-radius: 2px;
    background: var(--accent);
    transition: width 0.3s;

    &.sb-drive-bar-fill--warn { background: var(--danger); }
  }

  .sb-drive-info {
    font-size: 10px;
    color: var(--text-subtle);
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  /* ── Remote connection tree rows ─────────── */
  .sb-remote-tree-row {
    display: flex;
    align-items: flex-start;
    position: relative;

    &:hover .sb-disconnect-btn { opacity: 1; }
    &:hover .sb-proto-badge { opacity: 0; }
  }

  .sb-remote-tree-main {
    flex: 1;
    min-width: 0;
  }

  .sb-remote-actions {
    display: flex;
    align-items: center;
    flex-shrink: 0;
    gap: 2px;
    padding-top: 4px;
  }

  .sb-proto-badge {
    font-size: 9px;
    font-weight: 700;
    padding: 1px 4px;
    border-radius: 3px;
    letter-spacing: 0.04em;
    transition: opacity 0.1s;
    flex-shrink: 0;
  }

  .sb-disconnect-btn {
    flex-shrink: 0;
    width: 22px;
    height: 22px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    color: var(--text-muted);
    cursor: pointer;
    opacity: 0;
    border-radius: 3px;
    padding: 0;
    transition: opacity 0.1s, background 0.1s, color 0.1s;

    &:hover { background: var(--danger-soft); color: var(--danger); opacity: 1; }
    &:disabled { opacity: 0.4; cursor: default; }
  }

  /* ── Empty states ────────────────────────── */
  .sb-empty-hint {
    padding: 6px 10px;
    font-size: 11px;
    color: var(--text-subtle);
  }

  .sb-link {
    background: none;
    border: none;
    color: var(--accent);
    font: inherit;
    font-size: 11px;
    text-decoration: underline;
    cursor: pointer;
    padding: 0;
    display: block;
    margin-top: 2px;
  }

  /* ── Spinner ─────────────────────────────── */
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
