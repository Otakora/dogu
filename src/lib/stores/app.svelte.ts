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
  ProgressState,
  RenameState,
  EntryDto,
  ConnectionProfileDto,
  ActiveConnectionDto,
} from "../types/index.js";

// ─── Local types ───────────────────────────────────────────
type ConfirmDialogState = {
  title: string;
  message: string;
  confirmLabel?: string;
  onConfirm: () => void;
};

type StartJobOpts = {
  showDialog?: boolean;
  statusMessageOnSuccess?: string;
  statusMessageOnFailure?: string;
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
  fontScale: 1,
  compactUi: false,
  defaultViewMode: "list",
  defaultContentZoom: 1,
  defaultTreeZoom: 1,
  localLocations: [],
  confirmDelete: true,
  hideDeleteProgressPopup: false,
  selectionWeightMaxDepth: 5,
  selectionWeightCalculateAll: false,
  contentSortKey: "name",
  contentSortDirection: "asc",
  contentColumnWidths: DEFAULT_COLUMN_WIDTHS,
};

function loadSettings(): AppSettings {
  try {
    const raw = localStorage.getItem("dogu-settings");
    if (!raw) return { ...DEFAULT_SETTINGS };
    return { ...DEFAULT_SETTINGS, ...JSON.parse(raw) } as AppSettings;
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

// ─── Path helpers ──────────────────────────────────────────
function parentPath(path: string): string | null {
  // Remote: remote://session/a/b/c → remote://session/a/b
  const remoteMatch = path.match(/^(remote:\/\/[^/]+)(\/.*)?$/);
  if (remoteMatch) {
    const rest = (remoteMatch[2] ?? "").split("/").filter(Boolean);
    if (rest.length === 0) return null; // already at remote root
    rest.pop();
    return rest.length === 0
      ? remoteMatch[1]
      : `${remoteMatch[1]}/${rest.join("/")}`;
  }
  // Windows: C:\foo\bar → C:\foo
  const winMatch = path.match(/^([A-Za-z]:\\)(.*)$/);
  if (winMatch) {
    const parts = winMatch[2].split("\\").filter(Boolean);
    if (parts.length === 0) return null;
    parts.pop();
    return parts.length === 0 ? winMatch[1] : `${winMatch[1]}${parts.join("\\")}`;
  }
  // Unix: /a/b/c → /a/b
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

function makeNotification(
  kind: NotificationKind,
  text: string
): NotificationEntry {
  return { id: `n-${++_notifCounter}`, kind, text, createdAt: Date.now() };
}

// ─── App state (Svelte 5 runes) ────────────────────────────
function createAppState() {
  // Settings
  let settings = $state<AppSettings>(loadSettings());

  // Navigation
  let currentPath = $state<string | null>(null);
  let navigationHistory = $state<string[]>([]);
  let historyIndex = $state(-1);

  // Content panel
  let entries = $state<EntryDto[]>([]);
  let selectedPaths = $state<Set<string>>(new Set());
  let viewMode = $state<ViewMode>(settings.defaultViewMode);
  let sortKey = $state<ContentSortKey>(settings.contentSortKey);
  let sortDir = $state<SortDirection>(settings.contentSortDirection);
  let columnWidths = $state<ContentColumnWidths>({ ...settings.contentColumnWidths });
  let contentZoom = $state(settings.defaultContentZoom);
  let treeZoom = $state(settings.defaultTreeZoom);
  let isLoading = $state(false);

  // Search
  let searchQuery = $state<string | null>(null);
  let isSearching = $state(false);

  // Connections
  let connectionProfiles = $state<ConnectionProfileDto[]>([]);
  let activeConnections = $state<ActiveConnectionDto[]>([]);

  // Clipboard
  let clipboard = $state<ClipboardState | null>(null);

  // Rename
  let renaming = $state<RenameState | null>(null);

  // Progress / jobs
  let progress = $state<ProgressState | null>(null);

  // Notifications
  let notifications = $state<NotificationEntry[]>([]);

  // Busy overlay
  let busyCount = $state(0);

  // Connection manager visibility (inline panel)
  let connectionManagerOpen = $state(false);

  // Settings dialog
  let settingsOpen = $state(false);

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

    // ── Navigation ───────────────────────────────────────────
    get currentPath() { return currentPath; },
    get navigationHistory() { return navigationHistory; },
    get historyIndex() { return historyIndex; },

    navigate(path: string, pushHistory = true) {
      if (pushHistory && currentPath !== null) {
        navigationHistory = [...navigationHistory.slice(0, historyIndex + 1), currentPath];
        historyIndex = navigationHistory.length - 1;
      }
      currentPath = path;
      searchQuery = null;
      isSearching = false;
    },

    navigateBack() {
      if (historyIndex < 0) return;
      currentPath = navigationHistory[historyIndex];
      historyIndex -= 1;
      searchQuery = null;
      isSearching = false;
    },

    navigateForward() {
      if (historyIndex >= navigationHistory.length - 1) return;
      historyIndex += 1;
      currentPath = navigationHistory[historyIndex];
      searchQuery = null;
      isSearching = false;
    },

    navigateUp() {
      if (!currentPath) return;
      const parent = parentPath(currentPath);
      if (parent !== null) this.navigate(parent);
    },

    get canGoBack() { return historyIndex >= 0; },
    get canGoForward() { return historyIndex < navigationHistory.length - 1; },

    // ── Search ───────────────────────────────────────────────
    get searchQuery() { return searchQuery; },
    get isSearching() { return isSearching; },
    setSearch(q: string) { searchQuery = q; isSearching = true; },
    setIsSearching(v: boolean) { isSearching = v; },
    setSearchQuery(q: string | null) { searchQuery = q; },
    clearSearch() { searchQuery = null; isSearching = false; },

    // ── Content ──────────────────────────────────────────────
    get entries() { return entries; },
    setEntries(e: EntryDto[]) { entries = e; },

    get selectedPaths() { return selectedPaths; },
    setSelection(paths: string[]) { selectedPaths = new Set(paths); },
    clearSelection() { selectedPaths = new Set(); },
    toggleSelection(path: string) {
      const next = new Set(selectedPaths);
      if (next.has(path)) next.delete(path);
      else next.add(path);
      selectedPaths = next;
    },
    rangeSelect(fromPath: string, toPath: string) {
      // Mirror ContentPanel's sort to find the correct visual range
      const sorted = [...entries].sort((a, b) => {
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
      selectedPaths = new Set(paths.slice(start, end + 1));
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
    setContentZoom(z: number) { contentZoom = z; settings = { ...settings, defaultContentZoom: z }; persistSettings(settings); },

    get treeZoom() { return treeZoom; },
    setTreeZoom(z: number) { treeZoom = z; settings = { ...settings, defaultTreeZoom: z }; persistSettings(settings); },

    get isLoading() { return isLoading; },
    setLoading(v: boolean) { isLoading = v; },

    // ── Local locations ──────────────────────────────────────
    addLocalLocation(path: string) {
      if (settings.localLocations.includes(path)) return;
      const locs = [...settings.localLocations, path];
      settings = { ...settings, localLocations: locs };
      persistSettings(settings);
    },
    removeLocalLocation(path: string) {
      const locs = settings.localLocations.filter(l => l !== path);
      settings = { ...settings, localLocations: locs };
      persistSettings(settings);
    },

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

    // ── Rename ───────────────────────────────────────────────
    get renaming() { return renaming; },
    startRename(path: string, value: string) { renaming = { path, value }; },
    cancelRename() { renaming = null; },

    // ── Progress / job tracking ───────────────────────────────
    get progress() { return progress; },

    startJob(jobId: string, title: string, opts?: StartJobOpts) {
      progress = {
        jobId,
        title,
        progress: 0,
        message: "",
        logs: [],
        done: false,
        success: false,
        resultMessage: "",
        showDialog: opts?.showDialog ?? true,
        statusMessageOnSuccess: opts?.statusMessageOnSuccess ?? null,
        statusMessageOnFailure: opts?.statusMessageOnFailure ?? null,
      };
    },

    updateJobProgress(jobId: string, pct: number, message: string) {
      if (!progress || progress.jobId !== jobId) return;
      progress = { ...progress, progress: pct, message };
    },

    appendJobLog(jobId: string, line: string) {
      if (!progress || progress.jobId !== jobId) return;
      progress = { ...progress, logs: [...progress.logs, line] };
    },

    finishJob(jobId: string, success: boolean, message: string) {
      if (!progress || progress.jobId !== jobId) return;
      progress = {
        ...progress,
        progress: 100,
        done: true,
        success,
        resultMessage: message,
      };
      if (!progress.showDialog) {
        // Auto-dismiss after short delay
        setTimeout(() => { if (progress?.jobId === jobId) progress = null; }, 2000);
      }
    },

    clearProgress() { progress = null; },

    // ── Notifications ─────────────────────────────────────────
    get notifications() { return notifications; },
    notify(kind: NotificationKind, text: string) {
      const n = makeNotification(kind, text);
      notifications = [n, ...notifications].slice(0, 8);
      if (kind !== "error") {
        setTimeout(() => {
          notifications = notifications.filter((x) => x.id !== n.id);
        }, 4000);
      }
    },
    dismissNotification(id: string) {
      notifications = notifications.filter((x) => x.id !== id);
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

// Apply theme on module load
applyTheme(app.settings.theme);
