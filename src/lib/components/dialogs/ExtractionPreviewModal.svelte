<script lang="ts">
  import type { ExtractionPreviewRow } from "../../types/index.js";
  import Modal from "../ui/Modal.svelte";
  import Button from "../ui/Button.svelte";
  import { t } from "../../i18n/index.js";

  type Props = {
    rows: ExtractionPreviewRow[];
    onclose: () => void;
  };

  let { rows, onclose }: Props = $props();

  const folderIcon = `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/></svg>`;
  const fileIcon   = `<svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/><polyline points="13 2 13 9 20 9"/></svg>`;
</script>

<Modal title={t("extractionPreviewModal.title")} width="640px" {onclose}>
  {#snippet children()}
    <div class="pv-body">
      {#each rows as row}
        <div class="pv-group">
          <div class="pv-archive-header">
            <span class="pv-archive-name">{row.archivePath.split(/[\\/]/).pop()}</span>
            <span class="pv-arrow">→</span>
            <span class="pv-archive-dest" title={row.destinationRoot}>{row.destinationRoot}</span>
          </div>
          <div class="pv-entry-list">
            {#each row.entries as entry}
              <div class="pv-entry-row">
                <span class="pv-entry-icon">{@html entry.isDir ? folderIcon : fileIcon}</span>
                <span class="pv-entry-name" title={entry.name}>{entry.name}</span>
                <span class="pv-arrow">→</span>
                <span class="pv-entry-dest" title={entry.destinationPath}>{entry.destinationPath}</span>
              </div>
            {/each}
            {#if row.entries.length === 0}
              <div class="pv-empty">{t("extractionPreviewModal.emptyArchive")}</div>
            {/if}
          </div>
        </div>
      {/each}
    </div>
  {/snippet}

  {#snippet footer()}
    <Button variant="primary" onclick={onclose}>{t("extractionPreviewModal.close")}</Button>
  {/snippet}
</Modal>

<style>
  .pv-body {
    display: flex;
    flex-direction: column;
    gap: 16px;
  }

  .pv-group {
    display: flex;
    flex-direction: column;
    gap: 6px;
  }

  .pv-archive-header {
    display: flex;
    align-items: center;
    gap: 6px;
    font-size: 13px;
    font-weight: 600;
    color: var(--text);
    padding-bottom: 4px;
    border-bottom: 1px solid var(--line);
  }

  .pv-archive-dest {
    color: var(--text-muted);
    font-weight: 400;
    overflow-wrap: anywhere;
  }

  .pv-entry-list {
    display: flex;
    flex-direction: column;
    gap: 3px;
    padding-left: 4px;
  }

  .pv-entry-row {
    display: flex;
    align-items: flex-start;
    gap: 6px;
    font-size: 12px;
    color: var(--text-muted);
    padding: 2px 0;
  }

  .pv-entry-icon {
    flex-shrink: 0;
    margin-top: 2px;
    color: var(--text-subtle);
  }

  .pv-entry-name {
    color: var(--text);
    flex-shrink: 0;
    max-width: 220px;
    overflow-wrap: anywhere;
  }

  .pv-entry-dest {
    overflow-wrap: anywhere;
  }

  .pv-arrow {
    color: var(--text-subtle);
    flex-shrink: 0;
  }

  .pv-empty {
    font-size: 12px;
    color: var(--text-subtle);
    font-style: italic;
  }
</style>
