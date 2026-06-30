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

  // Connection manager visibility (inline panel, not separate window)
  let connectionManagerOpen = $state(false);

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
    },

    navigateBack() {
      if (historyIndex < 0) return;
      currentPath = navigationHistory[historyIndex];
      historyIndex -= 1;
    },

    navigateForward() {
      if (historyIndex >= navigationHistory.length - 1) return;
      historyIndex += 1;
      currentPath = navigationHistory[historyIndex];
    },

    get canGoBack() { return historyIndex >= 0; },
    get canGoForward() { return historyIndex < navigationHistory.length - 1; },

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

    // ── Connections ──────────────────────────────────────────
    get connectionProfiles() { return connectionProfiles; },
    setConnectionProfiles(p: ConnectionProfileDto[]) { connectionProfiles = p; },

    get activeConnections() { return activeConnections; },
    setActiveConnections(c: ActiveConnectionDto[]) { activeConnections = c; },

    // ── Connection manager ───────────────────────────────────
    get connectionManagerOpen() { return connectionManagerOpen; },
    openConnectionManager() { connectionManagerOpen = true; },
    closeConnectionManager() { connectionManagerOpen = false; },

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

    // ── Progress ─────────────────────────────────────────────
    get progress() { return progress; },
    setProgress(p: ProgressState | null) { progress = p; },

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
