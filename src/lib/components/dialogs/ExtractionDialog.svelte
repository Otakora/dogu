<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { ExtractionOptionsPayload, ExtractionPreviewRow } from "../../types/index.js";
  import Modal from "../ui/Modal.svelte";
  import Button from "../ui/Button.svelte";
  import { open } from "@tauri-apps/plugin-dialog";

  type Props = {
    archives: string[];
    onclose: () => void;
    onExtract: (archives: string[], opts: ExtractionOptionsPayload) => void;
  };

  let { archives, onclose, onExtract }: Props = $props();

  let opts = $state<ExtractionOptionsPayload>({
    individualFolders: true,
    destinationMode: "same",
    destinationPath: null,
    deleteArchives: false,
    overwrite: false,
  });

  let preview = $state<ExtractionPreviewRow[]>([]);
  let previewLoading = $state(false);

  $effect(() => {
    loadPreview();
  });

  async function loadPreview() {
    previewLoading = true;
    try {
      preview = await invoke<ExtractionPreviewRow[]>("build_extraction_preview", {
        archives,
        options: opts,
      });
    } catch {
      preview = [];
    } finally {
      previewLoading = false;
    }
  }

  async function pickDestination() {
    try {
      const selected = await open({ directory: true, multiple: false, title: "Select destination" });
      if (typeof selected === "string") {
        opts = { ...opts, destinationPath: selected, destinationMode: "custom" };
      }
    } catch { /* cancelled */ }
  }

  function submit() {
    onExtract(archives, opts);
    onclose();
  }
</script>

<Modal title="Extract archives" width="520px" {onclose}>
  {#snippet children()}
    <div class="ex-body">
      <div class="ex-section">
        <span class="ex-label">Destination</span>
        <div class="radio-group">
          <label class="radio-label">
            <input type="radio" bind:group={opts.destinationMode} value="same" />
            Same folder as archive
          </label>
          <label class="radio-label">
            <input type="radio" bind:group={opts.destinationMode} value="custom" />
            Custom folder
          </label>
        </div>
        {#if opts.destinationMode === "custom"}
          <div class="ex-path-row">
            <input
              class="ex-path-input"
              type="text"
              bind:value={opts.destinationPath}
              placeholder="Select a folder…"
              readonly
            />
            <Button variant="outline" size="sm" onclick={pickDestination}>Browse…</Button>
          </div>
        {/if}
      </div>

      <div class="ex-section">
        <span class="ex-label">Options</span>
        <label class="checkbox-label">
          <input type="checkbox" bind:checked={opts.individualFolders} />
          Extract each archive into its own folder
        </label>
        <label class="checkbox-label">
          <input type="checkbox" bind:checked={opts.overwrite} />
          Overwrite existing files
        </label>
        <label class="checkbox-label">
          <input type="checkbox" bind:checked={opts.deleteArchives} />
          Delete archives after extraction
        </label>
      </div>

      <!-- Preview -->
      {#if !previewLoading && preview.length > 0}
        <div class="ex-section">
          <span class="ex-label">Preview ({preview.length} archive{preview.length !== 1 ? "s" : ""})</span>
          <div class="ex-preview scrollbar-thin">
            {#each preview as row}
              <div class="ex-preview-row">
                <span class="ex-preview-src">{row.archivePath.split(/[\\/]/).pop()}</span>
                <span class="ex-arrow">→</span>
                <span class="ex-preview-dst">{row.destinationPath}</span>
              </div>
            {/each}
          </div>
        </div>
      {/if}
    </div>
  {/snippet}

  {#snippet footer()}
    <Button variant="ghost" onclick={onclose}>Cancel</Button>
    <Button
      variant="primary"
      onclick={submit}
      disabled={opts.destinationMode === "custom" && !opts.destinationPath}
    >
      Extract {archives.length} archive{archives.length !== 1 ? "s" : ""}
    </Button>
  {/snippet}
</Modal>

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

  .radio-group, .checkbox-label {
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

  .ex-path-row {
    display: flex;
    gap: 6px;
    align-items: center;
  }

  .ex-path-input {
    flex: 1;
    height: 28px;
    padding: 0 8px;
    border: 1px solid var(--line-strong);
    border-radius: 5px;
    background: var(--surface-alt);
    color: var(--text);
    font: inherit;
    font-size: 13px;
    min-width: 0;
    cursor: default;
  }

  .ex-preview {
    max-height: 150px;
    overflow-y: auto;
    background: var(--surface-alt);
    border: 1px solid var(--line);
    border-radius: 5px;
    padding: 6px 8px;
  }

  .ex-preview-row {
    display: flex;
    gap: 6px;
    align-items: center;
    font-size: 12px;
    color: var(--text-muted);
    padding: 2px 0;
  }

  .ex-preview-src { color: var(--text); font-weight: 500; flex-shrink: 0; }
  .ex-arrow { color: var(--text-subtle); flex-shrink: 0; }
  .ex-preview-dst { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
