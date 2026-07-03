<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import type { EntryDto } from "../../types/index.js";
  import TreeNode from "./TreeNode.svelte";
  import { t } from "../../i18n/index.js";

  export type TabHighlight = { path: string | null; color: string; isActive: boolean };

  type Props = {
    entry: EntryDto;
    depth?: number;
    iconVariant?: "folder" | "remote";
    tabHighlights?: TabHighlight[];
    onNavigate?: (path: string) => void;
    onContextMenu?: (e: MouseEvent, entry: EntryDto) => void;
  };

  let { entry, depth = 0, iconVariant = "folder", tabHighlights = [], onNavigate, onContextMenu }: Props = $props();

  let expanded = $state(false);
  let children = $state<EntryDto[]>([]);
  let loading = $state(false);
  let loaded = $state(false);

  // Find best matching tab color for this node.
  // Priority: exact match > ancestor match; active > inactive at same match type.
  const highlight = $derived((() => {
    let bestColor: string | null = null;
    let bestScore = -1;
    for (const tab of tabHighlights) {
      if (!tab.path) continue;
      const isExact = tab.path === entry.path;
      const isAncestor = !isExact && (
        tab.path.startsWith(entry.path + "/") ||
        tab.path.startsWith(entry.path + "\\")
      );
      if (!isExact && !isAncestor) continue;
      // Score: exact(3) > ancestor(0); active(+1) > inactive(+0)
      const score = (isExact ? 3 : 0) + (tab.isActive ? 1 : 0);
      if (score > bestScore) { bestScore = score; bestColor = tab.color; }
    }
    return bestColor;
  })());

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
    onNavigate?.(entry.path);
  }
</script>

<div class="tree-node" style="--depth:{depth}">
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="tree-row"
    class:tree-row--highlighted={!!highlight}
    style:--tab-color={highlight ?? "transparent"}
    role="treeitem"
    aria-selected={!!highlight}
    aria-expanded={hasChildren ? expanded : undefined}
    tabindex="0"
    onclick={navigate}
    onkeydown={(e) => e.key === "Enter" && navigate(e as unknown as MouseEvent)}
    oncontextmenu={(e) => { e.preventDefault(); e.stopPropagation(); onContextMenu?.(e, entry); }}
  >
    <span class="tree-indent"></span>

    <!-- Expand arrow -->
    <button
      class="tree-arrow"
      class:tree-arrow--expanded={expanded}
      class:tree-arrow--hidden={!hasChildren}
      onclick={toggle}
      tabindex="-1"
      aria-label={expanded ? t("treeNode.collapse") : t("treeNode.expand")}
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
      {#if iconVariant === "remote" && depth === 0}
        <svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.8" opacity="0.85">
          <rect x="2" y="3" width="20" height="14" rx="2"/>
          <path d="M8 21h8M12 17v4"/>
        </svg>
      {:else}
        <svg width="14" height="14" viewBox="0 0 24 24" fill="currentColor" opacity="0.7">
          <path d="M10 4H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2h-8l-2-2z"/>
        </svg>
      {/if}
    </span>

    <span class="tree-label">{entry.name}</span>
  </div>

  <!-- Children -->
  {#if expanded && children.length > 0}
    <div class="tree-children" role="group">
      {#each children as child (child.path)}
        <TreeNode entry={child} depth={depth + 1} {tabHighlights} {onNavigate} {onContextMenu} />
      {/each}
    </div>
  {/if}

  {#if expanded && loaded && children.length === 0}
    <div class="tree-empty" style="padding-left:calc({depth + 1} * 16px + 32px)">
      {t("treeNode.empty")}
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
    height: calc(26px * var(--tree-scale, 1));
    padding-left: calc(var(--depth, 0) * 14px + 4px);
    padding-right: 8px;
    border-radius: 5px;
    cursor: pointer;
    color: var(--text);
    border-left: 2px solid transparent;

    &:hover {
      background: var(--surface-hover);
    }
  }

  .tree-row--highlighted {
    background: color-mix(in srgb, var(--tab-color) 13%, transparent);
    color: var(--tab-color);
    border-left-color: var(--tab-color);

    .tree-icon { color: var(--tab-color); }

    &:hover {
      background: color-mix(in srgb, var(--tab-color) 20%, transparent);
    }
  }

  .tree-indent {
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
    font-size: calc(13px * var(--tree-scale, 1));
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
</style>
