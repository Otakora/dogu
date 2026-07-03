<script lang="ts">
  import { app } from "../../stores/app.svelte.js";
</script>

<div class="notif-stack" aria-live="polite" aria-label="Notifications">
  {#each app.notifications as n (n.id)}
    <div class="notif notif--{n.kind}" role="alert">
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
    top: 16px;
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
    box-shadow: 0 4px 12px rgba(0,0,0,0.25);
  }

  .notif--success {
    background: color-mix(in srgb, var(--accent) 18%, var(--surface));
    border-color: color-mix(in srgb, var(--accent) 40%, var(--line));
    color: var(--accent);
  }

  .notif--error {
    background: color-mix(in srgb, var(--danger, #e5484d) 18%, var(--surface));
    border-color: color-mix(in srgb, var(--danger, #e5484d) 40%, var(--line));
    color: var(--danger, #e5484d);
  }

  .notif--warn {
    background: color-mix(in srgb, #f59e0b 14%, var(--surface));
    border-color: color-mix(in srgb, #f59e0b 40%, var(--line));
    color: #92400e;
  }

  .notif--info {
    background: color-mix(in srgb, var(--text) 8%, var(--surface));
    border-color: var(--line-strong, var(--line));
    color: var(--text);
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

  @keyframes fade-in {
    from { opacity: 0; transform: translateY(-6px); }
    to   { opacity: 1; transform: translateY(0); }
  }
</style>
