<script lang="ts">
  import { app } from "../../stores/app.svelte.js";

  const ICONS = {
    success: `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><polyline points="20 6 9 17 4 12"/></svg>`,
    error:   `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.5"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>`,
    info:    `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>`,
  };
</script>

<div class="notif-stack" aria-live="polite" aria-label="Notifications">
  {#each app.notifications as n (n.id)}
    <div class="notif notif--{n.kind}" role="alert">
      <span class="notif-icon" aria-hidden="true">{@html ICONS[n.kind]}</span>
      <span class="notif-text">{n.text}</span>
      <button
        class="notif-dismiss"
        onclick={() => app.dismissNotification(n.id)}
        aria-label="Dismiss"
      >✕</button>
    </div>
  {/each}
</div>

<style>
  .notif-stack {
    position: fixed;
    bottom: 16px;
    right: 16px;
    z-index: 2000;
    display: flex;
    flex-direction: column;
    gap: 6px;
    pointer-events: none;
    max-width: 380px;
  }

  .notif {
    display: flex;
    align-items: flex-start;
    gap: 8px;
    padding: 9px 10px 9px 12px;
    border-radius: 8px;
    border: 1px solid;
    font-size: 13px;
    line-height: 1.4;
    pointer-events: auto;
    animation: fade-in 0.18s ease;
    box-shadow: var(--shadow);
  }

  .notif--success {
    background: var(--accent-soft);
    border-color: var(--accent-border);
    color: var(--accent);
  }

  .notif--error {
    background: var(--danger-soft);
    border-color: var(--danger-border);
    color: var(--danger);
  }

  .notif--info {
    background: var(--surface);
    border-color: var(--line-strong);
    color: var(--text);
  }

  .notif-icon {
    flex-shrink: 0;
    margin-top: 1px;
    display: flex;
  }

  .notif-text {
    flex: 1;
    min-width: 0;
    word-break: break-word;
  }

  .notif-dismiss {
    flex-shrink: 0;
    background: none;
    border: none;
    font-size: 11px;
    color: inherit;
    opacity: 0.5;
    padding: 0 2px;
    cursor: pointer;
    line-height: 1;
    margin-top: 1px;

    &:hover { opacity: 1; }
  }
</style>
