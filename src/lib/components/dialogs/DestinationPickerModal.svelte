<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { open as openNativeDialog } from "@tauri-apps/plugin-dialog";
  import type { ActiveConnectionDto, EntryDto, KnownFoldersDto, VolumeDto } from "../../types/index.js";
  import { app } from "../../stores/app.svelte.js";
  import { basenameOf, dirnameOf, normalizePath } from "../../utils/ghosts.js";
  import Modal from "../ui/Modal.svelte";
  import Button from "../ui/Button.svelte";
  import { t } from "../../i18n/index.js";

  type DestinationResult = { path: string; label: string };

  type Props = {
    /** Where to open initially (usually the current selection or the active pane path). */
    initialPath?: string | null;
    onselect: (result: DestinationResult) => void;
    onclose: () => void;
  };

  let { initialPath = null, onselect, onclose }: Props = $props();

  // ── Left panel state ──────────────────────────────────────
  let knownFolders = $state<KnownFoldersDto | null>(null);
  let volumes = $state<VolumeDto[]>([]);
  let connections = $state<ActiveConnectionDto[]>([]);

  // ── Right panel state ─────────────────────────────────────
  let breadcrumb = $state<{ path: string; label: string }[]>([]);
  let children = $state<EntryDto[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);

  // Editable absolute-path field (kept in sync with wherever we navigate).
  let pathInput = $state("");

  // Inline create-folder / rename state.
  let creatingFolder = $state(false);
  let newFolderName = $state("");
  let renamingPath = $state<string | null>(null);
  let renameValue = $state("");
  let deletingPath = $state<string | null>(null);
  // Windows-Explorer style: a single-clicked folder becomes the selection used by
  // "Select this folder", even without navigating into it. Null → use currentPath.
  let selectedPath = $state<string | null>(null);
  let busy = $state(false);

  // Back/forward history, mirroring Dogu's pane navigation.
  let history = $state<string[]>([]);
  let historyIdx = $state(-1);

  /** Tell the shell to refresh any pane currently showing this directory. */
  function notifyFsChanged(path: string | null) {
    if (path) {
      document.dispatchEvent(new CustomEvent("dogu:fs-changed", { detail: { paths: [path] } }));
    }
  }

  const currentPath = $derived(breadcrumb.length > 0 ? breadcrumb[breadcrumb.length - 1].path : null);
  const dirs = $derived(children.filter((e) => e.isDir));
  const isRemoteCurrent = $derived(!!currentPath && currentPath.startsWith("remote://"));
  const canBack = $derived(historyIdx > 0);
  const canForward = $derived(historyIdx < history.length - 1);

  async function loadLeft() {
    const [kf, vols, conns] = await Promise.all([
      invoke<KnownFoldersDto>("get_known_folders"),
      invoke<VolumeDto[]>("list_volumes"),
      invoke<ActiveConnectionDto[]>("list_active_connections"),
    ]);
    knownFolders = kf;
    volumes = vols;
    connections = conns;
  }

  /** Human label for a breadcrumb crumb: connection name at a remote root, else the leaf. */
  function crumbLabel(path: string): string {
    const match = connections.find((c) => normalizePath(c.rootPath) === normalizePath(path));
    if (match) return match.label;
    const leaf = basenameOf(path);
    return leaf || path;
  }

  /** Builds the full breadcrumb chain for an absolute path (local or remote). */
  function buildBreadcrumb(path: string): { path: string; label: string }[] {
    const crumbs: { path: string; label: string }[] = [];
    let cur = path;
    let guard = 0;
    while (cur && guard++ < 128) {
      crumbs.unshift({ path: cur, label: crumbLabel(cur) });
      const parent = dirnameOf(cur);
      if (normalizePath(parent) === normalizePath(cur)) break; // reached a root
      cur = parent;
    }
    return crumbs;
  }

  /** Core loader: lists a path and updates breadcrumb + path field. `push` records
   *  history (skip it when the move IS a history back/forward). `crumbs` overrides
   *  the rebuilt breadcrumb to preserve ancestor labels (used by breadcrumb clicks). */
  async function loadInto(
    path: string,
    opts: { push?: boolean; crumbs?: { path: string; label: string }[] } = {},
  ): Promise<boolean> {
    loading = true;
    error = null;
    cancelInlineEdits();
    selectedPath = null;
    try {
      const entries = await invoke<EntryDto[]>("list_children", { path });
      children = entries;
      breadcrumb = opts.crumbs ?? buildBreadcrumb(path);
      pathInput = path;
      if (opts.push !== false) pushHistory(path);
      return true;
    } catch (e) {
      error = String(e);
      return false;
    } finally {
      loading = false;
    }
  }

  /** Records a new navigation, dropping any forward entries (like a browser). */
  function pushHistory(path: string) {
    if (history[historyIdx] === path) return; // no-op re-open of the same folder
    history = [...history.slice(0, historyIdx + 1), path];
    historyIdx = history.length - 1;
  }

  /** Central navigation used by clicks, typed paths and the native picker. */
  function openPath(path: string): Promise<boolean> {
    return loadInto(path, { push: true });
  }

  function goBack() {
    if (!canBack) return;
    historyIdx -= 1;
    loadInto(history[historyIdx], { push: false });
  }

  function goForward() {
    if (!canForward) return;
    historyIdx += 1;
    loadInto(history[historyIdx], { push: false });
  }

  /** Re-list the current directory in place (after create/rename/delete), preserving the breadcrumb. */
  async function reloadChildren() {
    if (!currentPath) return;
    try {
      children = await invoke<EntryDto[]>("list_children", { path: currentPath });
      // Drop a stale selection whose folder no longer exists (e.g. it was deleted/renamed).
      if (selectedPath && !children.some((e) => e.path === selectedPath)) selectedPath = null;
    } catch (e) {
      error = String(e);
    }
  }

  function navigateCrumb(index: number) {
    const crumb = breadcrumb[index];
    return loadInto(crumb.path, { push: true, crumbs: breadcrumb.slice(0, index + 1) });
  }

  async function goToTypedPath() {
    const target = pathInput.trim();
    if (!target) return;
    const ok = await openPath(target);
    if (!ok) app.notify("error", t("destPicker.couldNotOpenPath", { error: error ?? "" }));
  }

  function goUp() {
    if (!currentPath) return;
    const parent = dirnameOf(currentPath);
    if (normalizePath(parent) !== normalizePath(currentPath)) openPath(parent);
  }

  async function openNative() {
    try {
      const picked = await openNativeDialog({
        directory: true,
        multiple: false,
        defaultPath: !isRemoteCurrent && currentPath ? currentPath : undefined,
      });
      if (typeof picked === "string") await openPath(picked);
    } catch (e) {
      app.notify("error", t("destPicker.couldNotOpenPath", { error: String(e) }));
    }
  }

  // ── Create folder ─────────────────────────────────────────
  function beginCreateFolder() {
    if (!currentPath) return;
    cancelInlineEdits();
    creatingFolder = true;
    newFolderName = "";
  }

  async function confirmCreateFolder() {
    // Guard: Escape flips this off before the input's blur fires — don't create then.
    if (!creatingFolder) return;
    const name = newFolderName.trim();
    if (!currentPath || !name) { creatingFolder = false; return; }
    busy = true;
    try {
      await invoke<string>("create_folder", { parent: currentPath, name });
      creatingFolder = false;
      newFolderName = "";
      await reloadChildren();
      notifyFsChanged(currentPath);
    } catch (e) {
      app.notify("error", t("destPicker.couldNotCreateFolder", { error: String(e) }));
    } finally {
      busy = false;
    }
  }

  // ── Rename ────────────────────────────────────────────────
  function beginRename(dir: EntryDto) {
    cancelInlineEdits();
    renamingPath = dir.path;
    renameValue = dir.name;
  }

  async function confirmRename(dir: EntryDto) {
    // Guard: Escape clears renamingPath before the input's blur fires — don't rename then.
    if (renamingPath !== dir.path) return;
    const newName = renameValue.trim();
    if (!newName || newName === dir.name) { renamingPath = null; return; }
    busy = true;
    try {
      await invoke<string>("rename_path", { path: dir.path, newName });
      renamingPath = null;
      await reloadChildren();
    } catch (e) {
      app.notify("error", t("destPicker.couldNotRename", { error: String(e) }));
    } finally {
      busy = false;
    }
  }

  // ── Delete ────────────────────────────────────────────────
  function beginDelete(dir: EntryDto) {
    cancelInlineEdits();
    deletingPath = dir.path;
  }

  async function confirmDelete(dir: EntryDto) {
    if (deletingPath !== dir.path) return;
    busy = true;
    try {
      await invoke("delete_entry", { path: dir.path });
      deletingPath = null;
      await reloadChildren();
      notifyFsChanged(currentPath);
    } catch (e) {
      app.notify("error", t("destPicker.couldNotDelete", { error: String(e) }));
    } finally {
      busy = false;
    }
  }

  function cancelInlineEdits() {
    creatingFolder = false;
    renamingPath = null;
    deletingPath = null;
  }

  /** Single click highlights a folder as the selection (without navigating into it). */
  function selectDir(dir: EntryDto) {
    if (renamingPath || deletingPath) return;
    selectedPath = dir.path;
  }

  /** Clicking the empty area of the list clears the selection (falls back to currentPath). */
  function clearSelectionOnEmpty(e: MouseEvent) {
    if (e.target === e.currentTarget) selectedPath = null;
  }

  function confirmSelection() {
    // A highlighted folder wins over the folder we're currently viewing.
    if (selectedPath) {
      const name = children.find((e) => e.path === selectedPath)?.name ?? basenameOf(selectedPath);
      onselect({ path: selectedPath, label: name });
      return;
    }
    if (!currentPath) return;
    const label = breadcrumb[breadcrumb.length - 1].label;
    onselect({ path: currentPath, label });
  }

  function iconFor(protocol: string) {
    const icons: Record<string, string> = { smb: "🖧", ssh: "🔒", ftp: "📡", ftps: "🔒" };
    return icons[protocol] ?? "🌐";
  }

  function knownFolderItems(kf: KnownFoldersDto) {
    const items: { label: string; path: string; icon: string }[] = [];
    if (kf.home)      items.push({ label: t("sidebar.home"),      path: kf.home,      icon: "🏠" });
    if (kf.desktop)   items.push({ label: t("sidebar.desktop"),   path: kf.desktop,   icon: "🖥" });
    if (kf.documents) items.push({ label: t("sidebar.documents"), path: kf.documents, icon: "📄" });
    if (kf.downloads) items.push({ label: t("sidebar.downloads"), path: kf.downloads, icon: "⬇" });
    return items;
  }

  // Seed captured once at init so a later pane change doesn't yank the picker around.
  // svelte-ignore state_referenced_locally
  const seedPath = initialPath ?? app.currentPath;

  // Open at the seeded location once locations load. The effect reads no reactive
  // state synchronously, so it runs a single time after mount.
  $effect(() => {
    let cancelled = false;
    (async () => {
      await loadLeft();
      if (!cancelled && seedPath) await openPath(seedPath);
    })();
    return () => { cancelled = true; };
  });
</script>

<Modal title={t("destPicker.title")} width="720px" onclose={onclose}>
  {#snippet children()}
    <div class="picker-layout">
      <!-- Left panel: locations -->
      <aside class="left-panel">
        <!-- Local section -->
        <div class="section-header">{t("destPicker.local")}</div>
        {#if knownFolders}
          {#each knownFolderItems(knownFolders) as item}
            <button
              class="location-item"
              class:active={!!currentPath && normalizePath(currentPath) === normalizePath(item.path)}
              onclick={() => openPath(item.path)}
            >
              <span class="loc-icon">{item.icon}</span>
              <span class="loc-label">{item.label}</span>
            </button>
          {/each}
        {/if}
        {#each volumes as vol}
          <button
            class="location-item"
            class:active={!!currentPath && normalizePath(currentPath) === normalizePath(vol.path)}
            onclick={() => openPath(vol.path)}
          >
            <span class="loc-icon">💾</span>
            <span class="loc-label">{vol.label || vol.name}</span>
          </button>
        {/each}

        <!-- Remote connections section -->
        <div class="section-header">{t("destPicker.connections")}</div>
        {#if connections.length === 0}
          <p class="empty-hint">{t("destPicker.noConnections")}</p>
        {:else}
          {#each connections as conn}
            <button
              class="location-item"
              class:active={!!breadcrumb[0] && normalizePath(breadcrumb[0].path) === normalizePath(conn.rootPath)}
              onclick={() => openPath(conn.rootPath)}
            >
              <span class="loc-icon">{iconFor(conn.protocol)}</span>
              <span class="loc-label">{conn.label}</span>
            </button>
          {/each}
        {/if}
      </aside>

      <!-- Right panel: directory browser -->
      <div class="right-panel">
        <!-- Absolute-path bar -->
        <div class="path-bar">
          <button
            class="icon-btn"
            title={t("destPicker.back")}
            aria-label={t("destPicker.back")}
            disabled={!canBack}
            onclick={goBack}
          >←</button>
          <button
            class="icon-btn"
            title={t("destPicker.forward")}
            aria-label={t("destPicker.forward")}
            disabled={!canForward}
            onclick={goForward}
          >→</button>
          <button
            class="icon-btn"
            title={t("destPicker.up")}
            aria-label={t("destPicker.up")}
            disabled={!currentPath}
            onclick={goUp}
          >↑</button>
          <input
            class="path-input"
            type="text"
            bind:value={pathInput}
            placeholder={t("destPicker.pathPlaceholder")}
            spellcheck="false"
            onkeydown={(e) => { if (e.key === "Enter") goToTypedPath(); }}
          />
          <Button size="sm" variant="outline" onclick={goToTypedPath}>{t("destPicker.go")}</Button>
        </div>

        <!-- Breadcrumb -->
        <div class="breadcrumb">
          {#if breadcrumb.length === 0}
            <span class="breadcrumb-hint">← {t("destPicker.local")}</span>
          {:else}
            {#each breadcrumb as crumb, i}
              {#if i > 0}<span class="breadcrumb-sep">/</span>{/if}
              <button
                class="breadcrumb-item"
                class:last={i === breadcrumb.length - 1}
                onclick={() => navigateCrumb(i)}
              >{crumb.label}</button>
            {/each}
          {/if}
        </div>

        <!-- Action toolbar -->
        <div class="action-bar">
          <button class="tool-btn" disabled={!currentPath || busy} onclick={beginCreateFolder}>
            <span class="tool-ico">＋</span>{t("destPicker.newFolder")}
          </button>
          {#if !isRemoteCurrent}
            <button class="tool-btn tool-btn--right" onclick={openNative}>
              {t("destPicker.openInSystem")} ↗
            </button>
          {/if}
        </div>

        <!-- Directory list -->
        <div class="dir-list" role="presentation" onclick={clearSelectionOnEmpty} onkeydown={undefined}>
          {#if creatingFolder}
            <div class="dir-row editing">
              <span class="dir-icon">📁</span>
              <!-- svelte-ignore a11y_autofocus -->
              <input
                class="inline-input"
                type="text"
                autofocus
                bind:value={newFolderName}
                placeholder={t("destPicker.folderNamePlaceholder")}
                spellcheck="false"
                onkeydown={(e) => {
                  if (e.key === "Enter") confirmCreateFolder();
                  else if (e.key === "Escape") { creatingFolder = false; }
                }}
                onblur={confirmCreateFolder}
              />
            </div>
          {/if}

          {#if loading}
            <p class="state-hint">{t("destPicker.loading")}</p>
          {:else if error}
            <p class="state-hint error">{error}</p>
          {:else if breadcrumb.length === 0}
            <p class="state-hint">{t("destPicker.local")}</p>
          {:else if dirs.length === 0 && !creatingFolder}
            <p class="state-hint">{t("destPicker.noChildren")}</p>
          {:else}
            {#each dirs as dir}
              {#if renamingPath === dir.path}
                <div class="dir-row editing">
                  <span class="dir-icon">📁</span>
                  <!-- svelte-ignore a11y_autofocus -->
                  <input
                    class="inline-input"
                    type="text"
                    autofocus
                    bind:value={renameValue}
                    spellcheck="false"
                    onkeydown={(e) => {
                      if (e.key === "Enter") confirmRename(dir);
                      else if (e.key === "Escape") { renamingPath = null; }
                    }}
                    onblur={() => confirmRename(dir)}
                  />
                </div>
              {:else if deletingPath === dir.path}
                <div class="dir-row confirming">
                  <span class="dir-icon">🗑</span>
                  <div class="confirm-text">
                    <span class="confirm-q">{t("destPicker.deleteConfirm", { name: dir.name })}</span>
                    <span class="confirm-warn">
                      {#if dir.hasChildren}{t("destPicker.deleteHasContent")} {/if}{t("destPicker.deleteWarning")}
                    </span>
                  </div>
                  <button class="confirm-btn" disabled={busy} onclick={() => (deletingPath = null)}>
                    {t("common.cancel")}
                  </button>
                  <button class="confirm-btn danger" disabled={busy} onclick={() => confirmDelete(dir)}>
                    {t("destPicker.delete")}
                  </button>
                </div>
              {:else}
                <div class="dir-row" class:selected={selectedPath === dir.path}>
                  <button class="dir-open" ondblclick={() => openPath(dir.path)} onclick={() => selectDir(dir)}>
                    <span class="dir-icon">📁</span>
                    <span class="dir-name">{dir.name}</span>
                  </button>
                  <button
                    class="row-action"
                    title={t("destPicker.rename")}
                    aria-label={t("destPicker.rename")}
                    disabled={busy}
                    onclick={() => beginRename(dir)}
                  >✎</button>
                  <button
                    class="row-action danger"
                    title={t("destPicker.delete")}
                    aria-label={t("destPicker.delete")}
                    disabled={busy}
                    onclick={() => beginDelete(dir)}
                  >🗑</button>
                </div>
              {/if}
            {/each}
          {/if}
        </div>
      </div>
    </div>
  {/snippet}

  {#snippet footer()}
    <Button variant="ghost" onclick={onclose}>{t("common.cancel")}</Button>
    <Button variant="primary" disabled={!currentPath && !selectedPath} onclick={confirmSelection}>
      {t("destPicker.selectHere")}
    </Button>
  {/snippet}
</Modal>

<style>
  .picker-layout {
    display: flex;
    gap: 0;
    height: 440px;
    margin: -16px;
    overflow: hidden;
    border-radius: 0 0 8px 8px;
  }

  .left-panel {
    width: 190px;
    flex-shrink: 0;
    border-right: 1px solid var(--line);
    overflow-y: auto;
    padding: 8px 0;
    scrollbar-width: thin;
    scrollbar-color: var(--line-strong) transparent;
  }

  .section-header {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-faint);
    padding: 10px 12px 4px;
  }

  .location-item {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    padding: 5px 12px;
    background: none;
    border: none;
    border-radius: 0;
    color: var(--text);
    font-size: 12px;
    text-align: left;
    cursor: pointer;

    &:hover { background: var(--surface-hover); }
    &.active { background: var(--accent-soft); color: var(--accent); }
  }

  .loc-icon { font-size: 13px; flex-shrink: 0; }
  .loc-label { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  .empty-hint {
    font-size: 11px;
    color: var(--text-faint);
    padding: 4px 12px;
    margin: 0;
  }

  .right-panel {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
    min-width: 0;
  }

  .path-bar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--line);
    flex-shrink: 0;
  }

  .icon-btn {
    flex-shrink: 0;
    width: 28px;
    height: 28px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: var(--surface);
    border: 1px solid var(--line-strong);
    border-radius: 5px;
    color: var(--text);
    font-size: 14px;
    cursor: pointer;

    &:hover:not(:disabled) { background: var(--surface-hover); }
    &:disabled { opacity: 0.4; cursor: default; }
  }

  .path-input {
    flex: 1;
    min-width: 0;
    padding: 5px 8px;
    background: var(--surface);
    border: 1px solid var(--line-strong);
    border-radius: 5px;
    color: var(--text);
    font-size: 12px;
    font-family: var(--font-mono, monospace);

    &:focus { outline: none; border-color: var(--accent); }
  }

  .breadcrumb {
    display: flex;
    align-items: center;
    flex-wrap: nowrap;
    gap: 2px;
    padding: 6px 12px;
    border-bottom: 1px solid var(--line);
    overflow: hidden;
    flex-shrink: 0;
    min-height: 30px;
  }

  .breadcrumb-hint {
    font-size: 11px;
    color: var(--text-faint);
  }

  .breadcrumb-sep {
    color: var(--text-faint);
    font-size: 11px;
  }

  .breadcrumb-item {
    background: none;
    border: none;
    padding: 2px 4px;
    border-radius: 3px;
    font-size: 11px;
    color: var(--accent);
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 160px;

    &:hover { background: var(--surface-hover); }
    &.last { color: var(--text); cursor: default; font-weight: 500; }
    &.last:hover { background: none; }
  }

  .action-bar {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 6px 12px;
    border-bottom: 1px solid var(--line);
    flex-shrink: 0;
  }

  .tool-btn {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    padding: 4px 9px;
    background: var(--surface);
    border: 1px solid var(--line-strong);
    border-radius: 5px;
    color: var(--text);
    font-size: 11.5px;
    cursor: pointer;

    &:hover:not(:disabled) { background: var(--surface-hover); }
    &:disabled { opacity: 0.4; cursor: default; }
  }

  .tool-btn--right { margin-left: auto; }
  .tool-ico { font-size: 13px; line-height: 1; }

  .dir-list {
    flex: 1;
    overflow-y: auto;
    padding: 4px 0;
    scrollbar-width: thin;
    scrollbar-color: var(--line-strong) transparent;
  }

  .state-hint {
    font-size: 12px;
    color: var(--text-faint);
    padding: 12px 16px;
    margin: 0;

    &.error { color: var(--danger); }
  }

  .dir-row {
    display: flex;
    align-items: center;
    width: 100%;
    padding: 0 8px 0 14px;

    &:hover { background: var(--surface-hover); }
    &:hover .row-action { opacity: 1; }
    &.editing { padding: 3px 8px 3px 14px; gap: 7px; }
    &.selected { background: var(--accent-soft); }
    &.selected .dir-name { color: var(--accent); }
    &.selected .row-action { opacity: 1; }
  }

  .dir-open {
    display: flex;
    align-items: center;
    gap: 7px;
    flex: 1;
    min-width: 0;
    padding: 5px 0;
    background: none;
    border: none;
    color: var(--text);
    font-size: 12px;
    text-align: left;
    cursor: pointer;
  }

  .dir-icon { font-size: 13px; flex-shrink: 0; }
  .dir-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  .row-action {
    flex-shrink: 0;
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    border-radius: 4px;
    color: var(--text-muted);
    font-size: 13px;
    cursor: pointer;
    opacity: 0;

    &:hover { background: var(--surface); color: var(--text); }
    &:disabled { opacity: 0; }
    &.danger:hover { color: var(--danger); }
  }

  .dir-row.confirming {
    align-items: center;
    gap: 8px;
    padding: 6px 8px 6px 14px;
    background: color-mix(in srgb, var(--danger) 10%, transparent);
  }

  .confirm-text {
    flex: 1;
    min-width: 0;
    display: flex;
    flex-direction: column;
    gap: 1px;
  }

  .confirm-q {
    font-size: 12px;
    color: var(--text);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .confirm-warn {
    font-size: 10.5px;
    color: var(--danger);
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .confirm-btn {
    flex-shrink: 0;
    padding: 3px 10px;
    background: var(--surface);
    border: 1px solid var(--line-strong);
    border-radius: 5px;
    color: var(--text);
    font-size: 11.5px;
    cursor: pointer;

    &:hover:not(:disabled) { background: var(--surface-hover); }
    &:disabled { opacity: 0.5; cursor: default; }

    &.danger {
      background: var(--danger);
      border-color: var(--danger);
      color: #fff;

      &:hover:not(:disabled) { filter: brightness(1.08); }
    }
  }

  .inline-input {
    flex: 1;
    min-width: 0;
    padding: 3px 6px;
    background: var(--surface);
    border: 1px solid var(--accent);
    border-radius: 4px;
    color: var(--text);
    font-size: 12px;

    &:focus { outline: none; }
  }
</style>
