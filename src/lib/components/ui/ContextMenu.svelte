<script lang="ts">
  import { onMount, onDestroy, untrack } from "svelte";

  export type MenuItem =
    | { kind: "action"; label: string; icon?: string; shortcut?: string; disabled?: boolean; danger?: boolean; onclick: () => void }
    | { kind: "separator" }
    | { kind: "submenu"; label: string; icon?: string; children: MenuItem[] };

  type Props = {
    x: number;
    y: number;
    items: MenuItem[];
    onclose: () => void;
  };

  let { x, y, items, onclose }: Props = $props();

  let menuEl = $state<HTMLElement | undefined>(undefined);
  // Snapshot initial position; context menus do not reposition when props change.
  let adjustedX = $state(untrack(() => x));
  let adjustedY = $state(untrack(() => y));

  $effect(() => {
    if (!menuEl) return;
    const rect = menuEl.getBoundingClientRect();
    const vw = window.innerWidth;
    const vh = window.innerHeight;
    adjustedX = rect.right > vw ? Math.max(0, x - rect.width) : x;
    adjustedY = rect.bottom > vh ? Math.max(0, y - rect.height) : y;
  });

  function handleOutsideClick(e: MouseEvent) {
    if (menuEl && !menuEl.contains(e.target as Node)) onclose();
  }

  function handleKey(e: KeyboardEvent) {
    if (e.key === "Escape") onclose();
  }

  onMount(() => {
    document.addEventListener("mousedown", handleOutsideClick, true);
    document.addEventListener("keydown", handleKey, true);
  });

  onDestroy(() => {
    document.removeEventListener("mousedown", handleOutsideClick, true);
    document.removeEventListener("keydown", handleKey, true);
  });
</script>

{#snippet menuItems(list: MenuItem[])}
  {#each list as item}
    {#if item.kind === "separator"}
      <div class="ctx-sep" role="separator"></div>
    {:else if item.kind === "action"}
      <button
        class="ctx-item"
        class:ctx-item--danger={item.danger}
        disabled={item.disabled}
        role="menuitem"
        onclick={() => { item.onclick(); onclose(); }}
      >
        {#if item.icon}
          <span class="ctx-icon" aria-hidden="true">{@html item.icon}</span>
        {:else}
          <span class="ctx-icon-placeholder"></span>
        {/if}
        <span class="ctx-label">{item.label}</span>
        {#if item.shortcut}
          <span class="ctx-shortcut">{item.shortcut}</span>
        {/if}
      </button>
    {:else if item.kind === "submenu"}
      <div class="ctx-submenu">
        <button class="ctx-item ctx-item--submenu" role="menuitem" aria-haspopup="menu">
          {#if item.icon}
            <span class="ctx-icon" aria-hidden="true">{@html item.icon}</span>
          {:else}
            <span class="ctx-icon-placeholder"></span>
          {/if}
          <span class="ctx-label">{item.label}</span>
          <span class="ctx-chevron">&gt;</span>
        </button>
        <div class="ctx-menu ctx-menu--nested" role="menu">
          {@render menuItems(item.children)}
        </div>
      </div>
    {/if}
  {/each}
{/snippet}

<div
  class="ctx-menu"
  style="left:{adjustedX}px;top:{adjustedY}px"
  bind:this={menuEl}
  role="menu"
  aria-label="Context menu"
>
  {@render menuItems(items)}
</div>

<style>
  .ctx-menu {
    position: fixed;
    z-index: 3000;
    background: var(--surface);
    border: 1px solid var(--line-strong);
    border-radius: 8px;
    padding: 4px;
    min-width: 180px;
    box-shadow: var(--shadow-lg);
    animation: fade-in 0.1s ease;
  }

  .ctx-menu--nested {
    display: none;
    position: absolute;
    left: calc(100% - 2px);
    top: -4px;
  }

  .ctx-submenu {
    position: relative;

    &:hover > .ctx-menu--nested,
    &:focus-within > .ctx-menu--nested {
      display: block;
    }
  }

  .ctx-sep {
    height: 1px;
    background: var(--line);
    margin: 3px 6px;
  }

  .ctx-item {
    display: flex;
    align-items: center;
    gap: 8px;
    width: 100%;
    padding: 5px 8px;
    border: none;
    border-radius: 5px;
    background: none;
    color: var(--text);
    font-size: 13px;
    text-align: left;
    cursor: pointer;
    white-space: nowrap;

    &:hover:not(:disabled) {
      background: var(--surface-hover);
    }

    &:disabled {
      opacity: 0.4;
      cursor: default;
    }
  }

  .ctx-item--danger {
    color: var(--danger);
    &:hover:not(:disabled) {
      background: var(--danger-soft);
    }
  }

  .ctx-icon {
    flex-shrink: 0;
    display: flex;
    color: var(--text-muted);
    width: 16px;
    justify-content: center;
  }

  .ctx-icon-placeholder {
    width: 16px;
    flex-shrink: 0;
  }

  .ctx-label {
    flex: 1;
  }

  .ctx-shortcut {
    font-size: 11px;
    color: var(--text-subtle);
    margin-left: 16px;
  }

  .ctx-chevron {
    color: var(--text-subtle);
    margin-left: 14px;
    font-size: 13px;
    line-height: 1;
  }
</style>
