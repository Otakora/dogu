<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { onDestroy, untrack } from "svelte";
  import { app } from "../../stores/app.svelte.js";
  import type { EntryDto, ContentSortKey } from "../../types/index.js";
  import FileRow from "./FileRow.svelte";
  import ContextMenu from "../ui/ContextMenu.svelte";
  import type { MenuItem } from "../ui/ContextMenu.svelte";
  import { t, tn } from "../../i18n/index.js";
  import { openTerminalAt, openRemoteTerminalAt } from "../../utils/terminal.js";

  type Props = {
    onDelete: (paths: string[]) => void;
    onCopy: (paths: string[]) => void;
    onCut: (paths: string[]) => void;
    onPaste: () => void;
    onExtractHere: (paths: string[]) => void;
    onExtractToFolder: (paths: string[]) => void;
    onExtractTo: (paths: string[]) => void;
    onCompressQuick: (paths: string[]) => void;
    onCompress: (paths: string[]) => void;
    onChd: (paths: string[]) => void;
    onM3u: (paths: string[]) => void;
    onProperties: (paths: string[]) => void;
    onOpenWith: (path: string) => void;
  };

  let { onDelete, onCopy, onCut, onPaste, onExtractHere, onExtractToFolder, onExtractTo, onCompressQuick, onCompress, onChd, onM3u, onProperties, onOpenWith }: Props = $props();

  // ── Panel / body element refs ────────────────────────────
  let panelEl = $state<HTMLElement | undefined>(undefined);
  let contentBodyEl = $state<HTMLElement | undefined>(undefined);

  // ── Context menu ─────────────────────────────────────────
  let contextMenu = $state<{ x: number; y: number; items: MenuItem[] } | null>(null);

  // ── Column resize ─────────────────────────────────────────
  let resizingCol = $state<{ key: "name" | "type" | "size" | "modified"; startX: number; startW: number } | null>(null);

  function startColResize(e: MouseEvent, key: "name" | "type" | "size" | "modified") {
    e.preventDefault();
    e.stopPropagation();
    resizingCol = { key, startX: e.clientX, startW: app.columnWidths[key] };
    window.addEventListener("mousemove", onColResizeMove);
    window.addEventListener("mouseup", onColResizeEnd);
  }

  function onColResizeMove(e: MouseEvent) {
    if (!resizingCol) return;
    const delta = e.clientX - resizingCol.startX;
    const newW = Math.max(50, Math.round(resizingCol.startW + delta));
    app.setColumnWidth(resizingCol.key, newW);
  }

  function onColResizeEnd() {
    resizingCol = null;
    window.removeEventListener("mousemove", onColResizeMove);
    window.removeEventListener("mouseup", onColResizeEnd);
  }

  // ── Content zoom (Ctrl+wheel) ─────────────────────────────
  function handleContentWheel(e: WheelEvent) {
    if (!e.ctrlKey) return;
    e.preventDefault();
    const delta = e.deltaY < 0 ? 0.1 : -0.1;
    const next = Math.max(0.6, Math.min(2.0, Math.round((app.contentZoom + delta) * 10) / 10));
    app.setContentZoom(next);
  }

  // ── Rubber-band selection ────────────────────────────────────────────────────
  // rbDiv lives in panelEl (overflow:hidden) → never extends contentBodyEl's scroll area.
  // rbSX/rbSY are stored in SCROLL-SPACE (relative to contentBodyEl content origin) so the
  // selection anchor stays attached to its content position as auto-scroll moves the viewport.
  // The visual rect is re-derived from scroll-space on every frame, compensating scroll offset.
  let rbDiv: HTMLDivElement | null = null;
  let rbSX = 0, rbSY = 0;   // drag-start in scroll-space (contentBodyEl coords)
  let rbActive = false;
  let consumeNextPanelClick = false;
  let lastMouse = { x: 0, y: 0 };
  let autoScrollRaf = 0;

  function ensureRubberBand() {
    if (!panelEl) return;
    if (!rbDiv) {
      rbDiv = document.createElement("div");
      rbDiv.className = "rubber-band";
      rbDiv.style.display = "none";
      panelEl.appendChild(rbDiv);
    }
  }

  // mx, my — current mouse position in VIEWPORT coords.
  // Converts scroll-space start + viewport end into panel-local rect for display.
  function setRubberBandRect(mx: number, my: number) {
    if (!rbDiv || !contentBodyEl || !panelEl) return;

    const bodyRect  = contentBodyEl.getBoundingClientRect();
    const panelRect = panelEl.getBoundingClientRect();

    // Current mouse in scroll-space
    const ex = mx - bodyRect.left + contentBodyEl.scrollLeft;
    const ey = my - bodyRect.top  + contentBodyEl.scrollTop;

    const dw = Math.abs(ex - rbSX);
    const dh = Math.abs(ey - rbSY);
    if (dw < 3 && dh < 3) { rbDiv.style.display = "none"; return; }

    // Convert scroll-space corners to panel-local for CSS positioning.
    // body offset inside panel is constant; scroll offset shifts the anchor visually.
    const bLeft = bodyRect.left - panelRect.left;
    const bTop  = bodyRect.top  - panelRect.top;

    const startPX = rbSX - contentBodyEl.scrollLeft + bLeft;
    const startPY = rbSY - contentBodyEl.scrollTop  + bTop;
    const endPX   = mx - panelRect.left;   // == ex - scrollLeft + bLeft
    const endPY   = my - panelRect.top;

    rbDiv.style.display = "block";
    rbDiv.style.left   = `${Math.min(startPX, endPX)}px`;
    rbDiv.style.top    = `${Math.min(startPY, endPY)}px`;
    rbDiv.style.width  = `${Math.abs(endPX - startPX)}px`;
    rbDiv.style.height = `${Math.abs(endPY - startPY)}px`;
  }

  // Selection comparison also uses scroll-space so items stay selected through auto-scroll.
  function updateRubberBandSelection(mx: number, my: number) {
    if (!contentBodyEl) return;
    const bodyRect = contentBodyEl.getBoundingClientRect();

    const ex = mx - bodyRect.left + contentBodyEl.scrollLeft;
    const ey = my - bodyRect.top  + contentBodyEl.scrollTop;

    const selL = Math.min(rbSX, ex);
    const selT = Math.min(rbSY, ey);
    const selR = Math.max(rbSX, ex);
    const selB = Math.max(rbSY, ey);

    const selected: string[] = [];
    contentBodyEl.querySelectorAll<HTMLElement>("[data-path]").forEach((row) => {
      const r  = row.getBoundingClientRect();
      const rL = r.left - bodyRect.left + contentBodyEl!.scrollLeft;
      const rT = r.top  - bodyRect.top  + contentBodyEl!.scrollTop;
      if (selL < rL + r.width && selR > rL && selT < rT + r.height && selB > rT) {
        const p = row.getAttribute("data-path");
        if (p) selected.push(p);
      }
    });
    app.setSelection(selected);
  }

  function handleBodyMousedown(e: MouseEvent) {
    if (e.button !== 0) return;
    if ((e.target as HTMLElement).closest("[data-path]")) return;
    if (!contentBodyEl) return;

    ensureRubberBand();
    const bodyRect = contentBodyEl.getBoundingClientRect();
    // Store start in scroll-space so anchor is content-relative
    rbSX = e.clientX - bodyRect.left + contentBodyEl.scrollLeft;
    rbSY = e.clientY - bodyRect.top  + contentBodyEl.scrollTop;
    rbActive = true;
    lastMouse = { x: e.clientX, y: e.clientY };
    if (rbDiv) rbDiv.style.display = "none";
    app.clearSelection();

    window.addEventListener("mousemove", onDragMove);
    window.addEventListener("mouseup", onDragEnd);
    autoScrollRaf = requestAnimationFrame(autoScrollTick);
  }

  function onDragMove(e: MouseEvent) {
    if (!rbActive) return;
    lastMouse = { x: e.clientX, y: e.clientY };
    setRubberBandRect(e.clientX, e.clientY);
    updateRubberBandSelection(e.clientX, e.clientY);
  }

  function autoScrollTick() {
    if (!rbActive || !contentBodyEl) return;
    const bodyRect = contentBodyEl.getBoundingClientRect();
    const ZONE = 40, SPEED = 10;
    if (lastMouse.y < bodyRect.top + ZONE && contentBodyEl.scrollTop > 0) {
      contentBodyEl.scrollTop -= SPEED;
    } else if (lastMouse.y > bodyRect.bottom - ZONE) {
      const maxScroll = contentBodyEl.scrollHeight - contentBodyEl.clientHeight;
      if (contentBodyEl.scrollTop < maxScroll) contentBodyEl.scrollTop += SPEED;
    }
    // Redraw with same mouse position — scroll changed so anchor shifts in panel-local space
    setRubberBandRect(lastMouse.x, lastMouse.y);
    updateRubberBandSelection(lastMouse.x, lastMouse.y);
    autoScrollRaf = requestAnimationFrame(autoScrollTick);
  }

  function onDragEnd() {
    const wasDragged = rbDiv?.style.display === "block";
    if (wasDragged) consumeNextPanelClick = true;
    rbActive = false;
    if (rbDiv) rbDiv.style.display = "none";
    cancelAnimationFrame(autoScrollRaf);
    window.removeEventListener("mousemove", onDragMove);
    window.removeEventListener("mouseup", onDragEnd);
  }

  onDestroy(() => {
    rbDiv?.remove();
    cancelAnimationFrame(autoScrollRaf);
    clearTimeout(scrollSaveTimer);
    window.removeEventListener("mousemove", onDragMove);
    window.removeEventListener("mouseup", onDragEnd);
    window.removeEventListener("mousemove", onColResizeMove);
    window.removeEventListener("mouseup", onColResizeEnd);
  });

  // ── Sorted entries ───────────────────────────────────────
  const sorted = $derived(sortEntries(app.entries, app.sortKey, app.sortDir));

  // ── Quick filter (Ctrl+F or type-ahead) ──────────────────
  let quickFilter = $state("");
  let quickFilterOpen = $state(false);
  let quickFilterInputEl = $state<HTMLInputElement | undefined>(undefined);

  const displayed = $derived(
    quickFilter
      ? sorted.filter(e => e.name.toLowerCase().includes(quickFilter.toLowerCase()))
      : sorted
  );

  // Type-ahead buffer
  let typeaheadBuffer = "";
  let typeaheadTimer: ReturnType<typeof setTimeout> | null = null;

  function handleTypeahead(char: string) {
    typeaheadBuffer += char;
    if (typeaheadTimer) clearTimeout(typeaheadTimer);
    typeaheadTimer = setTimeout(() => { typeaheadBuffer = ""; }, 800);
    const query = typeaheadBuffer.toLowerCase();
    const match = displayed.find(e => e.name.toLowerCase().startsWith(query));
    if (match) {
      app.setSelection([match.path]);
      lastClickedPath = match.path;
      const el = panelEl?.querySelector<HTMLElement>(`[data-path="${CSS.escape(match.path)}"]`);
      el?.scrollIntoView({ block: "nearest" });
    }
  }

  function closeQuickFilter() {
    quickFilter = "";
    quickFilterOpen = false;
    panelEl?.focus();
  }

  function sortEntries(entries: EntryDto[], key: ContentSortKey, dir: "asc" | "desc"): EntryDto[] {
    return [...entries].sort((a, b) => {
      // Directories always first
      if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;

      let cmp = 0;
      switch (key) {
        case "name":     cmp = a.name.localeCompare(b.name, undefined, { sensitivity: "base" }); break;
        case "type":     cmp = a.extension.localeCompare(b.extension); break;
        case "size":     cmp = a.size - b.size; break;
        case "modified": cmp = a.modifiedTs - b.modifiedTs; break;
      }
      return dir === "asc" ? cmp : -cmp;
    });
  }

  // ── Load on path / tab change (tab-aware) ────────────────
  let lastLoadKey = "";

  // Debounced scroll save
  let scrollSaveTimer: ReturnType<typeof setTimeout> | undefined;
  function saveScrollPosition() {
    clearTimeout(scrollSaveTimer);
    scrollSaveTimer = setTimeout(() => {
      if (contentBodyEl) app.setTabScrollTop(app.activeTabIdx, contentBodyEl.scrollTop);
    }, 100);
  }

  $effect(() => {
    const path = app.currentPath;
    const tabId = app.activeTabId;
    if (!path) return;
    const key = `${tabId}::${path}`;
    if (key === lastLoadKey) return;
    lastLoadKey = key;

    untrack(() => {
      const loadedPath = app.activeTab?.loadedPath;
      if (loadedPath === path) {
        // Tab already has this path loaded — just restore scroll
        requestAnimationFrame(() => {
          if (contentBodyEl) contentBodyEl.scrollTop = app.activeTab?.scrollTop ?? 0;
        });
      } else if (app.isSearching && app.searchQuery) {
        doSearch(path, app.searchQuery);
      } else {
        loadPath(path);
      }
    });
  });

  async function loadPath(path: string) {
    app.setLoading(true);
    app.clearSelection();
    try {
      const entries = await invoke<EntryDto[]>("list_children", { path });
      app.setEntries(entries);
      app.setTabLoadedPath(path);
      requestAnimationFrame(() => {
        if (contentBodyEl) contentBodyEl.scrollTop = app.activeTab?.scrollTop ?? 0;
      });
    } catch (e) {
      app.notify("error", t("contentPanel.couldNotLoad", { error: String(e) }));
      app.setEntries([]);
    } finally {
      app.setLoading(false);
      panelEl?.focus();
    }
  }

  async function doSearch(path: string, query: string) {
    app.setLoading(true);
    app.clearSelection();
    app.setIsSearching(true);
    try {
      const entries = await invoke<EntryDto[]>("search_entries", { path, query, recursive: true });
      app.setEntries(entries);
    } catch (e) {
      app.notify("error", t("contentPanel.searchFailed", { error: String(e) }));
      app.setEntries([]);
    } finally {
      app.setLoading(false);
      panelEl?.focus();
    }
  }

  export function refresh() {
    if (app.currentPath) {
      lastLoadKey = ""; // force reload
      loadPath(app.currentPath);
    }
  }

  export function search(q: string) {
    if (!app.currentPath) return;
    app.setSearch(q);
    doSearch(app.currentPath, q);
  }

  export function clearSearch() {
    app.clearSearch();
    if (app.currentPath) { lastLoadKey = ""; loadPath(app.currentPath); }
  }

  // ── Inline create helpers ─────────────────────────────────

  function findUniqueName(base: string): string {
    const existing = new Set(app.entries.map(e => e.name.toLowerCase()));
    if (!existing.has(base.toLowerCase())) return base;
    const dot = base.lastIndexOf(".");
    const [stem, ext] = dot > 0 ? [base.slice(0, dot), base.slice(dot)] : [base, ""];
    for (let i = 2; i < 200; i++) {
      const candidate = `${stem} (${i})${ext}`;
      if (!existing.has(candidate.toLowerCase())) return candidate;
    }
    return `${stem} ${Date.now()}${ext}`;
  }

  export async function createFolder() {
    if (!app.currentPath) return;
    const name = findUniqueName(t("contentPanel.newFolderName"));
    try {
      await invoke("create_folder", { parent: app.currentPath, name });
      await loadPath(app.currentPath);
      const entry = app.entries.find(e => e.name === name);
      if (entry) { app.setSelection([entry.path]); app.startRename(entry.path, entry.name); }
    } catch (e) { app.notify("error", t("contentPanel.couldNotCreateFolder", { error: String(e) })); }
  }

  export async function createFile() {
    if (!app.currentPath) return;
    const name = findUniqueName(t("contentPanel.newFileName"));
    try {
      await invoke("create_file", { parent: app.currentPath, name });
      await loadPath(app.currentPath);
      const entry = app.entries.find(e => e.name === name);
      if (entry) { app.setSelection([entry.path]); app.startRename(entry.path, entry.name); }
    } catch (e) { app.notify("error", t("contentPanel.couldNotCreateFile", { error: String(e) })); }
  }

  // Svelte action for grid rename — runs once on mount, no reactive reads.
  // Uncontrolled input: the DOM owns the value while editing.
  function initGridRenameInput(node: HTMLInputElement, entry: EntryDto) {
    const initial = app.renaming?.value ?? entry.name;
    node.value = initial;
    node.focus();
    const dot = initial.lastIndexOf(".");
    if (dot > 0 && !entry.isDir) node.setSelectionRange(0, dot); else node.select();
  }

  function commitGridRename(e: Event, entry: EntryDto) {
    if (!app.renaming) return; // already cancelled
    const val = (e.currentTarget as HTMLInputElement).value.trim();
    if (!val || val === entry.name) { app.cancelRename(); return; }
    document.dispatchEvent(new CustomEvent("dogu:rename-commit", { detail: { path: entry.path, newName: val } }));
  }

  // ── Selection handling ────────────────────────────────────
  let lastClickedPath = $state<string | null>(null);

  function handleMousedown(e: MouseEvent, entry: EntryDto) {
    if (e.button !== 0 && e.button !== 2) return;

    if (e.button === 2) {
      // Right-click: select if not already
      if (!app.selectedPaths.has(entry.path)) {
        app.setSelection([entry.path]);
        lastClickedPath = entry.path;
      }
      return;
    }

    if (e.shiftKey && lastClickedPath) {
      app.rangeSelect(lastClickedPath, entry.path);
    } else if (e.ctrlKey || e.metaKey) {
      app.toggleSelection(entry.path);
      lastClickedPath = entry.path;
    } else {
      app.setSelection([entry.path]);
      lastClickedPath = entry.path;
    }
  }

  function handleActivate(entry: EntryDto) {
    if (entry.isDir) {
      app.navigate(entry.path);
    } else {
      invoke("open_path", { path: entry.path }).catch(e => app.notify("error", String(e)));
    }
  }

  // ── Keyboard shortcuts ────────────────────────────────────
  function handleKeyDown(e: KeyboardEvent) {
    const sel = [...app.selectedPaths];

    // Ctrl+F: open quick filter
    if ((e.ctrlKey || e.metaKey) && e.key === "f") {
      e.preventDefault();
      quickFilterOpen = true;
      requestAnimationFrame(() => quickFilterInputEl?.focus());
      return;
    }

    // Escape priority: quick filter → search → selection
    if (e.key === "Escape") {
      if (quickFilterOpen) { closeQuickFilter(); return; }
      if (app.isSearching) { clearSearch(); return; }
      app.clearSelection();
      app.cancelRename();
      contextMenu = null;
      return;
    }

    if (e.key === "Delete" || e.key === "Backspace") {
      if (sel.length > 0) { e.preventDefault(); onDelete(sel); }
    }

    if (e.key === "F2" && sel.length === 1) {
      const entry = app.entries.find(en => en.path === sel[0]);
      if (entry) app.startRename(entry.path, entry.name);
    }

    if ((e.ctrlKey || e.metaKey) && e.key === "c" && sel.length > 0) {
      e.preventDefault(); onCopy(sel);
    }

    if ((e.ctrlKey || e.metaKey) && e.key === "x" && sel.length > 0) {
      e.preventDefault(); onCut(sel);
    }

    if ((e.ctrlKey || e.metaKey) && e.key === "v") {
      e.preventDefault(); onPaste();
    }

    if (e.key === "a" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      app.setSelection(app.entries.map(en => en.path));
    }

    // Arrow key navigation
    if (e.key === "ArrowDown" || e.key === "ArrowUp") {
      e.preventDefault();
      const paths = displayed.map(en => en.path);
      const lastSel = sel[sel.length - 1];
      const idx = paths.indexOf(lastSel ?? "");
      const newIdx = e.key === "ArrowDown"
        ? Math.min(idx + 1, paths.length - 1)
        : Math.max(idx - 1, 0);
      if (newIdx >= 0) {
        if (e.shiftKey) {
          app.rangeSelect(lastClickedPath ?? paths[0], paths[newIdx]);
        } else {
          app.setSelection([paths[newIdx]]);
          lastClickedPath = paths[newIdx];
        }
      }
    }

    if (e.key === "Enter" && sel.length === 1) {
      const entry = app.entries.find(en => en.path === sel[0]);
      if (entry) handleActivate(entry);
    }

    // Type-ahead: printable single char, no modifiers, not renaming
    if (!e.ctrlKey && !e.metaKey && !e.altKey && e.key.length === 1 && !app.renaming) {
      handleTypeahead(e.key);
    }
  }

  // ── Context menu ─────────────────────────────────────────
  function openContextMenu(e: MouseEvent, entry: EntryDto) {
    const sel = app.selectedPaths.has(entry.path) ? [...app.selectedPaths] : [entry.path];
    if (!app.selectedPaths.has(entry.path)) app.setSelection([entry.path]);

    const isMulti = sel.length > 1;
    const isRemote = sel.some(p => p.startsWith("remote://"));
    const allFiles = sel.every(p => !app.entries.find(en => en.path === p)?.isDir);
    const isSingleDir = !isMulti && !!app.entries.find(en => en.path === sel[0])?.isDir;
    const hasM3uDirs = sel.some(p => !!app.entries.find(en => en.path === p)?.isDir);
    const archivePaths = allFiles
      ? sel.filter(p => [".zip", ".7z", ".rar"].includes(app.entries.find(en => en.path === p)?.extension ?? ""))
      : [];
    const hasArchives = archivePaths.length > 0;
    const canCompress = true;
    const hasChdSources = allFiles && sel.some(p => {
      const ext = app.entries.find(en => en.path === p)?.extension ?? "";
      return [".cue", ".gdi", ".toc", ".iso"].includes(ext);
    });
    const hasChdFiles = allFiles && sel.some(p => {
      const ext = app.entries.find(en => en.path === p)?.extension ?? "";
      return ext === ".chd";
    });
    const hasChdDirs = sel.some(p => !!app.entries.find(en => en.path === p)?.isDir);

    const newTabIcon = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M9 3v18M3 9h6"/></svg>`;

    const items: MenuItem[] = [
      { kind: "action", label: isMulti ? t("menu.openCount", { count: sel.length }) : t("menu.open"), icon: openIcon, onclick: () => sel.forEach(p => invoke("open_path", { path: p })) },
    ];

    if (isSingleDir) {
      items.push({ kind: "action", label: t("tabs.openInNewTab"), icon: newTabIcon, onclick: () => app.addTab(sel[0]) });
      if (!isRemote) {
        items.push({ kind: "action", label: t("terminal.openTerminal"), icon: terminalIcon, onclick: () => openTerminalAt(sel[0]) });
      } else {
        items.push({ kind: "action", label: t("terminal.openSshTerminal"), icon: terminalIcon, onclick: () => openRemoteTerminalAt(sel[0]) });
      }
      if (app.isFavorite(sel[0])) {
        items.push({ kind: "action", label: t("menu.removeFromFavorites"), icon: favoriteIcon, onclick: () => app.removeFavorite(sel[0]) });
      } else {
        items.push({ kind: "action", label: t("menu.addToFavorites"), icon: favoriteIcon, onclick: () => app.addFavorite(sel[0]) });
      }
    }

    if (!isMulti) {
      items.push({ kind: "action", label: t("menu.openWith"), icon: openWithIcon, onclick: () => onOpenWith(sel[0]) });
      items.push({ kind: "action", label: t("menu.rename"), shortcut: "F2", icon: renameIcon, onclick: () => app.startRename(entry.path, entry.name) });
    }

    items.push({ kind: "separator" });
    items.push({ kind: "action", label: t("menu.copy"), shortcut: "Ctrl+C", icon: copyIcon, onclick: () => onCopy(sel) });
    items.push({ kind: "action", label: t("menu.cut"),  shortcut: "Ctrl+X", icon: cutIcon,  onclick: () => onCut(sel) });

    if (app.clipboard) {
      items.push({ kind: "action", label: t("menu.paste"), shortcut: "Ctrl+V", icon: pasteIcon, onclick: () => onPaste() });
    }

    if (hasArchives) {
      const archiveStem = archivePaths[0].split(/[\\/]/).pop()?.replace(/\.[^.]+$/, "") ?? archivePaths[0];
      const extractToFolderLabel = archivePaths.length === 1
        ? t("menu.extractToNamed", { name: archiveStem })
        : t("menu.extractToSeparateFolders");

      items.push({ kind: "separator" });
      items.push({ kind: "action", label: t("menu.extractHere"), icon: extractIcon, onclick: () => onExtractHere(archivePaths) });
      items.push({ kind: "action", label: extractToFolderLabel, icon: extractToFolderIcon, onclick: () => onExtractToFolder(archivePaths) });
      items.push({ kind: "action", label: t("menu.extractTo"), icon: extractToIcon, onclick: () => onExtractTo(archivePaths) });
    }

    if (canCompress) {
      items.push({ kind: "separator" });
      items.push({ kind: "action", label: t("menu.compressToZip"), icon: compressIcon, onclick: () => onCompressQuick(sel) });
      items.push({ kind: "action", label: t("menu.addToArchive"), icon: compressIcon, onclick: () => onCompress(sel) });
    }

    if (hasChdSources || hasChdFiles || hasChdDirs) {
      items.push({ kind: "separator" });
      items.push({
        kind: "action",
        label: hasChdSources ? t("menu.convertToChd") : hasChdFiles ? t("menu.restoreFromChd") : t("menu.convertToChd"),
        icon: chdIcon,
        onclick: () => onChd(sel),
      });
    }

    if (hasM3uDirs) {
      const m3uIcon = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg>`;
      items.push({ kind: "separator" });
      items.push({
        kind: "action",
        label: t("m3u.menuItem"),
        icon: m3uIcon,
        onclick: () => onM3u(sel.filter(p => !!app.entries.find(en => en.path === p)?.isDir)),
      });
    }

    items.push({ kind: "separator" });
    items.push({ kind: "action", label: t("menu.properties"), icon: propsIcon, onclick: () => onProperties(sel) });
    items.push({ kind: "separator" });
    items.push({ kind: "action", label: t("menu.delete"), shortcut: "Del", icon: deleteIcon, danger: true, onclick: () => onDelete(sel) });

    contextMenu = { x: e.clientX, y: e.clientY, items };
  }

  // ── Sort header click ────────────────────────────────────
  function clickSort(key: ContentSortKey) {
    if (app.sortKey === key) {
      app.setSort(key, app.sortDir === "asc" ? "desc" : "asc");
    } else {
      app.setSort(key, "asc");
    }
  }

  // ── Click on empty area ──────────────────────────────────
  function handlePanelClick(e: MouseEvent) {
    if ((e.target as HTMLElement).closest("[data-path]")) return;
    if (consumeNextPanelClick) { consumeNextPanelClick = false; return; }
    app.clearSelection();
    contextMenu = null;
  }

  function handlePanelContextMenu(e: MouseEvent) {
    if ((e.target as HTMLElement).closest("[data-path]")) return;
    e.preventDefault();
    const items: MenuItem[] = [];
    if (app.clipboard) items.push({ kind: "action", label: t("menu.paste"), icon: pasteIcon, onclick: () => onPaste() });
    items.push({ kind: "action", label: t("menu.newFolder"), icon: newFolderIcon, onclick: () => createFolder() });
    items.push({ kind: "action", label: t("menu.newFile"), icon: newFileIcon, onclick: () => createFile() });
    items.push({ kind: "action", label: t("menu.refresh"), shortcut: "F5", icon: refreshIcon, onclick: () => refresh() });
    if (app.currentPath && !app.currentPath.startsWith("remote://")) {
      items.push({ kind: "separator" });
      items.push({ kind: "action", label: t("terminal.openTerminal"), icon: terminalIcon, onclick: () => openTerminalAt(app.currentPath!) });
    }
    if (items.length > 0) contextMenu = { x: e.clientX, y: e.clientY, items };
  }

  // Inline SVG icons for context menu
  const openIcon     = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M18 13v6a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2V8a2 2 0 0 1 2-2h6"/><polyline points="15 3 21 3 21 9"/><line x1="10" y1="14" x2="21" y2="3"/></svg>`;
  const openWithIcon = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="3"/><path d="M19.4 15a1.65 1.65 0 0 0 .33 1.82l.06.06a2 2 0 0 1-2.83 2.83l-.06-.06a1.65 1.65 0 0 0-1.82-.33"/></svg>`;
  const renameIcon   = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M11 4H4a2 2 0 0 0-2 2v14a2 2 0 0 0 2 2h14a2 2 0 0 0 2-2v-7"/><path d="M18.5 2.5a2.121 2.121 0 0 1 3 3L12 15l-4 1 1-4 9.5-9.5z"/></svg>`;
  const copyIcon     = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="9" y="9" width="13" height="13" rx="2" ry="2"/><path d="M5 15H4a2 2 0 0 1-2-2V4a2 2 0 0 1 2-2h9a2 2 0 0 1 2 2v1"/></svg>`;
  const cutIcon      = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="6" cy="20" r="2"/><circle cx="20" cy="4" r="2"/><path d="M4 20l14.5-14.5M10.5 13.5l5.5 5.5M2 4l12 12"/><circle cx="6" cy="4" r="2"/></svg>`;
  const pasteIcon    = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M16 4h2a2 2 0 0 1 2 2v14a2 2 0 0 1-2 2H6a2 2 0 0 1-2-2V6a2 2 0 0 1 2-2h2"/><rect x="8" y="2" width="8" height="4" rx="1" ry="1"/></svg>`;
  const compressIcon = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="8 7 12 3 16 7"/><line x1="12" y1="3" x2="12" y2="12"/><path d="M20.88 18.09A5 5 0 0 0 18 9h-1.26A8 8 0 1 0 3 16.29"/></svg>`;
  const extractIcon  = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="8 17 12 21 16 17"/><line x1="12" y1="12" x2="12" y2="21"/><path d="M20.88 18.09A5 5 0 0 0 18 9h-1.26A8 8 0 1 0 3 16.29"/></svg>`;
  const extractToIcon = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/><polyline points="12 11 12 17"/><polyline points="9 14 12 17 15 14"/></svg>`;
  const extractToFolderIcon = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/><line x1="12" y1="11" x2="12" y2="17"/><line x1="9" y1="14" x2="15" y2="14"/></svg>`;
  const chdIcon       = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="9"/><circle cx="12" cy="12" r="3"/><line x1="12" y1="3" x2="12" y2="9"/><line x1="12" y1="15" x2="12" y2="21"/><line x1="3" y1="12" x2="9" y2="12"/><line x1="15" y1="12" x2="21" y2="12"/></svg>`;
  const propsIcon    = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="10"/><line x1="12" y1="8" x2="12" y2="12"/><line x1="12" y1="16" x2="12.01" y2="16"/></svg>`;
  const deleteIcon   = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="3 6 5 6 21 6"/><path d="M19 6l-1 14a2 2 0 0 1-2 2H8a2 2 0 0 1-2-2L5 6"/><path d="M10 11v6M14 11v6M9 6V4h6v2"/></svg>`;
  const newFolderIcon = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/><line x1="12" y1="11" x2="12" y2="17"/><line x1="9" y1="14" x2="15" y2="14"/></svg>`;
  const newFileIcon   = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/><polyline points="13 2 13 9 20 9"/><line x1="12" y1="13" x2="12" y2="19"/><line x1="9" y1="16" x2="15" y2="16"/></svg>`;
  const refreshIcon  = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="23 4 23 10 17 10"/><path d="M20.49 15a9 9 0 1 1-2.12-9.36L23 10"/></svg>`;
  const favoriteIcon = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polygon points="12 2 15.09 8.26 22 9.27 17 14.14 18.18 21.02 12 17.77 5.82 21.02 7 14.14 2 9.27 8.91 8.26 12 2"/></svg>`;
  const terminalIcon = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><polyline points="4 17 10 11 4 5"/><line x1="12" y1="19" x2="20" y2="19"/></svg>`;
</script>

<!-- svelte-ignore a11y_click_events_have_key_events a11y_no_static_element_interactions -->
<div
  class="content-panel"
  class:col-resizing={resizingCol !== null}
  bind:this={panelEl}
  role="grid"
  aria-label={t("contentPanel.fileList")}
  tabindex="0"
  style="--col-name:{app.columnWidths.name}px; --col-type:{app.columnWidths.type}px; --col-size:{app.columnWidths.size}px; --col-modified:{app.columnWidths.modified}px;"
  onkeydown={handleKeyDown}
  onclick={handlePanelClick}
  oncontextmenu={handlePanelContextMenu}
  onwheel={handleContentWheel}
>
  <!-- ── Column headers (list view only) ── -->
  {#if app.viewMode === "list"}
  <div class="content-header" role="row">
    <div class="col-gutter"></div>
    <button class="col-hdr col-hdr--name" onclick={() => clickSort("name")} role="columnheader">
      {t("contentPanel.name")}
      {#if app.sortKey === "name"}<span class="sort-arrow">{app.sortDir === "asc" ? "↑" : "↓"}</span>{/if}
    </button>
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="col-resize-handle" role="separator" onmousedown={(e) => startColResize(e, "name")}></div>
    <button class="col-hdr col-hdr--type" onclick={() => clickSort("type")} role="columnheader">
      {t("contentPanel.type")}
      {#if app.sortKey === "type"}<span class="sort-arrow">{app.sortDir === "asc" ? "↑" : "↓"}</span>{/if}
    </button>
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="col-resize-handle" role="separator" onmousedown={(e) => startColResize(e, "type")}></div>
    <button class="col-hdr col-hdr--size" onclick={() => clickSort("size")} role="columnheader">
      {t("contentPanel.size")}
      {#if app.sortKey === "size"}<span class="sort-arrow">{app.sortDir === "asc" ? "↑" : "↓"}</span>{/if}
    </button>
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="col-resize-handle" role="separator" onmousedown={(e) => startColResize(e, "size")}></div>
    <button class="col-hdr col-hdr--modified" onclick={() => clickSort("modified")} role="columnheader">
      {t("contentPanel.modified")}
      {#if app.sortKey === "modified"}<span class="sort-arrow">{app.sortDir === "asc" ? "↑" : "↓"}</span>{/if}
    </button>
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="col-resize-handle" role="separator" onmousedown={(e) => startColResize(e, "modified")}></div>
  </div>
  {/if}

  <!-- ── Quick filter bar ── -->
  {#if quickFilterOpen}
    <div class="quick-filter-bar">
      <svg class="qf-icon" width="12" height="12" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2">
        <circle cx="11" cy="11" r="8"/><line x1="21" y1="21" x2="16.65" y2="16.65"/>
      </svg>
      <input
        bind:this={quickFilterInputEl}
        class="qf-input"
        type="text"
        placeholder={t("contentPanel.quickFilterPlaceholder")}
        bind:value={quickFilter}
        onkeydown={(e) => {
          if (e.key === "Escape") { e.stopPropagation(); closeQuickFilter(); }
          if (e.key === "ArrowDown" || e.key === "ArrowUp") { e.preventDefault(); panelEl?.focus(); }
          if (e.key === "Enter") { panelEl?.focus(); }
        }}
      />
      {#if quickFilter}
        <span class="qf-count">{t("contentPanel.quickFilterCount", { shown: displayed.length, total: sorted.length })}</span>
      {/if}
      <button class="qf-close" onclick={closeQuickFilter} aria-label="Close">✕</button>
    </div>
  {/if}

  <!-- ── Content area (rubber-band div injected imperatively into this element) ── -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="content-body scrollbar-thin"
    bind:this={contentBodyEl}
    onmousedown={handleBodyMousedown}
    onscroll={saveScrollPosition}
    role="presentation"
  >
    {#if app.isLoading}
      <div class="content-state">
        <span class="loading-spinner"></span>
        <span>{t("contentPanel.loading")}</span>
      </div>

    {:else if !app.currentPath}
      <div class="content-state">
        <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
        </svg>
        <span>{t("contentPanel.selectLocation")}</span>
      </div>

    {:else if displayed.length === 0}
      <div class="content-state">
        <svg width="40" height="40" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1">
          <path d="M22 19a2 2 0 0 1-2 2H4a2 2 0 0 1-2-2V5a2 2 0 0 1 2-2h5l2 3h9a2 2 0 0 1 2 2z"/>
        </svg>
        <span>{app.isSearching ? t("contentPanel.noResults") : t("contentPanel.folderEmpty")}</span>
      </div>

    {:else}
      {#if app.viewMode === "list"}
        <div class="file-list" role="rowgroup">
          {#each displayed as entry (entry.path)}
            <FileRow
              {entry}
              isSelected={app.selectedPaths.has(entry.path)}
              onActivate={handleActivate}
              onContextMenu={openContextMenu}
              onMousedown={handleMousedown}
            />
          {/each}
        </div>

      {:else}
        <!-- Grid view -->
        <div class="file-grid" role="rowgroup">
          {#each displayed as entry (entry.path)}
            <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
            <div
              class="grid-item"
              class:grid-item--selected={app.selectedPaths.has(entry.path)}
              data-path={entry.path}
              role="gridcell"
              tabindex="0"
              onmousedown={(e) => handleMousedown(e, entry)}
              ondblclick={() => { if (app.renaming?.path !== entry.path) handleActivate(entry); }}
              oncontextmenu={(e) => { e.preventDefault(); openContextMenu(e, entry); }}
            >
              <div class="grid-icon" aria-hidden="true">
                {#if entry.isDir}
                  <svg width="32" height="32" viewBox="0 0 24 24" fill="currentColor" class="icon-dir-lg">
                    <path d="M10 4H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2h-8l-2-2z"/>
                  </svg>
                {:else}
                  <svg width="28" height="28" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.2" class="icon-file-lg">
                    <path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/>
                    <polyline points="13 2 13 9 20 9"/>
                  </svg>
                {/if}
              </div>
              {#if app.renaming?.path === entry.path}
                <input
                  class="grid-rename-input"
                  type="text"
                  use:initGridRenameInput={entry}
                  onblur={(e) => commitGridRename(e, entry)}
                  onkeydown={(e) => {
                    if (e.key === "Enter") { e.preventDefault(); commitGridRename(e, entry); }
                    if (e.key === "Escape") { e.preventDefault(); app.cancelRename(); }
                    e.stopPropagation();
                  }}
                  onclick={(e) => e.stopPropagation()}
                  onmousedown={(e) => e.stopPropagation()}
                  aria-label={t("contentPanel.renameInput")}
                />
              {:else}
                <span class="grid-name">{entry.name}</span>
              {/if}
            </div>
          {/each}
        </div>
      {/if}
    {/if}
  </div>

  <!-- ── Status bar ── -->
  <div class="status-bar">
    <span class="status-count">
      {#if app.selectedPaths.size > 0}
        {t("contentPanel.itemsSelected", { selected: app.selectedPaths.size, total: displayed.length })}
      {:else}
        {tn("contentPanel.itemCount", "contentPanel.itemCountPlural", displayed.length)}
      {/if}
    </span>
    {#if app.isSearching}
      <span class="status-search">
        {t("contentPanel.searchLabel", { query: app.searchQuery ?? "" })}
        <button class="status-clear-search" onclick={clearSearch}>
          {t("contentPanel.clearSearchAction")}
        </button>
      </span>
    {/if}
    {#if app.clipboard}
      <span class="status-clipboard">
        {#if app.clipboard.operation === "copy"}
          {tn("contentPanel.clipboardCopy", "contentPanel.clipboardCopyPlural", app.clipboard.paths.length)}
        {:else}
          {tn("contentPanel.clipboardCut", "contentPanel.clipboardCutPlural", app.clipboard.paths.length)}
        {/if}
      </span>
    {/if}
  </div>
</div>

<!-- ── Context menu ── -->
{#if contextMenu}
  <ContextMenu
    x={contextMenu.x}
    y={contextMenu.y}
    items={contextMenu.items}
    onclose={() => (contextMenu = null)}
  />
{/if}

<style>
  .content-panel {
    display: flex;
    flex-direction: column;
    flex: 1;
    min-width: 0;
    overflow: hidden;
    outline: none;
    background: var(--bg);
    position: relative; /* contains the rubber-band absolute div */
  }

  .content-panel.col-resizing { cursor: col-resize; user-select: none; }

  /* ── Column headers ── */
  .content-header {
    display: flex;
    align-items: center;
    height: 26px;
    padding: 0 8px;
    background: var(--surface);
    border-bottom: 1px solid var(--line);
    flex-shrink: 0;
  }

  .col-hdr {
    display: flex;
    align-items: center;
    gap: 4px;
    background: none;
    border: none;
    font-size: 12px;
    font-weight: 500;
    color: var(--text-muted);
    cursor: pointer;
    padding: 0 4px;
    height: 100%;
    white-space: nowrap;

    &:hover { color: var(--text); }
  }

  .col-gutter        { width: 16px; flex-shrink: 0; }
  .col-hdr--name     { width: var(--col-name, 280px); flex-shrink: 0; }
  .col-hdr--type     { width: var(--col-type, 80px); flex-shrink: 0; }
  .col-hdr--size     { width: var(--col-size, 90px); flex-shrink: 0; }
  .col-hdr--modified { width: var(--col-modified, 140px); flex-shrink: 0; padding-left: 8px; }

  .sort-arrow { color: var(--accent); font-size: 11px; }

  /* ── Column resize handle ── */
  .col-resize-handle {
    flex-shrink: 0;
    width: 5px;
    height: 100%;
    cursor: col-resize;
    position: relative;

    &::after {
      content: "";
      position: absolute;
      left: 2px;
      top: 20%;
      height: 60%;
      width: 1px;
      background: var(--line-strong);
      opacity: 0.5;
    }

    &:hover::after { opacity: 1; background: var(--accent); }
  }

  /* ── Content body ── */
  .content-body {
    flex: 1;
    overflow-y: auto;
    padding: 4px;
    padding-bottom: 60px; /* always leave empty space below items for rubber-band start */
    position: relative;
    user-select: none;
  }

  /* ── Rubber-band selection rect (div injected imperatively — needs :global) ── */
  :global(.rubber-band) {
    position: absolute;
    pointer-events: none;
    z-index: 50;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    border: 1px solid var(--accent);
    border-radius: 2px;
  }

  /* ── Empty / loading states ── */
  .content-state {
    display: flex;
    flex-direction: column;
    align-items: center;
    justify-content: center;
    gap: 12px;
    height: 200px;
    color: var(--text-subtle);
    font-size: 13px;
  }

  .loading-spinner {
    width: 24px;
    height: 24px;
    border: 2.5px solid var(--line-strong);
    border-top-color: var(--accent);
    border-radius: 50%;
    animation: spin 0.7s linear infinite;
  }

  /* ── File list view ── */
  .file-list {
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding-left: 16px;   /* left gutter — clickable empty space for rubber-band */
    align-items: flex-start; /* rows shrink to their column widths, leaving right-side space */
  }

  /* ── Grid view ── */
  .file-grid {
    display: grid;
    grid-template-columns: repeat(auto-fill, minmax(100px, 1fr));
    gap: 4px;
    padding: 4px;
  }

  .grid-item {
    display: flex;
    flex-direction: column;
    align-items: center;
    gap: 5px;
    padding: 8px 6px;
    border-radius: 6px;
    cursor: pointer;
    user-select: none;
    border: 1px solid transparent;
    text-align: center;

    &:hover { background: var(--surface-hover); border-color: var(--line); }
  }

  .grid-item--selected {
    background: var(--accent-soft);
    border-color: var(--accent-border);
  }

  .grid-icon { display: flex; align-items: center; justify-content: center; }
  .icon-dir-lg { color: #e0a030; }
  .icon-file-lg { color: var(--text-muted); }

  .grid-name {
    font-size: 11px;
    color: var(--text);
    line-height: 1.3;
    word-break: break-all;
    overflow: hidden;
    display: -webkit-box;
    -webkit-line-clamp: 2;
    line-clamp: 2;
    -webkit-box-orient: vertical;
  }

  .grid-rename-input {
    width: 100%;
    padding: 2px 5px;
    border: 1.5px solid var(--accent);
    border-radius: 3px;
    background: var(--surface);
    color: var(--text);
    font: inherit;
    font-size: 11px;
    text-align: center;
    outline: none;
    box-shadow: 0 0 0 2px var(--accent-soft);
    margin-top: 1px;
    box-sizing: border-box;
  }

  /* ── Quick filter bar ── */
  .quick-filter-bar {
    display: flex;
    align-items: center;
    gap: 6px;
    padding: 3px 8px;
    background: color-mix(in srgb, var(--accent) 8%, var(--surface));
    border-bottom: 1px solid color-mix(in srgb, var(--accent) 30%, var(--line));
    flex-shrink: 0;
    height: 28px;
  }

  .qf-icon {
    color: var(--accent);
    flex-shrink: 0;
  }

  .qf-input {
    flex: 1;
    background: none;
    border: none;
    outline: none;
    font-size: 13px;
    color: var(--text);
    min-width: 0;

    &::placeholder { color: var(--text-subtle); }
  }

  .qf-count {
    font-size: 11px;
    color: var(--text-muted);
    flex-shrink: 0;
  }

  .qf-close {
    background: none;
    border: none;
    color: var(--text-subtle);
    font-size: 11px;
    cursor: pointer;
    padding: 0 2px;
    line-height: 1;
    flex-shrink: 0;

    &:hover { color: var(--text); }
  }

  /* ── Status bar ── */
  .status-bar {
    display: flex;
    align-items: center;
    gap: 12px;
    padding: 0 10px;
    height: 22px;
    font-size: 11px;
    color: var(--text-muted);
    background: var(--surface);
    border-top: 1px solid var(--line);
    flex-shrink: 0;
    overflow: hidden;
  }

  .status-count { flex-shrink: 0; }

  .status-search {
    display: flex;
    align-items: center;
    gap: 5px;
    color: var(--accent);
  }

  .status-clear-search {
    background: none;
    border: none;
    color: var(--accent);
    font: inherit;
    font-size: 11px;
    cursor: pointer;
    padding: 0;
    text-decoration: underline;
  }

  .status-clipboard {
    margin-left: auto;
    color: var(--text-subtle);
    font-style: italic;
  }
</style>
