import { untrack } from "svelte";
import { Channel, invoke } from "@tauri-apps/api/core";
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
  GhostEntry,
  JobPausedDto,
  ToolStatusDto,
  UpdateChannel,
  UpdateDownloadEvent,
  UpdateDownloadProgress,
  UpdateMetadataDto,
} from "../types/index.js";
import { normalizePath, uniqueDisplayPath, basenameOf } from "../utils/ghosts.js";
import { collectAccentIssues, isAccentUnsafeKind } from "../utils/ascii.js";

// Per-tab vertical scroll memory, keyed by `${tabId}::${normalizedPath}`. Kept
// outside reactive state (a plain Map) so frequent scroll writes cause no churn.
// Lets Dogu restore the scroll position when navigating back into a folder.
const scrollMemory = new Map<string, number>();
let _retryQueueSeq = 0;

function newRetryQueueOpId(): string {
  return `qretry-${Date.now()}-${++_retryQueueSeq}`;
}

// ─── Local types ───────────────────────────────────────────
type ConfirmDialogState = {
  title: string;
  message: string;
  confirmLabel?: string;
  onConfirm: () => void;
};

export type TabState = {
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

export type PaneState = {
  id: string;
  tabs: TabState[];
  activeTabIdx: number;
  viewMode: ViewMode;
  sortKey: ContentSortKey;
  sortDir: SortDirection;
  columnWidths: ContentColumnWidths;
  contentZoom: number;
};

export type TabHighlight = {
  path: string | null;
  color: string;
  isActive: boolean;
};

export type CrossPaneDragState = {
  fromPaneIdx: number;
  fromTabIdx: number;
  toPaneIdx: number;
  toInsertIdx: number;
} | null;

export type FileDragState = {
  items: Array<{ path: string; isDir: boolean }>;
  sourcePaneIdx: number;
} | null;

// PaneView: pane-scoped interface used by Toolbar, TabBar, ContentPanel via Svelte context
export type PaneView = {
  readonly paneIdx: number;
  readonly isFocused: boolean;
  focus(): void;
  // Tabs
  readonly tabs: TabState[];
  readonly activeTabIdx: number;
  readonly activeTabId: string;
  readonly activeTab: TabState | undefined;
  addTab(path?: string): void;
  closeTab(id: string): void;
  setActiveTab(id: string): void;
  setActiveTabByIndex(idx: number): void;
  nextTab(): void;
  prevTab(): void;
  duplicateTab(): void;
  reorderTabs(fromIdx: number, insertIdx: number): void;
  setTabScrollTop(tabIdx: number, scrollTop: number): void;
  /** Remembers vertical scroll for a given path within this tab. */
  setPathScroll(path: string, scrollTop: number): void;
  /** Restores the remembered scroll for a path (0 if never visited). */
  pathScroll(path: string): number;
  setTabLoadedPath(path: string): void;
  // Navigation
  readonly currentPath: string | null;
  navigate(path: string, pushHistory?: boolean): void;
  navigateBack(): void;
  navigateForward(): void;
  navigateUp(): void;
  readonly canGoBack: boolean;
  readonly canGoForward: boolean;
  // Search
  readonly searchQuery: string | null;
  readonly isSearching: boolean;
  setSearch(q: string): void;
  setIsSearching(v: boolean): void;
  setSearchQuery(q: string | null): void;
  clearSearch(): void;
  // Content
  readonly entries: EntryDto[];
  setEntries(e: EntryDto[]): void;
  readonly selectedPaths: Set<string>;
  setSelection(paths: string[]): void;
  clearSelection(): void;
  toggleSelection(path: string): void;
  rangeSelect(fromPath: string, toPath: string): void;
  // Display prefs
  readonly viewMode: ViewMode;
  setViewMode(v: ViewMode): void;
  readonly sortKey: ContentSortKey;
  readonly sortDir: SortDirection;
  setSort(key: ContentSortKey, dir: SortDirection): void;
  readonly columnWidths: ContentColumnWidths;
  setColumnWidth(key: ContentSortKey, w: number): void;
  readonly contentZoom: number;
  setContentZoom(z: number): void;
  // Loading
  readonly isLoading: boolean;
  setLoading(v: boolean): void;
  // Rename
  readonly renaming: RenameState | null;
  startRename(path: string, value: string): void;
  cancelRename(): void;
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
  paused: boolean;
  pauseError: string | null;
  pauseFileName: string | null;
  pauseIsRecoverable: boolean;
  retryQueuedOp: QueuedOp | null;
  retried: boolean;
};

type StartJobOpts = {
  showDialog?: boolean;
  statusMessageOnSuccess?: string;
  statusMessageOnFailure?: string;
  retryQueuedOp?: QueuedOp | null;
};

type QueueRunStats = {
  total: number;
  completed: number;
  succeeded: number;
  failed: number;
  skipped: number;
  running: number;
  mode: QueueExecutionMode;
};

type QueueExecutionMode = "smart" | "parallel" | "sequential";

type QueuePlan = {
  total: number;
  hasDependencies: boolean;
  hasOrderingConstraints: boolean;
  dependencyEdges: number;
  orderingEdges: number;
  maxParallelWidth: number;
  maxConcurrent: number;
  firstWave: number;
  levels: string[][];
  blockingConflicts: QueueConflict[];
  hardPredecessors: Map<string, Set<string>>;
  hardSuccessors: Map<string, Set<string>>;
  orderPredecessors: Map<string, Set<string>>;
};

// ─── Default settings ─────────────────────────────────────
const DEFAULT_COLUMN_WIDTHS: ContentColumnWidths = {
  name: 280,
  type: 80,
  size: 90,
  modified: 140,
};

const DEFAULT_QUEUE_MAX_CONCURRENT = 3;
const MIN_QUEUE_MAX_CONCURRENT = 1;
const MAX_QUEUE_MAX_CONCURRENT = 6;

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
  queueMaxConcurrent: DEFAULT_QUEUE_MAX_CONCURRENT,
  defaultQueueMode: false,
  showQueueRelationMap: true,
  updateChannel: "stable",
  autoCheckUpdates: true,
  defaultOverwriteOnConflict: false,
  renameOnConflict: true,
  rvzPrimaryEngine: "nod",
  rvzEnableFallback: true,
  rvzFallbackEngine: "dolphin",
};

function clampQueueMaxConcurrent(value: unknown): number {
  const n = typeof value === "number" ? value : Number(value);
  if (!Number.isFinite(n)) return DEFAULT_QUEUE_MAX_CONCURRENT;
  return Math.min(MAX_QUEUE_MAX_CONCURRENT, Math.max(MIN_QUEUE_MAX_CONCURRENT, Math.round(n)));
}

function loadSettings(): AppSettings {
  try {
    const raw = localStorage.getItem("dogu-settings");
    if (!raw) return { ...DEFAULT_SETTINGS };
    const parsed = JSON.parse(raw) as Record<string, unknown>;
    if (Array.isArray(parsed.localLocations) && !parsed.favoriteLocations) {
      parsed.favoriteLocations = parsed.localLocations;
    }
    delete parsed.localLocations;
    return {
      ...DEFAULT_SETTINGS,
      ...parsed,
      queueMaxConcurrent: clampQueueMaxConcurrent(parsed.queueMaxConcurrent),
      showQueueRelationMap: parsed.showQueueRelationMap !== false,
      updateChannel: parsed.updateChannel === "beta" ? "beta" : "stable",
      autoCheckUpdates: parsed.autoCheckUpdates !== false,
    } as AppSettings;
  } catch {
    return { ...DEFAULT_SETTINGS };
  }
}

function persistSettings(s: AppSettings) {
  try {
    localStorage.setItem("dogu-settings", JSON.stringify(s));
  } catch {}
}

// ─── Tab helpers ───────────────────────────────────────────
const TAB_COLORS = [
  "#3b82f6", "#a855f7", "#f59e0b", "#ef4444",
  "#06b6d4", "#ec4899", "#f97316", "#84cc16",
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

// ─── Pane helpers ──────────────────────────────────────────
let _paneSeq = 0;

function makePaneState(
  settings: AppSettings,
  initialTabs?: TabState[],
  initialActiveIdx?: number,
): PaneState {
  return {
    id: `pane-${Date.now()}-${++_paneSeq}`,
    tabs: initialTabs ?? [makeTab()],
    activeTabIdx: initialActiveIdx ?? 0,
    viewMode: settings.defaultViewMode,
    sortKey: settings.contentSortKey,
    sortDir: settings.contentSortDirection,
    columnWidths: { ...settings.contentColumnWidths },
    contentZoom: settings.defaultContentZoom,
  };
}

function persistPanes(ps: PaneState[], focusedIdx: number) {
  try {
    localStorage.setItem("dogu-panes", JSON.stringify({
      panes: ps.map(p => ({
        paths: p.tabs.map(t => t.currentPath),
        active: p.activeTabIdx,
      })),
      focused: focusedIdx,
      split: ps.length > 1,
    }));
  } catch {}
}

function loadSavedPanes(settings: AppSettings): { initialPanes: PaneState[]; initialFocusedIdx: number } {
  try {
    // Try new multi-pane format first
    const newRaw = localStorage.getItem("dogu-panes");
    if (newRaw) {
      const parsed = JSON.parse(newRaw) as { panes?: unknown[]; focused?: unknown; split?: unknown };
      if (Array.isArray(parsed.panes) && parsed.panes.length > 0) {
        const paneList = (parsed.panes as Array<{ paths?: unknown[]; active?: unknown }>).map(p => {
          const paths = Array.isArray(p.paths) ? p.paths : [];
          const active = typeof p.active === "number" ? p.active : 0;
          const tabs = paths.length > 0
            ? paths.map((path: unknown) =>
                makeTab(typeof path === "string" && !path.startsWith("remote://") ? path : undefined)
              )
            : [makeTab()];
          const activeIdx = Math.max(0, Math.min(active, tabs.length - 1));
          return makePaneState(settings, tabs, activeIdx);
        });
        const focused = typeof parsed.focused === "number"
          ? Math.max(0, Math.min(parsed.focused, paneList.length - 1))
          : 0;
        return { initialPanes: paneList, initialFocusedIdx: focused };
      }
    }

    // Fall back to old single-pane "dogu-tabs" format
    const oldRaw = localStorage.getItem("dogu-tabs");
    if (oldRaw) {
      const parsed = JSON.parse(oldRaw) as { paths?: unknown[]; active?: unknown };
      const { paths, active } = parsed;
      if (Array.isArray(paths) && paths.length > 0) {
        const tabs = paths.map((p: unknown) =>
          makeTab(typeof p === "string" && !p.startsWith("remote://") ? p : undefined)
        );
        const activeIdx = typeof active === "number"
          ? Math.max(0, Math.min(active, tabs.length - 1))
          : 0;
        return { initialPanes: [makePaneState(settings, tabs, activeIdx)], initialFocusedIdx: 0 };
      }
    }
  } catch {}

  return { initialPanes: [makePaneState(settings)], initialFocusedIdx: 0 };
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

function deleteVsSourceSeverity(deletePath: string, sourcePath: string): ConflictSeverity | null {
  // Deleting the source itself (or one of its ancestors) makes a later consumer
  // impossible even in sequential order.
  if (pathContainsOther(deletePath, sourcePath)) return "blocking";
  // Deleting something *inside* a source directory just changes that tree's
  // contents; queue order still makes sequential execution safe.
  if (pathContainsOther(sourcePath, deletePath)) return "parallel-only";
  return null;
}

/**
 * Resolves the final output paths of every queued op against the outputs of
 * earlier ops (in queue order), applying the rename-on-conflict policy. This is
 * the single source of truth for "what each op's files will actually be called":
 * when two ops would produce the same name, the later one gets a " (2)" suffix
 * (if it renames), so it no longer collides. Returns opId → resolved outputs.
 */
function resolveQueueOutputs(ops: QueuedOp[]): Map<string, GhostEntry[]> {
  const taken = new Set<string>();
  const map = new Map<string, GhostEntry[]>();
  for (const op of ops) {
    const resolved: GhostEntry[] = [];
    for (const g of op.produces) {
      let path = g.path;
      if (taken.has(normalizePath(path)) && !op.overwrite && op.renameOnConflict) {
        path = uniqueDisplayPath(g.path, taken);
      }
      taken.add(normalizePath(path));
      resolved.push(path === g.path ? g : { ...g, path, name: basenameOf(path) });
    }
    map.set(op.id, resolved);
  }
  return map;
}

function detectConflicts(ops: QueuedOp[]): QueueConflict[] {
  const conflicts: QueueConflict[] = [];
  // Compare against RESOLVED outputs so two ops that would produce the same name
  // but rename around it are not reported as colliding.
  const resolvedOutputs = resolveQueueOutputs(ops);

  function push(opAId: string, opBId: string, kind: ConflictKind, severity: ConflictSeverity, pathA: string, pathB: string) {
    if (!conflicts.some(c => c.opAId === opAId && c.opBId === opBId && c.kind === kind)) {
      conflicts.push({ opAId, opBId, kind, severity, pathA, pathB });
    }
  }

  // Conflicts are computed against the actual output FILES each op creates
  // (`produces`), not the destination folders — deleting or writing a different
  // file inside a shared folder is not a conflict.
  const outputsOf = (op: QueuedOp): string[] =>
    (resolvedOutputs.get(op.id) ?? op.produces).map((g) => g.path);

  for (let i = 0; i < ops.length; i++) {
    for (let j = i + 1; j < ops.length; j++) {
      const a = ops[i], b = ops[j];
      const outA = outputsOf(a);
      const outB = outputsOf(b);

      // Both ops create the exact same output file.
      for (const pa of outA) {
        for (const pb of outB) {
          if (normalizePath(pa) === normalizePath(pb)) push(a.id, b.id, "dest-collision", "parallel-only", pa, pb);
        }
      }

      // `a` deletes something `b` needs as input, or is going to create.
      for (const d of a.deletes) {
        for (const s of b.sources) {
          const severity = deleteVsSourceSeverity(d, s);
          if (severity) push(a.id, b.id, "source-deleted", severity, d, s);
        }
        for (const p of outB)      { if (pathsOverlap(d, p)) push(a.id, b.id, "dest-deleted",   "blocking", d, p); }
      }

      for (const d of b.deletes) {
        for (const s of a.sources) {
          if (deleteVsSourceSeverity(d, s)) push(b.id, a.id, "source-deleted", "parallel-only", d, s);
        }
        for (const p of outA)      { if (pathsOverlap(d, p)) push(b.id, a.id, "dest-deleted",   "parallel-only", d, p); }
      }
    }
  }

  return conflicts;
}

// ─── Ghost dependency helpers ──────────────────────────────
/** Returns the id of the queued op that produces `path` (or contains it), if any. */
function ghostProducerOf(path: string, ops: QueuedOp[]): string | null {
  const np = normalizePath(path);
  const resolvedOutputs = resolveQueueOutputs(ops);
  for (const op of ops) {
    for (const g of resolvedOutputs.get(op.id) ?? op.produces) {
      const gp = normalizePath(g.path);
      if (np === gp || np.startsWith(gp + "/")) return op.id;
    }
  }
  return null;
}

function ghostProducersTouchingPath(path: string, ops: QueuedOp[], includeDescendantGhosts: boolean): string[] {
  const np = normalizePath(path);
  const producers = new Set<string>();
  const resolvedOutputs = resolveQueueOutputs(ops);
  for (const op of ops) {
    for (const g of resolvedOutputs.get(op.id) ?? op.produces) {
      const gp = normalizePath(g.path);
      const pathConsumesGhost = np === gp || np.startsWith(gp + "/");
      const pathContainsGhost = includeDescendantGhosts && gp.startsWith(np + "/");
      if (pathConsumesGhost || pathContainsGhost) producers.add(op.id);
    }
  }
  return [...producers];
}

/** Computes which already-queued ops an op depends on (consumes their outputs). */
function linkDependencies(op: QueuedOp, existing: QueuedOp[]): string[] {
  const deps = new Set<string>();
  for (const input of [...op.sources, ...op.deletes]) {
    for (const producer of ghostProducersTouchingPath(input, existing, true)) {
      if (producer !== op.id) deps.add(producer);
    }
  }
  for (const input of op.destinations) {
    const producer = ghostProducerOf(input, existing);
    if (producer && producer !== op.id) deps.add(producer);
  }
  return [...deps];
}

/** Transitive closure of ops that (directly or indirectly) depend on any of `ids`. */
function collectDependents(ids: string[], ops: QueuedOp[]): Set<string> {
  const result = new Set(ids);
  let changed = true;
  while (changed) {
    changed = false;
    for (const op of ops) {
      if (result.has(op.id)) continue;
      if (op.dependsOn.some((d) => result.has(d))) {
        result.add(op.id);
        changed = true;
      }
    }
  }
  return result;
}

function cloneQueuedOpForRetry(op: QueuedOp): QueuedOp {
  const retryId = newRetryQueueOpId();
  return {
    ...op,
    id: retryId,
    dependsOn: [],
    produces: op.produces.map((ghost) => ({ ...ghost, producedByOpId: retryId })),
  };
}

/** True when every op appears after all the ops it depends on. */
function orderRespectsDeps(ops: QueuedOp[]): boolean {
  const indexOf = new Map(ops.map((o, i) => [o.id, i]));
  return ops.every((op, i) => op.dependsOn.every((d) => (indexOf.get(d) ?? -1) < i));
}

function buildQueuePlan(ops: QueuedOp[], conflicts: QueueConflict[], maxConcurrent: number): QueuePlan {
  const ids = new Set(ops.map((o) => o.id));
  const indexOf = new Map(ops.map((o, i) => [o.id, i]));
  const hardPredecessors = new Map<string, Set<string>>();
  const hardSuccessors = new Map<string, Set<string>>();
  const orderPredecessors = new Map<string, Set<string>>();
  const allSuccessors = new Map<string, Set<string>>();

  for (const op of ops) {
    hardPredecessors.set(op.id, new Set());
    hardSuccessors.set(op.id, new Set());
    orderPredecessors.set(op.id, new Set());
    allSuccessors.set(op.id, new Set());
  }

  let dependencyEdges = 0;
  let orderingEdges = 0;

  const addEdge = (from: string, to: string, hard: boolean) => {
    if (from === to || !ids.has(from) || !ids.has(to)) return;
    const predecessors = hard ? hardPredecessors : orderPredecessors;
    const predSet = predecessors.get(to);
    const succSet = allSuccessors.get(from);
    if (!predSet || !succSet || predSet.has(from)) return;
    predSet.add(from);
    succSet.add(to);
    if (hard) {
      hardSuccessors.get(from)?.add(to);
      dependencyEdges += 1;
    } else {
      orderingEdges += 1;
    }
  };

  for (const op of ops) {
    for (const dep of op.dependsOn) addEdge(dep, op.id, true);
  }

  for (const conflict of conflicts) {
    if (conflict.severity !== "parallel-only") continue;
    const aIndex = indexOf.get(conflict.opAId);
    const bIndex = indexOf.get(conflict.opBId);
    if (aIndex === undefined || bIndex === undefined) continue;
    const earlier = ops[Math.min(aIndex, bIndex)];
    const later = ops[Math.max(aIndex, bIndex)];
    addEdge(earlier.id, later.id, false);
  }

  const remainingPreds = new Map<string, Set<string>>();
  for (const op of ops) {
    remainingPreds.set(op.id, new Set([
      ...(hardPredecessors.get(op.id) ?? []),
      ...(orderPredecessors.get(op.id) ?? []),
    ]));
  }

  const pending = new Set(ops.map((o) => o.id));
  const levels: string[][] = [];
  while (pending.size > 0) {
    const ready = ops
      .filter((op) => pending.has(op.id) && (remainingPreds.get(op.id)?.size ?? 0) === 0)
      .map((op) => op.id);
    if (ready.length === 0) {
      levels.push([...pending]);
      break;
    }
    levels.push(ready);
    for (const id of ready) {
      pending.delete(id);
      for (const child of allSuccessors.get(id) ?? []) {
        remainingPreds.get(child)?.delete(id);
      }
    }
  }

  return {
    total: ops.length,
    hasDependencies: dependencyEdges > 0,
    hasOrderingConstraints: orderingEdges > 0,
    dependencyEdges,
    orderingEdges,
    maxParallelWidth: Math.max(0, ...levels.map((level) => level.length)),
    maxConcurrent: Math.min(maxConcurrent, Math.max(0, ...levels.map((level) => level.length))),
    firstWave: levels[0]?.length ?? 0,
    levels,
    blockingConflicts: conflicts.filter((c) => c.severity === "blocking"),
    hardPredecessors,
    hardSuccessors,
    orderPredecessors,
  };
}

// ─── App state (Svelte 5 runes) ────────────────────────────
function createAppState() {
  // Settings
  let settings = $state<AppSettings>(loadSettings());

  // Panes (replaces flat tabs + activeTabIdx + display prefs)
  const { initialPanes, initialFocusedIdx } = untrack(() => loadSavedPanes(loadSettings()));
  let panes = $state<PaneState[]>(initialPanes);
  let focusedPaneIdx = $state(initialFocusedIdx);

  // Global display pref: tree zoom (sidebar only, one sidebar)
  let treeZoom = $state(untrack(() => settings.defaultTreeZoom));

  // Connections
  let connectionProfiles = $state<ConnectionProfileDto[]>([]);
  let activeConnections = $state<ActiveConnectionDto[]>([]);

  // System volumes and known folders
  let volumes = $state<VolumeDto[]>([]);
  let knownFolders = $state<KnownFoldersDto | null>(null);

  // External tool status (probed at startup) + derived capabilities.
  let toolStatus = $state<ToolStatusDto[]>([]);
  const capabilitySet = $derived(
    new Set(toolStatus.filter(ts => ts.runtime.available).flatMap(ts => ts.enables)),
  );

  // Clipboard (shared across panes/tabs)
  let clipboard = $state<ClipboardState | null>(null);

  // Jobs
  let jobs = $state<JobEntry[]>([]);
  let jobsPanelOpen = $state(false);
  let failedQueueRetryRunning = $state(false);
  const jobWaiters = new Map<string, Array<(success: boolean) => void>>();

  // Operation queue — starts in the user's configured default mode.
  let queueMode = $state(untrack(() => settings.defaultQueueMode));
  let opQueue = $state<QueuedOp[]>([]);
  let queueRunning = $state(false);
  let queueRunStats = $state<QueueRunStats | null>(null);
  const queueConflicts = $derived(detectConflicts(opQueue));
  const queueMaxConcurrent = $derived(clampQueueMaxConcurrent(settings.queueMaxConcurrent));
  const queuePlan = $derived(buildQueuePlan(opQueue, queueConflicts, queueMaxConcurrent));
  // Final output names per op after applying rename-on-conflict across the queue.
  const resolvedOutputs = $derived(resolveQueueOutputs(opQueue));
  // Any op consuming another op's ghost output forces ordered (sequential) execution.
  const queueHasDependencies = $derived(opQueue.some((o) => o.dependsOn.length > 0));
  // Ids of chd ops whose source *file names* carry accents chdman can't process.
  // These block the whole queue from running until they are de-accented.
  const queueAccentBlocked = $derived(
    new Set(
      opQueue
        .filter((o) => isAccentUnsafeKind(o.kind) && collectAccentIssues(o.sources).length > 0)
        .map((o) => o.id),
    ),
  );

  // Cross-pane tab drag
  let crossPaneDrag = $state<CrossPaneDragState>(null);
  let fileDrag = $state<FileDragState>(null);

  // Notifications
  let notifications = $state<NotificationEntry[]>([]);

  // Updates
  let updateChecking = $state(false);
  let updateInstalling = $state(false);
  let updateNoticeOpen = $state(false);
  let availableUpdate = $state<UpdateMetadataDto | null>(null);
  let updateDownloadProgress = $state<UpdateDownloadProgress | null>(null);
  let updateError = $state<string | null>(null);
  let lastUpdateCheckAt = $state<number | null>(null);

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

  // ── Pane helper shorthand ───────────────────────────────
  function fp(): PaneState { return panes[focusedPaneIdx] ?? panes[0]; }
  function ft(): TabState  { const p = fp(); return p.tabs[p.activeTabIdx] ?? p.tabs[0]; }

  // ── PaneView factory ────────────────────────────────────
  function getPaneView(idx: number): PaneView {
    function p(): PaneState { return panes[idx] ?? panes[0]; }
    function t(): TabState  { const pn = p(); return pn.tabs[pn.activeTabIdx] ?? pn.tabs[0]; }

    return {
      get paneIdx() { return idx; },
      get isFocused() { return focusedPaneIdx === idx; },
      focus() { focusedPaneIdx = idx; },

      // Tabs
      get tabs() { return p()?.tabs ?? []; },
      get activeTabIdx() { return p()?.activeTabIdx ?? 0; },
      get activeTabId() { return t()?.id ?? ""; },
      get activeTab() { return t(); },

      addTab(path?: string) {
        const pn = p();
        const newTab = makeTab(path);
        pn.tabs = [...pn.tabs, newTab];
        pn.activeTabIdx = pn.tabs.length - 1;
        persistPanes(panes, focusedPaneIdx);
      },

      closeTab(id: string) {
        const pn = p();
        if (pn.tabs.length <= 1) return;
        const i = pn.tabs.findIndex(tab => tab.id === id);
        if (i === -1) return;
        const newTabs = pn.tabs.filter(tab => tab.id !== id);
        pn.tabs = newTabs;
        if (pn.activeTabIdx >= newTabs.length) {
          pn.activeTabIdx = newTabs.length - 1;
        } else if (pn.activeTabIdx > i) {
          pn.activeTabIdx -= 1;
        }
        persistPanes(panes, focusedPaneIdx);
      },

      setActiveTab(id: string) {
        const pn = p();
        const i = pn.tabs.findIndex(tab => tab.id === id);
        if (i !== -1) pn.activeTabIdx = i;
      },

      setActiveTabByIndex(i: number) {
        const pn = p();
        if (i >= 0 && i < pn.tabs.length) pn.activeTabIdx = i;
      },

      nextTab() {
        const pn = p();
        pn.activeTabIdx = (pn.activeTabIdx + 1) % pn.tabs.length;
      },

      prevTab() {
        const pn = p();
        pn.activeTabIdx = (pn.activeTabIdx - 1 + pn.tabs.length) % pn.tabs.length;
      },

      duplicateTab() {
        const pn = p();
        const cur = pn.tabs[pn.activeTabIdx];
        if (!cur) return;
        const newTab = makeTab(cur.currentPath ?? undefined);
        pn.tabs = [...pn.tabs.slice(0, pn.activeTabIdx + 1), newTab, ...pn.tabs.slice(pn.activeTabIdx + 1)];
        pn.activeTabIdx = pn.activeTabIdx + 1;
        persistPanes(panes, focusedPaneIdx);
      },

      reorderTabs(fromIdx: number, insertIdx: number) {
        if (fromIdx === insertIdx || fromIdx + 1 === insertIdx) return;
        const pn = p();
        const activeId = pn.tabs[pn.activeTabIdx]?.id;
        const next = [...pn.tabs];
        const [moved] = next.splice(fromIdx, 1);
        next.splice(fromIdx < insertIdx ? insertIdx - 1 : insertIdx, 0, moved);
        pn.tabs = next;
        pn.activeTabIdx = activeId ? next.findIndex(tab => tab.id === activeId) : 0;
        persistPanes(panes, focusedPaneIdx);
      },

      setTabScrollTop(tabIdx: number, scrollTop: number) {
        const pn = p();
        if (pn.tabs[tabIdx]) pn.tabs[tabIdx].scrollTop = scrollTop;
      },

      setPathScroll(path: string, scrollTop: number) {
        const id = t()?.id;
        if (id) scrollMemory.set(`${id}::${normalizePath(path)}`, scrollTop);
      },

      pathScroll(path: string) {
        const id = t()?.id;
        return id ? (scrollMemory.get(`${id}::${normalizePath(path)}`) ?? 0) : 0;
      },

      setTabLoadedPath(path: string) {
        const tab = t();
        if (tab) tab.loadedPath = path;
      },

      // Navigation
      get currentPath() { return t()?.currentPath ?? null; },
      get canGoBack()    { return (t()?.historyIdx ?? -1) >= 0; },
      get canGoForward() {
        const tab = t();
        return tab ? tab.historyIdx < tab.history.length - 1 : false;
      },

      navigate(path: string, pushHistory = true) {
        const tab = t();
        if (!tab) return;
        if (pushHistory && tab.currentPath !== null) {
          tab.history = [...tab.history.slice(0, tab.historyIdx + 1), tab.currentPath];
          tab.historyIdx = tab.history.length - 1;
        }
        tab.currentPath = path;
        tab.searchQuery = null;
        tab.isSearching = false;
        tab.scrollTop = 0;
        persistPanes(panes, focusedPaneIdx);
      },

      navigateBack() {
        const tab = t();
        if (!tab || tab.historyIdx < 0) return;
        tab.currentPath = tab.history[tab.historyIdx];
        tab.historyIdx -= 1;
        tab.searchQuery = null;
        tab.isSearching = false;
        tab.scrollTop = 0;
      },

      navigateForward() {
        const tab = t();
        if (!tab || tab.historyIdx >= tab.history.length - 1) return;
        tab.historyIdx += 1;
        tab.currentPath = tab.history[tab.historyIdx];
        tab.searchQuery = null;
        tab.isSearching = false;
        tab.scrollTop = 0;
      },

      navigateUp() {
        const path = t()?.currentPath;
        if (path) {
          const parent = parentPath(path);
          if (parent !== null) getPaneView(idx).navigate(parent);
        }
      },

      // Search
      get searchQuery() { return t()?.searchQuery ?? null; },
      get isSearching()  { return t()?.isSearching ?? false; },

      setSearch(q: string) {
        const tab = t();
        if (tab) { tab.searchQuery = q; tab.isSearching = true; }
      },
      setIsSearching(v: boolean) {
        const tab = t();
        if (tab) tab.isSearching = v;
      },
      setSearchQuery(q: string | null) {
        const tab = t();
        if (tab) tab.searchQuery = q;
      },
      clearSearch() {
        const tab = t();
        if (tab) { tab.searchQuery = null; tab.isSearching = false; }
      },

      // Content
      get entries() { return t()?.entries ?? []; },
      setEntries(e: EntryDto[]) {
        const tab = t();
        if (tab) tab.entries = e;
      },

      get selectedPaths() { return t()?.selectedPaths ?? new Set<string>(); },
      setSelection(paths: string[]) {
        const tab = t();
        if (tab) tab.selectedPaths = new Set(paths);
      },
      clearSelection() {
        const tab = t();
        if (tab) tab.selectedPaths = new Set();
      },
      toggleSelection(path: string) {
        const tab = t();
        if (!tab) return;
        const next = new Set(tab.selectedPaths);
        if (next.has(path)) next.delete(path); else next.add(path);
        tab.selectedPaths = next;
      },
      rangeSelect(fromPath: string, toPath: string) {
        const pn = p();
        const tab = t();
        if (!tab) return;
        const sorted = [...tab.entries].sort((a, b) => {
          if (a.isDir !== b.isDir) return a.isDir ? -1 : 1;
          let cmp = 0;
          if (pn.sortKey === "name") cmp = a.name.localeCompare(b.name, undefined, { sensitivity: "base" });
          else if (pn.sortKey === "type") cmp = a.extension.localeCompare(b.extension);
          else if (pn.sortKey === "size") cmp = a.size - b.size;
          else if (pn.sortKey === "modified") cmp = a.modifiedTs - b.modifiedTs;
          return pn.sortDir === "asc" ? cmp : -cmp;
        });
        const paths = sorted.map(e => e.path);
        const fromIdx = paths.indexOf(fromPath);
        const toIdx   = paths.indexOf(toPath);
        if (fromIdx === -1 || toIdx === -1) return;
        const [start, end] = fromIdx < toIdx ? [fromIdx, toIdx] : [toIdx, fromIdx];
        tab.selectedPaths = new Set(paths.slice(start, end + 1));
      },

      // Display prefs
      get viewMode() { return p()?.viewMode ?? "list"; },
      setViewMode(v: ViewMode) {
        const pn = p();
        if (pn) pn.viewMode = v;
        settings = { ...settings, defaultViewMode: v };
        persistSettings(settings);
      },

      get sortKey() { return p()?.sortKey ?? "name"; },
      get sortDir()  { return p()?.sortDir ?? "asc"; },
      setSort(key: ContentSortKey, dir: SortDirection) {
        const pn = p();
        if (pn) { pn.sortKey = key; pn.sortDir = dir; }
        settings = { ...settings, contentSortKey: key, contentSortDirection: dir };
        persistSettings(settings);
      },

      get columnWidths() { return p()?.columnWidths ?? { ...DEFAULT_COLUMN_WIDTHS }; },
      setColumnWidth(key: ContentSortKey, w: number) {
        const pn = p();
        if (pn) pn.columnWidths = { ...pn.columnWidths, [key]: w };
        settings = { ...settings, contentColumnWidths: { ...(p()?.columnWidths ?? DEFAULT_COLUMN_WIDTHS) } };
        persistSettings(settings);
      },

      get contentZoom() { return p()?.contentZoom ?? 1; },
      setContentZoom(z: number) {
        const pn = p();
        if (pn) pn.contentZoom = z;
        settings = { ...settings, defaultContentZoom: z };
        persistSettings(settings);
      },

      // Loading
      get isLoading() { return t()?.isLoading ?? false; },
      setLoading(v: boolean) {
        const tab = t();
        if (tab) tab.isLoading = v;
      },

      // Rename
      get renaming() { return t()?.renaming ?? null; },
      startRename(path: string, value: string) {
        const tab = t();
        if (tab) tab.renaming = { path, value };
      },
      cancelRename() {
        const tab = t();
        if (tab) tab.renaming = null;
      },
    };
  }

  function pushNotification(kind: NotificationKind, text: string) {
    const n = makeNotification(kind, text);
    notifications = [n, ...notifications].slice(0, 8);
    if (kind !== "error" && kind !== "warn") {
      setTimeout(() => { notifications = notifications.filter(x => x.id !== n.id); }, 4000);
    }
  }

  function isUpdaterNotConfiguredError(error: string): boolean {
    return error.includes("Dogu updater is not configured") || error.includes("DOGU_UPDATER_PUBLIC_KEY");
  }

  async function checkForUpdates(options: { manual?: boolean; allowDowngrade?: boolean } = {}) {
    if (updateChecking || updateInstalling) return null;
    updateChecking = true;
    updateError = null;
    updateDownloadProgress = null;
    try {
      const update = await invoke<UpdateMetadataDto | null>("check_for_update", {
        channel: settings.updateChannel,
        allowDowngrade: !!options.allowDowngrade,
      });
      lastUpdateCheckAt = Date.now();
      availableUpdate = update;
      if (update) {
        updateNoticeOpen = true;
      }
      return update;
    } catch (error) {
      const message = String(error);
      const shouldSurface = options.manual || !isUpdaterNotConfiguredError(message);
      updateError = shouldSurface ? message : null;
      if (shouldSurface) {
        pushNotification("error", message);
      }
      return null;
    } finally {
      updateChecking = false;
    }
  }

  async function installAvailableUpdate() {
    if (!availableUpdate || updateInstalling) return;
    updateInstalling = true;
    updateError = null;
    updateDownloadProgress = { downloaded: 0, contentLength: null, percent: null };
    const onEvent = new Channel<UpdateDownloadEvent>((event) => {
      if (event.event === "started") {
        updateDownloadProgress = {
          downloaded: 0,
          contentLength: event.data.contentLength,
          percent: null,
        };
      } else if (event.event === "progress") {
        const total = event.data.contentLength;
        updateDownloadProgress = {
          downloaded: event.data.downloaded,
          contentLength: total,
          percent: total && total > 0 ? Math.min(100, Math.round(event.data.downloaded / total * 100)) : null,
        };
      }
    });

    try {
      await invoke("install_pending_update", { onEvent });
    } catch (error) {
      updateInstalling = false;
      updateError = String(error);
      pushNotification("error", updateError);
    }
  }

  return {
    // ── Settings ────────────────────────────────────────────
    get settings() { return settings; },
    updateSettings(patch: Partial<AppSettings>) {
      settings = {
        ...settings,
        ...patch,
        queueMaxConcurrent: clampQueueMaxConcurrent(patch.queueMaxConcurrent ?? settings.queueMaxConcurrent),
        updateChannel: patch.updateChannel === "beta" ? "beta" : (patch.updateChannel === "stable" ? "stable" : settings.updateChannel),
      };
      persistSettings(settings);
      applyTheme(settings.theme);
    },

    // ── Updates ─────────────────────────────────────────────
    get updateChecking() { return updateChecking; },
    get updateInstalling() { return updateInstalling; },
    get updateNoticeOpen() { return updateNoticeOpen; },
    get availableUpdate() { return availableUpdate; },
    get updateDownloadProgress() { return updateDownloadProgress; },
    get updateError() { return updateError; },
    get lastUpdateCheckAt() { return lastUpdateCheckAt; },
    async checkForUpdates(options?: { manual?: boolean; allowDowngrade?: boolean }) {
      return checkForUpdates(options);
    },
    maybeAutoCheckForUpdates() {
      if (!settings.autoCheckUpdates) return;
      void checkForUpdates({ manual: false, allowDowngrade: false });
    },
    async installAvailableUpdate() {
      await installAvailableUpdate();
    },
    dismissUpdateNotice() {
      updateNoticeOpen = false;
    },
    clearUpdateError() {
      updateError = null;
    },
    setUpdateChannel(channel: UpdateChannel) {
      settings = { ...settings, updateChannel: channel };
      persistSettings(settings);
    },

    // ── Theme ────────────────────────────────────────────────
    get theme() { return theme; },
    setTheme(t: ThemeMode) {
      settings = { ...settings, theme: t };
      persistSettings(settings);
      applyTheme(t);
    },

    // ── Panes ────────────────────────────────────────────────
    get panes() { return panes; },
    get focusedPaneIdx() { return focusedPaneIdx; },
    get isSplit() { return panes.length > 1; },

    focusPane(idx: number) {
      if (idx >= 0 && idx < panes.length) focusedPaneIdx = idx;
    },

    addPane(path?: string) {
      if (panes.length >= 2) return;
      const currentPath = path ?? fp().tabs[fp().activeTabIdx]?.currentPath ?? undefined;
      const newPane = makePaneState(settings, [makeTab(currentPath)], 0);
      panes = [...panes, newPane];
      focusedPaneIdx = 1;
      persistPanes(panes, focusedPaneIdx);
    },

    removePane(idx: number) {
      if (panes.length <= 1) return;
      panes = panes.filter((_, i) => i !== idx);
      focusedPaneIdx = Math.min(focusedPaneIdx, panes.length - 1);
      persistPanes(panes, focusedPaneIdx);
    },

    closeSecondPane() {
      if (panes.length <= 1) return;
      panes = [panes[0]];
      focusedPaneIdx = 0;
      persistPanes(panes, focusedPaneIdx);
    },

    getPaneView,

    // ── Cross-pane tab drag ───────────────────────────────────
    get crossPaneDrag() { return crossPaneDrag; },
    setCrossPaneDrag(s: CrossPaneDragState) { crossPaneDrag = s; },
    get fileDrag() { return fileDrag; },
    setFileDrag(s: FileDragState) { fileDrag = s; },

    moveTabToPane(fromPaneIdx: number, fromTabIdx: number, toPaneIdx: number, toInsertIdx: number) {
      const fromPane = panes[fromPaneIdx];
      const toPane   = panes[toPaneIdx];
      if (!fromPane || !toPane || fromPaneIdx === toPaneIdx) return;
      if (fromPane.tabs.length <= 1) return; // can't leave a pane empty
      const moved     = fromPane.tabs[fromTabIdx];
      const newFrom   = fromPane.tabs.filter((_, i) => i !== fromTabIdx);
      fromPane.tabs   = newFrom;
      if (fromPane.activeTabIdx >= newFrom.length)      fromPane.activeTabIdx = newFrom.length - 1;
      else if (fromPane.activeTabIdx > fromTabIdx)      fromPane.activeTabIdx -= 1;
      const at        = Math.min(toInsertIdx, toPane.tabs.length);
      toPane.tabs     = [...toPane.tabs.slice(0, at), moved, ...toPane.tabs.slice(at)];
      toPane.activeTabIdx = at;
      persistPanes(panes, focusedPaneIdx);
    },

    // ── Tabs (focused pane) ──────────────────────────────────
    get tabs() { return fp().tabs; },
    get activeTabIdx() { return fp().activeTabIdx; },
    get activeTabId() { return ft()?.id ?? ""; },
    get activeTab() { return ft(); },

    addTab(path?: string) { getPaneView(focusedPaneIdx).addTab(path); },
    closeTab(id: string) { getPaneView(focusedPaneIdx).closeTab(id); },
    setActiveTab(id: string) { getPaneView(focusedPaneIdx).setActiveTab(id); },
    setActiveTabByIndex(idx: number) { getPaneView(focusedPaneIdx).setActiveTabByIndex(idx); },
    nextTab() { getPaneView(focusedPaneIdx).nextTab(); },
    prevTab() { getPaneView(focusedPaneIdx).prevTab(); },
    duplicateTab() { getPaneView(focusedPaneIdx).duplicateTab(); },
    reorderTabs(fromIdx: number, insertIdx: number) { getPaneView(focusedPaneIdx).reorderTabs(fromIdx, insertIdx); },
    setTabScrollTop(tabIdx: number, scrollTop: number) { getPaneView(focusedPaneIdx).setTabScrollTop(tabIdx, scrollTop); },
    setTabLoadedPath(path: string) { getPaneView(focusedPaneIdx).setTabLoadedPath(path); },

    // ── Navigation (focused pane) ────────────────────────────
    get currentPath() { return ft()?.currentPath ?? null; },
    get canGoBack()    { return getPaneView(focusedPaneIdx).canGoBack; },
    get canGoForward() { return getPaneView(focusedPaneIdx).canGoForward; },
    navigate(path: string, pushHistory = true) { getPaneView(focusedPaneIdx).navigate(path, pushHistory); },
    navigateBack()    { getPaneView(focusedPaneIdx).navigateBack(); },
    navigateForward() { getPaneView(focusedPaneIdx).navigateForward(); },
    navigateUp()      { getPaneView(focusedPaneIdx).navigateUp(); },

    // ── Search (focused pane) ─────────────────────────────────
    get searchQuery() { return ft()?.searchQuery ?? null; },
    get isSearching()  { return ft()?.isSearching ?? false; },
    setSearch(q: string)          { getPaneView(focusedPaneIdx).setSearch(q); },
    setIsSearching(v: boolean)    { getPaneView(focusedPaneIdx).setIsSearching(v); },
    setSearchQuery(q: string | null) { getPaneView(focusedPaneIdx).setSearchQuery(q); },
    clearSearch()                 { getPaneView(focusedPaneIdx).clearSearch(); },

    // ── Content (focused pane) ───────────────────────────────
    get entries() { return ft()?.entries ?? []; },
    setEntries(e: EntryDto[]) { getPaneView(focusedPaneIdx).setEntries(e); },
    get selectedPaths() { return ft()?.selectedPaths ?? new Set<string>(); },
    setSelection(paths: string[]) { getPaneView(focusedPaneIdx).setSelection(paths); },
    clearSelection()              { getPaneView(focusedPaneIdx).clearSelection(); },
    toggleSelection(path: string) { getPaneView(focusedPaneIdx).toggleSelection(path); },
    rangeSelect(fromPath: string, toPath: string) { getPaneView(focusedPaneIdx).rangeSelect(fromPath, toPath); },

    get viewMode()    { return fp()?.viewMode ?? "list"; },
    setViewMode(v: ViewMode) { getPaneView(focusedPaneIdx).setViewMode(v); },
    get sortKey()  { return fp()?.sortKey ?? "name"; },
    get sortDir()  { return fp()?.sortDir ?? "asc"; },
    setSort(key: ContentSortKey, dir: SortDirection) { getPaneView(focusedPaneIdx).setSort(key, dir); },
    get columnWidths() { return fp()?.columnWidths ?? { ...DEFAULT_COLUMN_WIDTHS }; },
    setColumnWidth(key: ContentSortKey, w: number) { getPaneView(focusedPaneIdx).setColumnWidth(key, w); },
    get contentZoom() { return fp()?.contentZoom ?? 1; },
    setContentZoom(z: number) { getPaneView(focusedPaneIdx).setContentZoom(z); },
    get isLoading()  { return ft()?.isLoading ?? false; },
    setLoading(v: boolean) { getPaneView(focusedPaneIdx).setLoading(v); },
    get renaming()   { return ft()?.renaming ?? null; },
    startRename(path: string, value: string) { getPaneView(focusedPaneIdx).startRename(path, value); },
    cancelRename()   { getPaneView(focusedPaneIdx).cancelRename(); },

    // ── Sidebar: all tabs from all panes (for highlights) ────
    get allTabHighlights(): TabHighlight[] {
      const result: TabHighlight[] = [];
      for (let pIdx = 0; pIdx < panes.length; pIdx++) {
        const pn = panes[pIdx];
        for (let tIdx = 0; tIdx < pn.tabs.length; tIdx++) {
          const tab = pn.tabs[tIdx];
          result.push({
            path: tab.currentPath,
            color: tab.color,
            isActive: pIdx === focusedPaneIdx && tIdx === pn.activeTabIdx,
          });
        }
      }
      return result;
    },

    // ── Favorite locations ────────────────────────────────────
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
    get volumes()      { return volumes; },
    get knownFolders() { return knownFolders; },
    setVolumes(v: VolumeDto[]) { volumes = v; },
    setKnownFolders(k: KnownFoldersDto) { knownFolders = k; },

    // ── Tree zoom (global — sidebar only) ───────────────────
    get treeZoom() { return treeZoom; },
    setTreeZoom(z: number) {
      treeZoom = z;
      settings = { ...settings, defaultTreeZoom: z };
      persistSettings(settings);
      document.documentElement.style.setProperty("--tree-scale", String(z));
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
    openConnectionManager()  { connectionManagerOpen = true; },
    closeConnectionManager() { connectionManagerOpen = false; },

    // ── Settings dialog ──────────────────────────────────────
    get settingsOpen() { return settingsOpen; },
    openSettings()  { settingsOpen = true; },
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

    // ── Jobs / operation queue ────────────────────────────────
    get jobs() { return jobs; },
    get jobsPanelOpen() { return jobsPanelOpen; },
    get failedQueueRetryRunning() { return failedQueueRetryRunning; },
    openJobsPanel()   { jobsPanelOpen = true; },
    closeJobsPanel()  { jobsPanelOpen = false; },
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
        retryQueuedOp: opts?.retryQueuedOp ?? null,
        retried: false,
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
      if (success && !queueRunning) {
        setTimeout(() => { jobs = jobs.filter(j => j.id !== jobId); }, 5000);
      }
      const waiters = jobWaiters.get(jobId) ?? [];
      jobWaiters.delete(jobId);
      for (const w of waiters) w(success);
    },

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

    clearJobPause(jobId: string) {
      const idx = jobs.findIndex(j => j.id === jobId);
      if (idx === -1) return;
      jobs[idx] = { ...jobs[idx], paused: false, pauseError: null, pauseFileName: null, pauseIsRecoverable: false };
    },

    retryFailedJob(jobId: string) {
      if (queueRunning || failedQueueRetryRunning) return false;
      const idx = jobs.findIndex(j => j.id === jobId);
      if (idx === -1) return false;
      const job = jobs[idx];
      if (!job.done || job.success || job.retried || !job.retryQueuedOp) return false;
      jobs[idx] = { ...job, retried: true };
      const retryOp = cloneQueuedOpForRetry(job.retryQueuedOp);
      void retryOp.execute(undefined, retryOp);
      jobsPanelOpen = true;
      return true;
    },

    async retryAllFailedJobs() {
      if (queueRunning || failedQueueRetryRunning) return 0;
      const retryable = jobs
        .map((job, index) => ({ job, index }))
        .filter(({ job }) => job.done && !job.success && !job.retried && !!job.retryQueuedOp);
      if (retryable.length === 0) return 0;

      failedQueueRetryRunning = true;
      jobs = jobs.map((job, index) => {
        const shouldMark = retryable.some((entry) => entry.index === index);
        return shouldMark ? { ...job, retried: true } : job;
      });
      jobsPanelOpen = true;

      try {
        for (const { job } of retryable) {
          const retryOp = cloneQueuedOpForRetry(job.retryQueuedOp!);
          await retryOp.execute(undefined, retryOp);
        }
      } finally {
        failedQueueRetryRunning = false;
      }

      return retryable.length;
    },

    dismissJob(jobId: string) { jobs = jobs.filter(j => j.id !== jobId); },
    clearDoneJobs() { jobs = jobs.filter(j => !j.done); },

    get progress() { return null; },
    clearProgress() {},

    waitForJob(jobId: string): Promise<boolean> {
      return new Promise(resolve => {
        const job = jobs.find(j => j.id === jobId);
        if (job?.done) { resolve(job.success); return; }
        const list = jobWaiters.get(jobId) ?? [];
        list.push(resolve);
        jobWaiters.set(jobId, list);
      });
    },

    // ── External tools / capabilities ─────────────────────────
    get toolStatus() { return toolStatus; },
    setToolStatus(status: ToolStatusDto[]) { toolStatus = status; },
    /** CHD convert/restore available (needs chdman). */
    get canChd() { return capabilitySet.has("chd"); },
    /** .7z/.rar extraction available (needs 7-Zip). */
    get canArchives() { return capabilitySet.has("archives"); },
    /** DolphinTool RVZ engine available (nod is always available regardless). */
    get hasDolphinTool() { return capabilitySet.has("rvzDolphin"); },
    /** Tools that were probed and are not usable — drives the startup screen. */
    get toolProblems() { return toolStatus.filter(ts => !ts.runtime.available); },

    // ── Operation queue ───────────────────────────────────────
    get queueMode()      { return queueMode; },
    get opQueue()        { return opQueue; },
    get queueConflicts() { return queueConflicts; },
    get queuePlan()      { return queuePlan; },
    get queueMaxConcurrent() { return queueMaxConcurrent; },
    get queueRunning()   { return queueRunning; },
    get queueRunStats()  { return queueRunStats; },
    get queueHasDependencies() { return queueHasDependencies; },
    get queueAccentBlocked() { return queueAccentBlocked; },

    /**
     * All ghost outputs pending across the whole queue. Consumers that need
     * reactivity (e.g. the explorer overlay) must read `opQueue` directly in
     * their own derived — reading through a store method does not register
     * opQueue as a dependency. This getter is for one-shot, non-reactive checks.
     */
    get queueGhosts(): GhostEntry[] { return opQueue.flatMap((o) => o.produces); },

    /** Final (rename-resolved) output files for a queued op — what its files
     *  will actually be called after the queue's cross-op conflict resolution. */
    resolvedOutputsFor(opId: string): GhostEntry[] {
      return resolvedOutputs.get(opId) ?? [];
    },

    /**
     * Resolves candidate output paths (in order) against everything already
     * spoken for — queued ops' outputs and real files the app currently knows
     * about — returning the paths they'll actually get after the rename policy.
     * For dialog previews so they show the real " (2)"/" (3)" names. `overwrite`
     * mirrors the dialog's replace option (no rename when replacing).
     */
    resolvePreviewNames(basePaths: string[], overwrite: boolean): string[] {
      const taken = new Set<string>();
      for (const pane of panes) {
        for (const tab of pane.tabs) {
          for (const e of tab.entries) taken.add(normalizePath(e.path));
        }
      }
      for (const outs of resolvedOutputs.values()) {
        for (const g of outs) taken.add(normalizePath(g.path));
      }
      const rename = settings.renameOnConflict;
      return basePaths.map((base) => {
        let path = base;
        if (taken.has(normalizePath(base)) && !overwrite && rename) {
          path = uniqueDisplayPath(base, taken);
        }
        taken.add(normalizePath(path));
        return path;
      });
    },

    toggleQueueMode() { queueMode = !queueMode; },

    /** Persists the launch default for queue mode and applies it immediately. */
    setDefaultQueueMode(on: boolean) {
      settings = { ...settings, defaultQueueMode: on };
      persistSettings(settings);
      queueMode = on;
    },

    addToQueue(op: QueuedOp) {
      // Link the op to any already-queued ops whose ghost outputs it consumes.
      op.dependsOn = linkDependencies(op, opQueue);
      opQueue = [...opQueue, op];
      jobsPanelOpen = true;
    },

    /**
     * Removes an op and (cascading) every op that depends on its output, since
     * those consumers would otherwise be left pointing at a ghost that will
     * never be produced. Returns the number of ops actually removed.
     */
    removeFromQueue(id: string): number {
      const doomed = collectDependents([id], opQueue);
      opQueue = opQueue.filter((o) => !doomed.has(o.id));
      return doomed.size;
    },

    clearQueue() { opQueue = []; },

    moveOpQueueItem(fromIdx: number, delta: -1 | 1) {
      const toIdx = fromIdx + delta;
      if (toIdx < 0 || toIdx >= opQueue.length) return;
      const next = [...opQueue];
      [next[fromIdx], next[toIdx]] = [next[toIdx], next[fromIdx]];
      if (!orderRespectsDeps(next)) return; // would place a consumer before its producer
      opQueue = next;
    },

    reorderOpQueue(fromIdx: number, insertIdx: number) {
      if (fromIdx === insertIdx || fromIdx + 1 === insertIdx) return;
      const next = [...opQueue];
      const [moved] = next.splice(fromIdx, 1);
      next.splice(fromIdx < insertIdx ? insertIdx - 1 : insertIdx, 0, moved);
      if (!orderRespectsDeps(next)) return; // would break a dependency ordering
      opQueue = next;
    },

    async executeQueue(mode: QueueExecutionMode) {
      const ops = [...opQueue];
      if (ops.length === 0) return;
      const effectiveMode: QueueExecutionMode = mode === "parallel" ? "smart" : mode;
      const maxConcurrent = clampQueueMaxConcurrent(settings.queueMaxConcurrent);
      const plan = buildQueuePlan(ops, detectConflicts(ops), maxConcurrent);
      if (plan.blockingConflicts.length > 0) return;
      // Refuse to run while any op is blocked by accented file names (integrity guard).
      if (ops.some((o) => isAccentUnsafeKind(o.kind) && collectAccentIssues(o.sources).length > 0)) return;

      opQueue = [];
      queueRunning = true;
      const preExistingIds = new Set(jobs.map(j => j.id));
      queueRunStats = {
        total: ops.length,
        completed: 0,
        succeeded: 0,
        failed: 0,
        skipped: 0,
        running: 0,
        mode: effectiveMode,
      };

      const terminal = new Set<string>();
      const succeeded = new Set<string>();
      const failed = new Set<string>();
      const skipped = new Set<string>();
      const running = new Set<string>();

      const updateStats = (patch: Partial<QueueRunStats>) => {
        if (!queueRunStats) return;
        queueRunStats = { ...queueRunStats, ...patch };
      };

      const markSkipped = (id: string) => {
        if (terminal.has(id) || running.has(id)) return;
        terminal.add(id);
        skipped.add(id);
        updateStats({
          completed: queueRunStats!.completed + 1,
          skipped: queueRunStats!.skipped + 1,
        });
      };

      const skipHardDependents = (id: string) => {
        for (const child of plan.hardSuccessors.get(id) ?? []) {
          if (terminal.has(child) || running.has(child)) continue;
          markSkipped(child);
          skipHardDependents(child);
        }
      };

      const runOp = async (op: QueuedOp): Promise<boolean> => {
        running.add(op.id);
        updateStats({ running: queueRunStats!.running + 1 });
        let ok = false;
        try {
          ok = await op.execute(undefined, op);
        } catch {
          ok = false;
        }
        running.delete(op.id);
        terminal.add(op.id);
        if (ok) succeeded.add(op.id);
        else {
          failed.add(op.id);
          skipHardDependents(op.id);
        }
        updateStats({
          running: queueRunStats!.running - 1,
          completed: queueRunStats!.completed + 1,
          succeeded: queueRunStats!.succeeded + (ok ? 1 : 0),
          failed: queueRunStats!.failed + (ok ? 0 : 1),
        });
        return ok;
      };

      const hardDepsSucceeded = (op: QueuedOp) =>
        [...(plan.hardPredecessors.get(op.id) ?? [])].every((dep) => succeeded.has(dep));

      const orderDepsTerminal = (op: QueuedOp) =>
        [...(plan.orderPredecessors.get(op.id) ?? [])].every((dep) => terminal.has(dep));

      const runSequential = async () => {
        for (const op of ops) {
          if (terminal.has(op.id)) continue;
          if (!hardDepsSucceeded(op)) {
            markSkipped(op.id);
            skipHardDependents(op.id);
            continue;
          }
          await runOp(op);
        }
      };

      const runSmart = async () => {
        const pending = new Set(ops.map((op) => op.id));
        await new Promise<void>((resolve) => {
          const pump = () => {
            if (terminal.size >= ops.length) {
              resolve();
              return;
            }

            let launched = false;
            while (running.size < maxConcurrent) {
              const ready = ops.find((op) =>
                pending.has(op.id) && hardDepsSucceeded(op) && orderDepsTerminal(op)
              );
              if (!ready) break;
              pending.delete(ready.id);
              launched = true;
              void runOp(ready).then(pump);
            }

            if (!launched && running.size === 0 && pending.size > 0) {
              for (const id of [...pending]) {
                pending.delete(id);
                markSkipped(id);
              }
              resolve();
            }
          };
          pump();
        });
      };

      try {
        if (effectiveMode === "sequential") await runSequential();
        else await runSmart();
      } finally {
        queueRunning = false;
        const queueJobs = jobs.filter(j => !preExistingIds.has(j.id) && j.done);
        for (const j of queueJobs.filter(j => j.success)) {
          const id = j.id;
          setTimeout(() => { jobs = jobs.filter(x => x.id !== id); }, 5000);
        }
        queueRunStats = {
          total: ops.length,
          completed: terminal.size,
          succeeded: succeeded.size,
          failed: failed.size,
          skipped: skipped.size,
          running: 0,
          mode: effectiveMode,
        };
        setTimeout(() => { queueRunStats = null; }, 8000);
      }
    },

    // ── Notifications ─────────────────────────────────────────
    get notifications() { return notifications; },
    notify(kind: NotificationKind, text: string) {
      const n = makeNotification(kind, text);
      notifications = [n, ...notifications].slice(0, 8);
      if (kind !== "error" && kind !== "warn") {
        setTimeout(() => { notifications = notifications.filter(x => x.id !== n.id); }, 4000);
      }
    },
    dismissNotification(id: string) {
      notifications = notifications.filter(x => x.id !== id);
    },

    // ── Terminal panel ────────────────────────────────────────
    get terminalPanelOpen()   { return terminalPanelOpen; },
    get terminalPanelHeight() { return terminalPanelHeight; },
    get terminalTabs()        { return terminalTabs; },
    get activeTerminalId()    { return activeTerminalId; },

    openTerminalPanel()    { terminalPanelOpen = true; },
    closeTerminalPanel()   { terminalPanelOpen = false; },
    toggleTerminalPanel()  { terminalPanelOpen = !terminalPanelOpen; },
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

applyTheme(app.settings.theme);
document.documentElement.style.setProperty("--app-font-scale", String(app.settings.fontScale));
document.documentElement.style.setProperty("--tree-scale", String(app.settings.defaultTreeZoom));
