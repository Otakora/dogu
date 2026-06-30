<script lang="ts">
  import type {
    SelectionAnalysisDto,
    ChdConversionOptionsPayload,
    ChdRestoreOptionsPayload,
  } from "../../types/index.js";
  import { untrack } from "svelte";
  import Modal from "../ui/Modal.svelte";
  import Button from "../ui/Button.svelte";
  import { open } from "@tauri-apps/plugin-dialog";

  type Mode = "convert" | "restore";

  type Props = {
    analysis: SelectionAnalysisDto;
    initialMode?: Mode;
    onclose: () => void;
    onConvert: (paths: string[], opts: ChdConversionOptionsPayload) => void;
    onRestore: (paths: string[], opts: ChdRestoreOptionsPayload) => void;
  };

  let { analysis, initialMode = "convert", onclose, onConvert, onRestore }: Props = $props();

  let mode = $state<Mode>(untrack(() => initialMode));

  // Convert options
  let convOpts = $state<ChdConversionOptionsPayload>({
    deleteOriginals: false,
    nameAsContainer: true,
    depositToParent: false,
    deleteOriginalSubfolders: false,
    overwrite: false,
  });

  // Restore options
  let restOpts = $state<ChdRestoreOptionsPayload>({
    individualFolders: false,
    destinationMode: "same",
    destinationPath: null,
    deleteChd: false,
    overwrite: false,
    splitBin: false,
  });

  async function pickRestoreDestination() {
    try {
      const selected = await open({ directory: true, multiple: false, title: "Select destination" });
      if (typeof selected === "string") {
        restOpts = { ...restOpts, destinationPath: selected, destinationMode: "custom" };
      }
    } catch { /* cancelled */ }
  }

  function submit() {
    if (mode === "convert") {
      const paths = analysis.chdSources.map(s => s.sourcePath);
      onConvert(paths, convOpts);
    } else {
      onRestore(analysis.restorableChds, restOpts);
    }
    onclose();
  }

  const canConvert = $derived(analysis.chdSources.length > 0);
  const canRestore = $derived(analysis.restorableChds.length > 0);
  const title = $derived(
    mode === "convert"
      ? `Convert to CHD (${analysis.chdSources.length} source${analysis.chdSources.length !== 1 ? "s" : ""})`
      : `Restore from CHD (${analysis.restorableChds.length} file${analysis.restorableChds.length !== 1 ? "s" : ""})`
  );
</script>

<Modal {title} width="480px" {onclose}>
  {#snippet children()}
    <div class="chd-body">
      <!-- Mode tabs (only when both are possible) -->
      {#if canConvert && canRestore}
        <div class="chd-tabs" role="tablist">
          <button class="chd-tab" class:chd-tab--active={mode === "convert"} onclick={() => (mode = "convert")} role="tab" aria-selected={mode === "convert"}>Convert to CHD</button>
          <button class="chd-tab" class:chd-tab--active={mode === "restore"} onclick={() => (mode = "restore")} role="tab" aria-selected={mode === "restore"}>Restore from CHD</button>
        </div>
      {/if}

      {#if mode === "convert"}
        <div class="chd-section">
          <label class="checkbox-label">
            <input type="checkbox" bind:checked={convOpts.nameAsContainer} />
            Name CHD file after container folder
          </label>
          <label class="checkbox-label">
            <input type="checkbox" bind:checked={convOpts.depositToParent} />
            Place CHD in parent folder
          </label>
          <label class="checkbox-label">
            <input type="checkbox" bind:checked={convOpts.deleteOriginals} />
            Delete original files after conversion
          </label>
          <label class="checkbox-label">
            <input type="checkbox" bind:checked={convOpts.deleteOriginalSubfolders} disabled={!convOpts.deleteOriginals} />
            Also delete source subfolders
          </label>
          <label class="checkbox-label">
            <input type="checkbox" bind:checked={convOpts.overwrite} />
            Overwrite existing CHD files
          </label>
        </div>

        <!-- Source list -->
        {#if analysis.chdSources.length > 0}
          <div class="chd-sources scrollbar-thin">
            {#each analysis.chdSources as src}
              <div class="chd-source-row">
                <span class="chd-src-badge">{src.command}</span>
                <span class="chd-src-name">{src.sourcePath.split(/[\\/]/).pop()}</span>
                <span class="chd-src-exts">{src.displayExtensions.join(", ")}</span>
              </div>
            {/each}
          </div>
        {/if}

      {:else}
        <!-- Restore mode -->
        <div class="chd-section">
          <div class="setting-row">
            <span>Destination</span>
          </div>
          <div class="radio-group">
            <label class="radio-label">
              <input type="radio" bind:group={restOpts.destinationMode} value="same" />
              Same folder as CHD
            </label>
            <label class="radio-label">
              <input type="radio" bind:group={restOpts.destinationMode} value="custom" />
              Custom folder
            </label>
          </div>
          {#if restOpts.destinationMode === "custom"}
            <div class="ex-path-row">
              <input class="ex-path-input" type="text" bind:value={restOpts.destinationPath} placeholder="Select a folder…" readonly />
              <Button variant="outline" size="sm" onclick={pickRestoreDestination}>Browse…</Button>
            </div>
          {/if}
          <label class="checkbox-label">
            <input type="checkbox" bind:checked={restOpts.individualFolders} />
            Restore each CHD into its own folder
          </label>
          <label class="checkbox-label">
            <input type="checkbox" bind:checked={restOpts.splitBin} />
            Split output as .bin/.cue
          </label>
          <label class="checkbox-label">
            <input type="checkbox" bind:checked={restOpts.deleteChd} />
            Delete CHD after restore
          </label>
          <label class="checkbox-label">
            <input type="checkbox" bind:checked={restOpts.overwrite} />
            Overwrite existing files
          </label>
        </div>
      {/if}
    </div>
  {/snippet}

  {#snippet footer()}
    <Button variant="ghost" onclick={onclose}>Cancel</Button>
    <Button
      variant="primary"
      onclick={submit}
      disabled={mode === "restore" && restOpts.destinationMode === "custom" && !restOpts.destinationPath}
    >
      {mode === "convert" ? "Convert" : "Restore"}
    </Button>
  {/snippet}
</Modal>

<style>
  .chd-body { display: flex; flex-direction: column; gap: 12px; }

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

  .chd-section { display: flex; flex-direction: column; gap: 7px; }

  .checkbox-label, .radio-label {
    display: flex;
    align-items: center;
    gap: 7px;
    font-size: 13px;
    color: var(--text);
    cursor: pointer;
    input { accent-color: var(--accent); }
    input:disabled { opacity: 0.4; }
  }

  .radio-group { display: flex; flex-direction: column; gap: 5px; }

  .chd-sources {
    max-height: 150px;
    overflow-y: auto;
    background: var(--surface-alt);
    border: 1px solid var(--line);
    border-radius: 5px;
    padding: 6px 8px;
    display: flex;
    flex-direction: column;
    gap: 4px;
  }

  .chd-source-row {
    display: flex;
    align-items: center;
    gap: 8px;
    font-size: 12px;
    color: var(--text-muted);
  }

  .chd-src-badge {
    padding: 1px 5px;
    border-radius: 4px;
    background: var(--accent-soft);
    color: var(--accent);
    font-size: 10px;
    font-weight: 700;
    flex-shrink: 0;
  }

  .chd-src-name { color: var(--text); font-weight: 500; flex: 1; min-width: 0; overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
  .chd-src-exts { font-size: 11px; flex-shrink: 0; }

  .ex-path-row { display: flex; gap: 6px; align-items: center; }
  .ex-path-input {
    flex: 1; height: 28px; padding: 0 8px;
    border: 1px solid var(--line-strong); border-radius: 5px;
    background: var(--surface-alt); color: var(--text);
    font: inherit; font-size: 13px; min-width: 0; cursor: default;
  }

  .setting-row { display: flex; align-items: center; font-size: 13px; color: var(--text-muted); }
</style>
