<script lang="ts">
  import { onMount, onDestroy } from "svelte";

  type Props = {
    title?: string;
    width?: string;
    onclose?: () => void;
    children: import("svelte").Snippet;
    footer?: import("svelte").Snippet;
  };

  let { title, width = "480px", onclose, children, footer }: Props = $props();

  function handleBackdrop(e: MouseEvent) {
    if (e.target === e.currentTarget) onclose?.();
  }

  function handleKey(e: KeyboardEvent) {
    if (e.key === "Escape") onclose?.();
  }

  onMount(() => { document.addEventListener("keydown", handleKey); });
  onDestroy(() => { document.removeEventListener("keydown", handleKey); });
</script>

<div
  class="modal-backdrop"
  role="presentation"
  onclick={handleBackdrop}
  onkeydown={undefined}
>
  <div class="modal-box" style="width:{width}">
    {#if title}
      <div class="modal-header">
        <span class="modal-title">{title}</span>
        {#if onclose}
          <button class="modal-close" onclick={onclose} aria-label="Close">✕</button>
        {/if}
      </div>
    {/if}
    <div class="modal-body">
      {@render children()}
    </div>
    {#if footer}
      <div class="modal-footer">
        {@render footer()}
      </div>
    {/if}
  </div>
</div>

<style>
  .modal-backdrop {
    position: fixed;
    inset: 0;
    background: rgba(0, 0, 0, 0.45);
    display: flex;
    align-items: center;
    justify-content: center;
    z-index: 1000;
    backdrop-filter: blur(2px);
    animation: fade-in 0.12s ease;
  }

  .modal-box {
    background: var(--surface);
    border: 1px solid var(--line-strong);
    border-radius: 10px;
    box-shadow: var(--shadow-lg);
    max-width: calc(100vw - 48px);
    max-height: calc(100vh - 80px);
    display: flex;
    flex-direction: column;
    animation: slide-in-right 0.15s ease;
  }

  .modal-header {
    display: flex;
    align-items: center;
    gap: 8px;
    padding: 14px 16px 12px;
    border-bottom: 1px solid var(--line);
    flex-shrink: 0;
  }

  .modal-title {
    font-size: 14px;
    font-weight: 600;
    color: var(--text);
    flex: 1;
  }

  .modal-close {
    background: none;
    border: none;
    color: var(--text-muted);
    font-size: 14px;
    padding: 2px 6px;
    border-radius: 4px;
    line-height: 1;

    &:hover {
      background: var(--surface-hover);
      color: var(--text);
    }
  }

  .modal-body {
    padding: 16px;
    overflow-y: auto;
    flex: 1;
    scrollbar-width: thin;
    scrollbar-color: var(--line-strong) transparent;
  }

  .modal-footer {
    padding: 12px 16px;
    border-top: 1px solid var(--line);
    display: flex;
    justify-content: flex-end;
    gap: 8px;
    flex-shrink: 0;
  }
</style>
