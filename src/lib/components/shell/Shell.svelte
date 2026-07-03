<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, onDestroy } from "svelte";
  import { app } from "../../stores/app.svelte.js";
  import type {
    AppMetadataDto,
    JobProgressDto,
    JobLogDto,
    JobFinishedDto,
    JobPausedDto,
    SelectionAnalysisDto,
    ChdConversionOptionsPayload,
    ChdRestoreOptionsPayload,
    ExtractionOptionsPayload,
    CompressionOptionsPayload,
    CompressionCapabilitiesDto,
    VolumeDto,
    KnownFoldersDto,
    QueuedOp,
  } from "../../types/index.js";

  import Sidebar from "./Sidebar.svelte";
  import TabBar from "./TabBar.svelte";
  import Toolbar from "./Toolbar.svelte";
  import ContentPanel from "../content/ContentPanel.svelte";
  import TerminalPanel from "../terminal/TerminalPanel.svelte";
  import ConnectionManager from "../connections/ConnectionManager.svelte";
  import JobsPanel from "../dialogs/JobsPanel.svelte";
  import ConfirmDialog from "../dialogs/ConfirmDialog.svelte";
  import SettingsDialog from "../dialogs/SettingsDialog.svelte";
  import PropertiesDialog from "../dialogs/PropertiesDialog.svelte";
  import ExtractionDialog from "../dialogs/ExtractionDialog.svelte";
  import CompressionDialog from "../dialogs/CompressionDialog.svelte";
  import ChdDialog from "../dialogs/ChdDialog.svelte";
  import M3uDialog from "../dialogs/M3uDialog.svelte";
  import { t, tn } from "../../i18n/index.js";

  // ── ContentPanel ref for imperative calls ────────────────
  let contentPanel = $state<ReturnType<typeof ContentPanel> | undefined>(undefined);

  // ── Sidebar resize ────────────────────────────────────────
  let sidebarWidth = $state(220);

  function startSidebarResize(e: MouseEvent) {
    e.preventDefault();
    const startX = e.clientX;
    const startW = sidebarWidth;

    function onMove(ev: MouseEvent) {
      sidebarWidth = Math.max(140, Math.min(500, startW + (ev.clientX - startX)));
    }

    function onUp() {
      localStorage.setItem("dogu-sidebar-width", String(sidebarWidth));
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    }

    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }

  // ── Dialog state (local to Shell) ───────────────────────
  let propertiesState = $state<{ paths: string[] } | null>(null);
  let extractionState = $state<{ archives: string[] } | null>(null);
  let compressionState = $state<{ sources: string[] } | null>(null);
  let compressionCapabilities = $state<CompressionCapabilitiesDto>({ canCompressZip: true, canCompress7z: true, canCompressRar: false });
  let chdState = $state<{ analysis: SelectionAnalysisDto; mode: "convert" | "restore" } | null>(null);
  let m3uDirs = $state<string[] | null>(null);

  // ── Job / queue ID counters ───────────────────────────────
  let _jobSeq = 0;
  function newJobId()   { return `job-${Date.now()}-${++_jobSeq}`; }

  let _queueSeq = 0;
  function newQueueId() { return `qop-${Date.now()}-${++_queueSeq}`; }

  function parentDir(p: string): string {
    // Works for Windows paths (C:\foo\bar) and POSIX paths (/foo/bar)
    return p.replace(/[/\\][^/\\]+[/\\]?$/, "") || p;
  }

  function enqueueOrRun(op: QueuedOp) {
    if (app.queueMode) {
      app.addToQueue(op);
      app.notify("info", t("queue.operationQueued"));
    } else {
      op.execute();
    }
  }

  // ── Tauri event listeners ─────────────────────────────────
  const unlisten: Array<() => void> = [];

  onMount(async () => {
    const stored = localStorage.getItem("dogu-sidebar-width");
    if (stored) sidebarWidth = Math.max(140, Math.min(500, parseInt(stored, 10)));

    // Load system volumes, known folders, compression capabilities, and app metadata (non-critical)
    try {
      const [vols, kf, caps, meta] = await Promise.all([
        invoke<VolumeDto[]>("list_volumes"),
        invoke<KnownFoldersDto>("get_known_folders"),
        invoke<CompressionCapabilitiesDto>("get_compression_capabilities"),
        invoke<AppMetadataDto>("get_app_metadata"),
      ]);
      app.setVolumes(vols);
      app.setKnownFolders(kf);
      compressionCapabilities = caps;

      // On Linux, warn the user if the bundled chdman cannot be executed.
      if (meta.platform === "linux" && !meta.chdmanRuntime.available) {
        const msg = meta.chdmanRuntime.error
          ? t("startup.chdmanUnavailable", { error: meta.chdmanRuntime.error })
          : t("startup.chdmanMissing");
        app.notify("warn", msg);
      }
    } catch {
      // System info unavailable — sidebar will show without drives/known folders
    }

    unlisten.push(await listen<JobProgressDto>("job-progress", ({ payload }) => {
      // Backend emits 0.0–1.0; store/UI expects 0–100.
      app.updateJobProgress(payload.jobId, payload.progress * 100, payload.message);
    }));
    unlisten.push(await listen<JobLogDto>("job-log", ({ payload }) => {
      app.appendJobLog(payload.jobId, payload.line);
    }));
    unlisten.push(await listen<JobFinishedDto>("job-finished", ({ payload }) => {
      app.finishJob(payload.jobId, payload.success, payload.message);
      if (payload.success) refreshContent();
    }));
    unlisten.push(await listen<JobPausedDto>("job-paused", ({ payload }) => {
      app.pauseJob(payload);
    }));

    // Custom events from ContentPanel / FileRow
    document.addEventListener("dogu:rename-commit", handleRenameCommitRaw);
  });

  onDestroy(() => {
    unlisten.forEach(fn => fn());
    document.removeEventListener("dogu:rename-commit", handleRenameCommitRaw);
  });

  // ── Refresh helper ────────────────────────────────────────
  function refreshContent() {
    contentPanel?.refresh();
  }

  // ── Keyboard shortcuts (app-level) ───────────────────────
  function handleAppKey(e: KeyboardEvent) {
    if (e.key === "F5") { e.preventDefault(); refreshContent(); return; }

    if (e.key === "Escape") {
      if (app.connectionManagerOpen) app.closeConnectionManager();
      else if (app.settingsOpen) app.closeSettings();
      return;
    }

    // Terminal toggle
    if (e.ctrlKey && e.key === "`") {
      e.preventDefault();
      app.toggleTerminalPanel();
      return;
    }

    // Tab management shortcuts
    if (e.ctrlKey && !e.altKey) {
      if (e.key === "t" || e.key === "T") { e.preventDefault(); app.addTab(app.currentPath ?? undefined); return; }
      if (e.key === "w" || e.key === "W") { e.preventDefault(); app.closeTab(app.activeTabId); return; }
      if (e.key === "Tab") {
        e.preventDefault();
        if (e.shiftKey) app.prevTab(); else app.nextTab();
        return;
      }
      // Ctrl+1-9 jump to tab
      const num = parseInt(e.key);
      if (!isNaN(num) && num >= 1 && num <= 9) {
        e.preventDefault();
        app.setActiveTabByIndex(num - 1);
        return;
      }
    }
  }

  // ── Rename commit ─────────────────────────────────────────
  function handleRenameCommitRaw(e: Event) {
    const { path, newName } = (e as CustomEvent<{ path: string; newName: string }>).detail;
    app.cancelRename();
    invoke("rename_path", { path, newName })
      .then(() => refreshContent())
      .catch(err => app.notify("error", t("shell.renameFailed", { error: String(err) })));
  }

  // ── New folder / file ────────────────────────────────────
  function triggerNewFolder() { contentPanel?.createFolder(); }
  function triggerNewFile()   { contentPanel?.createFile(); }

  // ── Delete ────────────────────────────────────────────────
  function handleDelete(paths: string[]) {
    if (paths.length === 0) return;

    if (app.queueMode) {
      enqueueOrRun(buildDeleteOp(paths));
      return;
    }

    const label = paths.length === 1
      ? paths[0].split(/[\\/]/).filter(Boolean).pop() ?? paths[0]
      : t("shell.deleteItemsLabel", { count: paths.length });

    if (app.settings.confirmDelete) {
      app.openConfirm({
        title: t("shell.delete"),
        message: t("shell.deleteConfirm", { label }),
        confirmLabel: t("shell.delete"),
        onConfirm: () => executeDelete(paths),
      });
    } else {
      executeDelete(paths);
    }
  }

  function buildDeleteOp(paths: string[]): QueuedOp {
    return {
      id: newQueueId(),
      title: t("shell.deleting"),
      kind: 'delete',
      sources: [],
      destinations: [],
      deletes: paths,
      execute: () => executeDelete(paths),
    };
  }

  async function executeDelete(paths: string[]): Promise<void> {
    const jobId = newJobId();
    app.startJob(jobId, t("shell.deleting"), {
      statusMessageOnSuccess: t("shell.deleted"),
      statusMessageOnFailure: t("shell.deleteFailed"),
    });
    const done = app.waitForJob(jobId);
    try {
      await invoke("start_delete_paths", { jobId, paths });
    } catch (e) {
      app.notify("error", String(e));
      app.finishJob(jobId, false, String(e));
    }
    await done;
  }

  // ── Copy / Cut / Paste ────────────────────────────────────
  function handleCopy(paths: string[]) {
    app.setClipboard({ paths, operation: "copy" });
    app.notify("info", tn("shell.itemsCopied", "shell.itemsCopiedPlural", paths.length));
  }

  function handleCut(paths: string[]) {
    app.setClipboard({ paths, operation: "cut" });
    app.notify("info", tn("shell.itemsCut", "shell.itemsCutPlural", paths.length));
  }

  function handlePaste() {
    const cb = app.clipboard;
    const dest = app.currentPath;
    if (!cb || !dest) return;

    const isCut = cb.operation === "cut";
    const paths = [...cb.paths];

    if (isCut) app.setClipboard(null);

    enqueueOrRun({
      id: newQueueId(),
      title: isCut ? t("shell.moving") : t("shell.copying"),
      kind: isCut ? 'move' : 'copy',
      sources: paths,
      destinations: [dest],
      deletes: isCut ? paths : [],
      execute: () => executePaste(paths, dest, isCut),
    });
  }

  async function executePaste(paths: string[], dest: string, isCut: boolean): Promise<void> {
    const jobId = newJobId();
    const op = isCut ? "cut" : "copy";
    app.startJob(jobId, op === "copy" ? t("shell.copying") : t("shell.moving"), {
      statusMessageOnSuccess: op === "copy" ? t("shell.copied") : t("shell.moved"),
      statusMessageOnFailure: op === "copy" ? t("shell.copyFailed") : t("shell.moveFailed"),
    });
    const done = app.waitForJob(jobId);
    try {
      await invoke("start_copy_or_move_paths", { jobId, paths, destination: dest, operation: op, overwrite: false });
    } catch (e) {
      app.notify("error", String(e));
      app.finishJob(jobId, false, String(e));
    }
    await done;
  }

  // ── Extraction ────────────────────────────────────────────
  function handleExtractHere(archives: string[]) {
    const opts: ExtractionOptionsPayload = {
      individualFolders: false, splitEntries: false,
      destinationMode: "same", destinationPath: null,
      deleteArchives: false, overwrite: false,
    };
    enqueueOrRun(buildExtractOp(archives, opts));
  }

  function handleExtractToFolder(archives: string[]) {
    const opts: ExtractionOptionsPayload = {
      individualFolders: true, splitEntries: false,
      destinationMode: "same", destinationPath: null,
      deleteArchives: false, overwrite: false,
    };
    enqueueOrRun(buildExtractOp(archives, opts));
  }

  function handleExtractTo(paths: string[]) {
    extractionState = { archives: paths };
  }

  function buildExtractOp(archives: string[], opts: ExtractionOptionsPayload): QueuedOp {
    const destinations = opts.destinationPath
      ? [opts.destinationPath]
      : archives.map(a => parentDir(a));
    return {
      id: newQueueId(),
      title: t("shell.extracting"),
      kind: 'extract',
      sources: archives,
      destinations,
      deletes: opts.deleteArchives ? archives : [],
      execute: () => executeExtraction(archives, opts),
    };
  }

  async function executeExtraction(archives: string[], opts: ExtractionOptionsPayload): Promise<void> {
    const jobId = newJobId();
    app.startJob(jobId, t("shell.extracting"), {
      statusMessageOnSuccess: t("shell.extractionComplete"),
      statusMessageOnFailure: t("shell.extractionFailed"),
    });
    const done = app.waitForJob(jobId);
    try {
      await invoke("start_extract_archives", { jobId, archives, options: opts });
    } catch (e) {
      app.notify("error", String(e));
      app.finishJob(jobId, false, String(e));
    }
    await done;
  }

  // ── M3U ───────────────────────────────────────────────────
  export function openM3uDialog(paths: string[]) {
    const dirs = paths.filter((p) => p !== ""); // caller provides dirs
    if (dirs.length === 0) return;
    m3uDirs = dirs;
  }

  // ── CHD ───────────────────────────────────────────────────
  export async function openChdDialog(paths: string[]) {
    try {
      const analysis = await invoke<SelectionAnalysisDto>("scan_selection", { paths, maxDepth: app.settings.chdScanDepth });
      if (analysis.chdSources.length === 0 && analysis.restorableChds.length === 0) {
        app.notify("info", t("shell.noChdCompatible"));
        return;
      }
      const mode = analysis.chdSources.length > 0 ? "convert" : "restore";
      chdState = { analysis, mode };
    } catch (e) {
      app.notify("error", String(e));
    }
  }

  function buildConvertChdOp(paths: string[], opts: ChdConversionOptionsPayload): QueuedOp {
    return {
      id: newQueueId(),
      title: t("shell.convertingToChd"),
      kind: 'chd-convert',
      sources: paths,
      destinations: paths.map(p => parentDir(p)),
      deletes: opts.deleteOriginals ? paths : [],
      execute: () => executeConvertChd(paths, opts),
    };
  }

  async function executeConvertChd(paths: string[], opts: ChdConversionOptionsPayload): Promise<void> {
    const jobId = newJobId();
    app.startJob(jobId, t("shell.convertingToChd"), {
      statusMessageOnSuccess: t("shell.chdConversionComplete"),
      statusMessageOnFailure: t("shell.chdConversionFailed"),
    });
    const done = app.waitForJob(jobId);
    try {
      await invoke("start_convert_to_chd", { jobId, paths, options: opts });
    } catch (e) {
      app.notify("error", String(e));
      app.finishJob(jobId, false, String(e));
    }
    await done;
  }

  function buildRestoreChdOp(paths: string[], opts: ChdRestoreOptionsPayload): QueuedOp {
    const destinations = opts.destinationPath
      ? [opts.destinationPath]
      : paths.map(p => parentDir(p));
    return {
      id: newQueueId(),
      title: t("shell.restoringFromChd"),
      kind: 'chd-restore',
      sources: paths,
      destinations,
      deletes: opts.deleteChd ? paths : [],
      execute: () => executeRestoreChd(paths, opts),
    };
  }

  async function executeRestoreChd(paths: string[], opts: ChdRestoreOptionsPayload): Promise<void> {
    const jobId = newJobId();
    app.startJob(jobId, t("shell.restoringFromChd"), {
      statusMessageOnSuccess: t("shell.chdRestoreComplete"),
      statusMessageOnFailure: t("shell.chdRestoreFailed"),
    });
    const done = app.waitForJob(jobId);
    try {
      await invoke("start_restore_from_chd", { jobId, paths, options: opts });
    } catch (e) {
      app.notify("error", String(e));
      app.finishJob(jobId, false, String(e));
    }
    await done;
  }

  // ── Compress ──────────────────────────────────────────────
  function handleCompressQuick(paths: string[]) {
    if (paths.length === 0) return;
    const stem = paths[0].replace(/\\/g, "/").split("/").filter(Boolean).pop()?.replace(/\.[^.]+$/, "") ?? "archive";
    const opts: CompressionOptionsPayload = {
      format: "zip",
      archiveName: paths.length === 1 ? stem : "archive",
      destinationMode: "same",
      destinationPath: null,
      compressionLevel: 5,
      deleteOriginals: false,
      overwrite: false,
    };
    enqueueOrRun(buildCompressOp(paths, opts));
  }

  function handleCompress(paths: string[]) {
    if (paths.length === 0) return;
    compressionState = { sources: paths };
  }

  function buildCompressOp(sources: string[], opts: CompressionOptionsPayload): QueuedOp {
    const destinations = opts.destinationPath
      ? [opts.destinationPath]
      : sources.map(p => parentDir(p));
    return {
      id: newQueueId(),
      title: t("shell.compressing"),
      kind: 'compress',
      sources,
      destinations,
      deletes: opts.deleteOriginals ? sources : [],
      execute: () => executeCompress(sources, opts),
    };
  }

  async function executeCompress(sources: string[], opts: CompressionOptionsPayload): Promise<void> {
    const jobId = newJobId();
    app.startJob(jobId, t("shell.compressing"), {
      statusMessageOnSuccess: t("shell.compressionComplete"),
      statusMessageOnFailure: t("shell.compressionFailed"),
    });
    const done = app.waitForJob(jobId);
    try {
      await invoke("start_compress", { jobId, sources, options: opts });
    } catch (e) {
      app.notify("error", String(e));
      app.finishJob(jobId, false, String(e));
    }
    await done;
  }

  // ── Properties ────────────────────────────────────────────
  function handleProperties(paths: string[]) {
    propertiesState = { paths };
  }

  // ── Open with ─────────────────────────────────────────────
  function handleOpenWith(path: string) {
    invoke("open_with_dialog", { path }).catch(e => app.notify("error", String(e)));
  }

  // ── Search ────────────────────────────────────────────────
  function handleSearch(q: string) {
    contentPanel?.search(q);
  }

  function handleClearSearch() {
    contentPanel?.clearSearch();
  }
</script>

<svelte:window onkeydown={handleAppKey} />
<!-- Prevent the webview's built-in context menu from showing anywhere in the app -->
<svelte:document oncontextmenu={(e) => e.preventDefault()} />

<div class="shell" style="--sidebar-width: {sidebarWidth}px">
  <!-- ── Sidebar ── -->
  <Sidebar />

  <!-- ── Sidebar resize handle ── -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="sidebar-resizer" role="separator" onmousedown={startSidebarResize}></div>

  <!-- ── Main column ── -->
  <div class="shell-main">
    <TabBar />

    <Toolbar
      onRefresh={refreshContent}
      onNewFolder={triggerNewFolder}
      onNewFile={triggerNewFile}
      onSearch={handleSearch}
      onClearSearch={handleClearSearch}
    />

    <div class="shell-content">
      <!-- Connection Manager side panel (slides in from left over content) -->
      {#if app.connectionManagerOpen}
        <ConnectionManager />
      {/if}

      <!-- File list -->
      <ContentPanel
        bind:this={contentPanel}
        onDelete={handleDelete}
        onCopy={handleCopy}
        onCut={handleCut}
        onPaste={handlePaste}
        onExtractHere={handleExtractHere}
        onExtractToFolder={handleExtractToFolder}
        onExtractTo={handleExtractTo}
        onCompressQuick={handleCompressQuick}
        onCompress={handleCompress}
        onChd={openChdDialog}
        onM3u={openM3uDialog}
        onProperties={handleProperties}
        onOpenWith={handleOpenWith}
      />
    </div>

    <TerminalPanel />
    <JobsPanel />
  </div>
</div>

<!-- ── Dialogs ── -->
<ConfirmDialog />
<SettingsDialog />

{#if propertiesState}
  <PropertiesDialog paths={propertiesState.paths} onclose={() => (propertiesState = null)} />
{/if}

{#if extractionState}
  <ExtractionDialog
    archives={extractionState.archives}
    onclose={() => (extractionState = null)}
    onExtract={(archives, opts) => {
      enqueueOrRun(buildExtractOp(archives, opts));
      extractionState = null;
    }}
  />
{/if}

{#if compressionState}
  <CompressionDialog
    sources={compressionState.sources}
    capabilities={compressionCapabilities}
    onclose={() => (compressionState = null)}
    onCompress={(sources, opts) => {
      enqueueOrRun(buildCompressOp(sources, opts));
      compressionState = null;
    }}
  />
{/if}

{#if m3uDirs}
  <M3uDialog dirs={m3uDirs} currentDir={app.currentPath ?? ""} onclose={() => (m3uDirs = null)} />
{/if}

{#if chdState}
  <ChdDialog
    analysis={chdState.analysis}
    initialMode={chdState.mode}
    onclose={() => (chdState = null)}
    onConvert={(paths, opts) => {
      enqueueOrRun(buildConvertChdOp(paths, opts));
      chdState = null;
    }}
    onRestore={(paths, opts) => {
      enqueueOrRun(buildRestoreChdOp(paths, opts));
      chdState = null;
    }}
  />
{/if}

<style>
  .shell {
    display: flex;
    /* zoom scales everything; compensate height so content still fills the viewport */
    height: calc(100vh / var(--app-font-scale, 1));
    overflow: hidden;
    background: var(--bg);
    zoom: var(--app-font-scale, 1);
  }

  .sidebar-resizer {
    width: 4px;
    height: 100%;
    cursor: col-resize;
    flex-shrink: 0;
    background: transparent;
    margin-left: -1px; /* sit on top of sidebar border */
    z-index: 20;
    transition: background 0.15s;

    &:hover { background: color-mix(in srgb, var(--accent) 40%, transparent); }
  }

  .shell-main {
    flex: 1;
    display: flex;
    flex-direction: column;
    min-width: 0;
    overflow: hidden;
  }

  .shell-content {
    flex: 1;
    display: flex;
    overflow: hidden;
    min-height: 0;
  }
</style>
