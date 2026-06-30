<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { PropertiesSummaryDto } from "../../types/index.js";
  import Modal from "../ui/Modal.svelte";
  import Button from "../ui/Button.svelte";

  type Props = {
    paths: string[];
    onclose: () => void;
  };

  let { paths, onclose }: Props = $props();

  let summary = $state<PropertiesSummaryDto | null>(null);
  let loading = $state(true);
  let error = $state<string | null>(null);

  let requestId = Math.floor(Math.random() * 1e9);

  $effect(() => {
    loadSummary();
  });

  async function loadSummary() {
    loading = true;
    error = null;
    try {
      summary = await invoke<PropertiesSummaryDto>("summarize_paths", {
        requestId,
        paths,
        options: { maxDepth: null },
      });
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  const title = $derived(
    paths.length === 1
      ? paths[0].split(/[\\/]/).filter(Boolean).pop() ?? paths[0]
      : `${paths.length} items`
  );
</script>

<Modal title="Properties — {title}" width="420px" {onclose}>
  {#snippet children()}
    {#if loading}
      <div class="props-loading">
        <span class="props-spinner"></span>
        Calculating…
      </div>
    {:else if error}
      <div class="props-error">{error}</div>
    {:else if summary}
      <div class="props-body">
        <div class="props-row">
          <span class="props-key">Items</span>
          <span class="props-val">{summary.count.toLocaleString()}</span>
        </div>
        <div class="props-row">
          <span class="props-key">Files</span>
          <span class="props-val">{summary.files.toLocaleString()}</span>
        </div>
        <div class="props-row">
          <span class="props-key">Folders</span>
          <span class="props-val">{summary.directories.toLocaleString()}</span>
        </div>
        <div class="props-row">
          <span class="props-key">Total size</span>
          <span class="props-val">
            {summary.totalSizeLabel}
            <span class="props-bytes">({summary.totalSize.toLocaleString()} bytes)</span>
          </span>
        </div>
        {#if summary.lines.length > 0}
          <div class="props-divider"></div>
          {#each summary.lines as line}
            <div class="props-info-line">{line}</div>
          {/each}
        {/if}
      </div>
    {/if}
  {/snippet}

  {#snippet footer()}
    <Button variant="primary" onclick={onclose}>Close</Button>
  {/snippet}
</Modal>

<style>
  .props-loading {
    display: flex;
    align-items: center;
    gap: 10px;
    padding: 16px 0;
    font-size: 13px;
    color: var(--text-muted);
  }

  .props-spinner {
    width: 16px;
    height: 16px;
    border: 2px solid var(--line-strong);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
    flex-shrink: 0;
  }

  .props-error {
    color: var(--danger);
    font-size: 13px;
    padding: 8px 0;
  }

  .props-body {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .props-row {
    display: flex;
    justify-content: space-between;
    align-items: baseline;
    gap: 12px;
    font-size: 13px;
  }

  .props-key {
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .props-val {
    color: var(--text);
    font-weight: 500;
    text-align: right;
  }

  .props-bytes {
    font-size: 11px;
    color: var(--text-muted);
    font-weight: 400;
  }

  .props-divider {
    height: 1px;
    background: var(--line);
    margin: 4px 0;
  }

  .props-info-line {
    font-size: 12px;
    color: var(--text-muted);
  }
</style>
