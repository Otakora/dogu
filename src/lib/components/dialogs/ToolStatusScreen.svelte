<script lang="ts">
  import { openUrl as tauriOpenUrl } from "@tauri-apps/plugin-opener";
  import Modal from "../ui/Modal.svelte";
  import Button from "../ui/Button.svelte";
  import { app } from "../../stores/app.svelte.js";
  import { t } from "../../i18n/index.js";

  type Props = { onclose: () => void };
  let { onclose }: Props = $props();

  function openUrl(url: string) {
    tauriOpenUrl(url).catch(() => {});
  }

  // Human-readable list of features each capability unlocks.
  function capabilityLabels(enables: string[]): string {
    const labels: Record<string, string> = {
      chd: t("toolStatus.cap.chd"),
      archives: t("toolStatus.cap.archives"),
      rvzDolphin: t("toolStatus.cap.rvzDolphin"),
    };
    return enables.map((c) => labels[c]).filter(Boolean).join(", ");
  }

  const problems = $derived(app.toolProblems);
  const blocking = $derived(problems.filter((ts) => !ts.optional));
</script>

<Modal title={t("toolStatus.title")} width="560px" {onclose}>
  {#snippet children()}
    <div class="tools-body">
      <p class="tools-intro">
        {blocking.length > 0 ? t("toolStatus.introBlocking") : t("toolStatus.introOptional")}
      </p>

      <div class="tools-list">
        {#each app.toolStatus as tool}
          {@const ok = tool.runtime.available}
          <div class="tool-row" class:tool-row--ok={ok} class:tool-row--bad={!ok && !tool.optional} class:tool-row--warn={!ok && tool.optional}>
            <span class="tool-icon" aria-hidden="true">
              {#if ok}
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="20 6 9 17 4 12"/></svg>
              {:else}
                <svg width="16" height="16" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>
              {/if}
            </span>
            <div class="tool-main">
              <div class="tool-head">
                <span class="tool-name">{tool.displayName}</span>
                {#if ok}
                  <span class="tool-badge tool-badge--ok">{t("toolStatus.ready")}</span>
                {:else if tool.optional}
                  <span class="tool-badge tool-badge--warn">{t("toolStatus.optional")}</span>
                {:else}
                  <span class="tool-badge tool-badge--bad">{t("toolStatus.required")}</span>
                {/if}
              </div>
              {#if tool.enables.length > 0}
                <div class="tool-caps">{t("toolStatus.enables", { caps: capabilityLabels(tool.enables) })}</div>
              {/if}
              {#if !ok}
                <div class="tool-msg">
                  {tool.optional ? t("toolStatus.disabledOptional") : t("toolStatus.disabledRequired")}
                </div>
                {#if tool.runtime.error}
                  <div class="tool-err">{tool.runtime.error}</div>
                {/if}
                {#if tool.guidanceUrl}
                  <button class="tool-link" onclick={() => openUrl(tool.guidanceUrl!)}>
                    {t("toolStatus.howToGet")}
                  </button>
                {/if}
              {/if}
            </div>
          </div>
        {/each}
      </div>

      <p class="tools-foot">{t("toolStatus.continueHint")}</p>
    </div>
  {/snippet}

  {#snippet footer()}
    <Button variant="primary" onclick={onclose}>{t("toolStatus.continue")}</Button>
  {/snippet}
</Modal>

<style>
  .tools-body { display: flex; flex-direction: column; gap: 14px; }

  .tools-intro {
    margin: 0;
    font-size: 13px;
    line-height: 1.45;
    color: var(--text-muted);
  }

  .tools-list { display: flex; flex-direction: column; gap: 8px; }

  .tool-row {
    display: flex;
    gap: 10px;
    padding: 10px 12px;
    border: 1px solid var(--line);
    border-radius: 10px;
    background: var(--surface);
  }
  .tool-row--ok   { border-color: color-mix(in srgb, #10b981 30%, var(--line)); }
  .tool-row--warn { border-color: color-mix(in srgb, #f59e0b 34%, var(--line)); background: color-mix(in srgb, #f59e0b 6%, var(--surface)); }
  .tool-row--bad  { border-color: color-mix(in srgb, #ef4444 34%, var(--line)); background: color-mix(in srgb, #ef4444 6%, var(--surface)); }

  .tool-icon {
    flex-shrink: 0;
    width: 20px;
    display: flex;
    justify-content: center;
    padding-top: 1px;
  }
  .tool-row--ok   .tool-icon { color: #10b981; }
  .tool-row--warn .tool-icon { color: #d97706; }
  .tool-row--bad  .tool-icon { color: #ef4444; }

  .tool-main { display: flex; flex-direction: column; gap: 3px; min-width: 0; flex: 1; }

  .tool-head { display: flex; align-items: center; gap: 8px; }

  .tool-name { font-size: 13px; font-weight: 650; color: var(--text); }

  .tool-badge {
    font-size: 10px;
    font-weight: 800;
    text-transform: uppercase;
    letter-spacing: 0.04em;
    padding: 2px 7px;
    border-radius: 999px;
  }
  .tool-badge--ok   { color: #059669; background: color-mix(in srgb, #10b981 14%, transparent); }
  .tool-badge--warn { color: #b45309; background: color-mix(in srgb, #f59e0b 16%, transparent); }
  .tool-badge--bad  { color: #dc2626; background: color-mix(in srgb, #ef4444 14%, transparent); }

  .tool-caps { font-size: 11.5px; color: var(--text-subtle); }

  .tool-msg { font-size: 12px; color: var(--text-muted); line-height: 1.4; }

  .tool-err {
    font-size: 11px;
    color: var(--text-subtle);
    font-family: var(--font-mono, monospace);
    word-break: break-word;
  }

  .tool-link {
    align-self: flex-start;
    margin-top: 2px;
    padding: 0;
    background: none;
    border: none;
    color: var(--accent);
    font: inherit;
    font-size: 12px;
    font-weight: 600;
    cursor: pointer;
    text-decoration: underline;
  }

  .tools-foot {
    margin: 0;
    font-size: 11.5px;
    color: var(--text-subtle);
    line-height: 1.4;
  }
</style>
