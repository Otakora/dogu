<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { untrack } from "svelte";
  import type {
    ExtractionOptionsPayload, ExtractionPreviewRow,
    PreflightCheckResult, RemoteTransferOnError,
  } from "../../types/index.js";
  import Modal from "../ui/Modal.svelte";
  import Button from "../ui/Button.svelte";
  import ExtractionPreviewModal from "./ExtractionPreviewModal.svelte";
  import DestinationField from "./DestinationField.svelte";
  import { t } from "../../i18n/index.js";
  import { app } from "../../stores/app.svelte.js";

  type Props = {
    archives: string[];
    onclose: () => void;
    onExtract: (archives: string[], opts: ExtractionOptionsPayload) => void;
  };

  let { archives, onclose, onExtract }: Props = $props();

  const ENTRIES_PER_ARCHIVE_PREVIEW = 5;

  const hasRemoteArchives = $derived(archives.some(a => a.startsWith("remote://")));

  let opts = $state<ExtractionOptionsPayload>({
    individualFolders: true,
    splitEntries: false,
    destinationMode: untrack(() => hasRemoteArchives) ? "same" : "custom",
    destinationPath: null,
    deleteArchives: false,
    overwrite: untrack(() => app.settings.defaultOverwriteOnConflict),
    remoteDestination: null,
    remoteTransfer: null,
  });

  // Tracks the human-readable label for the current destination
  let destLabel = $state<string | null>(null);

  const isRemoteDest = $derived(opts.remoteDestination != null);
  const onError = $derived<RemoteTransferOnError>(
    (opts.remoteTransfer?.onError as RemoteTransferOnError | undefined) ?? "abort"
  );

  // Preflight
  let preflightResult = $state<PreflightCheckResult | null>(null);
  let preflightLoading = $state(false);

  async function runPreflight() {
    if (!isRemoteDest) { preflightResult = null; return; }
    preflightLoading = true;
    try {
      preflightResult = await invoke<PreflightCheckResult>("preflight_remote_transfer", {
        sources: archives,
        opKind: "extract",
      });
    } catch {
      preflightResult = null;
    } finally {
      preflightLoading = false;
    }
  }

  function setOnError(v: RemoteTransferOnError) {
    opts = { ...opts, remoteTransfer: { onError: v } };
  }

  function handleDestChange(path: string, label: string) {
    if (path.startsWith("remote://")) {
      opts = { ...opts, remoteDestination: path, destinationPath: null, destinationMode: "same" };
      runPreflight();
    } else {
      opts = { ...opts, destinationPath: path, remoteDestination: null, destinationMode: "custom" };
      preflightResult = null;
    }
    destLabel = label;
  }

  let preview = $state<ExtractionPreviewRow[]>([]);
  let previewLoading = $state(false);
  let previewError = $state<string | null>(null);
  let showFullPreview = $state(false);

  function archiveStem(path: string): string {
    return path.split(/[\\/]/).pop()?.replace(/\.[^.]+$/, "") ?? path;
  }

  const isSingleArchive = $derived(archives.length === 1);

  const individualFoldersLabel = $derived(
    isSingleArchive
      ? t("extractionDialog.individualFoldersSingle", { name: archiveStem(archives[0]) })
      : t("extractionDialog.individualFoldersMultiple")
  );

  const splitEntriesLabel = $derived(
    isSingleArchive
      ? t("extractionDialog.splitEntriesSingle")
      : t("extractionDialog.splitEntriesMultiple")
  );

  const splitEntriesHint = $derived(t("extractionDialog.splitEntriesHint"));

  $effect(() => {
    loadPreview();
  });

  async function loadPreview() {
    previewLoading = true;
    previewError = null;
    try {
      preview = await invoke<ExtractionPreviewRow[]>("build_extraction_preview", {
        archives,
        options: opts,
      });
    } catch (e) {
      preview = [];
      previewError = String(e);
    } finally {
      previewLoading = false;
    }
  }

  function submit() {
    onExtract(archives, opts);
    onclose();
  }
</script>

<Modal title={t("extractionDialog.title")} width="560px" {onclose}>
  {#snippet children()}
    <div class="ex-body">
      <div class="ex-section">
        <span class="ex-label">{t("extractionDialog.destination")}</span>
        <div class="radio-group">
          <label class="radio-label">
            <input
              type="radio"
              checked={opts.destinationMode === "same" && !isRemoteDest}
              onchange={() => { opts = { ...opts, destinationMode: "same", remoteDestination: null }; preflightResult = null; }}
            />
            {t("extractionDialog.sameFolderAsArchive")}
          </label>
          <label class="radio-label">
            <input
              type="radio"
              checked={opts.destinationMode === "custom" || isRemoteDest}
              onchange={() => { opts = { ...opts, destinationMode: "custom" }; }}
            />
            {t("extractionDialog.customFolder")}
          </label>
        </div>
        {#if opts.destinationMode === "custom" || isRemoteDest}
          <DestinationField
            path={opts.remoteDestination ?? opts.destinationPath}
            label={destLabel}
            onchange={handleDestChange}
          />

          {#if isRemoteDest}
            <!-- Transfer policy selector -->
            <div class="policy-row">
              <span class="policy-label">{t("transfer.policy.label")}</span>
              <div class="policy-options">
                {#each [["abort", "transfer.policy.abort"], ["skip", "transfer.policy.skip"], ["pause", "transfer.policy.pause"]] as [v, key]}
                  <label class="radio-label radio-label--small">
                    <input type="radio" name="ex-policy" value={v} checked={onError === v} onchange={() => setOnError(v as RemoteTransferOnError)} />
                    {t(key as Parameters<typeof t>[0])}
                  </label>
                {/each}
              </div>
            </div>

            <!-- Preflight warnings -->
            {#if preflightLoading}
              <div class="preflight-row preflight-row--loading">{t("destPicker.loading")}</div>
            {:else if preflightResult && preflightResult.warnings.length > 0}
              {#each preflightResult.warnings as w}
                <div class="preflight-row" class:preflight-row--critical={w.kind === "criticalSpace"}>
                  <strong>{t(("preflight." + w.kind) as Parameters<typeof t>[0])}</strong>
                  <span>{w.detail}</span>
                </div>
              {/each}
            {/if}
          {/if}
        {/if}
      </div>

      <div class="ex-section">
        <span class="ex-label">{t("extractionDialog.options")}</span>
        <label class="checkbox-label">
          <input type="checkbox" bind:checked={opts.individualFolders} />
          {individualFoldersLabel}
        </label>
        <label class="checkbox-label">
          <input type="checkbox" bind:checked={opts.splitEntries} />
          {splitEntriesLabel}
        </label>
        {#if opts.splitEntries}
          <span class="ex-hint">{splitEntriesHint}</span>
        {/if}
        <label class="checkbox-label">
          <input type="checkbox" bind:checked={opts.overwrite} />
          {t("extractionDialog.overwrite")}
        </label>
        {#if opts.overwrite}
          <span class="ex-hint">{t("extractionDialog.overwriteHint")}</span>
        {/if}
        <label class="checkbox-label">
          <input type="checkbox" bind:checked={opts.deleteArchives} />
          {t("extractionDialog.deleteArchives")}
        </label>
        {#if opts.deleteArchives}
          <span class="ex-hint">{t("extractionDialog.deleteArchivesHint")}</span>
        {/if}
      </div>

      <!-- Preview -->
      <div class="ex-section">
        <div class="ex-preview-header">
          <span class="ex-label">
            {t("extractionDialog.preview")} {#if preview.length > 0}{preview.length === 1 ? t("extractionDialog.previewCountSingle", { count: preview.length }) : t("extractionDialog.previewCountPlural", { count: preview.length })}{/if}
          </span>
          {#if preview.length > 0}
            <button class="ex-preview-expand" onclick={() => (showFullPreview = true)}>
              {t("extractionDialog.viewFullPreview")}
            </button>
          {/if}
        </div>
        {#if previewLoading}
          <span class="ex-preview-status">{t("extractionDialog.loadingPreview")}</span>
        {:else if previewError}
          <span class="ex-preview-status ex-preview-status--error">{previewError}</span>
        {:else if preview.length > 0}
          <div class="ex-preview scrollbar-thin">
            {#each preview as row}
              <div class="ex-preview-group">
                <div class="ex-preview-archive">
                  <span class="ex-preview-src">{row.archivePath.split(/[\\/]/).pop()}</span>
                  <span class="ex-arrow">→</span>
                  <span class="ex-preview-dst" title={row.destinationRoot}>{row.destinationRoot}</span>
                </div>
                {#each row.entries.slice(0, ENTRIES_PER_ARCHIVE_PREVIEW) as entry}
                  <div class="ex-preview-entry-row">
                    <span class="ex-preview-entry-name" title={entry.name}>{entry.name}</span>
                    <span class="ex-arrow">→</span>
                    <span class="ex-preview-dst" title={entry.destinationPath}>{entry.destinationPath}</span>
                  </div>
                {/each}
                {#if row.entries.length > ENTRIES_PER_ARCHIVE_PREVIEW}
                  <button class="ex-preview-more" onclick={() => (showFullPreview = true)}>
                    {t("extractionDialog.moreEntries", { count: row.entries.length - ENTRIES_PER_ARCHIVE_PREVIEW })}
                  </button>
                {/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>
    </div>
  {/snippet}

  {#snippet footer()}
    <Button variant="ghost" onclick={onclose}>{t("common.cancel")}</Button>
    <Button
      variant="primary"
      onclick={submit}
      disabled={
        (opts.destinationMode === "custom" && !opts.destinationPath && !isRemoteDest)
        || (preflightResult != null && !preflightResult.ok)
      }
    >
      {archives.length === 1 ? t("extractionDialog.extractCountSingle", { count: archives.length }) : t("extractionDialog.extractCountPlural", { count: archives.length })}
    </Button>
  {/snippet}
</Modal>

{#if showFullPreview}
  <ExtractionPreviewModal rows={preview} onclose={() => (showFullPreview = false)} />
{/if}

<style>
  .ex-body {
    display: flex;
    flex-direction: column;
    gap: 14px;
  }

  .ex-section {
    display: flex;
    flex-direction: column;
    gap: 7px;
  }

  .ex-label {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-subtle);
  }

  .ex-hint {
    font-size: 11px;
    color: var(--text-subtle);
    padding-left: 22px;
    margin-top: -3px;
  }

  .radio-group {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .radio-label, .checkbox-label {
    display: flex;
    flex-direction: row;
    align-items: center;
    gap: 7px;
    font-size: 13px;
    color: var(--text);
    cursor: pointer;
    input { accent-color: var(--accent); }
  }

  .ex-preview-header {
    display: flex;
    align-items: center;
    justify-content: space-between;
    gap: 8px;
  }

  .ex-preview-expand {
    background: none;
    border: none;
    color: var(--accent);
    font-size: 11px;
    cursor: pointer;
    padding: 0;

    &:hover { text-decoration: underline; }
  }

  .ex-preview-status {
    font-size: 12px;
    color: var(--text-subtle);
  }

  .ex-preview-status--error {
    color: var(--danger, #e5484d);
  }

  .ex-preview {
    max-height: 220px;
    overflow-y: auto;
    background: var(--surface-alt);
    border: 1px solid var(--line);
    border-radius: 5px;
    padding: 6px 8px;
    display: flex;
    flex-direction: column;
    gap: 10px;
  }

  .ex-preview-group {
    display: flex;
    flex-direction: column;
    gap: 2px;
  }

  .ex-preview-archive {
    display: flex;
    gap: 6px;
    align-items: center;
    font-size: 12px;
    font-weight: 600;
    color: var(--text);
    padding-bottom: 2px;
  }

  .ex-preview-entry-row {
    display: flex;
    gap: 6px;
    align-items: center;
    font-size: 11px;
    color: var(--text-muted);
    padding: 1px 0 1px 10px;
  }

  .ex-preview-entry-name {
    color: var(--text);
    flex-shrink: 0;
    max-width: 140px;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .ex-preview-more {
    align-self: flex-start;
    margin-left: 10px;
    background: none;
    border: none;
    color: var(--accent);
    font-size: 11px;
    cursor: pointer;
    padding: 1px 0;

    &:hover { text-decoration: underline; }
  }

  .ex-preview-src { color: var(--text); font-weight: 500; flex-shrink: 0; }
  .ex-arrow { color: var(--text-subtle); flex-shrink: 0; }
  .ex-preview-dst { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  .policy-row {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
  }

  .policy-label {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .policy-options {
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
  }

  .radio-label--small {
    font-size: 12px;
    gap: 5px;
  }

  .preflight-row {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 7px 10px;
    border-radius: 5px;
    font-size: 11.5px;
    background: color-mix(in srgb, #f59e0b 10%, var(--surface-alt));
    border: 1px solid color-mix(in srgb, #f59e0b 35%, var(--line));
    color: var(--text);

    strong { color: #b45309; }

    &.preflight-row--critical {
      background: color-mix(in srgb, #ef4444 10%, var(--surface-alt));
      border-color: color-mix(in srgb, #ef4444 35%, var(--line));
      strong { color: #dc2626; }
    }

    &.preflight-row--loading {
      background: var(--surface-alt);
      border-color: var(--line);
      color: var(--text-muted);
    }
  }
</style>
