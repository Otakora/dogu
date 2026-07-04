<script lang="ts">
  import { invoke } from "@tauri-apps/api/core";
  import { getContext, onDestroy, untrack } from "svelte";
  import { app } from "../../stores/app.svelte.js";
  import type { PaneView } from "../../stores/app.svelte.js";
  import type { EntryDto, ContentSortKey } from "../../types/index.js";
  import FileRow from "./FileRow.svelte";
  import ContextMenu from "../ui/ContextMenu.svelte";
  import type { MenuItem } from "../ui/ContextMenu.svelte";
  import { t, tn } from "../../i18n/index.js";
  import { openTerminalAt, openRemoteTerminalAt } from "../../utils/terminal.js";
  import { ghostToEntry, normalizePath } from "../../utils/ghosts.js";

  const pane = getContext<PaneView>("pane");

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
    resizingCol = { key, startX: e.clientX, startW: pane.columnWidths[key] };
    window.addEventListener("mousemove", onColResizeMove);
    window.addEventListener("mouseup", onColResizeEnd);
  }

  function onColResizeMove(e: MouseEvent) {
    if (!resizingCol) return;
    const delta = e.clientX - resizingCol.startX;
    const newW = Math.max(50, Math.round(resizingCol.startW + delta));
    pane.setColumnWidth(resizingCol.key, newW);
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
    const next = Math.max(0.6, Math.min(2.0, Math.round((pane.contentZoom + delta) * 10) / 10));
    pane.setContentZoom(next);
  }

  // ── Rubber-band selection ─────────────────────────────────
  let rbDiv: HTMLDivElement | null = null;
  let rbSX = 0, rbSY = 0;
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

  function setRubberBandRect(mx: number, my: number) {
    if (!rbDiv || !contentBodyEl || !panelEl) return;
    const bodyRect  = contentBodyEl.getBoundingClientRect();
    const panelRect = panelEl.getBoundingClientRect();
    const ex = mx - bodyRect.left + contentBodyEl.scrollLeft;
    const ey = my - bodyRect.top  + contentBodyEl.scrollTop;
    const dw = Math.abs(ex - rbSX);
    const dh = Math.abs(ey - rbSY);
    if (dw < 3 && dh < 3) { rbDiv.style.display = "none"; return; }
    const bLeft = bodyRect.left - panelRect.left;
    const bTop  = bodyRect.top  - panelRect.top;
    const startPX = rbSX - contentBodyEl.scrollLeft + bLeft;
    const startPY = rbSY - contentBodyEl.scrollTop  + bTop;
    const endPX   = mx - panelRect.left;
    const endPY   = my - panelRect.top;
    rbDiv.style.display = "block";
    rbDiv.style.left   = `${Math.min(startPX, endPX)}px`;
    rbDiv.style.top    = `${Math.min(startPY, endPY)}px`;
    rbDiv.style.width  = `${Math.abs(endPX - startPX)}px`;
    rbDiv.style.height = `${Math.abs(endPY - startPY)}px`;
  }

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
    pane.setSelection(selected);
  }

  function handleBodyMousedown(e: MouseEvent) {
    if (e.button !== 0) return;
    if ((e.target as HTMLElement).closest("[data-path]")) return;
    if (!contentBodyEl) return;
    ensureRubberBand();
    const bodyRect = contentBodyEl.getBoundingClientRect();
    rbSX = e.clientX - bodyRect.left + contentBodyEl.scrollLeft;
    rbSY = e.clientY - bodyRect.top  + contentBodyEl.scrollTop;
    rbActive = true;
    lastMouse = { x: e.clientX, y: e.clientY };
    if (rbDiv) rbDiv.style.display = "none";
    pane.clearSelection();
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

  // ── Ghost overlay ────────────────────────────────────────
  // Predicted outputs of queued operations that will land in this folder.
  // Hidden while searching (search results come straight from the backend).
  // Ghosts whose path collides with a real entry (or another ghost) are dropped
  // so the list never renders duplicate keys.
  const ghostEntries = $derived.by(() => {
    if (pane.isSearching || !pane.currentPath) return [];
    // Read the reactive opQueue getter directly (the same signal JobsPanel uses)
    // and filter to this folder inline. Going through a store method here does
    // NOT register opQueue as a dependency of this derived, so the overlay would
    // never update — read the getter directly instead.
    const dirKey = normalizePath(pane.currentPath);
    const seen = new Set(pane.entries.map((e) => normalizePath(e.path)));
    const result: EntryDto[] = [];
    for (const op of app.opQueue) {
      for (const g of op.produces) {
        if (normalizePath(g.parentDir) !== dirKey) continue;
        const key = normalizePath(g.path);
        if (seen.has(key)) continue;
        seen.add(key);
        result.push(ghostToEntry(g));
      }
    }
    return result;
  });
  const combinedEntries = $derived([...pane.entries, ...ghostEntries]);

  /** Looks up a display entry (real or ghost) by path. */
  function entryOf(path: string): EntryDto | undefined {
    return combinedEntries.find(en => en.path === path);
  }

  // ── Sorted entries ───────────────────────────────────────
  const sorted = $derived(sortEntries(combinedEntries, pane.sortKey, pane.sortDir));

  // ── Quick filter ──────────────────────────────────────────
  let quickFilter = $state("");
  let quickFilterOpen = $state(false);
  let quickFilterInputEl = $state<HTMLInputElement | undefined>(undefined);

  const displayed = $derived(
    quickFilter
      ? sorted.filter(e => e.name.toLowerCase().includes(quickFilter.toLowerCase()))
      : sorted
  );

  let typeaheadBuffer = "";
  let typeaheadTimer: ReturnType<typeof setTimeout> | null = null;

  function handleTypeahead(char: string) {
    typeaheadBuffer += char;
    if (typeaheadTimer) clearTimeout(typeaheadTimer);
    typeaheadTimer = setTimeout(() => { typeaheadBuffer = ""; }, 800);
    const query = typeaheadBuffer.toLowerCase();
    const match = displayed.find(e => e.name.toLowerCase().startsWith(query));
    if (match) {
      pane.setSelection([match.path]);
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

  // ── Load on path / tab change ────────────────────────────
  let lastLoadKey = "";
  let scrollSaveTimer: ReturnType<typeof setTimeout> | undefined;

  function saveScrollPosition() {
    clearTimeout(scrollSaveTimer);
    scrollSaveTimer = setTimeout(() => {
      if (contentBodyEl) pane.setTabScrollTop(pane.activeTabIdx, contentBodyEl.scrollTop);
    }, 100);
  }

  $effect(() => {
    const path = pane.currentPath;
    const tabId = pane.activeTabId;
    if (!path) return;
    const key = `${tabId}::${path}`;
    if (key === lastLoadKey) return;
    lastLoadKey = key;

    untrack(() => {
      const loadedPath = pane.activeTab?.loadedPath;
      if (loadedPath === path) {
        requestAnimationFrame(() => {
          if (contentBodyEl) contentBodyEl.scrollTop = pane.activeTab?.scrollTop ?? 0;
        });
      } else if (pane.isSearching && pane.searchQuery) {
        doSearch(path, pane.searchQuery);
      } else {
        loadPath(path);
      }
    });
  });

  async function loadPath(path: string) {
    pane.setLoading(true);
    pane.clearSelection();
    try {
      const entries = await invoke<EntryDto[]>("list_children", { path });
      pane.setEntries(entries);
      pane.setTabLoadedPath(path);
      requestAnimationFrame(() => {
        if (contentBodyEl) contentBodyEl.scrollTop = pane.activeTab?.scrollTop ?? 0;
      });
    } catch (e) {
      // A ghost directory doesn't exist on disk yet — show its predicted
      // children (from the overlay) without a spurious load error.
      const isGhostDir = app.queueGhosts.some(g => g.isDir && normalizePath(g.path) === normalizePath(path));
      if (!isGhostDir) app.notify("error", t("contentPanel.couldNotLoad", { error: String(e) }));
      pane.setEntries([]);
    } finally {
      pane.setLoading(false);
      panelEl?.focus();
    }
  }

  async function doSearch(path: string, query: string) {
    pane.setLoading(true);
    pane.clearSelection();
    pane.setIsSearching(true);
    try {
      const entries = await invoke<EntryDto[]>("search_entries", { path, query, recursive: true });
      pane.setEntries(entries);
    } catch (e) {
      app.notify("error", t("contentPanel.searchFailed", { error: String(e) }));
      pane.setEntries([]);
    } finally {
      pane.setLoading(false);
      panelEl?.focus();
    }
  }

  export function refresh() {
    if (pane.currentPath) {
      lastLoadKey = "";
      loadPath(pane.currentPath);
    }
  }

  export function search(q: string) {
    if (!pane.currentPath) return;
    pane.setSearch(q);
    doSearch(pane.currentPath, q);
  }

  export function clearSearch() {
    pane.clearSearch();
    if (pane.currentPath) { lastLoadKey = ""; loadPath(pane.currentPath); }
  }

  // ── Inline create helpers ─────────────────────────────────
  function findUniqueName(base: string): string {
    const existing = new Set(pane.entries.map(e => e.name.toLowerCase()));
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
    if (!pane.currentPath) return;
    const name = findUniqueName(t("contentPanel.newFolderName"));
    try {
      await invoke("create_folder", { parent: pane.currentPath, name });
      await loadPath(pane.currentPath);
      const entry = pane.entries.find(e => e.name === name);
      if (entry) { pane.setSelection([entry.path]); pane.startRename(entry.path, entry.name); }
    } catch (e) { app.notify("error", t("contentPanel.couldNotCreateFolder", { error: String(e) })); }
  }

  export async function createFile() {
    if (!pane.currentPath) return;
    const name = findUniqueName(t("contentPanel.newFileName"));
    try {
      await invoke("create_file", { parent: pane.currentPath, name });
      await loadPath(pane.currentPath);
      const entry = pane.entries.find(e => e.name === name);
      if (entry) { pane.setSelection([entry.path]); pane.startRename(entry.path, entry.name); }
    } catch (e) { app.notify("error", t("contentPanel.couldNotCreateFile", { error: String(e) })); }
  }

  function initGridRenameInput(node: HTMLInputElement, entry: EntryDto) {
    const initial = pane.renaming?.value ?? entry.name;
    node.value = initial;
    node.focus();
    const dot = initial.lastIndexOf(".");
    if (dot > 0 && !entry.isDir) node.setSelectionRange(0, dot); else node.select();
  }

  function commitGridRename(e: Event, entry: EntryDto) {
    if (!pane.renaming) return;
    const val = (e.currentTarget as HTMLInputElement).value.trim();
    if (!val || val === entry.name) { pane.cancelRename(); return; }
    document.dispatchEvent(new CustomEvent("dogu:rename-commit", { detail: { path: entry.path, newName: val } }));
  }

  // ── Selection handling ────────────────────────────────────
  let lastClickedPath = $state<string | null>(null);

  function handleMousedown(e: MouseEvent, entry: EntryDto) {
    if (e.button !== 0 && e.button !== 2) return;

    if (e.button === 2) {
      if (!pane.selectedPaths.has(entry.path)) {
        pane.setSelection([entry.path]);
        lastClickedPath = entry.path;
      }
      return;
    }

    if (e.shiftKey && lastClickedPath) {
      pane.rangeSelect(lastClickedPath, entry.path);
    } else if (e.ctrlKey || e.metaKey) {
      pane.toggleSelection(entry.path);
      lastClickedPath = entry.path;
    } else {
      pane.setSelection([entry.path]);
      lastClickedPath = entry.path;
    }
  }

  function handleActivate(entry: EntryDto) {
    if (entry.isGhost && !entry.isDir) return; // can't open a file that doesn't exist yet
    if (entry.isDir) {
      // Ghost directories are navigable: loadPath shows their predicted children.
      pane.navigate(entry.path);
    } else {
      invoke("open_path", { path: entry.path }).catch(e => app.notify("error", String(e)));
    }
  }

  // ── Keyboard shortcuts ────────────────────────────────────
  function handleKeyDown(e: KeyboardEvent) {
    const sel = [...pane.selectedPaths];

    if ((e.ctrlKey || e.metaKey) && e.key === "f") {
      e.preventDefault();
      quickFilterOpen = true;
      requestAnimationFrame(() => quickFilterInputEl?.focus());
      return;
    }

    if (e.key === "Escape") {
      if (quickFilterOpen) { closeQuickFilter(); return; }
      if (pane.isSearching) { clearSearch(); return; }
      pane.clearSelection();
      pane.cancelRename();
      contextMenu = null;
      return;
    }

    if (e.key === "Delete" || e.key === "Backspace") {
      if (sel.length > 0) { e.preventDefault(); onDelete(sel); }
    }

    if (e.key === "F2" && sel.length === 1) {
      const entry = pane.entries.find(en => en.path === sel[0]);
      if (entry) pane.startRename(entry.path, entry.name);
    }

    if ((e.ctrlKey || e.metaKey) && e.key === "c" && sel.length > 0) { e.preventDefault(); onCopy(sel); }
    if ((e.ctrlKey || e.metaKey) && e.key === "x" && sel.length > 0) { e.preventDefault(); onCut(sel); }
    if ((e.ctrlKey || e.metaKey) && e.key === "v") { e.preventDefault(); onPaste(); }

    if (e.key === "a" && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      pane.setSelection(pane.entries.map(en => en.path));
    }

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
          pane.rangeSelect(lastClickedPath ?? paths[0], paths[newIdx]);
        } else {
          pane.setSelection([paths[newIdx]]);
          lastClickedPath = paths[newIdx];
        }
      }
    }

    if (e.key === "Enter" && sel.length === 1) {
      const entry = pane.entries.find(en => en.path === sel[0]);
      if (entry) handleActivate(entry);
    }

    if (!e.ctrlKey && !e.metaKey && !e.altKey && e.key.length === 1 && !pane.renaming) {
      handleTypeahead(e.key);
    }
  }

  // ── Context menu ─────────────────────────────────────────
  function openContextMenu(e: MouseEvent, entry: EntryDto) {
    const sel = pane.selectedPaths.has(entry.path) ? [...pane.selectedPaths] : [entry.path];
    if (!pane.selectedPaths.has(entry.path)) pane.setSelection([entry.path]);

    const isMulti = sel.length > 1;
    const isRemote = sel.some(p => p.startsWith("remote://"));
    // Ghosts are predicted outputs of queued ops — they can be re-queued into
    // further operations, but not opened, renamed, inspected or navigated.
    const selHasGhost = sel.some(p => entryOf(p)?.isGhost);
    const normalizeExt = (p: string) => {
      const ext = entryOf(p)?.extension ?? "";
      return ext.startsWith(".") ? ext : ext ? `.${ext}` : "";
    };
    const allFiles = sel.every(p => !entryOf(p)?.isDir);
    const isSingleDir = !isMulti && !!entryOf(sel[0])?.isDir;
    const hasM3uDirs = sel.some(p => !!entryOf(p)?.isDir);
    const archivePaths = allFiles
      ? sel.filter(p => [".zip", ".7z", ".rar"].includes(normalizeExt(p)))
      : [];
    const hasArchives = archivePaths.length > 0;
    const canCompress = true;
    const hasChdSources = allFiles && sel.some(p => [".cue", ".gdi", ".toc", ".iso"].includes(normalizeExt(p)));
    const hasChdFiles = allFiles && sel.some(p => normalizeExt(p) === ".chd");
    const hasChdDirs = sel.some(p => !!entryOf(p)?.isDir);

    const newTabIcon = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2"/><path d="M9 3v18M3 9h6"/></svg>`;
    const splitPaneIcon = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><rect x="3" y="3" width="18" height="18" rx="2"/><line x1="12" y1="3" x2="12" y2="21"/></svg>`;

    const items: MenuItem[] = [];
    if (!selHasGhost) {
      items.push({ kind: "action", label: isMulti ? t("menu.openCount", { count: sel.length }) : t("menu.open"), icon: openIcon, onclick: () => sel.forEach(p => invoke("open_path", { path: p })) });
    }

    if (isSingleDir && !selHasGhost) {
      items.push({ kind: "action", label: t("tabs.openInNewTab"), icon: newTabIcon, onclick: () => pane.addTab(sel[0]) });

      // Open in other pane when split
      if (app.isSplit) {
        const otherPane = app.getPaneView(pane.paneIdx === 0 ? 1 : 0);
        items.push({ kind: "action", label: t("tabs.openInOtherPane"), icon: splitPaneIcon, onclick: () => otherPane.navigate(sel[0]) });
      }

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

    if (!isMulti && !selHasGhost) {
      items.push({ kind: "action", label: t("menu.openWith"), icon: openWithIcon, onclick: () => onOpenWith(sel[0]) });
      items.push({ kind: "action", label: t("menu.rename"), shortcut: "F2", icon: renameIcon, onclick: () => pane.startRename(entry.path, entry.name) });
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

    if (hasM3uDirs && !selHasGhost) {
      const m3uIcon = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><path d="M9 18V5l12-2v13"/><circle cx="6" cy="18" r="3"/><circle cx="18" cy="16" r="3"/></svg>`;
      items.push({ kind: "separator" });
      items.push({
        kind: "action",
        label: t("m3u.menuItem"),
        icon: m3uIcon,
        onclick: () => onM3u(sel.filter(p => !!entryOf(p)?.isDir)),
      });
    }

    if (!selHasGhost) {
      items.push({ kind: "separator" });
      items.push({ kind: "action", label: t("menu.properties"), icon: propsIcon, onclick: () => onProperties(sel) });
    }
    items.push({ kind: "separator" });
    items.push({ kind: "action", label: t("menu.delete"), shortcut: "Del", icon: deleteIcon, danger: true, onclick: () => onDelete(sel) });

    contextMenu = { x: e.clientX, y: e.clientY, items };
  }

  // ── Sort header click ────────────────────────────────────
  function clickSort(key: ContentSortKey) {
    if (pane.sortKey === key) {
      pane.setSort(key, pane.sortDir === "asc" ? "desc" : "asc");
    } else {
      pane.setSort(key, "asc");
    }
  }

  // ── Click on empty area ──────────────────────────────────
  function handlePanelClick(e: MouseEvent) {
    if ((e.target as HTMLElement).closest("[data-path]")) return;
    if (consumeNextPanelClick) { consumeNextPanelClick = false; return; }
    pane.clearSelection();
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
    if (pane.currentPath && !pane.currentPath.startsWith("remote://")) {
      items.push({ kind: "separator" });
      items.push({ kind: "action", label: t("terminal.openTerminal"), icon: terminalIcon, onclick: () => openTerminalAt(pane.currentPath!) });
    }
    if (items.length > 0) contextMenu = { x: e.clientX, y: e.clientY, items };
  }

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
  const chdIcon      = `<svg width="14" height="14" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="2"><circle cx="12" cy="12" r="9"/><circle cx="12" cy="12" r="3"/><line x1="12" y1="3" x2="12" y2="9"/><line x1="12" y1="15" x2="12" y2="21"/><line x1="3" y1="12" x2="9" y2="12"/><line x1="15" y1="12" x2="21" y2="12"/></svg>`;
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
  style="--col-name:{pane.columnWidths.name}px; --col-type:{pane.columnWidths.type}px; --col-size:{pane.columnWidths.size}px; --col-modified:{pane.columnWidths.modified}px;"
  onkeydown={handleKeyDown}
  onclick={handlePanelClick}
  oncontextmenu={handlePanelContextMenu}
  onwheel={handleContentWheel}
>
  <!-- ── Column headers (list view only) ── -->
  {#if pane.viewMode === "list"}
  <div class="content-header" role="row">
    <div class="col-gutter"></div>
    <button class="col-hdr col-hdr--name" onclick={() => clickSort("name")} role="columnheader">
      {t("contentPanel.name")}
      {#if pane.sortKey === "name"}<span class="sort-arrow">{pane.sortDir === "asc" ? "↑" : "↓"}</span>{/if}
    </button>
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="col-resize-handle" role="separator" onmousedown={(e) => startColResize(e, "name")}></div>
    <button class="col-hdr col-hdr--type" onclick={() => clickSort("type")} role="columnheader">
      {t("contentPanel.type")}
      {#if pane.sortKey === "type"}<span class="sort-arrow">{pane.sortDir === "asc" ? "↑" : "↓"}</span>{/if}
    </button>
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="col-resize-handle" role="separator" onmousedown={(e) => startColResize(e, "type")}></div>
    <button class="col-hdr col-hdr--size" onclick={() => clickSort("size")} role="columnheader">
      {t("contentPanel.size")}
      {#if pane.sortKey === "size"}<span class="sort-arrow">{pane.sortDir === "asc" ? "↑" : "↓"}</span>{/if}
    </button>
    <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
    <div class="col-resize-handle" role="separator" onmousedown={(e) => startColResize(e, "size")}></div>
    <button class="col-hdr col-hdr--modified" onclick={() => clickSort("modified")} role="columnheader">
      {t("contentPanel.modified")}
      {#if pane.sortKey === "modified"}<span class="sort-arrow">{pane.sortDir === "asc" ? "↑" : "↓"}</span>{/if}
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

  <!-- ── Content body ── -->
  <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
  <div
    class="content-body scrollbar-thin"
    bind:this={contentBodyEl}
    onmousedown={handleBodyMousedown}
    onscroll={saveScrollPosition}
    role="presentation"
  >
    {#if pane.isLoading}
      <div class="content-state">
        <span class="loading-spinner"></span>
        <span>{t("contentPanel.loading")}</span>
      </div>

    {:else if !pane.currentPath}
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
        <span>{pane.isSearching ? t("contentPanel.noResults") : t("contentPanel.folderEmpty")}</span>
      </div>

    {:else}
      {#if pane.viewMode === "list"}
        <div class="file-list" role="rowgroup">
          {#each displayed as entry (entry.path)}
            <FileRow
              {entry}
              isSelected={pane.selectedPaths.has(entry.path)}
              onActivate={handleActivate}
              onContextMenu={openContextMenu}
              onMousedown={handleMousedown}
            />
          {/each}
        </div>

      {:else}
        <div class="file-grid" role="rowgroup">
          {#each displayed as entry (entry.path)}
            <!-- svelte-ignore a11y_no_noninteractive_element_interactions -->
            <div
              class="grid-item"
              class:grid-item--selected={pane.selectedPaths.has(entry.path)}
              class:grid-item--ghost={entry.isGhost}
              data-path={entry.path}
              role="gridcell"
              tabindex="0"
              title={entry.isGhost ? (entry.ghostApproximate ? t("ghost.pendingApprox") : t("ghost.pending")) : undefined}
              onmousedown={(e) => handleMousedown(e, entry)}
              ondblclick={() => { if (pane.renaming?.path !== entry.path) handleActivate(entry); }}
              oncontextmenu={(e) => { e.preventDefault(); openContextMenu(e, entry); }}
            >
              <div class="grid-icon" aria-hidden="true">
                {#if entry.isGhost}
                  <svg width="30" height="30" viewBox="0 0 24 24" fill="none" stroke="currentColor" stroke-width="1.3" stroke-dasharray="3 2" class="icon-ghost-lg">
                    {#if entry.isDir}
                      <path d="M10 4H4c-1.1 0-2 .9-2 2v12c0 1.1.9 2 2 2h16c1.1 0 2-.9 2-2V8c0-1.1-.9-2-2-2h-8l-2-2z"/>
                    {:else}
                      <path d="M13 2H6a2 2 0 0 0-2 2v16a2 2 0 0 0 2 2h12a2 2 0 0 0 2-2V9z"/>
                      <polyline points="13 2 13 9 20 9"/>
                    {/if}
                  </svg>
                {:else if entry.isDir}
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
              {#if pane.renaming?.path === entry.path}
                <input
                  class="grid-rename-input"
                  type="text"
                  use:initGridRenameInput={entry}
                  onblur={(e) => commitGridRename(e, entry)}
                  onkeydown={(e) => {
                    if (e.key === "Enter") { e.preventDefault(); commitGridRename(e, entry); }
                    if (e.key === "Escape") { e.preventDefault(); pane.cancelRename(); }
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
      {#if pane.selectedPaths.size > 0}
        {t("contentPanel.itemsSelected", { selected: pane.selectedPaths.size, total: displayed.length })}
      {:else}
        {tn("contentPanel.itemCount", "contentPanel.itemCountPlural", displayed.length)}
      {/if}
    </span>
    {#if pane.isSearching}
      <span class="status-search">
        {t("contentPanel.searchLabel", { query: pane.searchQuery ?? "" })}
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
    position: relative;
  }

  .content-panel.col-resizing { cursor: col-resize; user-select: none; }

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

  .content-body {
    flex: 1;
    overflow-y: auto;
    padding: 4px;
    padding-bottom: 60px;
    position: relative;
    user-select: none;
  }

  :global(.rubber-band) {
    position: absolute;
    pointer-events: none;
    z-index: 50;
    background: color-mix(in srgb, var(--accent) 12%, transparent);
    border: 1px solid var(--accent);
    border-radius: 2px;
  }

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

  .file-list {
    display: flex;
    flex-direction: column;
    gap: 1px;
    padding-left: 16px;
    align-items: flex-start;
  }

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
  .icon-ghost-lg { color: var(--accent); opacity: 0.7; }

  .grid-item--ghost {
    opacity: 0.72;

    .grid-name { font-style: italic; color: var(--text-muted); }
  }
  .grid-item--ghost.grid-item--selected { opacity: 1; }

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

  .qf-icon { color: var(--accent); flex-shrink: 0; }

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

  .qf-count { font-size: 11px; color: var(--text-muted); flex-shrink: 0; }

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
