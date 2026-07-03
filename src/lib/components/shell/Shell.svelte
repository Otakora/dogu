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
    ChdSourceDto,
    ExtractionPreviewRow,
  } from "../../types/index.js";
  import {
    predictCopyMove,
    predictCompress,
    predictChdConvert,
    predictChdRestore,
    buildGhost,
    normalizePath,
    basenameOf,
    dirnameOf,
  } from "../../utils/ghosts.js";

  import Sidebar from "./Sidebar.svelte";
  import Pane from "./Pane.svelte";
  import TerminalPanel from "../terminal/TerminalPanel.svelte";
  import ConnectionManager from "../connections/ConnectionManager.svelte";
  import { openTerminalAt } from "../../utils/terminal.js";
  import JobsPanel from "../dialogs/JobsPanel.svelte";
  import ConfirmDialog from "../dialogs/ConfirmDialog.svelte";
  import SettingsDialog from "../dialogs/SettingsDialog.svelte";
  import PropertiesDialog from "../dialogs/PropertiesDialog.svelte";
  import ExtractionDialog from "../dialogs/ExtractionDialog.svelte";
  import CompressionDialog from "../dialogs/CompressionDialog.svelte";
  import ChdDialog from "../dialogs/ChdDialog.svelte";
  import M3uDialog from "../dialogs/M3uDialog.svelte";
  import { t, tn } from "../../i18n/index.js";

  // ── Pane refs for imperative calls ──────────────────────
  let pane0 = $state<ReturnType<typeof Pane> | undefined>(undefined);
  let pane1 = $state<ReturnType<typeof Pane> | undefined>(undefined);

  function focusedPane() {
    return app.focusedPaneIdx === 1 ? pane1 : pane0;
  }

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

  // ── Pane splitter resize ──────────────────────────────────
  let splitPercent = $state(50);
  let paneAreaEl = $state<HTMLElement | undefined>(undefined);

  function startPaneSplitResize(e: MouseEvent) {
    e.preventDefault();
    const startX = e.clientX;
    const startPct = splitPercent;
    const areaW = paneAreaEl?.getBoundingClientRect().width ?? 800;

    function onMove(ev: MouseEvent) {
      const delta = ev.clientX - startX;
      const deltaPct = (delta / areaW) * 100;
      splitPercent = Math.max(20, Math.min(80, startPct + deltaPct));
    }

    function onUp() {
      localStorage.setItem("dogu-split-percent", String(Math.round(splitPercent)));
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    }

    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }

  // ── Dialog state ─────────────────────────────────────────
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

    const storedSplit = localStorage.getItem("dogu-split-percent");
    if (storedSplit) splitPercent = Math.max(20, Math.min(80, parseInt(storedSplit, 10)));

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

      if (meta.platform === "linux" && !meta.chdmanRuntime.available) {
        const msg = meta.chdmanRuntime.error
          ? t("startup.chdmanUnavailable", { error: meta.chdmanRuntime.error })
          : t("startup.chdmanMissing");
        app.notify("warn", msg);
      }
    } catch {
      // System info unavailable
    }

    unlisten.push(await listen<JobProgressDto>("job-progress", ({ payload }) => {
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

    document.addEventListener("dogu:rename-commit", handleRenameCommitRaw);
  });

  onDestroy(() => {
    unlisten.forEach(fn => fn());
    document.removeEventListener("dogu:rename-commit", handleRenameCommitRaw);
  });

  // ── Refresh helper ────────────────────────────────────────
  function refreshContent() {
    focusedPane()?.refresh();
  }

  // ── Keyboard shortcuts ────────────────────────────────────
  function handleAppKey(e: KeyboardEvent) {
    if (e.key === "F5") { e.preventDefault(); focusedPane()?.refresh(); return; }

    if (e.key === "Escape") {
      if (app.connectionManagerOpen) app.closeConnectionManager();
      else if (app.settingsOpen) app.closeSettings();
      return;
    }

    if (e.ctrlKey && e.key === "`") {
      e.preventDefault();
      if (app.currentPath) {
        openTerminalAt(app.currentPath);
      }
      return;
    }

    // Switch pane focus with Alt+Left/Right or F6
    if (app.isSplit) {
      if (e.key === "F6" || (e.altKey && (e.key === "ArrowLeft" || e.key === "ArrowRight"))) {
        e.preventDefault();
        app.focusPane(app.focusedPaneIdx === 0 ? 1 : 0);
        return;
      }
    }

    if (e.ctrlKey && !e.altKey) {
      if (e.key === "t" || e.key === "T") { e.preventDefault(); app.addTab(app.currentPath ?? undefined); return; }
      if (e.key === "w" || e.key === "W") { e.preventDefault(); app.closeTab(app.activeTabId); return; }
      if (e.key === "Tab") {
        e.preventDefault();
        if (e.shiftKey) app.prevTab(); else app.nextTab();
        return;
      }
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
      .then(() => focusedPane()?.refresh())
      .catch(err => app.notify("error", t("shell.renameFailed", { error: String(err) })));
  }

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
      produces: [],
      dependsOn: [],
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

    const opId = newQueueId();
    enqueueOrRun({
      id: opId,
      title: isCut ? t("shell.moving") : t("shell.copying"),
      kind: isCut ? 'move' : 'copy',
      sources: paths,
      destinations: [dest],
      deletes: isCut ? paths : [],
      produces: predictCopyMove(paths, dest, opId),
      dependsOn: [],
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
    enqueueExtract(archives, opts);
  }

  function handleExtractToFolder(archives: string[]) {
    const opts: ExtractionOptionsPayload = {
      individualFolders: true, splitEntries: false,
      destinationMode: "same", destinationPath: null,
      deleteArchives: false, overwrite: false,
    };
    enqueueExtract(archives, opts);
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
      produces: [],
      dependsOn: [],
      execute: () => executeExtraction(archives, opts),
    };
  }

  /** True when `path` is currently a ghost output of some queued op. */
  function isGhostPath(path: string): boolean {
    const np = normalizePath(path);
    return app.queueGhosts.some(g => normalizePath(g.path) === np);
  }

  /**
   * Enqueues (or runs) an extraction. When queueing, the archive's top-level
   * contents are predicted via the backend preview so downstream operations can
   * target the extracted files. Prediction is skipped for remote archives (not
   * previewable without downloading) and for ghost archives (don't exist yet).
   */
  async function enqueueExtract(archives: string[], opts: ExtractionOptionsPayload) {
    const op = buildExtractOp(archives, opts);
    if (!app.queueMode) { op.execute(); return; }

    const previewable = archives.filter(a => !isGhostPath(a) && !a.startsWith("remote://"));
    if (previewable.length > 0) {
      try {
        const rows = await invoke<ExtractionPreviewRow[]>("build_extraction_preview", {
          archives: previewable, options: opts,
        });
        op.produces = rows.flatMap(r =>
          r.entries
            .filter(e => e.destinationPath)
            .map(e => buildGhost(e.destinationPath, e.isDir, op.id, false)),
        );
      } catch {
        // Leave produces empty if the preview fails — the op still runs fine.
      }
    }
    app.addToQueue(op);
    app.notify("info", t("queue.operationQueued"));
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
    const dirs = paths.filter((p) => p !== "");
    if (dirs.length === 0) return;
    m3uDirs = dirs;
  }

  // ── CHD ───────────────────────────────────────────────────
  const CD_GHOST_EXTS = new Set(["cue", "gdi", "toc"]);
  const DVD_GHOST_EXTS = new Set(["iso"]);

  function emptyAnalysis(): SelectionAnalysisDto {
    return {
      archives: [], chdSources: [], restorableChds: [],
      hasDirectories: false, hasFiles: false, uniqueExtensions: [],
      chdMenuLabel: null, chdRestoreMenuLabel: null,
      hasRemoteDirectories: false, orphanBins: [],
    };
  }

  /**
   * Builds a CHD analysis for ghost sources without touching disk. A ghost's
   * companion files (e.g. the .bin next to a .cue) are also ghosts and will
   * exist by the time the conversion runs, so nothing is reported missing.
   */
  function synthesizeGhostChdAnalysis(ghostPaths: string[]): Pick<SelectionAnalysisDto, "chdSources" | "restorableChds"> {
    const chdSources: ChdSourceDto[] = [];
    const restorableChds: string[] = [];
    for (const p of ghostPaths) {
      const ext = (basenameOf(p).split(".").pop() ?? "").toLowerCase();
      if (ext === "chd") { restorableChds.push(p); continue; }
      const isDvd = DVD_GHOST_EXTS.has(ext);
      const isCd = CD_GHOST_EXTS.has(ext);
      if (!isCd && !isDvd) continue; // not a directly convertible disc image
      chdSources.push({
        sourcePath: p,
        containerDir: dirnameOf(p),
        command: isDvd ? "createdvd" : "createcd",
        displayExtensions: [`.${ext}`],
        requiredPaths: [p],
        missingFiles: [],
      });
    }
    return { chdSources, restorableChds };
  }

  export async function openChdDialog(paths: string[]) {
    const ghostPaths = paths.filter(isGhostPath);
    const realPaths = paths.filter(p => !isGhostPath(p));
    try {
      let analysis = realPaths.length > 0
        ? await invoke<SelectionAnalysisDto>("scan_selection", { paths: realPaths, maxDepth: app.settings.chdScanDepth })
        : emptyAnalysis();

      if (ghostPaths.length > 0) {
        const synth = synthesizeGhostChdAnalysis(ghostPaths);
        analysis = {
          ...analysis,
          chdSources: [...analysis.chdSources, ...synth.chdSources],
          restorableChds: [...analysis.restorableChds, ...synth.restorableChds],
        };
      }

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
    const opId = newQueueId();
    return {
      id: opId,
      title: t("shell.convertingToChd"),
      kind: 'chd-convert',
      sources: paths,
      destinations: paths.map(p => parentDir(p)),
      deletes: opts.deleteOriginals ? paths : [],
      produces: predictChdConvert(paths, opts, opId),
      dependsOn: [],
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
    const opId = newQueueId();
    return {
      id: opId,
      title: t("shell.restoringFromChd"),
      kind: 'chd-restore',
      sources: paths,
      destinations,
      deletes: opts.deleteChd ? paths : [],
      produces: predictChdRestore(paths, opts, opId),
      dependsOn: [],
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
    const opId = newQueueId();
    // "Same folder as source" places the archive alongside the sources.
    const archiveDir = opts.destinationPath ?? opts.remoteDestination ?? dirnameOf(sources[0] ?? "");
    return {
      id: opId,
      title: t("shell.compressing"),
      kind: 'compress',
      sources,
      destinations,
      deletes: opts.deleteOriginals ? sources : [],
      produces: predictCompress(opts, archiveDir, opId),
      dependsOn: [],
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

  // Shared handlers object to pass to both panes
  const paneHandlers = {
    onDelete: handleDelete,
    onCopy: handleCopy,
    onCut: handleCut,
    onPaste: handlePaste,
    onExtractHere: handleExtractHere,
    onExtractToFolder: handleExtractToFolder,
    onExtractTo: handleExtractTo,
    onCompressQuick: handleCompressQuick,
    onCompress: handleCompress,
    onChd: openChdDialog,
    onM3u: openM3uDialog,
    onProperties: handleProperties,
    onOpenWith: handleOpenWith,
  };
</script>

<svelte:window onkeydown={handleAppKey} />
<svelte:document oncontextmenu={(e) => e.preventDefault()} />

<div class="shell" style="--sidebar-width: {sidebarWidth}px">
  <!-- ── Sidebar ── -->
  <Sidebar />

  <!-- ── Sidebar resize handle ── -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div class="sidebar-resizer" role="separator" onmousedown={startSidebarResize}></div>

  <!-- ── Main column ── -->
  <div class="shell-main">
    <!-- ── Pane area ── -->
    <div class="shell-content" bind:this={paneAreaEl}>
      <!-- Connection Manager side panel -->
      {#if app.connectionManagerOpen}
        <ConnectionManager />
      {/if}

      <!-- Primary pane (always shown) -->
      <div class="pane-wrap" style="flex: {app.isSplit ? splitPercent : 100};">
        <Pane
          bind:this={pane0}
          paneIdx={0}
          {...paneHandlers}
        />
      </div>

      <!-- Pane splitter + secondary pane (only when split) -->
      {#if app.isSplit}
        <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
        <div
          class="pane-splitter"
          role="separator"
          aria-label="Resize panes"
          onmousedown={startPaneSplitResize}
        ></div>

        <div class="pane-wrap" style="flex: {100 - splitPercent};">
          <Pane
            bind:this={pane1}
            paneIdx={1}
            {...paneHandlers}
          />
        </div>
      {/if}
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
      enqueueExtract(archives, opts);
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
    margin-left: -1px;
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
    position: relative;
  }

  .pane-wrap {
    display: flex;
    flex-direction: column;
    overflow: hidden;
    min-width: 0;
  }

  .pane-splitter {
    flex-shrink: 0;
    width: 5px;
    height: 100%;
    cursor: col-resize;
    background: var(--line);
    z-index: 5;
    transition: background 0.15s;
    position: relative;

    &:hover,
    &:active { background: var(--accent); }

    &::after {
      content: "";
      position: absolute;
      left: 50%;
      top: 50%;
      transform: translate(-50%, -50%);
      width: 3px;
      height: 40px;
      border-radius: 2px;
      background: color-mix(in srgb, var(--line-strong) 60%, transparent);
      pointer-events: none;
    }

    &:hover::after { background: color-mix(in srgb, var(--accent) 60%, transparent); }
  }
</style>
