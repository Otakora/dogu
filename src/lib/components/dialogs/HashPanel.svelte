<script lang="ts">
  import { invoke, Channel } from "@tauri-apps/api/core";
  import { onDestroy, untrack } from "svelte";
  import { t } from "../../i18n/index.js";
  import { app } from "../../stores/app.svelte.js";
  import Button from "../ui/Button.svelte";
  import { hashBlockedPaths } from "../../utils/hash-locks.js";
  export type HashResult = { path: string; hashes: Record<string, string>; error: string | null };
  type HashProgress = { path: string; index: number; count: number; bytes: number; total: number };
  type Props = { paths: string[]; initialAlgorithms?: string[]; autoStart?: boolean; initialResults?: HashResult[] };
  let { paths, initialAlgorithms = ["SHA-256"], autoStart = false, initialResults = [] }: Props = $props();
  const algorithms = ["MD5", "SHA-1", "SHA-256", "SHA-512", "CRC32", "BLAKE3"];
  let chosen = $state<string[]>(untrack(() => [...initialAlgorithms]));
  let results = $state<HashResult[]>(untrack(() => initialResults));
  let busy = $state(false);
  let progress = $state<HashProgress | null>(null);
  let error = $state("");
  let expected = $state("");
  let requestId: string | null = null;
  let destroyed = false;
  let started = false;
  const blocked = $derived(hashBlockedPaths(paths, app.hashBusyOperations));
  $effect(() => { if (autoStart && !started && initialResults.length === 0 && blocked.length === 0) { started = true; void run(); } });
  $effect(() => { if (busy && blocked.length > 0 && requestId) void invoke("cancel_hashes", { requestId }).catch(() => {}); });
  onDestroy(() => { destroyed = true; if (requestId) void invoke("cancel_hashes", { requestId }).catch(() => {}); });
  function toggle(a: string) { chosen = chosen.includes(a) ? chosen.filter(x => x !== a) : [...chosen, a]; }
  function errorText(value: string) {
    if (value.includes("[HASH_CANCELLED]")) return t("hash.cancelled");
    if (value.includes("[HASH_FILE_CHANGED]")) return t("hash.changed");
    if (value.includes("[HASH_NOT_FILE]")) return t("hash.notFile");
    return value;
  }
  async function run() {
    if (!chosen.length || busy || hashBlockedPaths(paths, app.hashBusyOperations).length > 0) return;
    busy = true; error = ""; results = []; progress = null;
    const id = `hash-${crypto.randomUUID()}`; requestId = id;
    const channel = new Channel<HashProgress>();
    channel.onmessage = value => { if (destroyed || hashBlockedPaths(paths, app.hashBusyOperations).length > 0) void invoke("cancel_hashes", { requestId: id }).catch(() => {}); else progress = value; };
    try {
      const value = await invoke<HashResult[]>("calculate_hashes", { requestId: id, paths, algorithms: chosen, progress: channel });
      if (!destroyed) results = value;
    } catch (e) { if (!destroyed) error = errorText(String(e)); }
    finally { if (requestId === id) requestId = null; if (!destroyed) busy = false; }
  }
  async function cancel() { if (requestId) await invoke("cancel_hashes", { requestId }).catch(() => {}); }
  async function copy(text: string) { try { await navigator.clipboard.writeText(text); app.notify("success", t("hash.copied")); } catch (e) { app.notify("error", String(e)); } }
  const allText = $derived(results.flatMap(row => Object.entries(row.hashes).map(([algorithm, value]) => `${algorithm}  ${value}  ${row.path}`)).join("\n"));
</script>
<div class="hash-panel">
  <span class="hash-info">{t("hash.info")}</span>
  <span class="hash-heading">{t("hash.select")}</span>
  <div class="hash-algorithms">
    {#each algorithms as algorithm}
      <label><input type="checkbox" checked={chosen.includes(algorithm)} disabled={busy} onchange={() => toggle(algorithm)} />{algorithm}</label>
    {/each}
  </div>
  <div class="hash-actions">
    <Button size="sm" variant="outline" onclick={run} disabled={busy || chosen.length === 0 || blocked.length > 0}>{t("hash.calculate")}</Button>
    {#if busy}<Button size="sm" variant="ghost" onclick={cancel} disabled={!progress}>{t("hash.cancel")}</Button>{/if}
    {#if allText}<Button size="sm" variant="ghost" onclick={() => copy(allText)}>{t("hash.copyAll")}</Button>{/if}
  </div>
  {#if blocked.length}<div class="hash-error" role="status">{t("hash.blocked")}<br />{blocked.join(", ")}</div>{/if}
  {#if busy}
    <div class="hash-status">{progress ? t("hash.calculating", { index: progress.index + 1, count: progress.count, name: progress.path.split(/[\\/]/).pop() ?? progress.path }) : t("propertiesDialog.calculating")}</div>
    {#if progress?.total}<progress max={progress.total} value={progress.bytes}></progress>{/if}
  {/if}
  {#if error}<div class="hash-error" role="alert">{error}</div>{/if}
  {#if results.length}
    <label class="hash-compare">{t("hash.expected")}<input type="text" bind:value={expected} placeholder={t("hash.expectedPlaceholder")} /></label>
    <div class="hash-results scrollbar-thin">
      {#each results as row (row.path)}
        <div class="hash-file"><strong title={row.path}>{row.path}</strong>
          {#if row.error}<div class="hash-error">{errorText(row.error)}</div>{/if}
          {#each Object.entries(row.hashes) as [algorithm, value]}
            <div class="hash-row"><span>{algorithm}</span><code>{value}</code><button onclick={() => copy(value)} aria-label="{t('hash.copy')} {algorithm}">{t("hash.copy")}</button></div>
          {/each}
          {#if expected.trim() && Object.keys(row.hashes).length}
            <span class:match={Object.values(row.hashes).some(v => v.toLowerCase() === expected.trim().toLowerCase())} class="hash-match">{Object.values(row.hashes).some(v => v.toLowerCase() === expected.trim().toLowerCase()) ? t("hash.match") : t("hash.noMatch")}</span>
          {/if}
        </div>
      {/each}
    </div>
  {/if}
</div>
<style>
.hash-panel { display:flex; flex-direction:column; gap:8px; min-width:0; }
.hash-info,.hash-status { color:var(--text-muted); font-size:12px; }
.hash-heading { font-weight:700; font-size:12px; }
.hash-algorithms,.hash-actions { display:flex; flex-wrap:wrap; gap:8px 14px; align-items:center; }
.hash-algorithms label { display:flex; align-items:center; gap:5px; font-size:12px; }
.hash-algorithms input { accent-color:var(--accent); }
.hash-compare { display:flex; flex-direction:column; gap:5px; color:var(--text-muted); font-size:12px; }
.hash-compare input { background:var(--surface-alt); color:var(--text); border:1px solid var(--line); border-radius:5px; padding:7px; }
.hash-results { overflow:auto; max-height:360px; display:flex; flex-direction:column; gap:8px; }
.hash-file { padding:8px; border:1px solid var(--line); border-radius:5px; min-width:0; font-size:12px; }
.hash-file strong { display:block; overflow-wrap:anywhere; margin-bottom:5px; }
.hash-row { display:grid; grid-template-columns:64px minmax(0,1fr) auto; gap:5px; align-items:start; padding:3px 0; }
.hash-row code { overflow-wrap:anywhere; user-select:text; }
.hash-row button { background:none; border:0; color:var(--accent); cursor:pointer; }
.hash-error,.hash-match { color:var(--danger, #e5484d); font-size:12px; overflow-wrap:anywhere; }
.hash-match.match { color:var(--accent); }
progress { width:100%; accent-color:var(--accent); }
</style>
