<script lang="ts">
  import Modal from "../ui/Modal.svelte";
  import Button from "../ui/Button.svelte";
  import { t } from "../../i18n/index.js";

  type Props = {
    fingerprint: string;
    profileLabel: string;
    onTrust: () => void;
    onReject: () => void;
  };

  let { fingerprint, profileLabel, onTrust, onReject }: Props = $props();

  let copied = $state(false);

  async function copyFingerprint() {
    await navigator.clipboard.writeText(fingerprint);
    copied = true;
    setTimeout(() => { copied = false; }, 2000);
  }
</script>

<Modal title={t("fingerprintDialog.title")} width="480px" onclose={onReject}>
  {#snippet children()}
    <div class="fp-body">
      <div class="fp-icon" aria-hidden="true">
        <svg width="38" height="38" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round"
            d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z"
          />
        </svg>
      </div>
      <div class="fp-content">
        <p class="fp-intro">
          {@html t("fingerprintDialog.intro", { host: `<strong>${profileLabel}</strong>` })}
        </p>
        <p class="fp-sub">{t("fingerprintDialog.subLabel")}</p>
        <div class="fp-box">
          <code class="fp-text">{fingerprint}</code>
          <button class="fp-copy" onclick={copyFingerprint} title={t("fingerprintDialog.copyFingerprint")} aria-label={t("fingerprintDialog.copyFingerprint")}>
            {#if copied}
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5">
                <polyline points="20 6 9 17 4 12"/>
              </svg>
            {:else}
              <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
                <rect x="9" y="9" width="13" height="13" rx="2"/>
                <path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/>
              </svg>
            {/if}
          </button>
        </div>
        <p class="fp-warn">
          {t("fingerprintDialog.warning")}
        </p>
      </div>
    </div>
  {/snippet}

  {#snippet footer()}
    <Button variant="ghost" onclick={onReject}>{t("fingerprintDialog.reject")}</Button>
    <Button variant="primary" onclick={onTrust}>{t("fingerprintDialog.trustAndConnect")}</Button>
  {/snippet}
</Modal>

<style>
  .fp-body {
    display: flex;
    gap: 14px;
    align-items: flex-start;
  }

  .fp-icon {
    flex-shrink: 0;
    color: #e0a040;
    margin-top: 2px;
  }

  .fp-content {
    flex: 1;
    min-width: 0;
  }

  .fp-intro {
    margin: 0 0 10px;
    font-size: 13px;
    color: var(--text);
    line-height: 1.5;
  }

  .fp-sub {
    margin: 0 0 6px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
  }

  .fp-box {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    background: var(--surface-alt);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 10px 10px 10px 12px;
    margin-bottom: 12px;
  }

  .fp-text {
    flex: 1;
    min-width: 0;
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.6;
    color: var(--accent);
    word-break: break-all;
    user-select: all;
    white-space: pre-wrap;
  }

  .fp-copy {
    flex-shrink: 0;
    width: 26px;
    height: 26px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: 1px solid var(--line-strong);
    border-radius: 5px;
    color: var(--text-muted);
    cursor: pointer;
    transition: background 0.1s, color 0.1s, border-color 0.1s;
    margin-top: 1px;

    &:hover {
      background: var(--surface-hover);
      color: var(--text);
    }
  }

  .fp-warn {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
    line-height: 1.5;
  }
</style>
