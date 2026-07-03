import { untrack } from "svelte";
import type {
  AppSettings,
  ThemeMode,
  ViewMode,
  ContentSortKey,
  SortDirection,
  ContentColumnWidths,
  ClipboardState,
  NotificationEntry,
  NotificationKind,
  RenameState,
  EntryDto,
  ConnectionProfileDto,
  ActiveConnectionDto,
  VolumeDto,
  KnownFoldersDto,
  TerminalTabInfo,
  QueuedOp,
  QueueConflict,
  ConflictKind,
  ConflictSeverity,
  JobPausedDto,
} from "../types/index.js";

// ─── Local types ───────────────────────────────────────────
type ConfirmDialogState = {
  title: string;
  message: string;
  confirmLabel?: string;
  onConfirm: () => void;
};

type TabState = {
  id: string;
  color: string;
  currentPath: string | null;
  history: string[];
  historyIdx: number;
  entries: EntryDto[];
  selectedPaths: Set<string>;
  isLoading: boolean;
  searchQuery: string | null;
  isSearching: boolean;
  renaming: RenameState | null;
  scrollTop: number;
  loadedPath: string | null;
};

type JobEntry = {
  id: string;
  title: string;
  progress: number;
  message: string;
  logs: string[];
  done: boolean;
  success: boolean;
  resultMessage: string;
  statusMessageOnSuccess?: string | null;
  statusMessageOnFailure?: string | null;
  createdAt: number;
  // Pause state — set when the backend emits job-paused
  paused: boolean;
  pauseError: string | null;
  pauseFileName: string | null;
  pauseIsRecoverable: boolean;
};

type StartJobOpts = {
  showDialog?: boolean; // kept for API compat, ignored — all jobs appear in the panel
  statusMessageOnSuccess?: string;
  statusMessageOnFailure?: string;
};

type QueueRunStats = {
  total: number;
  completed: number;
  succeeded: number;
  failed: number;
};

// ─── Default settings ─────────────────────────────────────
const DEFAULT_COLUMN_WIDTHS: ContentColumnWidths = {
  name: 280,
  type: 80,
  size: 90,
  modified: 140,
};

const DEFAULT_SETTINGS: AppSettings = {
  theme: "light",
  locale: "en",
  fontScale: 1,
  compactUi: false,
  defaultViewMode: "list",
  defaultContentZoom: 1,
  defaultTreeZoom: 1,
  favoriteLocations: [],
  confirmDelete: true,
  hideDeleteProgressPopup: false,
  selectionWeightMaxDepth: 5,
  selectionWeightCalculateAll: false,
  contentSortKey: "name",
  contentSortDirection: "asc",
  contentColumnWidths: DEFAULT_COLUMN_WIDTHS,
  chdScanDepth: 3,
};

function loadSettings(): AppSettings {
  try {
    const raw = localStorage.getItem("dogu-settings");
    if (!raw) return { ...DEFAULT_SETTINGS };
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    // Migrate: localLocations → favoriteLocations
    if (Array.isArray(parsed.localLocations) && !parsed.favoriteLocations) {
      parsed.favoriteLocations = parsed.localLocations;
    }
    delete parsed.localLocations;
    return { ...DEFAULT_SETTINGS, ...parsed } as AppSettings;
  } catch {
    return { ...DEFAULT_SETTINGS };
  }
}

function persistSettings(s: AppSettings) {
  try {
    localStorage.setItem("dogu-settings", JSON.stringify(s));
  } catch {
    // storage unavailable — ignore
  }
}

// ─── Tab helpers ───────────────────────────────────────────
const TAB_COLORS = [
  "#3b82f6", // blue
  "#a855f7", // purple
  "#f59e0b", // amber
  "#ef4444", // red
  "#06b6d4", // cyan
  "#ec4899", // pink
  "#f97316", // orange
  "#84cc16", // lime
];
let _tabSeq = 0;
let _tabColorIdx = 0;

function makeTab(path?: string): TabState {
  return {
    id: `tab-${Date.now()}-${++_tabSeq}`,
    color: TAB_COLORS[_tabColorIdx++ % TAB_COLORS.length],
    currentPath: path ?? null,
    history: [],
    historyIdx: -1,
    entries: [],
    selectedPaths: new Set(),
    isLoading: false,
    searchQuery: null,
    isSearching: false,
    renaming: null,
    scrollTop: 0,
    loadedPath: null,
  };
}

function persistTabs(ts: TabState[], idx: number) {
  try {
    localStorage.setItem("dogu-tabs", JSON.stringify({
      paths: ts.map(t => t.currentPath),
      active: idx,
    }));
  } catch {}
}

function loadSavedTabs(): { initialTabs: TabState[]; initialActiveIdx: number } {
  try {
    const raw = localStorage.getItem("dogu-tabs");
    if (!raw) return { initialTabs: [makeTab()], initialActiveIdx: 0 };
    const parsed = JSON.parse(raw) as { paths?: unknown[]; active?: unknown };
    const { paths, active } = parsed;
    if (!Array.isArray(paths) || paths.length === 0) {
      return { initialTabs: [makeTab()], initialActiveIdx: 0 };
    }
    const savedTabs = paths.map((p: unknown) =>
      makeTab(typeof p === "string" && !p.startsWith("remote://") ? p : undefined)
    );
    const activeIdx = typeof active === "number"
      ? Math.max(0, Math.min(active, savedTabs.length - 1))
      : 0;
    return { initialTabs: savedTabs, initialActiveIdx: activeIdx };
  } catch {
    return { initialTabs: [makeTab()], initialActiveIdx: 0 };
  }
}

// ─── Path helpers ──────────────────────────────────────────
function parentPath(path: string): string | null {
  const remoteMatch = path.match(/^(remote:\/\/[^/]+)(\/.*)?$/);
  if (remoteMatch) {
    const rest = (remoteMatch[2] ?? "").split("/").filter(Boolean);
    if (rest.length === 0) return null;
    rest.pop();
    return rest.length === 0
      ? remoteMatch[1]
      : `${remoteMatch[1]}/${rest.join("/")}`;
  }
  const winMatch = path.match(/^([A-Za-z]:\\)(.*)$/);
  if (winMatch) {
    const parts = winMatch[2].split("\\").filter(Boolean);
    if (parts.length === 0) return null;
    parts.pop();
    return parts.length === 0 ? winMatch[1] : `${winMatch[1]}${parts.join("\\")}`;
  }
  if (path.startsWith("/")) {
    const parts = path.split("/").filter(Boolean);
    if (parts.length === 0) return null;
    parts.pop();
    return parts.length === 0 ? "/" : `/${parts.join("/")}`;
  }
  return null;
}

// ─── Notification helpers ──────────────────────────────────
let _notifCounter = 0;

function makeNotification(kind: NotificationKind, text: string): NotificationEntry {
  return { id: `n-${++_notifCounter}`, kind, text, createdAt: Date.now() };
}

// ─── Queue conflict detection ──────────────────────────────
function pathContainsOther(parent: string, child: string): boolean {
  const p = parent.replace(/[/\\]+$/, "").toLowerCase();
  const c = child.replace(/[/\\]+$/, "").toLowerCase();
  return c === p || c.startsWith(p + "/") || c.startsWith(p + "\\");
}

function pathsOverlap(a: string, b: string): boolean {
  return pathContainsOther(a, b) || pathContainsOther(b, a);
}

function detectConflicts(ops: QueuedOp[]): QueueConflict[] {
  const conflicts: QueueConflict[] = [];

  function push(opAId: string, opBId: string, kind: ConflictKind, severity: ConflictSeverity, pathA: string, pathB: string) {
    if (!conflicts.some(c => c.opAId === opAId && c.opBId === opBId && c.kind === kind)) {
      conflicts.push({ opAId, opBId, kind, severity, pathA, pathB });
    }
  }

  for (let i = 0; i < ops.length; i++) {
    for (let j = i + 1; j < ops.length; j++) {
      const a = ops[i], b = ops[j]; // a runs BEFORE b in sequential mode

      // Destination collision: simultaneous writes to same location are a race condition.
      // In sequential order a writes first, then b — no race, but possible overwrite.
      // → parallel-only (the queue order makes sequential safe enough)
      for (const wa of a.destinations) {
        for (const wb of b.destinations) {
          if (pathsOverlap(wa, wb)) push(a.id, b.id, 'dest-collision', 'parallel-only', wa, wb);
        }
      }

      // a (earlier in queue) deletes something b (later) needs.
      // Sequential: a runs first → destroys what b needs → BLOCKING in both modes.
      for (const d of a.deletes) {
        for (const s of b.sources)      { if (pathsOverlap(d, s)) push(a.id, b.id, 'source-deleted', 'blocking',      d, s); }
        for (const w of b.destinations) { if (pathsOverlap(d, w)) push(a.id, b.id, 'dest-deleted',   'blocking',      d, w); }
      }

      // b (later in queue) deletes something a (earlier) needs.
      // Sequential: a finishes before b starts → a is already done when b deletes → safe.
      // Parallel: both run at the same time → b may delete while a still needs it → PARALLEL-ONLY.
      for (const d of b.deletes) {
        for (const s of a.sources)      { if (pathsOverlap(d, s)) push(b.id, a.id, 'source-deleted', 'parallel-only', d, s); }
        for (const w of a.destinations) { if (pathsOverlap(d, w)) push(b.id, a.id, 'dest-deleted',   'parallel-only', d, w); }
      }
    }
  }

  return conflicts;
}

// ─── App state (Svelte 5 runes) ────────────────────────────
function createAppState() {
  // Settings
  let settings = $state<AppSettings>(loadSettings());

  // Tabs
  const { initialTabs, initialActiveIdx } = untrack(() => loadSavedTabs());
  let tabs = $state<TabState[]>(initialTabs);
  let activeTabIdx = $state(initialActiveIdx);

  // Global display prefs
  let viewMode = $state<ViewMode>(untrack(() => settings.defaultViewMode));
  let sortKey = $state<ContentSortKey>(untrack(() => settings.contentSortKey));
  let sortDir = $state<SortDirection>(untrack(() => settings.contentSortDirection));
  let columnWidths = $state<ContentColumnWidths>(untrack(() => ({ ...settings.contentColumnWidths })));
  let contentZoom = $state(untrack(() => settings.defaultContentZoom));
  let treeZoom = $state(untrack(() => settings.defaultTreeZoom));

  // Connections
  let connectionProfiles = $state<ConnectionProfileDto[]>([]);
  let activeConnections = $state<ActiveConnectionDto[]>([]);

  // System volumes and known folders (loaded from backend on mount)
  let volumes = $state<VolumeDto[]>([]);
  let knownFolders = $state<KnownFoldersDto | null>(null);

  // Clipboard (shared across tabs)
  let clipboard = $state<ClipboardState | null>(null);

  // Jobs (replaces single progress)
  let jobs = $state<JobEntry[]>([]);
  let jobsPanelOpen = $state(false);

  // Job completion waiters (non-reactive — plain Map)
  const jobWaiters = new Map<string, Array<() => void>>();

  // Operation queue
  let queueMode = $state(false);
  let opQueue = $state<QueuedOp[]>([]);
  let queueRunning = $state(false);
  let queueRunStats = $state<QueueRunStats | null>(null);
  const queueConflicts = $derived(detectConflicts(opQueue));

  // Notifications
  let notifications = $state<NotificationEntry[]>([]);

  // Busy overlay
  let busyCount = $state(0);

  // Panels
  let connectionManagerOpen = $state(false);
  let settingsOpen = $state(false);

  // Terminal panel
  let terminalPanelOpen = $state(false);
  let terminalPanelHeight = $state(240);
  let terminalTabs = $state<TerminalTabInfo[]>([]);
  let activeTerminalId = $state<string | null>(null);

  // Confirm dialog
  let confirmDialog = $state<ConfirmDialogState | null>(null);

  // SSH fingerprint trust dialog
  let fingerprintPrompt = $state<{
    fingerprint: string;
    profileLabel: string;
    resolve: (trusted: boolean) => void;
  } | null>(null);

  // Derived
  const theme = $derived(settings.theme);
  const isBusy = $derived(busyCount > 0);

  return {
    // ── Settings ────────────────────────────────────────────
    get settings() { return settings; },
    updateSettings(patch: Partial<AppSettings>) {
      settings = { ...settings, ...patch };
      persistSettings(settings);
      applyTheme(settings.theme);
    },

    // ── Theme ────────────────────────────────────────────────
    get theme() { return theme; },
    setTheme(t: ThemeMode) {
      settings = { ...settings, theme: t };
      persistSettings(settings);
      applyTheme(t);
    },

    // ── Tabs ─────────────────────────────────────────────────
    get tabs() { return tabs; },
    get activeTabIdx() { return activeTabIdx; },
    get activeTabId() { return tabs[activeTabIdx]?.id ?? ""; },
    get activeTab() { return tabs[activeTabIdx]; },

    addTab(path?: string) {
      const newTab = makeTab(path);
      tabs = [...tabs, newTab];
      activeTabIdx = tabs.length - 1;
      persistTabs(tabs, activeTabIdx);
    },

    closeTab(id: string) {
      if (tabs.length <= 1) return;
      const idx = tabs.findIndex(t => t.id === id);
      if (idx === -1) return;
      const newTabs = tabs.filter(t => t.id !== id);
      tabs = newTabs;
      if (activeTabIdx >= newTabs.length) {
        activeTabIdx = newTabs.length - 1;
      } else if (activeTabIdx > idx) {
        activeTabIdx -= 1;
      }
      persistTabs(tabs, activeTabIdx);
    },

    setActiveTab(id: string) {
      const idx = tabs.findIndex(t => t.id === id);
      if (idx !== -1) activeTabIdx = idx;
    },

    setActiveTabByIndex(idx: number) {
      if (idx >= 0 && idx < tabs.length) activeTabIdx = idx;
    },

    nextTab() {
      activeTabIdx = (activeTabIdx + 1) % tabs.length;
    },

    prevTab() {
      activeTabIdx = (activeTabIdx - 1 + tabs.length) % tabs.length;
    },

    duplicateTab() {
      const current = tabs[activeTabIdx];
      if (!current) return;
      const newTab = makeTab(current.currentPath ?? undefined);
      tabs = [...tabs.slice(0, activeTabIdx + 1), newTab, ...tabs.slice(activeTabIdx + 1)];
      activeTabIdx = activeTabIdx + 1;
      persistTabs(tabs, activeTabIdx);
    },

    reorderTabs(fromIdx: number, insertIdx: number) {
      if (fromIdx === insertIdx || fromIdx + 1 === insertIdx) return;
      const activeId = tabs[activeTabIdx]?.id;
      const next = [...tabs];
      const [moved] = next.splice(fromIdx, 1);
      next.splice(fromIdx < insertIdx ? insertIdx - 1 : insertIdx, 0, moved);
      tabs = next;
      activeTabIdx = activeId ? next.findIndex(t => t.id === activeId) : 0;
      persistTabs(tabs, activeTabIdx);
    },

    setTabScrollTop(tabIdx: number, scrollTop: number) {
      if (tabs[tabIdx]) tabs[tabIdx].scrollTop = scrollTop;
    },

    setTabLoadedPath(path: string) {
      const tab = tabs[activeTabIdx];
      if (tab) tab.loadedPath = path;
    },

    // ── Navigation (per active tab) ──────────────────────────
    get currentPath() { return tabs[activeTabIdx]?.currentPath ?? null; },
    get navigationHistory() { return tabs[activeTabIdx]?.history ?? []; },
    get historyIndex() { return tabs[activeTabIdx]?.historyIdx ?? -1; },

    navigate(path: string, pushHistory = true) {
      const tab = tabs[activeTabIdx];
      if (!tab) return;
      if (pushHistory && tab.currentPath !== null) {
        tab.history = [...tab.history.slice(0, tab.historyIdx + 1), tab.currentPath];
        tab.historyIdx = tab.history.length - 1;
      }
      tab.currentPath = path;
      tab.searchQuery = null;
      tab.isSearching = false;
      tab.scrollTop = 0;
      persistTabs(tabs, activeTabIdx);
    },

    navigateBack() {
      const tab = tabs[activeTabIdx];
      if (!tab || tab.historyIdx < 0) return;
      tab.currentPath = tab.history[tab.historyIdx];
      tab.historyIdx -= 1;
      tab.searchQuery = null;
      tab.isSearching = false;
      tab.scrollTop = 0;
    },

    navigateForward() {
      const tab = tabs[activeTabIdx];
      if (!tab || tab.historyIdx >= tab.history.length - 1) return;
      tab.historyIdx += 1;
      tab.currentPath = tab.history[tab.historyIdx];
      tab.searchQuery = null;
      tab.isSearching = false;
      tab.scrollTop = 0;
    },

    navigateUp() {
      const p = tabs[activeTabIdx]?.currentPath;
      if (p) {
        const parent = parentPath(p);
        if (parent !== null) this.navigate(parent);
      }
    },

    get canGoBack() { return (tabs[activeTabIdx]?.historyIdx ?? -1) >= 0; },
    get canGoForward() {
      const tab = tabs[activeTabIdx];
      return tab ? tab.historyIdx < tab.history.length - 1 : false;
    },

    // ── Search (per active tab) ───────────────────────────────
    get searchQuery() { return tabs[activeTabIdx]?.searchQuery ?? null; },
    get isSearching() { return tabs[activeTabIdx]?.isSearching ?? false; },
    setSearch(q: string) {
      const tab = tabs[activeTabIdx];
      if (tab) { tab.searchQuery = q; tab.isSearching = true; }
    },
    setIsSearching(v: boolean) {
      const tab = tabs[activeTabIdx];
      if (tab) tab.isSearching = v;
    },
    setSearchQuery(q: string | null) {
      const tab = tabs[activeTabIdx];
      if (tab) tab.searchQuery = q;
    },
    clearSearch() {
      const tab = tabs[activeTabIdx];
      if (tab) { tab.searchQuery = null; tab.isSearching = false; }
    },

    // ── Content (per active tab) ─────────────────────────────
    get entries() { return tabs[activeTabIdx]?.entries ?? []; },
    setEntries(e: EntryDto[]) {
      const tab = tabs[activeTabIdx];
      if (tab) tab.entries = e;
    },

    get selectedPaths() { return tabs[activeTabIdx]?.selectedPaths ?? new Set<string>(); },
    setSelection(paths: string[]) {
      const tab = tabs[activeTabIdx];
      if (tab) tab.selectedPaths = new Set(paths);
    },
    clearSelection() {
      const tab = tabs[activeTabIdx];
      if (tab) tab.selectedPaths = new Set();
    },
    toggleSelection(path: string) {
      const tab = tabs[activeTabIdx];
      if (!tab) return;
      const next = new Set(tab.selectedPaths);
      if (next.has(path)) next.delete(path); else next.add(path);
      tab.selectedPaths = next;
    },
    rangeSelect(fromPath: string, toPath: string) {
      const tab = tabs[activeTabIdx];
      if (!tab) return;
      const sorted = [...tab.entries].sort((a, b) => {
        if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
        let cmp = 0;
        if (sortKey === "name") cmp = a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
        else if (sortKey === "type") cmp = a.extension.localeCompare(b.extension);
        else if (sortKey === "size") cmp = a.size - b.size;
        else if (sortKey === "modified") cmp = a.modifiedTs - b.modifiedTs;
        return sortDir === "asc" ? cmp : -cmp;
      });
      const paths = sorted.map(e => e.path);
      const fromIdx = paths.indexOf(fromPath);
      const toIdx = paths.indexOf(toPath);
      if (fromIdx === -1 || toIdx === -1) return;
      const [start, end] = fromIdx < toIdx ? [fromIdx, toIdx] : [toIdx, fromIdx];
      tab.selectedPaths = new Set(paths.slice(start, end + 1));
    },

    get viewMode() { return viewMode; },
    setViewMode(v: ViewMode) {
      viewMode = v;
      settings = { ...settings, defaultViewMode: v };
      persistSettings(settings);
    },

    get sortKey() { return sortKey; },
    get sortDir() { return sortDir; },
    setSort(key: ContentSortKey, dir: SortDirection) {
      sortKey = key; sortDir = dir;
      settings = { ...settings, contentSortKey: key, contentSortDirection: dir };
      persistSettings(settings);
    },

    get columnWidths() { return columnWidths; },
    setColumnWidth(key: ContentSortKey, w: number) {
      columnWidths = { ...columnWidths, [key]: w };
      settings = { ...settings, contentColumnWidths: { ...columnWidths } };
      persistSettings(settings);
    },

    get contentZoom() { return contentZoom; },
    setContentZoom(z: number) {
      contentZoom = z;
      settings = { ...settings, defaultContentZoom: z };
      persistSettings(settings);
      document.documentElement.style.setProperty("--content-scale", String(z));
    },

    get treeZoom() { return treeZoom; },
    setTreeZoom(z: number) {
      treeZoom = z;
      settings = { ...settings, defaultTreeZoom: z };
      persistSettings(settings);
      document.documentElement.style.setProperty("--tree-scale", String(z));
    },

    get isLoading() { return tabs[activeTabIdx]?.isLoading ?? false; },
    setLoading(v: boolean) {
      const tab = tabs[activeTabIdx];
      if (tab) tab.isLoading = v;
    },

    // ── Favorite locations (replaces localLocations) ─────────
    get favoriteLocations() { return settings.favoriteLocations; },
    addFavorite(path: string) {
      if (settings.favoriteLocations.includes(path)) return;
      const locs = [...settings.favoriteLocations, path];
      settings = { ...settings, favoriteLocations: locs };
      persistSettings(settings);
    },
    removeFavorite(path: string) {
      const locs = settings.favoriteLocations.filter(l => l !== path);
      settings = { ...settings, favoriteLocations: locs };
      persistSettings(settings);
    },
    isFavorite(path: string) { return settings.favoriteLocations.includes(path); },
    toggleFavorite(path: string) {
      if (settings.favoriteLocations.includes(path)) {
        const locs = settings.favoriteLocations.filter(l => l !== path);
        settings = { ...settings, favoriteLocations: locs };
      } else {
        const locs = [...settings.favoriteLocations, path];
        settings = { ...settings, favoriteLocations: locs };
      }
      persistSettings(settings);
    },

    // ── System volumes / known folders ───────────────────────
    get volumes() { return volumes; },
    get knownFolders() { return knownFolders; },
    setVolumes(v: VolumeDto[]) { volumes = v; },
    setKnownFolders(k: KnownFoldersDto) { knownFolders = k; },

    // ── Connections ──────────────────────────────────────────
    get connectionProfiles() { return connectionProfiles; },
    setConnectionProfiles(p: ConnectionProfileDto[]) { connectionProfiles = p; },

    get activeConnections() { return activeConnections; },
    setActiveConnections(c: ActiveConnectionDto[]) { activeConnections = c; },
    removeActiveConnection(sessionId: string) {
      activeConnections = activeConnections.filter(c => c.sessionId !== sessionId);
    },

    // ── Connection manager ───────────────────────────────────
    get connectionManagerOpen() { return connectionManagerOpen; },
    openConnectionManager() { connectionManagerOpen = true; },
    closeConnectionManager() { connectionManagerOpen = false; },

    // ── Settings dialog ──────────────────────────────────────
    get settingsOpen() { return settingsOpen; },
    openSettings() { settingsOpen = true; },
    closeSettings() { settingsOpen = false; },

    // ── Confirm dialog ───────────────────────────────────────
    get confirmDialog() { return confirmDialog; },
    openConfirm(dialog: ConfirmDialogState) { confirmDialog = dialog; },
    closeConfirm() { confirmDialog = null; },

    // ── Fingerprint prompt ───────────────────────────────────
    get fingerprintPrompt() { return fingerprintPrompt; },
    promptFingerprint(fingerprint: string, profileLabel: string): Promise<boolean> {
      return new Promise((resolve) => {
        fingerprintPrompt = { fingerprint, profileLabel, resolve };
      });
    },
    resolveFingerprint(trusted: boolean) {
      fingerprintPrompt?.resolve(trusted);
      fingerprintPrompt = null;
    },

    // ── Clipboard ────────────────────────────────────────────
    get clipboard() { return clipboard; },
    setClipboard(c: ClipboardState | null) { clipboard = c; },

    // ── Rename (per active tab) ───────────────────────────────
    get renaming() { return tabs[activeTabIdx]?.renaming ?? null; },
    startRename(path: string, value: string) {
      const tab = tabs[activeTabIdx];
      if (tab) tab.renaming = { path, value };
    },
    cancelRename() {
      const tab = tabs[activeTabIdx];
      if (tab) tab.renaming = null;
    },

    // ── Jobs / operation queue ────────────────────────────────
    get jobs() { return jobs; },
    get jobsPanelOpen() { return jobsPanelOpen; },
    openJobsPanel() { jobsPanelOpen = true; },
    closeJobsPanel() { jobsPanelOpen = false; },
    toggleJobsPanel() { jobsPanelOpen = !jobsPanelOpen; },

    startJob(jobId: string, title: string, opts?: StartJobOpts) {
      const job: JobEntry = {
        id: jobId,
        title,
        progress: 0,
        message: "",
        logs: [],
        done: false,
        success: false,
        resultMessage: "",
        statusMessageOnSuccess: opts?.statusMessageOnSuccess ?? null,
        statusMessageOnFailure: opts?.statusMessageOnFailure ?? null,
        createdAt: Date.now(),
        paused: false,
        pauseError: null,
        pauseFileName: null,
        pauseIsRecoverable: false,
      };
      jobs = [job, ...jobs];
      jobsPanelOpen = true;
    },

    updateJobProgress(jobId: string, pct: number, message: string) {
      const idx = jobs.findIndex(j => j.id === jobId);
      if (idx === -1) return;
      jobs[idx] = { ...jobs[idx], progress: pct, message };
    },

    appendJobLog(jobId: string, line: string) {
      const idx = jobs.findIndex(j => j.id === jobId);
      if (idx === -1) return;
      jobs[idx] = { ...jobs[idx], logs: [...jobs[idx].logs, line] };
    },

    finishJob(jobId: string, success: boolean, message: string) {
      const idx = jobs.findIndex(j => j.id === jobId);
      if (idx === -1) return;
      const job = jobs[idx];
      const resultMessage = message
        || (success ? (job.statusMessageOnSuccess ?? "") : (job.statusMessageOnFailure ?? ""));
      jobs[idx] = { ...job, progress: 100, done: true, success, resultMessage };
      // Auto-dismiss successful jobs after 5s — but only when NO queue is running.
      // When a queue runs, dismissal is batched in executeQueue's finally block so the
      // user can see the full list of completed operations until the queue finishes.
      if (success && !queueRunning) {
        setTimeout(() => { jobs = jobs.filter(j => j.id !== jobId); }, 5000);
      }
      // Signal any waiters (for sequential queue execution)
      const waiters = jobWaiters.get(jobId) ?? [];
      jobWaiters.delete(jobId);
      for (const w of waiters) w();
    },

    /** Called when the backend emits job-paused. Freezes the job UI with error context. */
    pauseJob(dto: JobPausedDto) {
      const idx = jobs.findIndex(j => j.id === dto.jobId);
      if (idx === -1) return;
      jobs[idx] = {
        ...jobs[idx],
        paused: true,
        pauseError: dto.error,
        pauseFileName: dto.fileName,
        pauseIsRecoverable: dto.isRecoverable,
      };
    },

    /** Called after the user sends a resume decision — clears the pause state. */
    clearJobPause(jobId: string) {
      const idx = jobs.findIndex(j => j.id === jobId);
      if (idx === -1) return;
      jobs[idx] = {
        ...jobs[idx],
        paused: false,
        pauseError: null,
        pauseFileName: null,
        pauseIsRecoverable: false,
      };
    },

    dismissJob(jobId: string) {
      jobs = jobs.filter(j => j.id !== jobId);
    },

    clearDoneJobs() {
      jobs = jobs.filter(j => !j.done);
    },

    // Legacy compat — no-op (ProgressDialog removed)
    get progress() { return null; },
    clearProgress() {},

    // Returns a Promise that resolves when finishJob(jobId) is called.
    // Must be called after startJob to ensure the job exists in the array.
    waitForJob(jobId: string): Promise<void> {
      return new Promise(resolve => {
        const job = jobs.find(j => j.id === jobId);
        if (job?.done) { resolve(); return; }
        const list = jobWaiters.get(jobId) ?? [];
        list.push(resolve);
        jobWaiters.set(jobId, list);
      });
    },

    // ── Operation queue ───────────────────────────────────────
    get queueMode() { return queueMode; },
    get opQueue() { return opQueue; },
    get queueConflicts() { return queueConflicts; },
    get queueRunning() { return queueRunning; },
    get queueRunStats() { return queueRunStats; },

    toggleQueueMode() { queueMode = !queueMode; },
    addToQueue(op: QueuedOp) { opQueue = [...opQueue, op]; jobsPanelOpen = true; },
    removeFromQueue(id: string) { opQueue = opQueue.filter(o => o.id !== id); },
    clearQueue() { opQueue = []; },

    moveOpQueueItem(fromIdx: number, delta: -1 | 1) {
      const toIdx = fromIdx + delta;
      if (toIdx < 0 || toIdx >= opQueue.length) return;
      const next = [...opQueue];
      [next[fromIdx], next[toIdx]] = [next[toIdx], next[fromIdx]];
      opQueue = next;
    },

    reorderOpQueue(fromIdx: number, insertIdx: number) {
      if (fromIdx === insertIdx || fromIdx + 1 === insertIdx) return;
      const next = [...opQueue];
      const [moved] = next.splice(fromIdx, 1);
      next.splice(fromIdx < insertIdx ? insertIdx - 1 : insertIdx, 0, moved);
      opQueue = next;
    },

    async executeQueue(mode: 'parallel' | 'sequential') {
      const ops = [...opQueue];
      if (ops.length === 0) return;
      opQueue = [];
      queueRunning = true;
      // Snapshot which jobs already exist so we can identify queue-spawned jobs later.
      const preExistingIds = new Set(jobs.map(j => j.id));
      queueRunStats = { total: ops.length, completed: 0, succeeded: 0, failed: 0 };

      // Wrap each op so we can track per-op completion count.
      const wrapOp = async (op: QueuedOp) => {
        await op.execute().catch(() => {});
        queueRunStats = { ...queueRunStats!, completed: queueRunStats!.completed + 1 };
      };

      try {
        if (mode === 'parallel') {
          await Promise.all(ops.map(wrapOp));
        } else {
          for (const op of ops) await wrapOp(op);
        }
      } finally {
        queueRunning = false;

        // Tally succeeded / failed from the jobs that were created by this queue run.
        const queueJobs = jobs.filter(j => !preExistingIds.has(j.id) && j.done);
        const succeeded = queueJobs.filter(j => j.success).length;
        const failed = queueJobs.filter(j => !j.success).length;
        queueRunStats = { total: ops.length, completed: ops.length, succeeded, failed };

        // Now that the queue is done, schedule auto-dismiss for successful jobs
        // (these were held back during the run so the user could see them all).
        for (const j of queueJobs.filter(j => j.success)) {
          const id = j.id;
          setTimeout(() => { jobs = jobs.filter(x => x.id !== id); }, 5000);
        }
        // Keep the stats summary visible for a few seconds after the queue finishes.
        setTimeout(() => { queueRunStats = null; }, 8000);
      }
    },

    // ── Notifications ─────────────────────────────────────────
    get notifications() { return notifications; },
    notify(kind: NotificationKind, text: string) {
      const n = makeNotification(kind, text);
      notifications = [n, ...notifications].slice(0, 8);
      // "error" and "warn" are persistent — the user must dismiss them explicitly.
      if (kind !== "error" && kind !== "warn") {
        setTimeout(() => {
          notifications = notifications.filter((x) => x.id !== n.id);
        }, 4000);
      }
    },
    dismissNotification(id: string) {
      notifications = notifications.filter((x) => x.id !== id);
    },

    // ── Terminal panel ────────────────────────────────────────
    get terminalPanelOpen() { return terminalPanelOpen; },
    get terminalPanelHeight() { return terminalPanelHeight; },
    get terminalTabs() { return terminalTabs; },
    get activeTerminalId() { return activeTerminalId; },

    openTerminalPanel() { terminalPanelOpen = true; },
    closeTerminalPanel() { terminalPanelOpen = false; },
    toggleTerminalPanel() { terminalPanelOpen = !terminalPanelOpen; },
    setTerminalPanelHeight(h: number) { terminalPanelHeight = h; },

    addTerminalTab(tab: TerminalTabInfo) {
      terminalTabs = [...terminalTabs, tab];
      activeTerminalId = tab.id;
      terminalPanelOpen = true;
    },
    closeTerminalTab(id: string) {
      terminalTabs = terminalTabs.filter(t => t.id !== id);
      if (activeTerminalId === id) {
        activeTerminalId = terminalTabs[terminalTabs.length - 1]?.id ?? null;
      }
      if (terminalTabs.length === 0) terminalPanelOpen = false;
    },
    setActiveTerminalTab(id: string) { activeTerminalId = id; },
    renameTerminalTab(id: string, title: string) {
      terminalTabs = terminalTabs.map(t => t.id === id ? { ...t, title: title.trim() || t.title } : t);
    },
    updateTerminalTabCwd(id: string, cwd: string) {
      terminalTabs = terminalTabs.map(t => t.id === id ? { ...t, cwd } : t);
    },
    reorderTerminalTabs(fromIdx: number, insertIdx: number) {
      if (fromIdx === insertIdx || fromIdx + 1 === insertIdx) return;
      const next = [...terminalTabs];
      const [moved] = next.splice(fromIdx, 1);
      next.splice(fromIdx < insertIdx ? insertIdx - 1 : insertIdx, 0, moved);
      terminalTabs = next;
    },

    // ── Busy overlay ──────────────────────────────────────────
    get isBusy() { return isBusy; },
    async withBusy<T>(fn: () => Promise<T>): Promise<T> {
      busyCount += 1;
      try {
        return await fn();
      } finally {
        busyCount -= 1;
      }
    },
  };
}

function applyTheme(t: ThemeMode) {
  document.documentElement.setAttribute("data-theme", t);
}

export const app = createAppState();

// Apply theme and scale CSS variables on module load
applyTheme(app.settings.theme);
document.documentElement.style.setProperty("--app-font-scale", String(app.settings.fontScale));
document.documentElement.style.setProperty("--content-scale", String(app.settings.defaultContentZoom));
document.documentElement.style.setProperty("--tree-scale", String(app.settings.defaultTreeZoom));
