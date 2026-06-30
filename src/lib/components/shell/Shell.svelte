<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { listen } from "@tauri-apps/api/event";
  import { onMount, onDestroy } from "svelte";
  import { app } from "../../stores/app.svelte.js";
  import type {
    JobProgressDto,
    JobLogDto,
    JobFinishedDto,
    SelectionAnalysisDto,
    ChdConversionOptionsPayload,
    ChdRestoreOptionsPayload,
    ExtractionOptionsPayload,
  } from "../../types/index.js";

  import Sidebar from "./Sidebar.svelte";
  import Toolbar from "./Toolbar.svelte";
  import ContentPanel from "../content/ContentPanel.svelte";
  import ConnectionManager from "../connections/ConnectionManager.svelte";
  import ProgressDialog from "../dialogs/ProgressDialog.svelte";
  import ConfirmDialog from "../dialogs/ConfirmDialog.svelte";
  import SettingsDialog from "../dialogs/SettingsDialog.svelte";
  import PropertiesDialog from "../dialogs/PropertiesDialog.svelte";
  import ExtractionDialog from "../dialogs/ExtractionDialog.svelte";
  import ChdDialog from "../dialogs/ChdDialog.svelte";

  // ── ContentPanel ref for imperative calls ────────────────
  let contentPanel = $state<ReturnType<typeof ContentPanel> | undefined>(undefined);

  // ── Dialog state (local to Shell) ───────────────────────
  let propertiesState = $state<{ paths: string[] } | null>(null);
  let extractionState = $state<{ archives: string[] } | null>(null);
  let chdState = $state<{ analysis: SelectionAnalysisDto; mode: "convert" | "restore" } | null>(null);

  // ── Job ID counter ────────────────────────────────────────
  let _jobSeq = 0;
  function newJobId() { return `job-${Date.now()}-${++_jobSeq}`; }

  // ── Tauri event listeners ─────────────────────────────────
  const unlisten: Array<() => void> = [];

  onMount(async () => {
    unlisten.push(await listen<JobProgressDto>("job-progress", ({ payload }) => {
      app.updateJobProgress(payload.jobId, payload.progress, payload.message);
    }));
    unlisten.push(await listen<JobLogDto>("job-log", ({ payload }) => {
      app.appendJobLog(payload.jobId, payload.line);
    }));
    unlisten.push(await listen<JobFinishedDto>("job-finished", ({ payload }) => {
      app.finishJob(payload.jobId, payload.success, payload.message);
      if (payload.success) refreshContent();
    }));

    // Custom events from ContentPanel / FileRow
    document.addEventListener("dogu:rename-commit", handleRenameCommitRaw);
    document.addEventListener("dogu:new-folder", handleRenameNewFolderRaw);
  });

  onDestroy(() => {
    unlisten.forEach(fn => fn());
    document.removeEventListener("dogu:rename-commit", handleRenameCommitRaw);
    document.removeEventListener("dogu:new-folder", handleRenameNewFolderRaw);
  });

  // ── Refresh helper ────────────────────────────────────────
  function refreshContent() {
    contentPanel?.refresh();
  }

  // ── Keyboard shortcuts (app-level) ───────────────────────
  function handleAppKey(e: KeyboardEvent) {
    if (e.key === "F5") { e.preventDefault(); refreshContent(); }
    if (e.key === "Escape") {
      if (app.connectionManagerOpen) app.closeConnectionManager();
      else if (app.settingsOpen) app.closeSettings();
    }
  }

  // ── Rename commit ─────────────────────────────────────────
  function handleRenameCommitRaw(e: Event) {
    const { path, newName } = (e as CustomEvent<{ path: string; newName: string }>).detail;
    app.cancelRename();
    invoke("rename_path", { path, newName })
      .then(() => refreshContent())
      .catch(err => app.notify("error", `Rename failed: ${err}`));
  }

  function handleRenameNewFolderRaw(_e: Event) { triggerNewFolder(); }

  // ── New folder ────────────────────────────────────────────
  // (exposed as custom event handler above)

  async function triggerNewFolder() {
    if (!app.currentPath) return;
    // Prompt for name — TODO: replace with inline input someday
    const name = window.prompt("New folder name:");
    if (!name?.trim()) return;
    try {
      await invoke("create_folder", { parent: app.currentPath, name: name.trim() });
      refreshContent();
    } catch (e) {
      app.notify("error", `Could not create folder: ${e}`);
    }
  }

  // ── Delete ────────────────────────────────────────────────
  function handleDelete(paths: string[]) {
    if (paths.length === 0) return;
    const label = paths.length === 1
      ? paths[0].split(/[\\/]/).filter(Boolean).pop() ?? paths[0]
      : `${paths.length} items`;

    if (app.settings.confirmDelete) {
      app.openConfirm({
        title: "Delete",
        message: `Permanently delete ${label}? This cannot be undone.`,
        confirmLabel: "Delete",
        onConfirm: () => executeDelete(paths),
      });
    } else {
      executeDelete(paths);
    }
  }

  async function executeDelete(paths: string[]) {
    const jobId = newJobId();
    const hide = app.settings.hideDeleteProgressPopup;
    app.startJob(jobId, "Deleting…", {
      showDialog: !hide,
      statusMessageOnSuccess: "Deleted.",
      statusMessageOnFailure: "Delete failed.",
    });
    try {
      await invoke("start_delete_paths", { jobId, paths });
    } catch (e) {
      app.notify("error", String(e));
    }
  }

  // ── Copy / Cut / Paste ────────────────────────────────────
  function handleCopy(paths: string[]) {
    app.setClipboard({ paths, operation: "copy" });
    app.notify("info", `${paths.length} item${paths.length !== 1 ? "s" : ""} copied to clipboard.`);
  }

  function handleCut(paths: string[]) {
    app.setClipboard({ paths, operation: "cut" });
    app.notify("info", `${paths.length} item${paths.length !== 1 ? "s" : ""} cut to clipboard.`);
  }

  async function handlePaste() {
    const cb = app.clipboard;
    const dest = app.currentPath;
    if (!cb || !dest) return;

    const jobId = newJobId();
    const isCut = cb.operation === "cut";
    const op = isCut ? "move" : "copy";
    app.startJob(jobId, op === "copy" ? "Copying…" : "Moving…", {
      statusMessageOnSuccess: op === "copy" ? "Copied." : "Moved.",
      statusMessageOnFailure: `${op === "copy" ? "Copy" : "Move"} failed.`,
    });

    if (isCut) app.setClipboard(null);

    try {
      await invoke("start_copy_or_move_paths", {
        jobId,
        paths: cb.paths,
        destination: dest,
        operation: op,
        overwrite: false,
      });
    } catch (e) {
      app.notify("error", String(e));
    }
  }

  // ── Extraction ────────────────────────────────────────────
  function handleExtract(paths: string[]) {
    extractionState = { archives: paths };
  }

  async function executeExtraction(archives: string[], opts: ExtractionOptionsPayload) {
    const jobId = newJobId();
    app.startJob(jobId, "Extracting…", {
      statusMessageOnSuccess: "Extraction complete.",
      statusMessageOnFailure: "Extraction failed.",
    });
    try {
      await invoke("start_extract_archives", { jobId, archives, options: opts });
    } catch (e) {
      app.notify("error", String(e));
    }
  }

  // ── CHD ───────────────────────────────────────────────────
  export async function openChdDialog(paths: string[]) {
    try {
      const analysis = await invoke<SelectionAnalysisDto>("scan_selection", { paths });
      if (analysis.chdSources.length === 0 && analysis.restorableChds.length === 0) {
        app.notify("info", "No CHD-compatible files found in selection.");
        return;
      }
      const mode = analysis.chdSources.length > 0 ? "convert" : "restore";
      chdState = { analysis, mode };
    } catch (e) {
      app.notify("error", String(e));
    }
  }

  async function executeConvertChd(paths: string[], opts: ChdConversionOptionsPayload) {
    const jobId = newJobId();
    app.startJob(jobId, "Converting to CHD…", {
      statusMessageOnSuccess: "CHD conversion complete.",
      statusMessageOnFailure: "CHD conversion failed.",
    });
    try {
      await invoke("start_convert_to_chd", { jobId, paths, options: opts });
    } catch (e) {
      app.notify("error", String(e));
    }
  }

  async function executeRestoreChd(paths: string[], opts: ChdRestoreOptionsPayload) {
    const jobId = newJobId();
    app.startJob(jobId, "Restoring from CHD…", {
      statusMessageOnSuccess: "CHD restore complete.",
      statusMessageOnFailure: "CHD restore failed.",
    });
    try {
      await invoke("start_restore_from_chd", { jobId, paths, options: opts });
    } catch (e) {
      app.notify("error", String(e));
    }
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
</script>

<svelte:window onkeydown={handleAppKey} />

<div class="shell">
  <!-- ── Sidebar ── -->
  <Sidebar />

  <!-- ── Main column ── -->
  <div class="shell-main">
    <Toolbar
      onRefresh={refreshContent}
      onNewFolder={triggerNewFolder}
      onSearch={handleSearch}
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
        onExtract={handleExtract}
        onProperties={handleProperties}
        onOpenWith={handleOpenWith}
      />
    </div>
  </div>
</div>

<!-- ── Dialogs ── -->
<ProgressDialog />
<ConfirmDialog />
<SettingsDialog />

{#if propertiesState}
  <PropertiesDialog paths={propertiesState.paths} onclose={() => (propertiesState = null)} />
{/if}

{#if extractionState}
  <ExtractionDialog
    archives={extractionState.archives}
    onclose={() => (extractionState = null)}
    onExtract={executeExtraction}
  />
{/if}

{#if chdState}
  <ChdDialog
    analysis={chdState.analysis}
    initialMode={chdState.mode}
    onclose={() => (chdState = null)}
    onConvert={executeConvertChd}
    onRestore={executeRestoreChd}
  />
{/if}

<style>
  .shell {
    display: flex;
    height: 100vh;
    overflow: hidden;
    background: var(--bg);
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
