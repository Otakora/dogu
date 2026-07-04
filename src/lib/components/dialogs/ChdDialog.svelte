<script lang="ts">
  import type {
    SelectionAnalysisDto,
    ChdSourceDto,
    ChdConversionOptionsPayload,
    ChdRestoreOptionsPayload,
    RemoteTransferOnError,
  } from "../../types/index.js";
  import { untrack } from "svelte";
  import Modal from "../ui/Modal.svelte";
  import Button from "../ui/Button.svelte";
  import DestinationField from "./DestinationField.svelte";
  import { t } from "../../i18n/index.js";
  import { app } from "../../stores/app.svelte.js";

  type Mode = "convert" | "restore";
  type NamingMode = "container" | "source" | "custom";
  type RestoreNamingMode = "chdStem" | "custom";

  type Props = {
    analysis: SelectionAnalysisDto;
    initialMode?: Mode;
    onclose: () => void;
    onConvert: (paths: string[], opts: ChdConversionOptionsPayload) => void;
    onRestore: (paths: string[], opts: ChdRestoreOptionsPayload) => void;
  };

  let { analysis, initialMode = "convert", onclose, onConvert, onRestore }: Props = $props();

  let mode = $state<Mode>(untrack(() => initialMode));

  // ── Convert: naming ───────────────────────────────────────
  let namingMode = $state<NamingMode>("container");
  let customInput = $state("");

  const isSingle = $derived(analysis.chdSources.length === 1);
  const isSingleChd = $derived(analysis.restorableChds.length === 1);

  function autoStem(src: ChdSourceDto): string {
    return namingMode === "container"
      ? leafName(src.containerDir)
      : stemOf(leafName(src.sourcePath));
  }

  function effectiveStem(src: ChdSourceDto): string {
    if (namingMode === "custom" && isSingle) {
      const trimmed = customInput.trim().replace(/\.chd$/i, "");
      return trimmed || autoStem(src);
    }
    return autoStem(src);
  }

  $effect(() => {
    const m = namingMode;
    if (m !== "custom" && isSingle) {
      const computed = analysis.chdSources[0] ? autoStem(analysis.chdSources[0]) : "";
      untrack(() => { customInput = computed; });
    }
  });

  // ── Convert: destination ──────────────────────────────────
  let convDestMode      = $state<"same" | "parent" | "custom">("same");
  let convDestPath      = $state<string | null>(null);
  let convDestLabel     = $state<string | null>(null);
  let convRemoteDest    = $state<string | null>(null);
  let convOnError       = $state<RemoteTransferOnError>("abort");
  const isConvRemoteDest  = $derived(convRemoteDest != null);
  const hasConvCustomDest = $derived(convDestMode === "custom" || isConvRemoteDest);

  function handleConvDestChange(path: string, label: string) {
    if (path.startsWith("remote://")) {
      convRemoteDest = path;
      convDestPath   = null;
      convDestMode   = "same";
    } else {
      convRemoteDest = null;
      convDestPath   = path;
      convDestMode   = "custom";
    }
    convDestLabel = label;
  }

  // ── Convert: other options ────────────────────────────────
  let deleteOriginals  = $state(false);
  let deleteSubfolders = $state(false);
  let overwriteChd     = $state(untrack(() => app.settings.defaultOverwriteOnConflict));

  // ── Restore: destination ──────────────────────────────────
  let restDestMode       = $state<"same" | "custom">("same");
  let restDestPath       = $state<string | null>(null);
  let restDestLabel      = $state<string | null>(null);
  let restRemoteDest     = $state<string | null>(null);
  let restOnError        = $state<RemoteTransferOnError>("abort");
  const isRestRemoteDest = $derived(restRemoteDest != null);

  // ── Restore: output file naming ───────────────────────────
  let restOutputNaming  = $state<RestoreNamingMode>("chdStem");
  let restOutputInput   = $state("");

  // ── Restore: folder naming ────────────────────────────────
  let restIndividual    = $state(false);
  let restFolderNaming  = $state<RestoreNamingMode>("chdStem");
  let restFolderInput   = $state("");

  // ── Restore: misc ─────────────────────────────────────────
  let restSplitBin      = $state(false);
  let restDeleteChd     = $state(false);
  let restOverwrite     = $state(untrack(() => app.settings.defaultOverwriteOnConflict));

  // Sync output-name input when switching away from "custom"
  $effect(() => {
    const m = restOutputNaming;
    if (m !== "custom" && isSingleChd && analysis.restorableChds[0]) {
      const stem = chdStem(analysis.restorableChds[0]);
      untrack(() => { restOutputInput = stem; });
    }
  });

  // Sync folder-name input when switching away from "custom"
  $effect(() => {
    const m = restFolderNaming;
    if (m !== "custom" && isSingleChd && analysis.restorableChds[0]) {
      const stem = chdStem(analysis.restorableChds[0]);
      untrack(() => { restFolderInput = stem; });
    }
  });

  function effectiveOutputStem(chdPath: string): string {
    if (restOutputNaming === "custom" && isSingleChd) {
      const trimmed = restOutputInput.trim();
      return trimmed || chdStem(chdPath);
    }
    return chdStem(chdPath);
  }

  function effectiveFolderName(chdPath: string): string {
    if (restFolderNaming === "custom" && isSingleChd) {
      const trimmed = restFolderInput.trim();
      return trimmed || chdStem(chdPath);
    }
    return chdStem(chdPath);
  }

  function handleRestDestChange(path: string, label: string) {
    if (path.startsWith("remote://")) {
      restRemoteDest = path;
      restDestPath   = null;
      restDestMode   = "same";
    } else {
      restRemoteDest = null;
      restDestPath   = path;
      restDestMode   = "custom";
    }
    restDestLabel = label;
  }

  // ── Submit ─────────────────────────────────────────────────
  function submit() {
    if (mode === "convert") {
      const paths = validSources.map(s => s.sourcePath);
      const opts: ChdConversionOptionsPayload = {
        nameAsContainer: namingMode === "container",
        depositToParent: convDestMode === "parent",
        deleteOriginals,
        deleteOriginalSubfolders: deleteSubfolders,
        overwrite: overwriteChd,
        customName: (namingMode === "custom" && isSingle)
          ? (customInput.trim().replace(/\.chd$/i, "") || null)
          : null,
        destinationMode: convDestMode,
        destinationPath: convDestPath,
        remoteDestination: convRemoteDest,
        remoteTransfer: isConvRemoteDest ? { onError: convOnError } : null,
      };
      onConvert(paths, opts);
    } else {
      const opts: ChdRestoreOptionsPayload = {
        individualFolders: restIndividual,
        destinationMode: restDestMode,
        destinationPath: restDestPath,
        outputNamingMode: restOutputNaming,
        customOutputName: (restOutputNaming === "custom" && isSingleChd)
          ? (restOutputInput.trim() || null)
          : null,
        folderNamingMode: restFolderNaming,
        customFolderName: (restIndividual && restFolderNaming === "custom" && isSingleChd)
          ? (restFolderInput.trim() || null)
          : null,
        deleteChd: restDeleteChd,
        overwrite: restOverwrite,
        splitBin: restSplitBin,
        remoteDestination: restRemoteDest,
        remoteTransfer: isRestRemoteDest ? { onError: restOnError } : null,
      };
      onRestore(analysis.restorableChds, opts);
    }
    onclose();
  }

  // ── Helpers ────────────────────────────────────────────────
  function leafName(path: string): string {
    return path.replace(/\\/g, "/").split("/").filter(Boolean).pop() ?? path;
  }
  function stemOf(name: string): string {
    const dot = name.lastIndexOf(".");
    return dot > 0 ? name.slice(0, dot) : name;
  }
  function chdStem(path: string): string {
    return stemOf(leafName(path));
  }

  // ── Display ────────────────────────────────────────────────
  const validSources  = $derived(analysis.chdSources.filter(s => s.missingFiles.length === 0));
  const canConvert    = $derived(validSources.length > 0);
  const canRestore    = $derived(analysis.restorableChds.length > 0);

  const title = $derived(
    mode === "convert"
      ? (analysis.chdSources.length === 1
          ? t("chdDialog.convertTitle",       { count: analysis.chdSources.length })
          : t("chdDialog.convertTitlePlural", { count: analysis.chdSources.length }))
      : (analysis.restorableChds.length === 1
          ? t("chdDialog.restoreTitle",       { count: analysis.restorableChds.length })
          : t("chdDialog.restoreTitlePlural", { count: analysis.restorableChds.length }))
  );
</script>

<Modal {title} width="520px" {onclose}>
  {#snippet children()}
    <div class="chd-body">

      <!-- Mode tabs -->
      {#if canConvert && canRestore}
        <div class="chd-tabs" role="tablist">
          <button class="chd-tab" class:chd-tab--active={mode === "convert"}
            onclick={() => (mode = "convert")} role="tab" aria-selected={mode === "convert"}>
            {t("chdDialog.convertTab")}
          </button>
          <button class="chd-tab" class:chd-tab--active={mode === "restore"}
            onclick={() => (mode = "restore")} role="tab" aria-selected={mode === "restore"}>
            {t("chdDialog.restoreTab")}
          </button>
        </div>
      {/if}

      {#if mode === "convert"}

        <!-- ── Convert: naming ───────────────────────────── -->
        <fieldset class="naming-fieldset">
          <legend class="naming-legend">{t("chdDialog.naming")}</legend>

          <div class="naming-options">
            <label class="radio-label">
              <input type="radio" bind:group={namingMode} value="container" />
              {t("chdDialog.nameAsContainer")}
            </label>
            <label class="radio-label">
              <input type="radio" bind:group={namingMode} value="source" />
              {t("chdDialog.nameAsSource")}
            </label>
            {#if isSingle}
              <label class="radio-label">
                <input type="radio" bind:group={namingMode} value="custom" />
                {t("chdDialog.nameCustom")}
              </label>
            {/if}
          </div>

          <div class="name-input-row" class:name-input-row--disabled={namingMode !== "custom"}>
            <input
              class="name-input"
              type="text"
              value={namingMode === "custom" ? customInput : (isSingle && analysis.chdSources[0] ? effectiveStem(analysis.chdSources[0]) : "")}
              oninput={(e) => { if (namingMode === "custom") customInput = (e.target as HTMLInputElement).value; }}
              readonly={namingMode !== "custom"}
              placeholder={t("chdDialog.nameCustomPlaceholder")}
              spellcheck="false"
              tabindex={namingMode === "custom" ? 0 : -1}
            />
            <span class="name-ext">.chd</span>
          </div>
        </fieldset>

        <!-- ── Convert: destination ──────────────────────── -->
        <fieldset class="naming-fieldset">
          <legend class="naming-legend">{t("chdDialog.destination")}</legend>
          <div class="naming-options">
            <label class="radio-label">
              <input
                type="radio"
                checked={convDestMode === "same" && !isConvRemoteDest}
                onchange={() => { convDestMode = "same"; convRemoteDest = null; convDestPath = null; }}
              />
              {t("chdDialog.sameFolderAsSource")}
            </label>
            <label class="radio-label">
              <input
                type="radio"
                checked={convDestMode === "parent" && !isConvRemoteDest}
                onchange={() => { convDestMode = "parent"; convRemoteDest = null; convDestPath = null; }}
              />
              {t("chdDialog.placeInParent")}
            </label>
            <label class="radio-label">
              <input
                type="radio"
                checked={convDestMode === "custom" || isConvRemoteDest}
                onchange={() => { convDestMode = "custom"; }}
              />
              {t("chdDialog.customFolder")}
            </label>
          </div>
          {#if hasConvCustomDest}
            <DestinationField
              path={convRemoteDest ?? convDestPath}
              label={convDestLabel}
              onchange={handleConvDestChange}
            />
            {#if isConvRemoteDest}
              <div class="policy-row">
                <span class="policy-label">{t("transfer.policy.label")}</span>
                <div class="policy-options">
                  {#each [["abort", "transfer.policy.abort"], ["skip", "transfer.policy.skip"], ["pause", "transfer.policy.pause"]] as [v, key]}
                    <label class="radio-label radio-label--small">
                      <input type="radio" name="chd-conv-policy" value={v} checked={convOnError === v} onchange={() => (convOnError = v as RemoteTransferOnError)} />
                      {t(key as Parameters<typeof t>[0])}
                    </label>
                  {/each}
                </div>
              </div>
            {/if}
          {/if}
        </fieldset>

        <!-- ── Convert: other options ────────────────────── -->
        <div class="chd-section">
          <label class="checkbox-label">
            <input type="checkbox" bind:checked={deleteOriginals} />
            {t("chdDialog.deleteOriginals")}
          </label>
          <label class="checkbox-label">
            <input type="checkbox" bind:checked={deleteSubfolders} disabled={!deleteOriginals} />
            {t("chdDialog.alsoDeleteSubfolders")}
          </label>
          <label class="checkbox-label">
            <input type="checkbox" bind:checked={overwriteChd} />
            {t("chdDialog.overwriteChd")}
          </label>
        </div>

        <!-- ── Convert: source list with preview ─────────── -->
        <div class="chd-sources scrollbar-thin">
          {#each analysis.chdSources as src}
            {@const hasMissing = src.missingFiles.length > 0}
            {@const stem = effectiveStem(src)}
            <div class="chd-source-row" class:chd-source-row--invalid={hasMissing}>
              <div class="src-top">
                {#if hasMissing}
                  <svg class="src-warn-icon" width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
                    <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
                    <line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/>
                  </svg>
                {/if}
                <span class="chd-src-badge" class:chd-src-badge--warn={hasMissing}>{src.command}</span>
                <span class="chd-src-name">{leafName(src.sourcePath)}</span>
                <span class="chd-src-exts">{src.displayExtensions.join(", ")}</span>
              </div>
              {#if hasMissing}
                <div class="src-missing">
                  {t("chdDialog.missingFiles")}: {src.missingFiles.join(", ")}
                </div>
              {:else}
                <div class="src-preview">
                  <span class="preview-arrow">→</span>
                  <span class="preview-name">{stem}.chd</span>
                </div>
              {/if}
            </div>
          {/each}
        </div>

        <!-- ── Orphan .bin files (no matching .cue) ──────── -->
        {#if analysis.orphanBins.length > 0}
          <div class="orphan-box">
            <div class="orphan-header">
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
                <path d="M10.29 3.86L1.82 18a2 2 0 0 0 1.71 3h16.94a2 2 0 0 0 1.71-3L13.71 3.86a2 2 0 0 0-3.42 0z"/>
                <line x1="12" y1="9" x2="12" y2="13"/><line x1="12" y1="17" x2="12.01" y2="17"/>
              </svg>
              <strong>{t("chdDialog.orphanBinsTitle")}</strong>
            </div>
            {#each analysis.orphanBins as binPath}
              <div class="orphan-row">{leafName(binPath)}</div>
            {/each}
          </div>
        {/if}

      {:else}

        <!-- ── Restore: destination ──────────────────────── -->
        <fieldset class="naming-fieldset">
          <legend class="naming-legend">{t("chdDialog.destination")}</legend>
          <div class="naming-options">
            <label class="radio-label">
              <input
                type="radio"
                checked={restDestMode === "same" && !isRestRemoteDest}
                onchange={() => { restDestMode = "same"; restRemoteDest = null; }}
              />
              {t("chdDialog.sameFolderAsChd")}
            </label>
            <label class="radio-label">
              <input
                type="radio"
                checked={restDestMode === "custom" || isRestRemoteDest}
                onchange={() => { restDestMode = "custom"; }}
              />
              {t("chdDialog.customFolder")}
            </label>
          </div>
          {#if restDestMode === "custom" || isRestRemoteDest}
            <DestinationField
              path={restRemoteDest ?? restDestPath}
              label={restDestLabel}
              onchange={handleRestDestChange}
            />
            {#if isRestRemoteDest}
              <div class="policy-row">
                <span class="policy-label">{t("transfer.policy.label")}</span>
                <div class="policy-options">
                  {#each [["abort", "transfer.policy.abort"], ["skip", "transfer.policy.skip"], ["pause", "transfer.policy.pause"]] as [v, key]}
                    <label class="radio-label radio-label--small">
                      <input type="radio" name="chd-rest-policy" value={v} checked={restOnError === v} onchange={() => (restOnError = v as RemoteTransferOnError)} />
                      {t(key as Parameters<typeof t>[0])}
                    </label>
                  {/each}
                </div>
              </div>
            {/if}
          {/if}
        </fieldset>

        <!-- ── Restore: individual folder ───────────────── -->
        <div class="chd-section">
          <label class="checkbox-label">
            <input type="checkbox" bind:checked={restIndividual} />
            {t("chdDialog.restoreEachOwnFolder")}
          </label>

          {#if restIndividual}
            <fieldset class="naming-fieldset sub-fieldset">
              <legend class="naming-legend">{t("chdDialog.restoreFolderNaming")}</legend>
              <div class="naming-options">
                <label class="radio-label">
                  <input type="radio" bind:group={restFolderNaming} value="chdStem" />
                  {t("chdDialog.restoreFolderAsChdStem")}
                </label>
                {#if isSingleChd}
                  <label class="radio-label">
                    <input type="radio" bind:group={restFolderNaming} value="custom" />
                    {t("chdDialog.restoreFolderCustom")}
                  </label>
                {/if}
              </div>
              <div class="name-input-row" class:name-input-row--disabled={restFolderNaming !== "custom"}>
                <input
                  class="name-input"
                  type="text"
                  value={restFolderNaming === "custom"
                    ? restFolderInput
                    : (isSingleChd && analysis.restorableChds[0] ? effectiveFolderName(analysis.restorableChds[0]) : "")}
                  oninput={(e) => { if (restFolderNaming === "custom") restFolderInput = (e.target as HTMLInputElement).value; }}
                  readonly={restFolderNaming !== "custom"}
                  placeholder={t("chdDialog.restoreFolderCustomPlaceholder")}
                  spellcheck="false"
                  tabindex={restFolderNaming === "custom" ? 0 : -1}
                />
              </div>
            </fieldset>
          {/if}
        </div>

        <!-- ── Restore: output file naming ──────────────── -->
        <fieldset class="naming-fieldset">
          <legend class="naming-legend">{t("chdDialog.restoreOutputNaming")}</legend>
          <div class="naming-options">
            <label class="radio-label">
              <input type="radio" bind:group={restOutputNaming} value="chdStem" />
              {t("chdDialog.restoreNameAsChdStem")}
            </label>
            {#if isSingleChd}
              <label class="radio-label">
                <input type="radio" bind:group={restOutputNaming} value="custom" />
                {t("chdDialog.restoreNameCustom")}
              </label>
            {/if}
          </div>
          <div class="name-input-row" class:name-input-row--disabled={restOutputNaming !== "custom"}>
            <input
              class="name-input"
              type="text"
              value={restOutputNaming === "custom"
                ? restOutputInput
                : (isSingleChd && analysis.restorableChds[0] ? effectiveOutputStem(analysis.restorableChds[0]) : "")}
              oninput={(e) => { if (restOutputNaming === "custom") restOutputInput = (e.target as HTMLInputElement).value; }}
              readonly={restOutputNaming !== "custom"}
              placeholder={t("chdDialog.restoreNameCustomPlaceholder")}
              spellcheck="false"
              tabindex={restOutputNaming === "custom" ? 0 : -1}
            />
            <span class="name-ext">.cue / .bin / .iso</span>
          </div>
        </fieldset>

        <!-- ── Restore: misc options ─────────────────────── -->
        <div class="chd-section">
          <label class="checkbox-label">
            <input type="checkbox" bind:checked={restSplitBin} />
            {t("chdDialog.splitOutput")}
          </label>
          <label class="checkbox-label">
            <input type="checkbox" bind:checked={restDeleteChd} />
            {t("chdDialog.deleteChdAfterRestore")}
          </label>
          <label class="checkbox-label">
            <input type="checkbox" bind:checked={restOverwrite} />
            {t("chdDialog.overwriteFiles")}
          </label>
        </div>

        <!-- ── Restore: CHD list with preview ────────────── -->
        <div class="chd-sources scrollbar-thin">
          {#each analysis.restorableChds as chdPath}
            {@const outStem = effectiveOutputStem(chdPath)}
            {@const folderName = restIndividual ? effectiveFolderName(chdPath) : null}
            <div class="chd-source-row">
              <div class="src-top">
                <span class="chd-src-badge">extractcd/dvd</span>
                <span class="chd-src-name">{leafName(chdPath)}</span>
              </div>
              <div class="src-preview">
                <span class="preview-arrow">→</span>
                {#if folderName}
                  <span class="preview-folder">{folderName}/</span>
                  <span class="preview-sep">·</span>
                {/if}
                <span class="preview-name">{outStem}.cue + .bin</span>
              </div>
            </div>
          {/each}
        </div>

      {/if}
    </div>
  {/snippet}

  {#snippet footer()}
    <Button variant="ghost" onclick={onclose}>{t("common.cancel")}</Button>
    <Button
      variant="primary"
      onclick={submit}
      disabled={mode === "restore" && restDestMode === "custom" && !restDestPath}
    >
      {mode === "convert" ? t("chdDialog.convert") : t("chdDialog.restore")}
    </Button>
  {/snippet}
</Modal>

<style>
  .chd-body { display: flex; flex-direction: column; gap: 12px; }

  /* ── Tabs ─────────────────────────────────────────────── */
  .chd-tabs {
    display: flex;
    border-bottom: 1px solid var(--line);
    margin-bottom: 4px;
  }

  .chd-tab {
    padding: 6px 14px;
    background: none;
    border: none;
    border-bottom: 2px solid transparent;
    font: inherit;
    font-size: 13px;
    color: var(--text-muted);
    cursor: pointer;
    margin-bottom: -1px;
    &:hover { color: var(--text); }
  }

  .chd-tab--active {
    color: var(--accent);
    border-bottom-color: var(--accent);
    font-weight: 500;
  }

  /* ── Naming fieldsets ─────────────────────────────────── */
  .naming-fieldset {
    border: 1px solid var(--line);
    border-radius: 7px;
    padding: 8px 12px 10px;
    margin: 0;
    display: flex;
    flex-direction: column;
    gap: 8px;
  }

  .sub-fieldset {
    margin-left: 22px;
    border-color: color-mix(in srgb, var(--line) 70%, transparent);
    background: color-mix(in srgb, var(--surface-alt) 40%, transparent);
  }

  .naming-legend {
    font-size: 11px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.05em;
    color: var(--text-muted);
    padding: 0 4px;
  }

  .naming-options {
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .name-input-row {
    display: flex;
    align-items: center;
    border: 1px solid var(--line-strong);
    border-radius: 5px;
    background: var(--surface);
    overflow: hidden;
    transition: border-color 0.12s, box-shadow 0.12s, opacity 0.15s;

    &:focus-within:not(.name-input-row--disabled) {
      border-color: var(--accent);
      box-shadow: 0 0 0 2px color-mix(in srgb, var(--accent) 18%, transparent);
    }

    &.name-input-row--disabled {
      opacity: 0.55;
      background: var(--surface-alt);
    }
  }

  .name-input {
    flex: 1;
    min-width: 0;
    height: 28px;
    padding: 0 8px;
    border: none;
    background: transparent;
    color: var(--text);
    font: inherit;
    font-size: 13px;
    outline: none;
    cursor: default;
    &:not([readonly]) { cursor: text; }
  }

  .name-ext {
    padding: 0 9px 0 2px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    white-space: nowrap;
    flex-shrink: 0;
  }

  /* ── Options section ──────────────────────────────────── */
  .chd-section { display: flex; flex-direction: column; gap: 7px; }

  .checkbox-label, .radio-label {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 13px;
    color: var(--text);
    cursor: pointer;
    user-select: none;
    input { accent-color: var(--accent); }
    input:disabled { opacity: 0.4; }
  }

  /* ── Source / CHD list ────────────────────────────────── */
  .chd-sources {
    max-height: 180px;
    overflow-y: auto;
    background: var(--surface-alt);
    border: 1px solid var(--line);
    border-radius: 5px;
    padding: 5px 8px;
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .chd-source-row {
    display: flex;
    flex-direction: column;
    gap: 2px;

    &.chd-source-row--invalid { opacity: 0.75; }
  }

  .src-top {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--text-muted);
  }

  .src-warn-icon {
    flex-shrink: 0;
    color: #b45309;
  }

  .src-missing {
    font-size: 11px;
    color: #b45309;
    padding-left: 2px;
    font-style: italic;
  }

  .chd-src-badge {
    padding: 1px 5px;
    border-radius: 4px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 10px;
    font-weight: 700;
    flex-shrink: 0;

    &.chd-src-badge--warn {
      background: color-mix(in srgb, #f59e0b 15%, transparent);
      color: #b45309;
    }
  }

  /* ── Orphan .bin warning box ──────────────────────────── */
  .orphan-box {
    padding: 8px 10px;
    border-radius: 6px;
    background: color-mix(in srgb, #f59e0b 8%, var(--surface-alt));
    border: 1px solid color-mix(in srgb, #f59e0b 35%, var(--line));
    display: flex;
    flex-direction: column;
    gap: 5px;
  }

  .orphan-header {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 11.5px;
    font-weight: 600;
    color: #92400e;
    svg { stroke: #b45309; flex-shrink: 0; }
  }

  .orphan-row {
    font-size: 11.5px;
    color: #92400e;
    padding-left: 19px;
    font-family: var(--font-mono, monospace);
  }

  .chd-src-name {
    color: var(--text);
    font-weight: 500;
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  .chd-src-exts { font-size: 11px; flex-shrink: 0; }

  .src-preview {
    display: flex;
    align-items: center;
    gap: 5px;
    padding-left: 2px;
    flex-wrap: wrap;
  }

  .preview-arrow {
    font-size: 11px;
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .preview-folder {
    font-size: 12px;
    font-weight: 600;
    color: var(--text-muted);
    font-family: monospace;
  }

  .preview-sep {
    font-size: 11px;
    color: var(--text-muted);
  }

  .preview-name {
    font-size: 12px;
    font-weight: 600;
    color: var(--accent);
    font-family: monospace;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
  }

  /* ── Transfer policy (shared with restore destination) ── */
  .policy-row {
    display: flex;
    align-items: center;
    gap: 10px;
    flex-wrap: wrap;
    margin-top: 2px;
  }
  .policy-label { font-size: 11px; font-weight: 600; color: var(--text-muted); flex-shrink: 0; }
  .policy-options { display: flex; gap: 12px; flex-wrap: wrap; }
  .radio-label--small { font-size: 12px; gap: 5px; }
</style>
