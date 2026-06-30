<script lang="ts">
  import { app } from "./lib/stores/app.svelte.js";
  import ConnectionManager from "./lib/components/connections/ConnectionManager.svelte";
  import FingerprintDialog from "./lib/components/connections/FingerprintDialog.svelte";
</script>

<div class="app-layout">
  <!-- ── Connection Manager side panel ── -->
  {#if app.connectionManagerOpen}
    <ConnectionManager />
  {/if}

  <!-- ── Main content area (Shell — Phase 3) ── -->
  <div class="app-main">
    <div class="app-placeholder">
      <div class="app-placeholder-inner">
        <svg width="32" height="32" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2" style="color:var(--text-subtle)">
          <rect x="2" y="3" width="20" height="14" rx="2"/>
          <path d="M8 21h8M12 17v4"/>
        </svg>
        <span>Dogu — file manager shell coming in Phase 3</span>
        <button
          class="open-cm-btn"
          onclick={() => app.openConnectionManager()}
        >
          Open Connection Manager
        </button>
      </div>
    </div>
  </div>

  <!-- ── SSH fingerprint trust overlay ── -->
  {#if app.fingerprintPrompt}
    <FingerprintDialog
      fingerprint={app.fingerprintPrompt.fingerprint}
      profileLabel={app.fingerprintPrompt.profileLabel}
      onTrust={() => app.resolveFingerprint(true)}
      onReject={() => app.resolveFingerprint(false)}
    />
  {/if}
</div>

<style>
  .app-layout {
    display: flex;
    height: 100vh;
    overflow: hidden;
    background: var(--bg);
  }

  .app-main {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    display: flex;
    flex-direction: column;
  }

  .app-placeholder {
    flex: 1;
    display: flex;
    align-items: center;
    justify-content: center;
  }

  .app-placeholder-inner {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 10px;
    color: var(--text-muted);
    font-size: 13px;
  }

  .open-cm-btn {
    margin-top: 4px;
    padding: 6px 14px;
    border-radius: 6px;
    border: 1px solid var(--accent-border);
    background: var(--accent-soft);
    color: var(--accent);
    font: inherit;
    font-size: 13px;
    cursor: pointer;

    &:hover {
      background: var(--accent);
      color: #fff;
    }
  }
</style>
