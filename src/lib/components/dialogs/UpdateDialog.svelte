<script lang="ts">
  import { openUrl as tauriOpenUrl } from "@tauri-apps/plugin-opener";
  import { app } from "../../stores/app.svelte.js";
  import { t } from "../../i18n/index.js";
  import Modal from "../ui/Modal.svelte";
  import Button from "../ui/Button.svelte";

  const update = $derived(app.availableUpdate);

  function progressLabel(): string {
    const progress = app.updateDownloadProgress;
    if (!progress) return t("updateDialog.preparing");
    if (progress.percent !== null) return `${progress.percent}%`;
    return t("updateDialog.downloading");
  }

  async function openRelease(url: string) {
    try {
      await tauriOpenUrl(url);
      app.dismissUpdateNotice();
    } catch (error) {
      app.notify("error", String(error));
    }
  }
</script>

{#if app.updateNoticeOpen && update}
  <Modal title={update.isDowngrade ? t("updateDialog.downgradeTitle") : t("updateDialog.title")} width="500px" onclose={() => app.dismissUpdateNotice()}>
    {#snippet children()}
      <div class="update-body">
        <div class="update-version-row">
          <span class={`update-channel update-channel--${update.channel}`}>
            {update.channel === "beta" ? t("updates.channel.beta") : t("updates.channel.stable")}
          </span>
          <span class="update-version">{update.currentVersion} -> {update.version}</span>
        </div>

        <p class="update-copy">
          {#if !update.canInstall}
            {t("updateDialog.manualBody")}
          {:else if update.isDowngrade}
            {t("updateDialog.downgradeBody")}
          {:else if update.channel === "beta"}
            {t("updateDialog.betaBody")}
          {:else}
            {t("updateDialog.body")}
          {/if}
        </p>

        {#if update.body}
          <div class="update-notes">
            {update.body}
          </div>
        {/if}

        {#if app.updateInstalling}
          <div class="update-progress" aria-label={progressLabel()}>
            <div class="update-progress-track">
              <div
                class="update-progress-fill"
                style:width={app.updateDownloadProgress?.percent !== null && app.updateDownloadProgress?.percent !== undefined ? `${app.updateDownloadProgress.percent}%` : "35%"}
              ></div>
            </div>
            <span>{progressLabel()}</span>
          </div>
        {/if}

        {#if app.updateError}
          <p class="update-error">{app.updateError}</p>
        {/if}
      </div>
    {/snippet}

    {#snippet footer()}
      <Button variant="ghost" onclick={() => app.dismissUpdateNotice()} disabled={app.updateInstalling}>
        {t("updateDialog.later")}
      </Button>
      {#if update.canInstall}
        <Button variant="primary" onclick={() => app.installAvailableUpdate()} disabled={app.updateInstalling}>
          {app.updateInstalling ? t("updateDialog.installing") : t("updateDialog.install")}
        </Button>
      {:else}
        <Button variant="primary" onclick={() => void openRelease(update.manualUrl)}>
          {t("updateDialog.openRelease")}
        </Button>
      {/if}
    {/snippet}
  </Modal>
{/if}

<style>
  .update-body {
    display: flex;
    flex-direction: column;
    gap: 12px;
  }

  .update-version-row {
    display: flex;
    align-items: center;
    gap: 8px;
    flex-wrap: wrap;
  }

  .update-channel {
    padding: 3px 8px;
    border-radius: 999px;
    font-size: 10.5px;
    font-weight: 800;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .update-channel--stable {
    color: var(--accent);
    background: color-mix(in srgb, var(--accent) 14%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent) 28%, transparent);
  }

  .update-channel--beta {
    color: #b45309;
    background: color-mix(in srgb, #f59e0b 14%, transparent);
    border: 1px solid color-mix(in srgb, #f59e0b 32%, transparent);
  }

  .update-version {
    color: var(--text);
    font-size: 13px;
    font-weight: 700;
  }

  .update-copy {
    margin: 0;
    color: var(--text-muted);
    font-size: 12.5px;
    line-height: 1.5;
  }

  .update-notes {
    max-height: 170px;
    overflow: auto;
    padding: 9px 10px;
    border-radius: 7px;
    border: 1px solid var(--line);
    background: var(--surface-alt);
    color: var(--text-muted);
    font-size: 12px;
    line-height: 1.45;
    white-space: pre-wrap;
  }

  .update-progress {
    display: flex;
    align-items: center;
    gap: 9px;
    color: var(--text-muted);
    font-size: 12px;
    font-weight: 700;
  }

  .update-progress-track {
    flex: 1;
    height: 7px;
    border-radius: 999px;
    overflow: hidden;
    background: var(--surface-alt);
    border: 1px solid var(--line);
  }

  .update-progress-fill {
    height: 100%;
    border-radius: inherit;
    background: var(--accent);
    transition: width 120ms ease;
  }

  .update-error {
    margin: 0;
    color: var(--danger, #e5484d);
    font-size: 12px;
    line-height: 1.45;
  }
</style>
