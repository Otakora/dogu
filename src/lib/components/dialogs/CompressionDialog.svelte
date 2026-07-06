<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type {
    CompressionOptionsPayload,
    CompressionCapabilitiesDto,
    CompressionFormat,
    PreflightCheckResult,
    RemoteTransferOnError,
  } from "../../types/index.js";
  import Modal from "../ui/Modal.svelte";
  import Button from "../ui/Button.svelte";
  import DestinationField from "./DestinationField.svelte";
  import { untrack } from "svelte";
  import { t } from "../../i18n/index.js";
  import { app } from "../../stores/app.svelte.js";

  type Props = {
    sources: string[];
    capabilities: CompressionCapabilitiesDto;
    onclose: () => void;
    onCompress: (sources: string[], opts: CompressionOptionsPayload) => void;
  };

  let { sources, capabilities, onclose, onCompress }: Props = $props();

  // ── Auto-generate archive name from selection ─────────────
  function defaultName(): string {
    if (sources.length === 1) {
      const leaf = sources[0].replace(/\\/g, "/").split("/").filter(Boolean).pop() ?? "archive";
      // Strip only a real trailing extension (alphanumeric, ≤8 chars). This avoids
      // mangling version-dotted names/folders like "Game (v1.1)".
      return leaf.replace(/\.[A-Za-z0-9]{1,8}$/, "");
    }
    const parent = sources[0].replace(/\\/g, "/").split("/").filter(Boolean);
    return parent.length >= 2 ? parent[parent.length - 2] : "archive";
  }

  // ── State ─────────────────────────────────────────────────
  let archiveName = $state(defaultName());
  let format = $state<CompressionFormat>(untrack(() => capabilities.canCompress7z ? "7z" : "zip"));
  let destinationMode = $state<"same" | "custom">("same");
  let destinationPath = $state<string | null>(null);
  let destLabel = $state<string | null>(null);
  let remoteDest = $state<string | null>(null);
  let onError = $state<RemoteTransferOnError>("abort");
  let compressionLevel = $state(5);
  let deleteOriginals = $state(false);
  let overwrite = $state(untrack(() => app.settings.defaultOverwriteOnConflict));

  const isRemoteDest = $derived(remoteDest != null);
  const hasCustomDest = $derived(destinationMode === "custom" || isRemoteDest);

  // Preflight (only relevant when using a remote destination)
  let preflightResult = $state<PreflightCheckResult | null>(null);
  let preflightLoading = $state(false);

  async function runPreflight() {
    if (!isRemoteDest) { preflightResult = null; return; }
    preflightLoading = true;
    try {
      preflightResult = await invoke<PreflightCheckResult>("preflight_remote_transfer", {
        sources,
        opKind: "compress",
      });
    } catch {
      preflightResult = null;
    } finally {
      preflightLoading = false;
    }
  }

  function handleDestChange(path: string, label: string) {
    if (path.startsWith("remote://")) {
      remoteDest = path;
      destinationPath = null;
      destinationMode = "same";
      runPreflight();
    } else {
      remoteDest = null;
      destinationPath = path;
      destinationMode = "custom";
      preflightResult = null;
    }
    destLabel = label;
  }

  // ── Available formats ─────────────────────────────────────
  type FormatOption = { id: CompressionFormat; label: string; available: boolean };
  const formats = $derived<FormatOption[]>([
    { id: "zip", label: "ZIP", available: capabilities.canCompressZip },
    { id: "7z",  label: "7Z",  available: capabilities.canCompress7z  },
    { id: "rar", label: "RAR", available: capabilities.canCompressRar },
  ]);

  // ── Levels ────────────────────────────────────────────────
  const levels = [
    { value: 0, key: "compressDialog.levelStore" },
    { value: 3, key: "compressDialog.levelFast" },
    { value: 5, key: "compressDialog.levelNormal" },
    { value: 9, key: "compressDialog.levelMaximum" },
  ] as const;

  // ── Submit ────────────────────────────────────────────────
  const canSubmit = $derived(
    archiveName.trim().length > 0
    && (!hasCustomDest || destinationPath != null || isRemoteDest)
    && !(preflightResult != null && !preflightResult.ok)
  );

  function submit() {
    const opts: CompressionOptionsPayload = {
      format,
      archiveName: archiveName.trim(),
      destinationMode,
      destinationPath,
      compressionLevel,
      deleteOriginals,
      overwrite,
      remoteDestination: remoteDest,
      remoteTransfer: isRemoteDest ? { onError } : null,
    };
    onCompress(sources, opts);
    onclose();
  }

  const compressLabel = $derived(
    sources.length === 1
      ? t("compressDialog.compressCountSingle", { count: sources.length })
      : t("compressDialog.compressCountPlural", { count: sources.length })
  );
</script>

<Modal title={t("compressDialog.title")} width="520px" {onclose}>
  {#snippet children()}
    <div class="cd-body">

      <!-- ── Archive name ───────────────────────────────────── -->
      <div class="cd-section">
        <span class="cd-label">{t("compressDialog.archiveName")}</span>
        <div class="cd-name-row">
          <input
            class="cd-name-input"
            type="text"
            bind:value={archiveName}
            spellcheck="false"
            placeholder="archive"
          />
          <span class="cd-name-ext">.{format}</span>
        </div>
      </div>

      <!-- ── Format ────────────────────────────────────────── -->
      <div class="cd-section">
        <span class="cd-label">{t("compressDialog.format")}</span>
        <div class="cd-formats">
          {#each formats as f}
            <label class="format-btn" class:format-btn--active={format === f.id} class:format-btn--disabled={!f.available}>
              <input
                type="radio"
                name="compress-format"
                value={f.id}
                bind:group={format}
                disabled={!f.available}
              />
              {f.label}
            </label>
          {/each}
        </div>
        {#if format === "rar" && !capabilities.canCompressRar}
          <span class="cd-hint cd-hint--warn">{t("compressDialog.rarNotAvailable")}</span>
        {/if}
      </div>

      <!-- ── Destination ───────────────────────────────────── -->
      <div class="cd-section">
        <span class="cd-label">{t("compressDialog.destination")}</span>
        <div class="radio-group">
          <label class="radio-label">
            <input
              type="radio"
              checked={destinationMode === "same" && !isRemoteDest}
              onchange={() => { destinationMode = "same"; remoteDest = null; destinationPath = null; preflightResult = null; }}
            />
            {t("compressDialog.sameFolderAsSource")}
          </label>
          <label class="radio-label">
            <input
              type="radio"
              checked={hasCustomDest}
              onchange={() => { destinationMode = "custom"; }}
            />
            {t("compressDialog.customFolder")}
          </label>
        </div>
        {#if hasCustomDest}
          <DestinationField
            path={remoteDest ?? destinationPath}
            label={destLabel}
            onchange={handleDestChange}
          />
          {#if isRemoteDest}
            <div class="policy-row">
              <span class="policy-label">{t("transfer.policy.label")}</span>
              <div class="policy-options">
                {#each [["abort", "transfer.policy.abort"], ["skip", "transfer.policy.skip"], ["pause", "transfer.policy.pause"]] as [v, key]}
                  <label class="radio-label radio-label--small">
                    <input type="radio" name="cd-policy" value={v} checked={onError === v} onchange={() => (onError = v as RemoteTransferOnError)} />
                    {t(key as Parameters<typeof t>[0])}
                  </label>
                {/each}
              </div>
            </div>
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

      <!-- ── Compression level ─────────────────────────────── -->
      <div class="cd-section">
        <span class="cd-label">{t("compressDialog.compressionLevel")}</span>
        <div class="cd-levels">
          {#each levels as lv}
            <label class="radio-label">
              <input type="radio" name="cd-level" value={lv.value} bind:group={compressionLevel} />
              {t(lv.key)}
            </label>
          {/each}
        </div>
      </div>

      <!-- ── Options ───────────────────────────────────────── -->
      <div class="cd-section">
        <label class="checkbox-label">
          <input type="checkbox" bind:checked={overwrite} />
          {t("compressDialog.overwrite")}
        </label>
        <label class="checkbox-label">
          <input type="checkbox" bind:checked={deleteOriginals} />
          {t("compressDialog.deleteOriginals")}
        </label>
        {#if deleteOriginals}
          <span class="cd-hint">{t("compressDialog.deleteOriginalsHint")}</span>
        {/if}
      </div>

    </div>
  {/snippet}

  {#snippet footer()}
    <Button variant="ghost" onclick={onclose}>{t("common.cancel")}</Button>
    <Button variant="primary" onclick={submit} disabled={!canSubmit}>
      {compressLabel}
    </Button>
  {/snippet}
</Modal>

<style>
  .cd-body {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .cd-section {
    display: flex;
    flex-direction: column;
    gap: 7px;
  }

  .cd-label {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-subtle);
  }

  .cd-hint {
    font-size: 11px;
    color: var(--text-subtle);
    padding-left: 22px;
    margin-top: -3px;
  }

  .cd-hint--warn {
    color: var(--warning, #f59e0b);
    padding-left: 0;
  }

  /* ── Archive name ── */
  .cd-name-row {
    display: flex;
    align-items: center;
    gap: 0;
    border: 1px solid var(--line);
    border-radius: 5px;
    overflow: hidden;
    background: var(--input-bg, var(--surface));

    &:focus-within { outline: 2px solid var(--accent); outline-offset: -1px; }
  }

  .cd-name-input {
    flex: 1;
    border: none;
    background: transparent;
    padding: 6px 10px;
    font-size: 13px;
    color: var(--text);
    outline: none;
    min-width: 0;
  }

  .cd-name-ext {
    padding: 6px 10px;
    font-size: 12px;
    color: var(--text-subtle);
    background: var(--surface-alt);
    border-left: 1px solid var(--line);
    white-space: nowrap;
    font-family: monospace;
  }

  /* ── Format selector ── */
  .cd-formats {
    display: flex;
    gap: 6px;
    flex-wrap: wrap;
  }

  .format-btn {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 5px 14px;
    border: 1px solid var(--line);
    border-radius: 5px;
    font-size: 13px;
    font-weight: 600;
    cursor: pointer;
    color: var(--text);
    background: var(--surface);
    transition: border-color 0.1s, background 0.1s;

    input[type="radio"] { display: none; }

    &:hover:not(.format-btn--disabled) {
      border-color: var(--accent);
      background: color-mix(in srgb, var(--accent) 8%, transparent);
    }

    &.format-btn--active {
      border-color: var(--accent);
      background: color-mix(in srgb, var(--accent) 12%, transparent);
      color: var(--accent);
    }

    &.format-btn--disabled {
      opacity: 0.4;
      cursor: not-allowed;
    }
  }

  /* ── Levels ── */
  .cd-levels {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  /* ── Shared radio/checkbox styles ── */
  .radio-group {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .radio-label, .checkbox-label {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 13px;
    color: var(--text);
    cursor: pointer;
    input { accent-color: var(--accent); }
  }

  .radio-label--small {
    font-size: 12px;
  }

  /* ── Policy row ── */
  .policy-row {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    padding-top: 4px;
  }

  .policy-label {
    font-size: 11px;
    font-weight: 600;
    color: var(--text-subtle);
    white-space: nowrap;
  }

  .policy-options {
    display: flex;
    gap: 12px;
    flex-wrap: wrap;
  }

  /* ── Preflight warnings ── */
  .preflight-row {
    display: flex;
    flex-direction: column;
    gap: 2px;
    padding: 6px 10px;
    border-radius: 5px;
    font-size: 11px;
    background: color-mix(in srgb, var(--warning, #f59e0b) 12%, transparent);
    border: 1px solid color-mix(in srgb, var(--warning, #f59e0b) 40%, transparent);
    color: var(--text);
  }

  .preflight-row--loading {
    background: var(--surface-alt);
    border-color: var(--line);
    color: var(--text-subtle);
  }

  .preflight-row--critical {
    background: color-mix(in srgb, var(--danger, #e5484d) 12%, transparent);
    border-color: color-mix(in srgb, var(--danger, #e5484d) 40%, transparent);
  }
</style>
