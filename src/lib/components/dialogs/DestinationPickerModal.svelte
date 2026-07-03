<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { ActiveConnectionDto, EntryDto, KnownFoldersDto, VolumeDto } from "../../types/index.js";
  import Modal from "../ui/Modal.svelte";
  import Button from "../ui/Button.svelte";
  import { t } from "../../i18n/index.js";

  type DestinationResult = { path: string; label: string };

  type Props = {
    onselect: (result: DestinationResult) => void;
    onclose: () => void;
  };

  let { onselect, onclose }: Props = $props();

  // ── Left panel state ──────────────────────────────────────
  let knownFolders = $state<KnownFoldersDto | null>(null);
  let volumes = $state<VolumeDto[]>([]);
  let connections = $state<ActiveConnectionDto[]>([]);

  // ── Right panel state ─────────────────────────────────────
  let breadcrumb = $state<{ path: string; label: string }[]>([]);
  let children = $state<EntryDto[]>([]);
  let loading = $state(false);
  let error = $state<string | null>(null);

  const currentPath = $derived(breadcrumb.length > 0 ? breadcrumb[breadcrumb.length - 1].path : null);
  const dirs = $derived(children.filter((e) => e.isDir));

  async function loadLeft() {
    const [kf, vols, conns] = await Promise.all([
      invoke<KnownFoldersDto>("get_known_folders"),
      invoke<VolumeDto[]>("list_volumes"),
      invoke<ActiveConnectionDto[]>("list_active_connections"),
    ]);
    knownFolders = kf;
    volumes = vols;
    connections = conns;
  }

  async function navigateTo(path: string, label: string, reset = false) {
    loading = true;
    error = null;
    try {
      const entries = await invoke<EntryDto[]>("list_children", { path });
      children = entries;
      if (reset) {
        breadcrumb = [{ path, label }];
      } else {
        breadcrumb = [...breadcrumb, { path, label }];
      }
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  async function navigateCrumb(index: number) {
    const crumb = breadcrumb[index];
    loading = true;
    error = null;
    try {
      const entries = await invoke<EntryDto[]>("list_children", { path: crumb.path });
      children = entries;
      breadcrumb = breadcrumb.slice(0, index + 1);
    } catch (e) {
      error = String(e);
    } finally {
      loading = false;
    }
  }

  function confirmSelection() {
    if (!currentPath) return;
    const label = breadcrumb[breadcrumb.length - 1].label;
    onselect({ path: currentPath, label });
  }

  function iconFor(protocol: string) {
    const icons: Record<string, string> = { smb: "🖧", ssh: "🔒", ftp: "📡", ftps: "🔒" };
    return icons[protocol] ?? "🌐";
  }

  function knownFolderItems(kf: KnownFoldersDto) {
    const items: { label: string; path: string; icon: string }[] = [];
    if (kf.home)      items.push({ label: t("sidebar.home"),      path: kf.home,      icon: "🏠" });
    if (kf.desktop)   items.push({ label: t("sidebar.desktop"),   path: kf.desktop,   icon: "🖥" });
    if (kf.documents) items.push({ label: t("sidebar.documents"), path: kf.documents, icon: "📄" });
    if (kf.downloads) items.push({ label: t("sidebar.downloads"), path: kf.downloads, icon: "⬇" });
    return items;
  }

  $effect(() => { loadLeft(); });
</script>

<Modal title={t("destPicker.title")} width="700px" onclose={onclose}>
  {#snippet children()}
    <div class="picker-layout">
      <!-- Left panel: locations -->
      <aside class="left-panel">
        <!-- Local section -->
        <div class="section-header">{t("destPicker.local")}</div>
        {#if knownFolders}
          {#each knownFolderItems(knownFolders) as item}
            <button
              class="location-item"
              class:active={currentPath === item.path}
              onclick={() => navigateTo(item.path, item.label, true)}
            >
              <span class="loc-icon">{item.icon}</span>
              <span class="loc-label">{item.label}</span>
            </button>
          {/each}
        {/if}
        {#each volumes as vol}
          <button
            class="location-item"
            class:active={currentPath === vol.path}
            onclick={() => navigateTo(vol.path, vol.label || vol.name, true)}
          >
            <span class="loc-icon">💾</span>
            <span class="loc-label">{vol.label || vol.name}</span>
          </button>
        {/each}

        <!-- Remote connections section -->
        <div class="section-header">{t("destPicker.connections")}</div>
        {#if connections.length === 0}
          <p class="empty-hint">{t("destPicker.noConnections")}</p>
        {:else}
          {#each connections as conn}
            <button
              class="location-item"
              class:active={breadcrumb[0]?.path === conn.rootPath}
              onclick={() => navigateTo(conn.rootPath, conn.label, true)}
            >
              <span class="loc-icon">{iconFor(conn.protocol)}</span>
              <span class="loc-label">{conn.label}</span>
            </button>
          {/each}
        {/if}
      </aside>

      <!-- Right panel: directory browser -->
      <div class="right-panel">
        <!-- Breadcrumb -->
        <div class="breadcrumb">
          {#if breadcrumb.length === 0}
            <span class="breadcrumb-hint">← {t("destPicker.local")}</span>
          {:else}
            {#each breadcrumb as crumb, i}
              {#if i > 0}<span class="breadcrumb-sep">/</span>{/if}
              <button
                class="breadcrumb-item"
                class:last={i === breadcrumb.length - 1}
                onclick={() => navigateCrumb(i)}
              >{crumb.label}</button>
            {/each}
          {/if}
        </div>

        <!-- Directory list -->
        <div class="dir-list">
          {#if loading}
            <p class="state-hint">{t("destPicker.loading")}</p>
          {:else if error}
            <p class="state-hint error">{error}</p>
          {:else if breadcrumb.length === 0}
            <p class="state-hint">{t("destPicker.local")}</p>
          {:else if dirs.length === 0}
            <p class="state-hint">{t("destPicker.noChildren")}</p>
          {:else}
            {#each dirs as dir}
              <button
                class="dir-row"
                ondblclick={() => navigateTo(dir.path, dir.name)}
                onclick={() => navigateTo(dir.path, dir.name)}
              >
                <span class="dir-icon">📁</span>
                <span class="dir-name">{dir.name}</span>
              </button>
            {/each}
          {/if}
        </div>
      </div>
    </div>
  {/snippet}

  {#snippet footer()}
    <Button variant="ghost" onclick={onclose}>{t("common.cancel")}</Button>
    <Button variant="primary" disabled={!currentPath} onclick={confirmSelection}>
      {t("destPicker.selectHere")}
    </Button>
  {/snippet}
</Modal>

<style>
  .picker-layout {
    display: flex;
    gap: 0;
    height: 420px;
    margin: -16px;
    overflow: hidden;
    border-radius: 0 0 8px 8px;
  }

  .left-panel {
    width: 190px;
    flex-shrink: 0;
    border-right: 1px solid var(--line);
    overflow-y: auto;
    padding: 8px 0;
    scrollbar-width: thin;
    scrollbar-color: var(--line-strong) transparent;
  }

  .section-header {
    font-size: 10px;
    font-weight: 700;
    text-transform: uppercase;
    letter-spacing: 0.06em;
    color: var(--text-faint);
    padding: 10px 12px 4px;
  }

  .location-item {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    padding: 5px 12px;
    background: none;
    border: none;
    border-radius: 0;
    color: var(--text);
    font-size: 12px;
    text-align: left;
    cursor: pointer;

    &:hover { background: var(--surface-hover); }
    &.active { background: var(--accent-soft); color: var(--accent); }
  }

  .loc-icon { font-size: 13px; flex-shrink: 0; }
  .loc-label { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }

  .empty-hint {
    font-size: 11px;
    color: var(--text-faint);
    padding: 4px 12px;
    margin: 0;
  }

  .right-panel {
    flex: 1;
    display: flex;
    flex-direction: column;
    overflow: hidden;
  }

  .breadcrumb {
    display: flex;
    align-items: center;
    flex-wrap: nowrap;
    gap: 2px;
    padding: 8px 12px;
    border-bottom: 1px solid var(--line);
    overflow: hidden;
    flex-shrink: 0;
    min-height: 34px;
  }

  .breadcrumb-hint {
    font-size: 11px;
    color: var(--text-faint);
  }

  .breadcrumb-sep {
    color: var(--text-faint);
    font-size: 11px;
  }

  .breadcrumb-item {
    background: none;
    border: none;
    padding: 2px 4px;
    border-radius: 3px;
    font-size: 11px;
    color: var(--accent);
    cursor: pointer;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
    max-width: 160px;

    &:hover { background: var(--surface-hover); }
    &.last { color: var(--text); cursor: default; font-weight: 500; }
    &.last:hover { background: none; }
  }

  .dir-list {
    flex: 1;
    overflow-y: auto;
    padding: 4px 0;
    scrollbar-width: thin;
    scrollbar-color: var(--line-strong) transparent;
  }

  .state-hint {
    font-size: 12px;
    color: var(--text-faint);
    padding: 12px 16px;
    margin: 0;

    &.error { color: var(--danger); }
  }

  .dir-row {
    display: flex;
    align-items: center;
    gap: 7px;
    width: 100%;
    padding: 5px 14px;
    background: none;
    border: none;
    color: var(--text);
    font-size: 12px;
    text-align: left;
    cursor: pointer;

    &:hover { background: var(--surface-hover); }
  }

  .dir-icon { font-size: 13px; flex-shrink: 0; }
  .dir-name { overflow: hidden; text-overflow: ellipsis; white-space: nowrap; }
</style>
