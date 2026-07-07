<script lang="ts">
  import { getContext, onDestroy } from "svelte";
  import { app } from "../../stores/app.svelte.js";
  import type { PaneView } from "../../stores/app.svelte.js";
  import { t } from "../../i18n/index.js";
  import ContextMenu from "../ui/ContextMenu.svelte";
  import type { MenuItem } from "../ui/ContextMenu.svelte";

  const pane = getContext<PaneView>("pane");

  let contextMenu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

  // ── Drag-to-reorder / drag-to-other-pane ─────────────────────
  type DragState = { fromIdx: number; startX: number; active: boolean };
  let drag: DragState | null = null;
  let draggedIdx    = $state<number | null>(null);
  let dropInsertIdx = $state<number | null>(null); // insert position within THIS pane

  let tabListEl = $state<HTMLElement | undefined>();

  function onTabMouseDown(e: MouseEvent, idx: number) {
    if (e.button !== 0) return;
    drag = { fromIdx: idx, startX: e.clientX, active: false };
    window.addEventListener("mousemove", onWindowMouseMove);
    window.addEventListener("mouseup", onWindowMouseUp);
  }

  function onWindowMouseMove(e: MouseEvent) {
    if (!drag) return;
    if (!drag.active) {
      if (Math.abs(e.clientX - drag.startX) < 5) return;
      drag.active = true;
      draggedIdx = drag.fromIdx;
      document.body.style.cursor = "grabbing";
    }

    // Which tab-bar is the pointer over?
    const underEl = document.elementFromPoint(e.clientX, e.clientY);
    const targetBar = underEl?.closest<HTMLElement>("[data-pane-idx]");
    const targetPaneIdx = targetBar ? parseInt(targetBar.dataset.paneIdx ?? "-1") : -1;

    if (targetPaneIdx === pane.paneIdx || targetPaneIdx === -1) {
      // ── Within own tab bar ──
      if (app.crossPaneDrag !== null) app.setCrossPaneDrag(null);

      const tabEls = tabListEl?.querySelectorAll<HTMLElement>(".tab");
      if (!tabEls) return;
      let insert = tabEls.length;
      for (let i = 0; i < tabEls.length; i++) {
        const rect = tabEls[i].getBoundingClientRect();
        if (e.clientX < rect.left + rect.width / 2) { insert = i; break; }
      }
      dropInsertIdx = insert;
    } else {
      // ── Crossed into another pane's tab bar ──
      dropInsertIdx = null;

      const foreignTabEls = targetBar!.querySelectorAll<HTMLElement>(".tab");
      let insertIdx = foreignTabEls.length;
      for (let i = 0; i < foreignTabEls.length; i++) {
        const rect = foreignTabEls[i].getBoundingClientRect();
        if (e.clientX < rect.left + rect.width / 2) { insertIdx = i; break; }
      }

      app.setCrossPaneDrag({
        fromPaneIdx: pane.paneIdx,
        fromTabIdx: drag.fromIdx,
        toPaneIdx: targetPaneIdx,
        toInsertIdx: insertIdx,
      });
    }
  }

  function onWindowMouseUp() {
    if (drag?.active) {
      const cpd = app.crossPaneDrag;
      if (cpd && cpd.fromPaneIdx === pane.paneIdx) {
        // Cross-pane drop
        app.moveTabToPane(cpd.fromPaneIdx, cpd.fromTabIdx, cpd.toPaneIdx, cpd.toInsertIdx);
      } else if (dropInsertIdx !== null) {
        // Same-pane reorder
        pane.reorderTabs(drag.fromIdx, dropInsertIdx);
      }
    }

    app.setCrossPaneDrag(null);
    drag = null;
    draggedIdx = null;
    dropInsertIdx = null;
    document.body.style.cursor = "";
    window.removeEventListener("mousemove", onWindowMouseMove);
    window.removeEventListener("mouseup", onWindowMouseUp);
  }

  onDestroy(() => {
    window.removeEventListener("mousemove", onWindowMouseMove);
    window.removeEventListener("mouseup", onWindowMouseUp);
    document.body.style.cursor = "";
    app.setCrossPaneDrag(null);
  });

  // ── Foreign drop indicator (driven by source pane via store) ──
  const foreignDrop = $derived(
    app.isSplit && app.crossPaneDrag?.toPaneIdx === pane.paneIdx
      ? app.crossPaneDrag
      : null
  );

  // ── Tab helpers ────────────────────────────────────────────────
  function tabTitle(path: string | null): string {
    if (!path) return t("tabs.newTab");
    return path.split(/[/\\]/).filter(Boolean).pop() ?? t("tabs.newTab");
  }

  function closeTab(e: MouseEvent, id: string) {
    e.stopPropagation();
    pane.closeTab(id);
  }

  function openTabMenu(e: MouseEvent, tabId: string, idx: number) {
    e.preventDefault();
    e.stopPropagation();
    const tab = pane.tabs[idx];
    if (!tab) return;

    const items: MenuItem[] = [];

    if (pane.tabs.length > 1) {
      items.push({
        kind: "action",
        label: t("tabs.close"),
        icon: ICON_CLOSE,
        onclick: () => pane.closeTab(tabId),
      });
      items.push({
        kind: "action",
        label: t("tabs.closeOthers"),
        icon: ICON_CLOSE_OTHERS,
        onclick: () => {
          const toClose = pane.tabs.filter(t => t.id !== tabId).map(t => t.id);
          pane.setActiveTab(tabId);
          for (const id of toClose) pane.closeTab(id);
        },
      });
      items.push({ kind: "separator" });
    }

    items.push({
      kind: "action",
      label: t("tabs.duplicateTab"),
      icon: ICON_DUPLICATE,
      onclick: () => {
        pane.setActiveTabByIndex(idx);
        pane.duplicateTab();
      },
    });

    if (tab.currentPath) {
      items.push({
        kind: "action",
        label: t("tabs.addTab"),
        icon: ICON_NEW_TAB,
        onclick: () => pane.addTab(tab.currentPath ?? undefined),
      });

      if (app.isSplit) {
        const otherPane = app.getPaneView(pane.paneIdx === 0 ? 1 : 0);
        items.push({ kind: "separator" });
        items.push({
          kind: "action",
          label: t("tabs.openInOtherPane"),
          icon: ICON_SPLIT,
          onclick: () => otherPane.addTab(tab.currentPath ?? undefined),
        });
        if (pane.tabs.length > 1) {
          items.push({
            kind: "action",
            label: t("tabs.moveToOtherPane"),
            icon: ICON_MOVE_PANE,
            onclick: () => app.moveTabToPane(pane.paneIdx, idx, pane.paneIdx === 0 ? 1 : 0, otherPane.tabs.length),
          });
        }
      }

      items.push({ kind: "separator" });
      items.push({
        kind: "action",
        label: t("tabs.copyPath"),
        icon: ICON_COPY,
        onclick: () => {
          navigator.clipboard.writeText(tab.currentPath!);
          app.notify("info", t("sidebar.pathCopied"));
        },
      });
    }

    contextMenu = { x: e.clientX, y: e.clientY, items };
  }

  const ICON_CLOSE        = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>`;
  const ICON_CLOSE_OTHERS = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2"/><line x1="9" y1="9" x2="15" y2="15"/><line x1="15" y1="9" x2="9" y2="15"/></svg>`;
  const ICON_DUPLICATE    = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="9" y="9" width="13" height="13" rx="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>`;
  const ICON_NEW_TAB      = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M9 3v18M3 9h6"/></svg>`;
  const ICON_COPY         = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M21 15v4a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2v-4"/><polyline points="7 10 12 15 17 10"/><line x1="12" y1="15" x2="12" y2="3"/></svg>`;
  const ICON_SPLIT        = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2"/><line x1="12" y1="3" x2="12" y2="21"/></svg>`;
  const ICON_MOVE_PANE    = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2"/><line x1="12" y1="3" x2="12" y2="21"/><polyline points="16 8 21 12 16 16"/></svg>`;
</script>

<div
  class="tab-bar"
  class:tab-bar--pane-focused={pane.isFocused}
  class:tab-bar--drop-target={foreignDrop !== null}
  data-pane-idx={pane.paneIdx}
  role="tablist"
  aria-label="File browser tabs"
>
  <div class="tab-list" bind:this={tabListEl}>
    {#if dropInsertIdx === 0 || foreignDrop?.toInsertIdx === 0}
      <div class="drop-indicator" aria-hidden="true"></div>
    {/if}

    {#each pane.tabs as tab, i (tab.id)}
      <!-- svelte-ignore a11y_interactive_supports_focus -->
      <div
        class="tab"
        class:active={i === pane.activeTabIdx}
        class:dragging={draggedIdx === i}
        role="tab"
        aria-selected={i === pane.activeTabIdx}
        title={tab.currentPath ?? t("tabs.newTab")}
        style:--tab-color={tab.color}
        onmousedown={(e) => onTabMouseDown(e, i)}
        onclick={() => { if (!drag?.active) pane.setActiveTabByIndex(i); }}
        onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") pane.setActiveTabByIndex(i); }}
        oncontextmenu={(e) => openTabMenu(e, tab.id, i)}
      >
        <span class="tab-dot" aria-hidden="true"></span>
        <span class="tab-title">{tabTitle(tab.currentPath)}</span>
        {#if pane.tabs.length > 1}
          <button
            class="tab-close"
            aria-label={t("tabs.close")}
            title={t("tabs.close")}
            onmousedown={(e) => e.stopPropagation()}
            onclick={(e) => closeTab(e, tab.id)}
            tabindex={-1}
          >
            <svg width="10" height="10" viewBox="0 0 10 10" fill="none" aria-hidden="true">
              <path d="M1.5 1.5L8.5 8.5M8.5 1.5L1.5 8.5" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
            </svg>
          </button>
        {/if}
      </div>

      {#if dropInsertIdx === i + 1 || foreignDrop?.toInsertIdx === i + 1}
        <div class="drop-indicator" aria-hidden="true"></div>
      {/if}
    {/each}

    <button
      class="tab-add"
      aria-label={t("tabs.addTab")}
      title={t("tabs.addTab")}
      onclick={() => pane.addTab()}
    >
      <svg width="13" height="13" viewBox="0 0 14 14" fill="none" aria-hidden="true">
        <path d="M7 1.5V12.5M1.5 7H12.5" stroke="currentColor" stroke-width="1.6" stroke-linecap="round"/>
      </svg>
    </button>
  </div>
</div>

{#if contextMenu}
  <ContextMenu
    x={contextMenu.x}
    y={contextMenu.y}
    items={contextMenu.items}
    onclose={() => (contextMenu = null)}
  />
{/if}

<style>
  .tab-bar {
    display: flex;
    align-items: stretch;
    height: 34px;
    background: var(--surface);
    border-bottom: 1px solid var(--line);
    padding: 0 4px 0 0;
    flex-shrink: 0;
    overflow: hidden;
    transition: background 0.15s, box-shadow 0.15s;
  }

  .tab-bar--pane-focused {
    background:
      linear-gradient(180deg, color-mix(in srgb, var(--accent) 5%, transparent), transparent 80%),
      var(--surface);
  }

  /* Visual hint when a foreign tab is being dragged over this bar */
  .tab-bar--drop-target {
    box-shadow: inset 0 0 0 2px var(--accent);
  }

  .tab-list {
    display: flex;
    align-items: stretch;
    flex: 1;
    min-width: 0;
    overflow-x: auto;
    scrollbar-width: none;

    &::-webkit-scrollbar { display: none; }
  }

  .drop-indicator {
    flex-shrink: 0;
    width: 2px;
    height: 70%;
    background: var(--accent);
    border-radius: 1px;
    align-self: center;
    pointer-events: none;
  }

  .tab {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 0 10px;
    min-width: 80px;
    max-width: 200px;
    height: 100%;
    background: none;
    border: none;
    border-right: 1px solid var(--line);
    border-bottom: 2px solid transparent;
    cursor: grab;
    color: var(--text-muted);
    font-size: 12px;
    font-family: var(--font-ui);
    white-space: nowrap;
    position: relative;
    flex-shrink: 0;
    user-select: none;
    transition: background 0.1s, color 0.1s, border-bottom-color 0.1s, opacity 0.15s;

    &:hover {
      background: var(--surface-hover);
      color: var(--text);
      border-bottom-color: color-mix(in srgb, var(--tab-color) 40%, transparent);
    }

    &.active {
      background: color-mix(in srgb, var(--tab-color) 10%, var(--surface));
      color: var(--text);
      border-bottom-color: var(--tab-color);
    }

    .tab-bar--pane-focused &.active {
      box-shadow:
        inset 0 1px 0 color-mix(in srgb, var(--tab-color) 22%, transparent),
        inset 0 -12px 20px color-mix(in srgb, var(--tab-color) 7%, transparent);
      font-weight: 650;
    }

    .tab-bar:not(.tab-bar--pane-focused) &.active {
      background: color-mix(in srgb, var(--tab-color) 5%, var(--surface));
      color: var(--text-muted);
      border-bottom-color: color-mix(in srgb, var(--tab-color) 55%, transparent);
    }

    &.dragging {
      opacity: 0.4;
      cursor: grabbing;
    }
  }

  .tab-dot {
    flex-shrink: 0;
    width: 8px;
    height: 8px;
    border-radius: 50%;
    background: var(--tab-color);
    opacity: 0.75;
    pointer-events: none;
    transition: opacity 0.1s;

    .tab.active & { opacity: 1; }
  }

  .tab-title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    text-align: left;
    pointer-events: none;
  }

  .tab-close {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 18px;
    height: 18px;
    border-radius: 3px;
    background: none;
    border: none;
    cursor: pointer;
    color: var(--text-muted);
    padding: 0;
    transition: background 0.1s, color 0.1s;
    opacity: 0;

    .tab:hover &,
    .tab.active & { opacity: 1; }

    &:hover {
      background: var(--surface-hover);
      color: var(--text);
    }
  }

  .tab-add {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    justify-content: center;
    width: 36px;
    min-width: 36px;
    max-width: 36px;
    height: calc(100% - 5px);
    align-self: flex-end;
    background: color-mix(in srgb, var(--surface-alt) 70%, transparent);
    border: 1px solid var(--line);
    border-bottom-color: transparent;
    border-radius: 7px 7px 0 0;
    cursor: pointer;
    color: var(--text-muted);
    transition: background 0.1s, color 0.1s, border-color 0.1s;
    margin-left: 4px;
    margin-right: 4px;

    &:hover {
      background: var(--surface-hover);
      border-color: color-mix(in srgb, var(--accent) 28%, var(--line));
      border-bottom-color: transparent;
      color: var(--text);
    }

    .tab-bar--pane-focused & {
      color: color-mix(in srgb, var(--accent) 72%, var(--text-muted));
    }
  }
</style>
