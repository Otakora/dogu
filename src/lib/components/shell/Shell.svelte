<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, onDestroy } from "svelte";
  import { app } from "../../stores/app.svelte.js";
  import type {
    ToolStatusDto,
    JobProgressDto,
    JobLogDto,
    JobFinishedDto,
    JobPausedDto,
    SelectionAnalysisDto,
    ChdConversionOptionsPayload,
    ChdRestoreOptionsPayload,
    DiscImageOptionsPayload,
    ExtractionOptionsPayload,
    CompressionOptionsPayload,
    CompressionCapabilitiesDto,
    VolumeDto,
    KnownFoldersDto,
    QueuedOp,
    ChdSourceDto,
    ExtractionPreviewRow,
    M3uGeneratePayload,
    DeaccentRenameResult,
  } from "../../types/index.js";
  import { collectAccentIssues, type AccentIssue } from "../../utils/ascii.js";
  import {
    predictCompress,
    predictChdConvert,
    predictChdRestore,
    predictCsoConvert,
    predictCsoRestore,
    predictXisoPack,
    predictXisoUnpack,
    predictRvzConvert,
    predictRvzRestore,
    buildGhost,
    normalizePath,
    basenameOf,
    dirnameOf,
    joinPath,
    stemOf,
  } from "../../utils/ghosts.js";

  import Sidebar from "./Sidebar.svelte";
  import Pane from "./Pane.svelte";
  import TerminalPanel from "../terminal/TerminalPanel.svelte";
  import ConnectionManager from "../connections/ConnectionManager.svelte";
  import { openTerminalAt } from "../../utils/terminal.js";
  import JobsPanel from "../dialogs/JobsPanel.svelte";
  import ConfirmDialog from "../dialogs/ConfirmDialog.svelte";
  import SettingsDialog from "../dialogs/SettingsDialog.svelte";
  import UpdateDialog from "../dialogs/UpdateDialog.svelte";
  import PropertiesDialog from "../dialogs/PropertiesDialog.svelte";
  import ExtractionDialog from "../dialogs/ExtractionDialog.svelte";
  import CompressionDialog from "../dialogs/CompressionDialog.svelte";
  import ChdDialog from "../dialogs/ChdDialog.svelte";
  import AccentWarningModal from "../dialogs/AccentWarningModal.svelte";
  import DiscImageDialog from "../dialogs/DiscImageDialog.svelte";
  import type { DiscImageMode } from "../dialogs/DiscImageDialog.svelte";
  import ToolStatusScreen from "../dialogs/ToolStatusScreen.svelte";
  import M3uDialog from "../dialogs/M3uDialog.svelte";
  import { t, tn } from "../../i18n/index.js";
  import type { FileTransferItem, FileTransferOperation } from "../../utils/transfers.js";
  import { inferFileTransferOperation, isNoOpFileTransfer, isRecursiveFileTransfer } from "../../utils/transfers.js";

  type QueuedM3uGroup = {
    outputPath: string;
    baseName: string;
    absoluteEntries: string[];
  };

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
  let discImageState = $state<{ mode: DiscImageMode; sources: string[] } | null>(null);
  let toolScreenOpen = $state(false);
  let m3uDirs = $state<string[] | null>(null);
  let startupReady = $state(false);

  // ── Job / queue ID counters ───────────────────────────────
  let _jobSeq = 0;
  function newJobId()   { return `job-${Date.now()}-${++_jobSeq}`; }

  let _queueSeq = 0;
  function newQueueId() { return `qop-${Date.now()}-${++_queueSeq}`; }

  let _batchSeq = 0;
  function newBatchId() { return `qbatch-${Date.now()}-${++_batchSeq}`; }

  function parentDir(p: string): string {
    return p.replace(/[/\\][^/\\]+[/\\]?$/, "") || p;
  }

  function withBatch(op: QueuedOp, batchId: string, batchTitle: string, index: number, total: number): QueuedOp {
    return { ...op, batchId, batchTitle, batchIndex: index, batchTotal: total };
  }

  function enqueueOrRun(op: QueuedOp) {
    if (app.queueMode) {
      app.addToQueue(op);
      app.notify("info", t("queue.operationQueued"));
    } else {
      op.execute();
    }
  }

  function enqueueOps(ops: QueuedOp[]) {
    if (ops.length === 0) return;
    for (const op of ops) app.addToQueue(op);
    app.notify("info", ops.length === 1
      ? t("queue.operationQueued")
      : t("queue.operationsQueued", { count: ops.length }));
  }

  // ── Tauri event listeners ─────────────────────────────────
  const unlisten: Array<() => void> = [];

  onMount(async () => {
    const stored = localStorage.getItem("dogu-sidebar-width");
    if (stored) sidebarWidth = Math.max(140, Math.min(500, parseInt(stored, 10)));

    const storedSplit = localStorage.getItem("dogu-split-percent");
    if (storedSplit) splitPercent = Math.max(20, Math.min(80, parseInt(storedSplit, 10)));

    try {
      const [vols, kf, caps, tools] = await Promise.all([
        invoke<VolumeDto[]>("list_volumes"),
        invoke<KnownFoldersDto>("get_known_folders"),
        invoke<CompressionCapabilitiesDto>("get_compression_capabilities"),
        invoke<ToolStatusDto[]>("check_tools"),
      ]);
      app.setVolumes(vols);
      app.setKnownFolders(kf);
      compressionCapabilities = caps;
      app.setToolStatus(tools);

      // Every launch we re-check tools. Auto-open the preparation screen only
      // when a *required* tool is missing/not runnable; optional ones (e.g.
      // DolphinTool — nod still covers RVZ) are surfaced discreetly in
      // Settings/About instead of nagging on every launch.
      if (app.toolProblems.some(p => !p.optional)) toolScreenOpen = true;
    } catch {
      // System info unavailable
    } finally {
      startupReady = true;
      app.maybeAutoCheckForUpdates();
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
    document.addEventListener("dogu:fs-changed", handleFsChangedRaw);
  });

  onDestroy(() => {
    unlisten.forEach(fn => fn());
    document.removeEventListener("dogu:rename-commit", handleRenameCommitRaw);
    document.removeEventListener("dogu:fs-changed", handleFsChangedRaw);
  });

  // Refresh any pane showing a directory that changed out-of-band (e.g. the
  // destination picker created or deleted a folder inside it).
  function handleFsChangedRaw(e: Event) {
    const { paths } = (e as CustomEvent<{ paths: string[] }>).detail;
    if (Array.isArray(paths) && paths.length > 0) refreshVisibleLocations(paths);
  }

  // ── Refresh helper ────────────────────────────────────────
  function refreshContent() {
    focusedPane()?.refresh();
  }

  function refreshVisibleLocations(paths: string[]) {
    const watched = new Set(paths.map((path) => normalizePath(path)));
    const paneRefs = [pane0, pane1];
    app.panes.forEach((paneState, idx) => {
      const currentPath = paneState.tabs[paneState.activeTabIdx]?.currentPath;
      if (currentPath && watched.has(normalizePath(currentPath))) {
        paneRefs[idx]?.refresh();
      }
    });
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
      const op = buildDeleteOp(paths);
      if (op) enqueueOrRun(op);
      else app.notify("info", t("shell.deleteAlreadyCovered"));
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

  function queuedDeleteCoversPath(path: string): boolean {
    return app.opQueue.some((op) =>
      op.deletes.some((deletedPath) => pathContainsOrEquals(deletedPath, path))
    );
  }

  function pathContainsOrEquals(parent: string, child: string): boolean {
    const p = normalizePath(parent);
    const c = normalizePath(child);
    if (!p) return c === p;
    return c === p || c.startsWith(`${p}/`);
  }

  function compactDeletePaths(paths: string[]): string[] {
    const unique = uniquePaths(paths)
      .filter((path) => !queuedDeleteCoversPath(path))
      .sort((a, b) => normalizePath(a).split("/").length - normalizePath(b).split("/").length);

    const compacted: string[] = [];
    for (const path of unique) {
      if (compacted.some((parent) => pathContainsOrEquals(parent, path))) continue;
      compacted.push(path);
    }
    return compacted;
  }

  function buildDeleteOp(paths: string[]): QueuedOp | null {
    const deletePaths = compactDeletePaths(paths);
    if (deletePaths.length === 0) return null;
    return {
      id: newQueueId(),
      title: t("shell.deleting"),
      kind: 'delete',
      sources: [],
      destinations: [],
      deletes: deletePaths,
      produces: [],
      dependsOn: [],
      overwrite: false,
      renameOnConflict: false,
      execute: (jobId, retryQueuedOp) => executeDelete(deletePaths, jobId, retryQueuedOp),
    };
  }

  async function executeDelete(paths: string[], providedJobId?: string, retryQueuedOp?: QueuedOp | null): Promise<boolean> {
    const jobId = providedJobId ?? newJobId();
    app.startJob(jobId, t("shell.deleting"), {
      statusMessageOnSuccess: t("shell.deleted"),
      statusMessageOnFailure: t("shell.deleteFailed"),
      retryQueuedOp,
    });
    const done = app.waitForJob(jobId);
    try {
      await invoke("start_delete_paths", { jobId, paths });
    } catch (e) {
      app.notify("error", String(e));
      app.finishJob(jobId, false, String(e));
    }
    return await done;
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

    const items = paths.map((path) => ({ path, isDir: !/\.[^./\\]+$/.test(path) }));
    enqueueOrRun(buildTransferOp(items, dest, isCut ? "move" : "copy"));
  }

  async function executePaste(paths: string[], dest: string, isCut: boolean, overwrite: boolean, renameOnConflict: boolean, providedJobId?: string, retryQueuedOp?: QueuedOp | null): Promise<boolean> {
    const jobId = providedJobId ?? newJobId();
    const op = isCut ? "cut" : "copy";
    app.startJob(jobId, op === "copy" ? t("shell.copying") : t("shell.moving"), {
      statusMessageOnSuccess: op === "copy" ? t("shell.copied") : t("shell.moved"),
      statusMessageOnFailure: op === "copy" ? t("shell.copyFailed") : t("shell.moveFailed"),
      retryQueuedOp,
    });
    const done = app.waitForJob(jobId);
    try {
      await invoke("start_copy_or_move_paths", { jobId, paths, destination: dest, operation: op, overwrite, renameOnConflict });
    } catch (e) {
      app.notify("error", String(e));
      app.finishJob(jobId, false, String(e));
    }
    const ok = await done;
    if (ok) refreshVisibleLocations([dest, ...paths.map((path) => dirnameOf(path))]);
    return ok;
  }

  function isQueueBackedPath(path: string): boolean {
    const key = normalizePath(path);
    return app.queueGhosts.some((ghost) => {
      const ghostKey = normalizePath(ghost.path);
      return key === ghostKey || (ghost.isDir && key.startsWith(ghostKey + "/"));
    });
  }

  function transferTouchesGhostTree(items: FileTransferItem[], dest: string): boolean {
    return isQueueBackedPath(dest) || items.some((item) => isQueueBackedPath(item.path));
  }

  function buildTransferOp(items: FileTransferItem[], dest: string, operation: FileTransferOperation): QueuedOp {
    const paths = items.map((item) => item.path);
    const opId = newQueueId();
    const overwrite = app.settings.defaultOverwriteOnConflict;
    const renameOnConflict = app.settings.renameOnConflict;
    const isMove = operation === "move";
    return {
      id: opId,
      title: isMove ? t("shell.moving") : t("shell.copying"),
      kind: isMove ? "move" : "copy",
      sources: paths,
      destinations: [dest],
      deletes: isMove ? paths : [],
      produces: items.map((item) => buildGhost(joinPath(dest, basenameOf(item.path)), item.isDir, opId, false)),
      dependsOn: [],
      overwrite,
      renameOnConflict,
      execute: (jobId, retryQueuedOp) => executePaste(paths, dest, isMove, overwrite, renameOnConflict, jobId, retryQueuedOp),
    };
  }

  function handleDropItems(items: FileTransferItem[], dest: string, preferredOperation?: FileTransferOperation | null) {
    if (items.length === 0) return;
    const operation = inferFileTransferOperation(items, dest, preferredOperation);
    if (isNoOpFileTransfer(items, dest, operation)) return;
    if (isRecursiveFileTransfer(items, dest)) return;
    const op = buildTransferOp(items, dest, operation);
    if (transferTouchesGhostTree(items, dest) && !app.queueMode) {
      app.addToQueue(op);
      app.notify("info", t("queue.operationQueued"));
      return;
    }
    enqueueOrRun(op);
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
      overwrite: opts.overwrite,
      renameOnConflict: opts.renameOnConflict ?? false,
      execute: (jobId, retryQueuedOp) => executeExtraction(archives, opts, jobId, retryQueuedOp),
    };
  }

  /** True when `path` is currently a ghost output of some queued op. */
  function isGhostPath(path: string): boolean {
    const np = normalizePath(path);
    return app.queueGhosts.some(g => normalizePath(g.path) === np);
  }

  /**
   * Enqueues (or runs) an extraction. In queue mode each archive becomes its
   * own planned operation, so the scheduler can overlap independent archives.
   * Local archives still get a deep ghost preview for downstream operations.
   */
  async function enqueueExtract(archives: string[], opts: ExtractionOptionsPayload) {
    opts = { ...opts, renameOnConflict: app.settings.renameOnConflict };
    if (!app.queueMode) {
      void buildExtractOp(archives, opts).execute();
      return;
    }

    const batchId = archives.length > 1 ? newBatchId() : null;
    const batchTitle = archives.length > 1
      ? t("queue.batch.extract", { count: archives.length })
      : "";
    const ops = archives.map((archive, index) => {
      const op = buildExtractOp([archive], opts);
      return batchId ? withBatch(op, batchId, batchTitle, index + 1, archives.length) : op;
    });

    const previewable = archives.filter(a => !isGhostPath(a) && !a.startsWith("remote://"));
    if (previewable.length > 0) {
      try {
        const rows = await invoke<ExtractionPreviewRow[]>("build_extraction_preview_deep", {
          archives: previewable, options: opts,
        });
        const rowsByArchive = new Map(rows.map((row) => [normalizePath(row.archivePath), row]));
        for (const op of ops) {
          const row = rowsByArchive.get(normalizePath(op.sources[0] ?? ""));
          if (!row) continue;
          op.produces = row.entries
            .filter(e => e.destinationPath)
            .map(e => buildGhost(e.destinationPath, e.isDir, op.id, false));
        }
      } catch (e) {
        app.notify("warn", t("queue.extractGhostPreviewFailed", { error: String(e) }));
      }
    }
    enqueueOps(ops);
  }

  async function executeExtraction(archives: string[], opts: ExtractionOptionsPayload, providedJobId?: string, retryQueuedOp?: QueuedOp | null): Promise<boolean> {
    const jobId = providedJobId ?? newJobId();
    app.startJob(jobId, t("shell.extracting"), {
      statusMessageOnSuccess: t("shell.extractionComplete"),
      statusMessageOnFailure: t("shell.extractionFailed"),
      retryQueuedOp,
    });
    const done = app.waitForJob(jobId);
    try {
      await invoke("start_extract_archives", { jobId, archives, options: opts });
    } catch (e) {
      app.notify("error", String(e));
      app.finishJob(jobId, false, String(e));
    }
    return await done;
  }

  // ── M3U ───────────────────────────────────────────────────
  export function openM3uDialog(paths: string[]) {
    const dirs = paths.filter((p) => p !== "");
    if (dirs.length === 0) return;
    m3uDirs = dirs;
  }

  function m3uRelativePath(fromDir: string, targetFile: string): string {
    const from = fromDir.replace(/\\/g, "/").replace(/\/+$/, "");
    const target = targetFile.replace(/\\/g, "/").replace(/\/+$/, "");
    const prefix = `${from}/`;
    return target.startsWith(prefix) ? target.slice(prefix.length) : targetFile;
  }

  function appendRelativePath(base: string, rel: string): string {
    const parts = rel.split(/[\\/]+/).filter(Boolean);
    return parts.reduce((acc, part) => joinPath(acc, part), base);
  }

  function relativePathWithin(root: string, path: string): string | null {
    const rootParts = root.replace(/\\/g, "/").replace(/\/+$/, "").split("/").filter(Boolean);
    const pathParts = path.replace(/\\/g, "/").replace(/\/+$/, "").split("/").filter(Boolean);
    if (pathParts.length < rootParts.length) return null;
    for (let i = 0; i < rootParts.length; i++) {
      if (rootParts[i].toLowerCase() !== pathParts[i].toLowerCase()) return null;
    }
    return pathParts.slice(rootParts.length).join("/");
  }

  function remapPathThroughQueuedOps(path: string): string {
    let current = path;
    for (const op of app.opQueue) {
      if (op.kind !== "copy" && op.kind !== "move") continue;
      const outputs = app.resolvedOutputsFor(op.id);
      for (let i = 0; i < op.sources.length; i++) {
        const source = op.sources[i];
        const output = outputs[i]?.path;
        if (!output) continue;
        const rel = relativePathWithin(source, current);
        if (rel === null) continue;
        current = rel ? appendRelativePath(output, rel) : output;
        break;
      }
    }
    return current;
  }

  function buildQueuedM3uPayload(
    groups: QueuedM3uGroup[],
    useRelativePaths: boolean,
    overwrite: boolean,
  ): { payload: M3uGeneratePayload; outputPaths: string[]; sources: string[] } {
    const resolvedGroups = groups.map((group) => {
      const outputPath = remapPathThroughQueuedOps(group.outputPath);
      const absoluteEntries = uniquePaths(group.absoluteEntries.map((entry) => remapPathThroughQueuedOps(entry)));
      return {
        outputPath,
        baseName: group.baseName,
        absoluteEntries,
        entries: absoluteEntries.map((entry) =>
          useRelativePaths ? m3uRelativePath(dirnameOf(outputPath), entry) : entry,
        ),
      };
    });
    return {
      payload: {
        overwrite,
        renameOnConflict: app.settings.renameOnConflict,
        groups: resolvedGroups.map((group) => ({
          outputPath: group.outputPath,
          baseName: group.baseName,
          entries: group.entries,
        })),
      },
      outputPaths: resolvedGroups.map((group) => group.outputPath),
      sources: uniquePaths(resolvedGroups.flatMap((group) => group.absoluteEntries)),
    };
  }

  function buildM3uOp(groups: QueuedM3uGroup[], useRelativePaths: boolean, overwrite: boolean): QueuedOp {
    const opId = newQueueId();
    const { payload, outputPaths, sources } = buildQueuedM3uPayload(groups, useRelativePaths, overwrite);
    return {
      id: opId,
      title: t("shell.generatingM3u"),
      kind: 'm3u',
      sources,
      destinations: outputPaths.map(p => parentDir(p)),
      deletes: [],
      produces: outputPaths.map(p => buildGhost(p, false, opId, false)),
      dependsOn: [],
      overwrite: payload.overwrite,
      renameOnConflict: payload.renameOnConflict ?? false,
      execute: (jobId, retryQueuedOp) => executeM3u(payload, jobId, retryQueuedOp),
    };
  }

  async function executeM3u(payload: M3uGeneratePayload, providedJobId?: string, retryQueuedOp?: QueuedOp | null): Promise<boolean> {
    const jobId = providedJobId ?? newJobId();
    app.startJob(jobId, t("shell.generatingM3u"), {
      statusMessageOnSuccess: t("shell.m3uComplete"),
      statusMessageOnFailure: t("shell.m3uFailed"),
      retryQueuedOp,
    });
    const done = app.waitForJob(jobId);
    try {
      await invoke("generate_m3u_files", { payload });
      app.finishJob(jobId, true, t("shell.m3uComplete"));
    } catch (e) {
      app.notify("error", String(e));
      app.finishJob(jobId, false, String(e));
    }
    return await done;
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

  function uniquePaths(paths: string[]): string[] {
    const seen = new Set<string>();
    const result: string[] = [];
    for (const path of paths) {
      const key = normalizePath(path);
      if (seen.has(key)) continue;
      seen.add(key);
      result.push(path);
    }
    return result;
  }

  function expandGhostSelections(paths: string[]): string[] {
    const expanded = new Map<string, string>();
    const selected = paths.map((path) => ({ path, key: normalizePath(path) }));

    for (const { path, key } of selected) {
      expanded.set(key, path);
      for (const ghost of app.queueGhosts) {
        const ghostKey = normalizePath(ghost.path);
        if (ghostKey === key || ghostKey.startsWith(key + "/")) {
          expanded.set(ghostKey, ghost.path);
        }
      }
    }

    return [...expanded.values()];
  }

  function cueSiblingForGhostBin(binPath: string, ghostSet: Map<string, string>): string | null {
    const dir = dirnameOf(binPath);
    const sameStemCue = joinPath(dir, `${stemOf(basenameOf(binPath))}.cue`);
    const sameStemKey = normalizePath(sameStemCue);
    if (ghostSet.has(sameStemKey)) return ghostSet.get(sameStemKey)!;

    for (const [key, path] of ghostSet) {
      if (normalizePath(dirnameOf(path)) !== normalizePath(dir)) continue;
      if (key.endsWith(".cue")) return path;
    }
    return null;
  }

  /**
   * Builds a CHD analysis for ghost sources without touching disk. A ghost's
   * companion files (e.g. the .bin next to a .cue) are also ghosts and will
   * exist by the time the conversion runs, so nothing is reported missing.
   */
  function synthesizeGhostChdAnalysis(ghostPaths: string[]): Pick<SelectionAnalysisDto, "chdSources" | "restorableChds"> {
    const chdSources: ChdSourceDto[] = [];
    const restorableChds: string[] = [];
    const seenSources = new Set<string>();
    const seenChds = new Set<string>();
    const expanded = expandGhostSelections(ghostPaths);
    const ghostSet = new Map(expanded.map((path) => [normalizePath(path), path]));

    for (const p of expanded) {
      const ext = (basenameOf(p).split(".").pop() ?? "").toLowerCase();
      if (ext === "chd") {
        const key = normalizePath(p);
        if (seenChds.has(key)) continue;
        seenChds.add(key);
        restorableChds.push(p);
        continue;
      }
      const isDvd = DVD_GHOST_EXTS.has(ext);
      const isCd = CD_GHOST_EXTS.has(ext);
      if (!isCd && !isDvd && ext !== "bin") continue; // not a directly convertible disc image

      let sourcePath = p;
      let displayExtensions = [`.${ext}`];
      let requiredPaths = [p];

      if (ext === "bin") {
        const cuePath = cueSiblingForGhostBin(p, ghostSet);
        if (!cuePath) continue;
        sourcePath = cuePath;
        displayExtensions = [".bin", ".cue"];
        requiredPaths = [cuePath, p];
      }

      const key = normalizePath(sourcePath);
      if (seenSources.has(key)) continue;
      seenSources.add(key);
      chdSources.push({
        sourcePath,
        containerDir: dirnameOf(sourcePath),
        command: isDvd ? "createdvd" : "createcd",
        displayExtensions,
        requiredPaths,
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

  // ── Accent (non-ASCII path) guard for chd operations ──────
  // chdman can't process non-ASCII *file names* (accented folders are handled by
  // the backend cwd workaround). Outside the queue we prompt to de-accent-rename
  // the offending files first; in the queue such ops are blocked until de-accented.
  type AccentPrompt = { issues: AccentIssue[]; onRename: () => void | Promise<void>; onCancel: () => void };
  let accentPrompt = $state<AccentPrompt | null>(null);
  let accentBusy = $state(false);

  async function deaccentFixable(issues: AccentIssue[]): Promise<Map<string, string>> {
    const fixable = issues.filter((i) => i.fixable);
    const results = await invoke<DeaccentRenameResult[]>("deaccent_rename", {
      renames: fixable.map((i) => ({ path: i.path, newName: i.newName })),
    });
    if (results.length) app.notify("info", t("accents.renamedNotice", { count: results.length }));
    return new Map(results.map((r) => [r.oldPath, r.newPath]));
  }

  /** Runs an accent-unsafe (chd) op outside the queue, prompting to de-accent first. */
  function guardChdRun(sourcePaths: string[], proceed: (renamed: Map<string, string>) => void) {
    const issues = collectAccentIssues(sourcePaths);
    if (issues.length === 0) { proceed(new Map()); return; }
    accentPrompt = {
      issues,
      onCancel: () => { accentPrompt = null; },
      onRename: async () => {
        accentBusy = true;
        try {
          const map = await deaccentFixable(issues);
          accentPrompt = null;
          proceed(map);
        } catch (e) {
          app.notify("error", t("accents.renameFailed", { error: String(e) }));
        } finally {
          accentBusy = false;
        }
      },
    };
  }

  function remapChdSource(s: ChdSourceDto, map: Map<string, string>): ChdSourceDto {
    if (map.size === 0) return s;
    return {
      ...s,
      sourcePath: map.get(s.sourcePath) ?? s.sourcePath,
      requiredPaths: s.requiredPaths.map((p) => map.get(p) ?? p),
    };
  }

  /** Queue-side de-accent: prompt, rename the files, then replace the blocked op
   *  with a fresh one pointing at the renamed paths. */
  function handleQueueDeaccent(op: QueuedOp) {
    if (!op.rebuildWithRenames) return;
    const issues = collectAccentIssues(op.sources);
    if (issues.length === 0) return;
    accentPrompt = {
      issues,
      onCancel: () => { accentPrompt = null; },
      onRename: async () => {
        accentBusy = true;
        try {
          const map = await deaccentFixable(issues);
          const newOp = op.rebuildWithRenames!(map);
          app.removeFromQueue(op.id);
          app.addToQueue(newOp);
          accentPrompt = null;
        } catch (e) {
          app.notify("error", t("accents.renameFailed", { error: String(e) }));
        } finally {
          accentBusy = false;
        }
      },
    };
  }

  function buildConvertChdOp(chdSources: ChdSourceDto[], rawOpts: ChdConversionOptionsPayload): QueuedOp {
    const paths = chdSources.map((source) => source.sourcePath);
    const opts: ChdConversionOptionsPayload = { ...rawOpts, renameOnConflict: app.settings.renameOnConflict };
    const deletePaths = opts.deleteOriginals
      ? uniquePaths([
          ...chdSources.flatMap((source) => source.requiredPaths),
          ...(opts.deleteOriginalSubfolders
            ? chdSources
                .map((source) => source.containerDir)
                .filter((dir) => !dir.startsWith("remote://"))
            : []),
        ])
      : [];
    const opId = newQueueId();
    return {
      id: opId,
      title: t("shell.convertingToChd"),
      kind: 'chd-convert',
      sources: paths,
      destinations: paths.map(p => parentDir(p)),
      deletes: deletePaths,
      produces: predictChdConvert(paths, opts, opId),
      dependsOn: [],
      overwrite: opts.overwrite,
      renameOnConflict: opts.renameOnConflict ?? false,
      execute: (jobId, retryQueuedOp) => executeConvertChd(paths, opts, jobId, retryQueuedOp),
      rebuildWithRenames: (map) =>
        buildConvertChdOp(chdSources.map((s) => remapChdSource(s, map)), rawOpts),
    };
  }

  function buildConvertChdOps(chdSources: ChdSourceDto[], rawOpts: ChdConversionOptionsPayload): QueuedOp[] {
    if (chdSources.length === 0) return [];
    if (chdSources.length <= 1) return [buildConvertChdOp(chdSources, rawOpts)];
    const batchId = newBatchId();
    const batchTitle = t("queue.batch.chdConvert", { count: chdSources.length });
    return chdSources.map((source, index) =>
      withBatch(buildConvertChdOp([source], rawOpts), batchId, batchTitle, index + 1, chdSources.length)
    );
  }

  async function executeConvertChd(paths: string[], opts: ChdConversionOptionsPayload, providedJobId?: string, retryQueuedOp?: QueuedOp | null): Promise<boolean> {
    const jobId = providedJobId ?? newJobId();
    app.startJob(jobId, t("shell.convertingToChd"), {
      statusMessageOnSuccess: t("shell.chdConversionComplete"),
      statusMessageOnFailure: t("shell.chdConversionFailed"),
      retryQueuedOp,
    });
    const done = app.waitForJob(jobId);
    try {
      await invoke("start_convert_to_chd", { jobId, paths, options: opts });
    } catch (e) {
      app.notify("error", String(e));
      app.finishJob(jobId, false, String(e));
    }
    return await done;
  }

  function buildRestoreChdOp(paths: string[], rawOpts: ChdRestoreOptionsPayload): QueuedOp {
    const opts: ChdRestoreOptionsPayload = { ...rawOpts, renameOnConflict: app.settings.renameOnConflict };
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
      overwrite: opts.overwrite,
      renameOnConflict: opts.renameOnConflict ?? false,
      execute: (jobId, retryQueuedOp) => executeRestoreChd(paths, opts, jobId, retryQueuedOp),
      rebuildWithRenames: (map) =>
        buildRestoreChdOp(paths.map((p) => map.get(p) ?? p), rawOpts),
    };
  }

  function buildRestoreChdOps(paths: string[], rawOpts: ChdRestoreOptionsPayload): QueuedOp[] {
    if (paths.length === 0) return [];
    if (paths.length <= 1) return [buildRestoreChdOp(paths, rawOpts)];
    const batchId = newBatchId();
    const batchTitle = t("queue.batch.chdRestore", { count: paths.length });
    return paths.map((path, index) =>
      withBatch(buildRestoreChdOp([path], rawOpts), batchId, batchTitle, index + 1, paths.length)
    );
  }

  async function executeRestoreChd(paths: string[], opts: ChdRestoreOptionsPayload, providedJobId?: string, retryQueuedOp?: QueuedOp | null): Promise<boolean> {
    const jobId = providedJobId ?? newJobId();
    app.startJob(jobId, t("shell.restoringFromChd"), {
      statusMessageOnSuccess: t("shell.chdRestoreComplete"),
      statusMessageOnFailure: t("shell.chdRestoreFailed"),
      retryQueuedOp,
    });
    const done = app.waitForJob(jobId);
    try {
      await invoke("start_restore_from_chd", { jobId, paths, options: opts });
    } catch (e) {
      app.notify("error", String(e));
      app.finishJob(jobId, false, String(e));
    }
    return await done;
  }

  // ── Compress ──────────────────────────────────────────────
  // ---- Native disc-image treatments (CSO / XISO / RVZ) ----
  // Remote sources are supported: the backend stages them to a local temp,
  // operates, and uploads the result back (in-place for "same folder"), just
  // like CHD. Remote sources convert in place; a local custom folder pulls the
  // output down instead.
  function openDiscImageDialog(mode: DiscImageMode, paths: string[]) {
    if (paths.length === 0) return;
    discImageState = { mode, sources: paths };
  }

  function discImageTitle(mode: DiscImageMode): string {
    if (mode === "cso-convert") return t("shell.convertingToCso");
    if (mode === "cso-restore") return t("shell.restoringFromCso");
    if (mode === "xiso-pack") return t("shell.packingToXiso");
    if (mode === "rvz-convert") return t("shell.convertingToRvz");
    if (mode === "rvz-restore") return t("shell.restoringFromRvz");
    return t("shell.unpackingXiso");
  }

  function discImageSuccess(mode: DiscImageMode): string {
    if (mode === "cso-convert") return t("shell.csoConversionComplete");
    if (mode === "cso-restore") return t("shell.csoRestoreComplete");
    if (mode === "xiso-pack") return t("shell.xisoPackComplete");
    if (mode === "rvz-convert") return t("shell.rvzConversionComplete");
    if (mode === "rvz-restore") return t("shell.rvzRestoreComplete");
    return t("shell.xisoUnpackComplete");
  }

  function discImageFailure(mode: DiscImageMode): string {
    if (mode === "cso-convert") return t("shell.csoConversionFailed");
    if (mode === "cso-restore") return t("shell.csoRestoreFailed");
    if (mode === "xiso-pack") return t("shell.xisoPackFailed");
    if (mode === "rvz-convert") return t("shell.rvzConversionFailed");
    if (mode === "rvz-restore") return t("shell.rvzRestoreFailed");
    return t("shell.xisoUnpackFailed");
  }

  function discImageCommand(mode: DiscImageMode): string {
    if (mode === "cso-convert") return "start_convert_to_cso";
    if (mode === "cso-restore") return "start_restore_from_cso";
    if (mode === "xiso-pack") return "start_pack_to_xiso";
    if (mode === "rvz-convert") return "start_convert_to_rvz";
    if (mode === "rvz-restore") return "start_restore_from_rvz";
    return "start_unpack_xiso";
  }

  function buildDiscImageOp(mode: DiscImageMode, paths: string[], rawOpts: DiscImageOptionsPayload): QueuedOp {
    const opts: DiscImageOptionsPayload = {
      ...rawOpts,
      renameOnConflict: app.settings.renameOnConflict,
      // RVZ engine routing comes from settings; ignored by non-RVZ modes.
      rvzPrimaryEngine: app.settings.rvzPrimaryEngine,
      rvzFallbackEngine: app.settings.rvzFallbackEngine,
      rvzEnableFallback: app.settings.rvzEnableFallback,
    };
    const opId = newQueueId();
    const destinations = opts.destinationPath ? [opts.destinationPath] : paths.map(p => parentDir(p));
    const produces =
      mode === "cso-convert" ? predictCsoConvert(paths, opts, opId)
      : mode === "cso-restore" ? predictCsoRestore(paths, opts, opId)
      : mode === "xiso-pack" ? predictXisoPack(paths, opts, opId)
      : mode === "rvz-convert" ? predictRvzConvert(paths, opts, opId)
      : mode === "rvz-restore" ? predictRvzRestore(paths, opts, opId)
      : predictXisoUnpack(paths, opts, opId);
    return {
      id: opId,
      title: discImageTitle(mode),
      kind: mode,
      sources: paths,
      destinations,
      deletes: opts.deleteOriginals ? paths : [],
      produces,
      dependsOn: [],
      overwrite: opts.overwrite,
      renameOnConflict: opts.renameOnConflict ?? false,
      execute: (jobId, retryQueuedOp) => executeDiscImage(mode, paths, opts, jobId, retryQueuedOp),
    };
  }

  function buildDiscImageOps(mode: DiscImageMode, paths: string[], opts: DiscImageOptionsPayload): QueuedOp[] {
    if (paths.length === 0) return [];
    if (paths.length <= 1) return [buildDiscImageOp(mode, paths, opts)];
    const batchId = newBatchId();
    const batchTitle = t(`queue.batch.${mode}`, { count: paths.length });
    return paths.map((path, index) =>
      withBatch(buildDiscImageOp(mode, [path], opts), batchId, batchTitle, index + 1, paths.length)
    );
  }

  async function executeDiscImage(mode: DiscImageMode, paths: string[], opts: DiscImageOptionsPayload, providedJobId?: string, retryQueuedOp?: QueuedOp | null): Promise<boolean> {
    const jobId = providedJobId ?? newJobId();
    app.startJob(jobId, discImageTitle(mode), {
      statusMessageOnSuccess: discImageSuccess(mode),
      statusMessageOnFailure: discImageFailure(mode),
      retryQueuedOp,
    });
    const done = app.waitForJob(jobId);
    try {
      await invoke(discImageCommand(mode), { jobId, paths, options: opts });
    } catch (e) {
      app.notify("error", String(e));
      app.finishJob(jobId, false, String(e));
    }
    return await done;
  }

  function handleCompressQuick(paths: string[]) {
    if (paths.length === 0) return;
    const stem = paths[0].replace(/\\/g, "/").split("/").filter(Boolean).pop()?.replace(/\.[A-Za-z0-9]{1,8}$/, "") ?? "archive";
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

  function buildCompressOp(sources: string[], rawOpts: CompressionOptionsPayload): QueuedOp {
    const opts: CompressionOptionsPayload = { ...rawOpts, renameOnConflict: app.settings.renameOnConflict };
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
      overwrite: opts.overwrite,
      renameOnConflict: opts.renameOnConflict ?? false,
      execute: (jobId, retryQueuedOp) => executeCompress(sources, opts, jobId, retryQueuedOp),
    };
  }

  async function executeCompress(sources: string[], opts: CompressionOptionsPayload, providedJobId?: string, retryQueuedOp?: QueuedOp | null): Promise<boolean> {
    const jobId = providedJobId ?? newJobId();
    app.startJob(jobId, t("shell.compressing"), {
      statusMessageOnSuccess: t("shell.compressionComplete"),
      statusMessageOnFailure: t("shell.compressionFailed"),
      retryQueuedOp,
    });
    const done = app.waitForJob(jobId);
    try {
      await invoke("start_compress", { jobId, sources, options: opts });
    } catch (e) {
      app.notify("error", String(e));
      app.finishJob(jobId, false, String(e));
    }
    return await done;
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
    onDropItems: handleDropItems,
    onExtractHere: handleExtractHere,
    onExtractToFolder: handleExtractToFolder,
    onExtractTo: handleExtractTo,
    onCompressQuick: handleCompressQuick,
    onCompress: handleCompress,
    onChd: openChdDialog,
    onDiscImage: openDiscImageDialog,
    onM3u: openM3uDialog,
    onProperties: handleProperties,
    onOpenWith: handleOpenWith,
  };
</script>

<svelte:window onkeydown={handleAppKey} />
<svelte:document oncontextmenu={(e) => e.preventDefault()} />

<div class="shell" class:startup-loading={!startupReady} style="--sidebar-width: {sidebarWidth}px">
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
    <JobsPanel onDeaccentOp={handleQueueDeaccent} />
  </div>
</div>

{#if !startupReady}
  <div class="startup-overlay" role="status" aria-live="polite" aria-label={t("contentPanel.loading")}>
    <span class="startup-spinner"></span>
  </div>
{/if}

<!-- ── Dialogs ── -->
<ConfirmDialog />
<SettingsDialog />
<UpdateDialog />

{#if accentPrompt}
  <AccentWarningModal
    issues={accentPrompt.issues}
    busy={accentBusy}
    onRename={accentPrompt.onRename}
    onCancel={accentPrompt.onCancel}
  />
{/if}

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
  <M3uDialog
    dirs={m3uDirs}
    currentDir={app.currentPath ?? ""}
    onclose={() => (m3uDirs = null)}
    onEnqueue={(groups, useRelativePaths, overwrite) => {
      enqueueOrRun(buildM3uOp(groups, useRelativePaths, overwrite));
      m3uDirs = null;
    }}
  />
{/if}

{#if chdState}
  <ChdDialog
    analysis={chdState.analysis}
    initialMode={chdState.mode}
    onclose={() => (chdState = null)}
    onConvert={(sources, opts) => {
      if (app.queueMode) enqueueOps(buildConvertChdOps(sources, opts));
      else guardChdRun(sources.map((s) => s.sourcePath), (map) => {
        void buildConvertChdOp(sources.map((s) => remapChdSource(s, map)), opts).execute();
      });
      chdState = null;
    }}
    onRestore={(paths, opts) => {
      if (app.queueMode) enqueueOps(buildRestoreChdOps(paths, opts));
      else guardChdRun(paths, (map) => {
        void buildRestoreChdOp(paths.map((p) => map.get(p) ?? p), opts).execute();
      });
      chdState = null;
    }}
  />
{/if}

{#if discImageState}
  <DiscImageDialog
    mode={discImageState.mode}
    sources={discImageState.sources}
    onclose={() => (discImageState = null)}
    onConfirm={(sources, opts) => {
      if (app.queueMode) enqueueOps(buildDiscImageOps(discImageState!.mode, sources, opts));
      else void buildDiscImageOp(discImageState!.mode, sources, opts).execute();
      discImageState = null;
    }}
  />
{/if}

{#if toolScreenOpen}
  <ToolStatusScreen onclose={() => (toolScreenOpen = false)} />
{/if}

<style>
  .shell {
    display: flex;
    height: calc(100vh / var(--app-font-scale, 1));
    overflow: hidden;
    background: var(--bg);
    zoom: var(--app-font-scale, 1);
  }

  .shell.startup-loading {
    filter: blur(4px);
    pointer-events: none;
    user-select: none;
  }

  .startup-overlay {
    position: fixed;
    inset: 0;
    z-index: 900;
    display: grid;
    place-items: center;
    background: color-mix(in srgb, var(--bg) 42%, transparent);
    backdrop-filter: blur(2px);
  }

  .startup-spinner {
    width: 52px;
    height: 52px;
    border-radius: 50%;
    border: 4px solid color-mix(in srgb, var(--line-strong) 45%, transparent);
    border-top-color: var(--accent);
    border-right-color: color-mix(in srgb, var(--accent) 62%, var(--line));
    box-shadow: 0 0 0 1px color-mix(in srgb, var(--surface) 50%, transparent);
    animation: spin 0.75s linear infinite;
  }

  @media (prefers-reduced-motion: reduce) {
    .startup-spinner {
      animation: none;
    }
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
