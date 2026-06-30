<script lang="ts">
  import Modal from "../ui/Modal.svelte";
  import Button from "../ui/Button.svelte";

  type Props = {
    fingerprint: string;
    profileLabel: string;
    onTrust: () => void;
    onReject: () => void;
  };

  let { fingerprint, profileLabel, onTrust, onReject }: Props = $props();

  // Format SHA256 fingerprint as colon-separated hex pairs for readability
  const formatted = $derived(() => {
    const clean = fingerprint.replace(/^SHA256:/i, "").replace(/[^a-zA-Z0-9+/=]/g, "");
    // If it looks like base64, display as-is with the prefix; otherwise format hex
    if (fingerprint.startsWith("SHA256:")) return fingerprint;
    // Hex: group into pairs separated by colons
    const hex = clean.toLowerCase();
    return hex.match(/.{1,2}/g)?.join(":") ?? fingerprint;
  });

  // Split into rows of 8 pairs for visual alignment
  const fingerprintRows = $derived(() => {
    const pairs = formatted().replace(/^SHA256:/i, "").split(":").filter(Boolean);
    const rows: string[][] = [];
    for (let i = 0; i < pairs.length; i += 8) {
      rows.push(pairs.slice(i, i + 8));
    }
    return rows;
  });
</script>

<Modal title="Unknown host key" width="500px" onclose={onReject}>
  {#snippet children()}
    <div class="fp-body">
      <div class="fp-icon" aria-hidden="true">
        <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.5">
          <path stroke-linecap="round" stroke-linejoin="round"
            d="M12 9v3.75m-9.303 3.376c-.866 1.5.217 3.374 1.948 3.374h14.71c1.73 0 2.813-1.874 1.948-3.374L13.949 3.378c-.866-1.5-3.032-1.5-3.898 0L2.697 16.126zM12 15.75h.007v.008H12v-.008z"
          />
        </svg>
      </div>
      <div class="fp-content">
        <p class="fp-intro">
          The authenticity of host <strong>{profileLabel}</strong> cannot be established.
        </p>
        <p class="fp-sub">SSH host key fingerprint:</p>
        <div class="fp-box">
          {#if fingerprintRows().length > 0}
            {#each fingerprintRows() as row}
              <div class="fp-row">
                {#each row as pair, i}
                  <span class="fp-pair" class:fp-pair--sep={i > 0}>{pair}</span>
                {/each}
              </div>
            {/each}
          {:else}
            <span class="fp-raw">{fingerprint}</span>
          {/if}
        </div>
        <p class="fp-warn">
          If you trust this fingerprint, future connections to this host will be verified
          against it. Reject if you did not expect to connect to this server.
        </p>
      </div>
    </div>
  {/snippet}

  {#snippet footer()}
    <Button variant="ghost" onclick={onReject}>Reject</Button>
    <Button variant="primary" onclick={onTrust}>Trust &amp; Connect</Button>
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
    font-size: 12px;
    color: var(--text-muted);
    font-weight: 500;
    text-transform: uppercase;
    letter-spacing: 0.04em;
  }

  .fp-box {
    background: var(--surface-alt);
    border: 1px solid var(--line);
    border-radius: 6px;
    padding: 10px 12px;
    margin-bottom: 12px;
    font-family: var(--font-mono);
    font-size: 12px;
    line-height: 1.7;
    color: var(--text);
  }

  .fp-row {
    display: flex;
    flex-wrap: wrap;
    gap: 0;
  }

  .fp-pair {
    color: var(--accent);
  }

  .fp-pair--sep::before {
    content: ":";
    color: var(--text-muted);
  }

  .fp-raw {
    word-break: break-all;
    color: var(--accent);
  }

  .fp-warn {
    margin: 0;
    font-size: 12px;
    color: var(--text-muted);
    line-height: 1.5;
  }
</style>
