<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { PropertiesSummaryDto } from "../../types/index.js";
  import Modal from "../ui/Modal.svelte";
  import Button from "../ui/Button.svelte";
  import { t } from "../../i18n/index.js";
  import { app } from "../../stores/app.svelte.js";
  import HashPanel from "./HashPanel.svelte";
  import { dirnameOf } from "../../utils/ghosts.js";

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
      : t("propertiesDialog.itemsLabel", { count: paths.length })
  );
  function dateLabel(ts: number): string {
    return new Intl.DateTimeFormat(app.settings.locale, { dateStyle: "medium", timeStyle: "short" }).format(new Date(ts * 1000));
  }
  function fileType(name: string, isDir: boolean): string {
    if (isDir) return t("propertiesDialog.folderType");
    const dot = name.lastIndexOf(".");
    const ext = dot > 0 && dot < name.length - 1 ? name.slice(dot) : "";
    return ext ? `${t("propertiesDialog.fileType")} ${ext}` : t("propertiesDialog.fileType");
  }
</script>

<Modal title={t("propertiesDialog.title", { name: title })} width="680px" {onclose}>
  {#snippet children()}
    {#if loading}
      <div class="props-loading">
        <span class="props-spinner"></span>
        {t("propertiesDialog.calculating")}
      </div>
    {:else if error}
      <div class="props-error">{error}</div>
    {:else if summary}
      <div class="props-body">
        {#if summary.items.length === 1}
          {@const item = summary.items[0]}
          <div class="props-hero"><div class="props-badge">{item.isDir ? "📁" : "📄"}</div><div class="props-hero-text"><strong>{item.name}</strong><span>{fileType(item.name, item.isDir)}</span></div><strong>{item.sizeLabel}</strong></div>
          <div class="props-details">
            <div class="props-row"><span class="props-key">{t("propertiesDialog.name")}</span><span class="props-val">{item.name}</span></div>
            <div class="props-row"><span class="props-key">{t("propertiesDialog.location")}</span><span class="props-val props-path">{dirnameOf(item.path)}</span></div>
            <div class="props-row"><span class="props-key">{t("propertiesDialog.type")}</span><span class="props-val">{fileType(item.name, item.isDir)}</span></div>
            <div class="props-row"><span class="props-key">{t("propertiesDialog.size")}</span><span class="props-val">{item.sizeLabel} <span class="props-bytes">{t("propertiesDialog.bytes", { count: item.sizeBytes.toLocaleString() })}</span></span></div>
            {#if item.isDir}<div class="props-row"><span class="props-key">{t("propertiesDialog.contents")}</span><span class="props-val">{item.files} {t("propertiesDialog.files").toLowerCase()} · {item.directories} {t("propertiesDialog.folders").toLowerCase()}</span></div>{/if}
            {#if item.modifiedTs != null}<div class="props-row"><span class="props-key">{t("propertiesDialog.modified")}</span><span class="props-val">{dateLabel(item.modifiedTs)}</span></div>{/if}
            {#if item.createdTs != null}<div class="props-row"><span class="props-key">{t("propertiesDialog.created")}</span><span class="props-val">{dateLabel(item.createdTs)}</span></div>{/if}
            {#if item.readonly != null}<div class="props-row"><span class="props-key">{t("propertiesDialog.readonly")}</span><span class="props-val">{item.readonly ? t("propertiesDialog.yes") : t("propertiesDialog.no")}</span></div>{/if}
          </div>
        {:else}
          <div class="props-hero"><div class="props-badge">▤</div><div class="props-hero-text"><strong>{t("propertiesDialog.itemsLabel", { count: summary.count })}</strong><span>{t("propertiesDialog.selection")}</span></div><strong>{summary.totalSizeLabel}</strong></div>
          <div class="props-details">
            <div class="props-row"><span class="props-key">{t("propertiesDialog.selectedFiles")}</span><span class="props-val">{summary.items.filter(item => !item.isDir).length.toLocaleString()}</span></div>
            <div class="props-row"><span class="props-key">{t("propertiesDialog.selectedFolders")}</span><span class="props-val">{summary.items.filter(item => item.isDir).length.toLocaleString()}</span></div>
            <div class="props-row"><span class="props-key">{t("propertiesDialog.totalSize")}</span><span class="props-val">{summary.totalSizeLabel} <span class="props-bytes">{t("propertiesDialog.bytes", { count: summary.totalSize.toLocaleString() })}</span></span></div>
            {#if summary.items.some(item => item.isDir)}<div class="props-row"><span class="props-key">{t("propertiesDialog.contents")}</span><span class="props-val">{summary.files.toLocaleString()} {t("propertiesDialog.files").toLowerCase()} · {summary.directories.toLocaleString()} {t("propertiesDialog.folders").toLowerCase()}</span></div>{/if}
          </div>
          <div class="props-items scrollbar-thin">
            {#each summary.items as item}
              <div class="props-item">
                <div class="props-item-header"><strong>{item.name}</strong><span>{item.sizeLabel}</span></div>
                <div class="props-info-line" title={item.path}>{item.path}</div>
                {#if item.isDir}<div class="props-info-line">{t("propertiesDialog.files")}: {item.files} · {t("propertiesDialog.folders")}: {item.directories}</div>{/if}
              </div>
            {/each}
          </div>
        {/if}
      </div>
    {/if}
    {#if summary && summary.items.some(item => !item.isDir)}
    <div class="props-hashes">
      <strong>{t("hash.properties")}</strong>
      <span class="props-info-line">{t("hash.filesOnly")}</span>
      <HashPanel paths={summary.items.filter(item => !item.isDir).map(item => item.path)} autoStart={app.settings.propertiesAutoHash} initialAlgorithms={app.settings.propertiesHashAlgorithm === "all" ? ["MD5", "SHA-1", "SHA-256", "SHA-512", "CRC32", "BLAKE3"] : [app.settings.propertiesHashAlgorithm]} />
    </div>
    {/if}
  {/snippet}

  {#snippet footer()}
    <Button variant="primary" onclick={onclose}>{t("propertiesDialog.close")}</Button>
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

  .props-info-line {
    font-size: 12px;
    color: var(--text-muted);
  }
  .props-hashes { margin-top:16px; padding-top:12px; border-top:1px solid var(--line); display:flex; flex-direction:column; gap:8px; }
  .props-items { display:flex; flex-direction:column; gap:6px; max-height:220px; overflow-y:auto; }
  .props-item { border:1px solid var(--line); border-radius:5px; padding:8px; min-width:0; }
  .props-item-header { display:flex; justify-content:space-between; gap:10px; font-size:12px; }
  .props-item-header strong, .props-info-line { overflow-wrap:anywhere; }
  .props-hero { display:flex; gap:12px; align-items:center; padding:12px; background:var(--surface-alt); border:1px solid var(--line); border-radius:8px; }
  .props-badge { display:grid; place-items:center; width:36px; height:36px; border-radius:8px; background:var(--surface); font-size:20px; flex-shrink:0; }
  .props-hero-text { display:flex; flex:1; flex-direction:column; gap:3px; min-width:0; font-size:13px; }
  .props-hero-text strong { overflow-wrap:anywhere; }
  .props-hero-text span { color:var(--text-muted); font-size:12px; }
  .props-details { display:flex; flex-direction:column; gap:8px; padding:10px 2px; }
  .props-path { overflow-wrap:anywhere; user-select:text; }
</style>
