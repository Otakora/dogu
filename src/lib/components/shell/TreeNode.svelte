<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { app } from "../../stores/app.svelte.js";
  import type { EntryDto } from "../../types/index.js";
  import TreeNode from "./TreeNode.svelte";

  type Props = {
    entry: EntryDto;
    depth?: number;
    onNavigate?: (path: string) => void;
  };

  let { entry, depth = 0, onNavigate }: Props = $props();

  let expanded = $state(false);
  let children = $state<EntryDto[]>([]);
  let loading = $state(false);
  let loaded = $state(false);

  const isActive = $derived(
    app.currentPath === entry.path ||
    (app.currentPath?.startsWith(entry.path + "/") ?? false) ||
    (app.currentPath?.startsWith(entry.path + "\\") ?? false)
  );

  const hasChildren = $derived(entry.hasDirectoryChildren);

  async function toggle(e: MouseEvent) {
    e.stopPropagation();
    if (!hasChildren) return;
    if (!expanded && !loaded) {
      loading = true;
      try {
        const all = await invoke<EntryDto[]>("list_children", { path: entry.path });
        children = all.filter(c => c.isDir);
        loaded = true;
      } catch {
        children = [];
        loaded = true;
      } finally {
        loading = false;
      }
    }
    expanded = !expanded;
  }

  function navigate(e: MouseEvent) {
    e.stopPropagation();
    if (onNavigate) {
      onNavigate(entry.path);
    } else {
      app.navigate(entry.path);
    }
  }
</script>

<div class="tree-node" style="--depth:{depth}">
  <div
    class="tree-row"
    class:tree-row--active={isActive}
    role="treeitem"
    aria-selected={isActive}
    aria-expanded={hasChildren ? expanded : undefined}
    tabindex="0"
    onclick={navigate}
    onkeydown={(e) => e.key === "Enter" && navigate(e as unknown as MouseEvent)}
  >
    <span class="tree-indent"></span>

    <!-- Expand arrow -->
    <button
      class="tree-arrow"
      class:tree-arrow--expanded={expanded}
      class:tree-arrow--hidden={!hasChildren}
      onclick={toggle}
      tabindex="-1"
      aria-label={expanded ? "Collapse" : "Expand"}
    >
      {#if loading}
        <span class="tree-spinner"></span>
      {:else}
        <svg width="10" height="10" viewBox="0 0 24 24" fill="currentColor">
          <path d="M8 5l8 7-8 7V5z"/>
        </svg>
      {/if}
    </button>

    <!-- Icon -->
    <span class="tree-icon" aria-hidden="true">
      <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" opacity="0.7">
        <path d="M10 4H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2h-8l-2-2z"/>
      </svg>
    </span>

    <span class="tree-label">{entry.name}</span>
  </div>

  <!-- Children -->
  {#if expanded && children.length > 0}
    <div class="tree-children" role="group">
      {#each children as child (child.path)}
        <TreeNode entry={child} depth={depth + 1} {onNavigate} />
      {/each}
    </div>
  {/if}

  {#if expanded && loaded && children.length === 0}
    <div class="tree-empty" style="padding-left:calc({depth + 1} * 16px + 32px)">
      Empty
    </div>
  {/if}
</div>

<style>
  .tree-node {
    user-select: none;
  }

  .tree-row {
    display: flex;
    align-items: center;
    gap: 2px;
    height: 26px;
    padding-left: calc(var(--depth, 0) * 14px + 4px);
    padding-right: 8px;
    border-radius: 5px;
    cursor: pointer;
    color: var(--text);

    &:hover {
      background: var(--surface-hover);
    }
  }

  .tree-row--active {
    background: var(--accent-soft);
    color: var(--accent);

    .tree-icon { color: var(--accent); }
  }

  .tree-indent {
    /* handled by padding-left above */
    display: none;
  }

  .tree-arrow {
    flex-shrink: 0;
    width: 18px;
    height: 18px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: none;
    color: var(--text-subtle);
    border-radius: 3px;
    cursor: pointer;
    padding: 0;
    transition: transform 0.15s;

    &:hover {
      background: var(--surface-active);
      color: var(--text);
    }
  }

  .tree-arrow--expanded {
    transform: rotate(90deg);
  }

  .tree-arrow--hidden {
    visibility: hidden;
    pointer-events: none;
  }

  .tree-icon {
    flex-shrink: 0;
    display: flex;
    color: #e0a030;
  }

  .tree-label {
    flex: 1;
    min-width: 0;
    font-size: 13px;
    white-space: nowrap;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .tree-spinner {
    display: inline-block;
    width: 9px;
    height: 9px;
    border: 1.5px solid transparent;
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.6s linear infinite;
  }

  .tree-empty {
    font-size: 11px;
    color: var(--text-subtle);
    padding-top: 2px;
    padding-bottom: 2px;
  }

  /* .tree-children — depth handled by children's own padding-left */
</style>
