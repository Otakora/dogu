<script lang="ts">
  import { onMount, onDestroy, tick } from "svelte";
  import { invoke } from "@tauri-apps/api/core";
  import { app } from "../../stores/app.svelte.js";
  import { t } from "../../i18n/index.js";
  import type { AvailableShell } from "../../types/index.js";
  import { openTerminalAt } from "../../utils/terminal.js";
  import TerminalTab from "./TerminalTab.svelte";
  import ContextMenu from "../ui/ContextMenu.svelte";
  import type { MenuItem } from "../ui/ContextMenu.svelte";

  // ── Tab component refs (indexed to match app.terminalTabs order) ──
  let tabComponents = $state<(ReturnType<typeof TerminalTab> | undefined)[]>([]);

  // ── Shell discovery ───────────────────────────────────────
  let availableShells = $state<AvailableShell[]>([]);

  // ── Context menus ─────────────────────────────────────────
  let shellMenu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);
  let tabMenu  = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

  // ── Inline rename ─────────────────────────────────────────
  let renamingId    = $state<string | null>(null);
  let renameValue   = $state("");
  let renameInputEl = $state<HTMLInputElement | undefined>();

  // ── Drag-to-reorder (mouse events — reliable in Tauri WebView) ───
  type TermDragState = { fromIdx: number; startX: number; active: boolean };
  let termDrag: TermDragState | null = null;
  let draggedTermIdx = $state<number | null>(null);
  let dropTermInsert = $state<number | null>(null);
  let termTabsEl = $state<HTMLElement | undefined>();

  function onTermTabMouseDown(e: MouseEvent, idx: number) {
    if (e.button !== 0) return;
    termDrag = { fromIdx: idx, startX: e.clientX, active: false };
    window.addEventListener("mousemove", onTermWindowMouseMove);
    window.addEventListener("mouseup", onTermWindowMouseUp);
  }

  function onTermWindowMouseMove(e: MouseEvent) {
    if (!termDrag) return;
    if (!termDrag.active) {
      if (Math.abs(e.clientX - termDrag.startX) < 5) return;
      termDrag.active = true;
      draggedTermIdx = termDrag.fromIdx;
      document.body.style.cursor = "grabbing";
    }
    const tabEls = termTabsEl?.querySelectorAll<HTMLElement>(".term-tab");
    if (!tabEls) return;
    let insert = tabEls.length;
    for (let i = 0; i < tabEls.length; i++) {
      const rect = tabEls[i].getBoundingClientRect();
      if (e.clientX < rect.left + rect.width / 2) { insert = i; break; }
    }
    dropTermInsert = insert;
  }

  function onTermWindowMouseUp() {
    if (termDrag?.active && dropTermInsert !== null) {
      app.reorderTerminalTabs(termDrag.fromIdx, dropTermInsert);
    }
    termDrag = null;
    draggedTermIdx = null;
    dropTermInsert = null;
    document.body.style.cursor = "";
    window.removeEventListener("mousemove", onTermWindowMouseMove);
    window.removeEventListener("mouseup", onTermWindowMouseUp);
  }

  onDestroy(() => {
    window.removeEventListener("mousemove", onTermWindowMouseMove);
    window.removeEventListener("mouseup", onTermWindowMouseUp);
    document.body.style.cursor = "";
  });

  onMount(async () => {
    try {
      availableShells = await invoke<AvailableShell[]>("get_available_shells");
    } catch {
      availableShells = [];
    }
  });

  // ── Color sync: exact CWD match against browser tabs ─────
  // Use every browser tab, not only the focused pane. This keeps terminal
  // colors stable in split view while still prioritising the focused tab.
  function terminalTabColor(cwd: string | null): string | null {
    if (!cwd) return null;
    const norm = (p: string) => p.replace(/[/\\]+$/, "").toLowerCase();
    const ncwd = norm(cwd);
    const matches = app.allTabHighlights.filter(tab => tab.path && norm(tab.path) === ncwd);
    return matches.find(tab => tab.isActive)?.color ?? matches[0]?.color ?? null;
  }

  // ── Resize ────────────────────────────────────────────────
  function startResize(e: MouseEvent) {
    e.preventDefault();
    const startY = e.clientY;
    const startH = app.terminalPanelHeight;
    function onMove(ev: MouseEvent) {
      app.setTerminalPanelHeight(Math.max(80, Math.min(700, startH + (startY - ev.clientY))));
    }
    function onUp() {
      fitActive();
      window.removeEventListener("mousemove", onMove);
      window.removeEventListener("mouseup", onUp);
    }
    window.addEventListener("mousemove", onMove);
    window.addEventListener("mouseup", onUp);
  }

  function fitActive() {
    const idx = app.terminalTabs.findIndex(t => t.id === app.activeTerminalId);
    if (idx >= 0) tabComponents[idx]?.fit();
  }

  // ── Tab management ────────────────────────────────────────
  async function addTab(shell?: string) {
    await openTerminalAt(app.currentPath ?? "", shell);
  }

  async function closeTab(e: MouseEvent, id: string) {
    e.stopPropagation();
    try { await invoke("close_terminal", { id }); } catch {}
    app.closeTerminalTab(id);
  }

  function switchTab(id: string, idx: number) {
    app.setActiveTerminalTab(id);
    setTimeout(() => tabComponents[idx]?.fit(), 20);
  }

  // ── Shell picker ─────────────────────────────────────────
  function openShellPicker(e: MouseEvent) {
    e.stopPropagation();
    if (availableShells.length <= 1) {
      addTab(availableShells[0]?.path);
      return;
    }
    shellMenu = {
      x: e.clientX,
      y: e.clientY,
      items: availableShells.map(sh => ({
        kind: "action" as const,
        label: sh.name,
        icon: ICON_TERMINAL,
        onclick: () => addTab(sh.path),
      })),
    };
  }

  // ── Tab context menu ─────────────────────────────────────
  function openTabMenu(e: MouseEvent, id: string) {
    e.preventDefault();
    e.stopPropagation();
    tabMenu = {
      x: e.clientX,
      y: e.clientY,
      items: [
        {
          kind: "action",
          label: t("terminal.rename"),
          icon: ICON_RENAME,
          onclick: () => startRename(id),
        },
        { kind: "separator" },
        {
          kind: "action",
          label: t("terminal.closeTerminal"),
          icon: ICON_CLOSE,
          danger: true,
          onclick: async () => {
            try { await invoke("close_terminal", { id }); } catch {}
            app.closeTerminalTab(id);
          },
        },
      ],
    };
  }

  // ── Inline rename ─────────────────────────────────────────
  async function startRename(id: string) {
    const tab = app.terminalTabs.find(t => t.id === id);
    if (!tab) return;
    renamingId  = id;
    renameValue = tab.title;
    await tick();
    renameInputEl?.focus();
    renameInputEl?.select();
  }

  function commitRename() {
    if (renamingId) {
      app.renameTerminalTab(renamingId, renameValue);
      renamingId = null;
    }
  }

  function cancelRename() {
    renamingId = null;
  }

  function handleRenameKey(e: KeyboardEvent) {
    if (e.key === "Enter")  { e.preventDefault(); commitRename(); }
    if (e.key === "Escape") { e.preventDefault(); cancelRename(); }
  }

  // ── Icons ─────────────────────────────────────────────────
  const ICON_TERMINAL = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>`;
  const ICON_RENAME   = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>`;
  const ICON_CLOSE    = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><line x1="18" y1="6" x2="6" y2="18"/><line x1="6" y1="6" x2="18" y2="18"/></svg>`;
</script>

{#if app.terminalPanelOpen}
  <div class="term-panel" style:height="{app.terminalPanelHeight}px">
    <!-- ── Drag resize handle ── -->
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="term-resize-handle" role="separator" onmousedown={startResize}></div>

    <!-- ── Panel header ── -->
    <div class="term-header">
      <span class="term-panel-label">
        <svg width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
          <polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/>
        </svg>
        {t("terminal.panel")}
      </span>

      <!-- Tabs -->
      <div class="term-tabs" role="tablist" bind:this={termTabsEl}>
        {#each app.terminalTabs as tab, i (tab.id)}
          {@const color = terminalTabColor(tab.cwd)}
          <!-- svelte-ignore a11y_interactive_supports_focus -->
          <div
            class="term-tab"
            class:term-tab--active={tab.id === app.activeTerminalId}
            class:term-tab--dragging={draggedTermIdx === i}
            class:drop-before={draggedTermIdx !== null && dropTermInsert === i}
            class:drop-after={draggedTermIdx !== null && dropTermInsert === i + 1}
            style:--tt-color={color ?? "transparent"}
            style:border-bottom-color={tab.id === app.activeTerminalId
              ? (color ?? "var(--accent)")
              : "transparent"}
            role="tab"
            aria-selected={tab.id === app.activeTerminalId}
            onclick={() => { if (!termDrag?.active) switchTab(tab.id, i); }}
            onkeydown={(e) => { if (e.key === "Enter" || e.key === " ") switchTab(tab.id, i); }}
            oncontextmenu={(e) => openTabMenu(e, tab.id)}
            onmousedown={(e) => onTermTabMouseDown(e, i)}
          >
            {#if color}
              <span
                class="term-tab-dot"
                style:background={color}
                aria-hidden="true"
              ></span>
            {/if}

            {#if renamingId === tab.id}
              <input
                bind:this={renameInputEl}
                class="term-tab-rename-input"
                type="text"
                bind:value={renameValue}
                onkeydown={handleRenameKey}
                onblur={commitRename}
                spellcheck="false"
                onmousedown={(e) => e.stopPropagation()}
                onclick={(e) => e.stopPropagation()}
              />
            {:else}
              <span
                class="term-tab-title"
                role="button"
                tabindex="0"
                ondblclick={(e) => { e.stopPropagation(); startRename(tab.id); }}
                onkeydown={(e) => { if (e.key === "Enter") { e.stopPropagation(); startRename(tab.id); } }}
              >{tab.title}</span>
            {/if}

            <button
              class="term-tab-close"
              aria-label={t("terminal.closeTerminal")}
              onmousedown={(e) => e.stopPropagation()}
              onclick={(e) => closeTab(e, tab.id)}
            >×</button>
          </div>

        {/each}
      </div>

      <!-- New tab (left-click = default, right-click = shell picker) -->
      <button
        class="term-hdr-btn term-hdr-btn--primary"
        title={t("terminal.newTerminal")}
        aria-label={t("terminal.newTerminal")}
        onclick={() => addTab()}
        oncontextmenu={openShellPicker}
      >
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" stroke="currentColor" stroke-width="1.5">
          <path d="M6 1.5V10.5M1.5 6H10.5" stroke-linecap="round"/>
        </svg>
      </button>

      <div class="term-hdr-sep"></div>

      <!-- Hide panel; terminal sessions stay alive in the background. -->
      <button
        class="term-hdr-btn term-hdr-btn--quiet"
        title={t("terminal.hidePanel")}
        aria-label={t("terminal.hidePanel")}
        onclick={() => app.closeTerminalPanel()}
      >
        <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" aria-hidden="true">
          <polyline points="6 9 12 15 18 9" stroke-linecap="round" stroke-linejoin="round"/>
        </svg>
      </button>
    </div>

    <!-- ── Terminal content ── -->
    <div class="term-content">
      {#each app.terminalTabs as tab, i (tab.id)}
        <TerminalTab
          bind:this={tabComponents[i]}
          id={tab.id}
          active={tab.id === app.activeTerminalId}
        />
      {/each}
      {#if app.terminalTabs.length === 0}
        <div class="term-empty">
          <button class="term-empty-btn" onclick={() => addTab()}>
            {t("terminal.newTerminal")}
          </button>
        </div>
      {/if}
    </div>
  </div>
{:else if app.terminalTabs.length > 0}
  <!-- svelte-ignore a11y_no_static_element_interactions -->
  <div
    class="term-collapsed"
    role="button"
    tabindex="0"
    title={t("terminal.showPanel")}
    onclick={() => app.openTerminalPanel()}
    onkeydown={(e) => {
      if (e.key === "Enter" || e.key === " ") {
        e.preventDefault();
        app.openTerminalPanel();
      }
    }}
  >
    <span class="term-collapsed-label">
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2" aria-hidden="true">
        <polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/>
      </svg>
      {t("terminal.panel")}
    </span>
    <span class="term-collapsed-count">
      {app.terminalTabs.length}
      {app.terminalTabs.length === 1 ? t("terminal.session") : t("terminal.sessions")}
    </span>
    <span class="term-collapsed-active">{app.terminalTabs.find(tab => tab.id === app.activeTerminalId)?.title ?? app.terminalTabs[0]?.title}</span>
    <span class="term-collapsed-action">
      {t("terminal.showPanel")}
      <svg width="13" height="13" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2.2" aria-hidden="true">
        <polyline points="18 15 12 9 6 15" stroke-linecap="round" stroke-linejoin="round"/>
      </svg>
    </span>
  </div>
{/if}

{#if shellMenu}
  <ContextMenu x={shellMenu.x} y={shellMenu.y} items={shellMenu.items} onclose={() => (shellMenu = null)} />
{/if}
{#if tabMenu}
  <ContextMenu x={tabMenu.x} y={tabMenu.y} items={tabMenu.items} onclose={() => (tabMenu = null)} />
{/if}

<style>
  .term-panel {
    flex-shrink: 0;
    display: flex;
    flex-direction: column;
    background: #0d0d0d;
    border-top: 1px solid var(--line);
    min-height: 80px;
    max-height: 700px;
    overflow: hidden;
  }

  .term-resize-handle {
    height: 4px;
    width: 100%;
    cursor: row-resize;
    background: transparent;
    flex-shrink: 0;
    transition: background 0.15s;
    &:hover { background: color-mix(in srgb, var(--accent) 40%, transparent); }
  }

  .term-header {
    display: flex;
    align-items: center;
    gap: 3px;
    height: 30px;
    padding: 0 6px;
    background: var(--surface);
    border-bottom: 1px solid var(--line);
    flex-shrink: 0;
    overflow: hidden;
  }

  .term-panel-label {
    display: flex;
    align-items: center;
    gap: 5px;
    font-size: 11px;
    font-weight: 600;
    color: var(--text-muted);
    text-transform: uppercase;
    letter-spacing: 0.05em;
    white-space: nowrap;
    flex-shrink: 0;
    padding-right: 8px;
    border-right: 1px solid var(--line);
    margin-right: 4px;
  }

  .term-tabs {
    display: flex;
    align-items: stretch;
    flex: 1;
    min-width: 0;
    overflow-x: auto;
    scrollbar-width: none;
    &::-webkit-scrollbar { display: none; }
  }

  .term-tab {
    display: flex;
    align-items: center;
    gap: 5px;
    padding: 0 8px;
    height: 100%;
    font-size: 12px;
    color: var(--text-muted);
    cursor: grab;
    border-right: 1px solid var(--line);
    border-bottom: 2px solid transparent;
    white-space: nowrap;
    flex-shrink: 0;
    max-width: 180px;
    user-select: none;
    position: relative;
    transition: background 0.1s, color 0.1s, border-bottom-color 0.15s, opacity 0.1s;

    &:hover { background: var(--surface-hover); color: var(--text); }

    &.term-tab--active {
      background: #0d0d0d;
      color: #cccccc;
      font-weight: 650;
      box-shadow:
        inset 0 1px 0 color-mix(in srgb, var(--tt-color, var(--accent)) 26%, transparent),
        inset 0 -10px 18px color-mix(in srgb, var(--tt-color, var(--accent)) 7%, transparent);
    }

    &.term-tab--dragging {
      opacity: 0.4;
      cursor: grabbing;
    }

    /* Drop position indicators via pseudo-elements — no DOM changes, no layout shifts */
    &.drop-before::before,
    &.drop-after::after {
      content: '';
      position: absolute;
      top: 15%;
      height: 70%;
      width: 2px;
      background: var(--accent);
      border-radius: 1px;
      pointer-events: none;
    }

    &.drop-before::before { left: -1px; }
    &.drop-after::after  { right: -1px; }
  }

  .term-tab-dot {
    flex-shrink: 0;
    width: 7px;
    height: 7px;
    border-radius: 50%;
    opacity: 0.8;
    box-shadow: 0 0 0 3px color-mix(in srgb, var(--tt-color) 12%, transparent);
  }

  .term-collapsed {
    flex-shrink: 0;
    display: flex;
    align-items: center;
    gap: 10px;
    min-height: 30px;
    padding: 0 10px;
    background:
      linear-gradient(180deg, color-mix(in srgb, var(--accent) 7%, transparent), transparent),
      var(--surface);
    border-top: 1px solid var(--line);
    color: var(--text-muted);
    cursor: pointer;
    user-select: none;
    transition: background 0.12s, color 0.12s, border-color 0.12s;

    &:hover,
    &:focus-visible {
      background:
        linear-gradient(180deg, color-mix(in srgb, var(--accent) 11%, transparent), transparent),
        var(--surface-hover);
      color: var(--text);
      outline: none;
    }
  }

  .term-collapsed-label,
  .term-collapsed-count,
  .term-collapsed-action {
    display: inline-flex;
    align-items: center;
    gap: 5px;
    flex-shrink: 0;
    font-size: 11px;
    font-weight: 750;
    text-transform: uppercase;
    letter-spacing: 0.055em;
  }

  .term-collapsed-label {
    color: var(--accent);
  }

  .term-collapsed-count {
    padding: 2px 7px;
    border-radius: 999px;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    border: 1px solid color-mix(in srgb, var(--accent) 24%, transparent);
    color: color-mix(in srgb, var(--accent) 82%, var(--text));
    text-transform: none;
    letter-spacing: 0;
  }

  .term-collapsed-active {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
    white-space: nowrap;
    color: var(--text-muted);
    font-size: 12px;
  }

  .term-collapsed-action {
    color: var(--text-subtle);
  }

  .term-tab-title {
    flex: 1;
    min-width: 0;
    overflow: hidden;
    text-overflow: ellipsis;
  }

  .term-tab-rename-input {
    flex: 1;
    min-width: 0;
    background: var(--surface-alt);
    border: 1px solid var(--accent);
    border-radius: 3px;
    padding: 1px 4px;
    font-size: 12px;
    font-family: inherit;
    color: var(--text);
    outline: none;
  }

  .term-tab-close {
    flex-shrink: 0;
    background: none;
    border: none;
    color: inherit;
    font-size: 14px;
    line-height: 1;
    cursor: pointer;
    padding: 0 2px;
    opacity: 0;
    transition: opacity 0.1s;

    .term-tab:hover &,
    .term-tab.term-tab--active & { opacity: 0.6; }
    &:hover { opacity: 1 !important; }
  }

  .term-hdr-sep {
    width: 1px;
    height: 16px;
    background: var(--line);
    flex-shrink: 0;
    margin: 0 2px;
  }

  .term-hdr-btn {
    flex-shrink: 0;
    width: 24px;
    height: 24px;
    display: flex;
    align-items: center;
    justify-content: center;
    background: none;
    border: 1px solid transparent;
    color: var(--text-muted);
    border-radius: 6px;
    cursor: pointer;
    transition: background 0.1s, color 0.1s, border-color 0.1s, box-shadow 0.1s;
    &:hover {
      background: var(--surface-hover);
      border-color: var(--line);
      color: var(--text);
    }
  }

  .term-hdr-btn--primary {
    background: var(--accent-soft);
    border-color: color-mix(in srgb, var(--accent) 24%, transparent);
    color: var(--accent);

    &:hover {
      background: color-mix(in srgb, var(--accent) 18%, transparent);
      border-color: color-mix(in srgb, var(--accent) 42%, transparent);
      color: var(--accent);
      box-shadow: 0 1px 5px color-mix(in srgb, var(--accent) 18%, transparent);
    }
  }

  .term-hdr-btn--quiet {
    opacity: 0.82;

    &:hover {
      opacity: 1;
      color: var(--text-muted);
    }
  }

  .term-content {
    flex: 1;
    position: relative;
    overflow: hidden;
    background: #0d0d0d;
    min-height: 0;
  }

  .term-empty {
    display: flex;
    align-items: center;
    justify-content: center;
    height: 100%;
  }

  .term-empty-btn {
    background: var(--accent-soft);
    color: var(--accent);
    border: 1px solid color-mix(in srgb, var(--accent) 30%, transparent);
    border-radius: 6px;
    padding: 6px 16px;
    font-size: 13px;
    cursor: pointer;
    transition: background 0.1s;
    &:hover { background: color-mix(in srgb, var(--accent) 20%, transparent); }
  }
</style>
