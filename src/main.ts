import { invoke } from "@tauri-apps/api/core";
import { emit, listen } from "@tauri-apps/api/event";
import { WebviewWindow } from "@tauri-apps/api/webviewWindow";
import { open } from "@tauri-apps/plugin-dialog";

import "./styles.css";

type EntryDto = {
  path: string;
  name: string;
  isDir: boolean;
  size: number;
  sizeLabel: string;
  modifiedTs: number;
  modifiedLabel: string;
  extension: string;
  hasChildren: boolean;
  hasDirectoryChildren: boolean;
  locationKind: string;
  displayPath: string;
  rootLabel: string | null;
};

type ConnectionProfileDto = {
  id: string;
  label: string;
  protocol: "smb" | "ssh" | "ftp" | "ftps";
  host: string;
  port: number;
  username: string;
  password: string;
  share: string;
  workgroup: string;
  startPath: string;
  sshMode: "sftp" | "scp";
  ftpMode: "passive" | "active";
  ftpSecureImplicit: boolean;
  ftpAcceptInvalidCertificates: boolean;
  ftpAcceptInvalidHostnames: boolean;
  trustedFingerprints: string[];
};

type ConnectionProfilePayload = {
  id: string | null;
  label: string;
  protocol: "smb" | "ssh" | "ftp" | "ftps";
  host: string;
  port: number | null;
  username: string;
  password: string;
  share: string;
  workgroup: string;
  startPath: string;
  sshMode: "sftp" | "scp";
  ftpMode: "passive" | "active";
  ftpSecureImplicit: boolean;
  ftpAcceptInvalidCertificates: boolean;
  ftpAcceptInvalidHostnames: boolean;
};

type ActiveConnectionDto = {
  sessionId: string;
  profileId: string;
  label: string;
  protocol: "smb" | "ssh" | "ftp" | "ftps";
  host: string;
  rootPath: string;
  displayPath: string;
  detail: string;
};

type ConnectionOpenResultDto = {
  connected: boolean;
  requiresTrust: boolean;
  fingerprint: string | null;
  message: string | null;
  connection: ActiveConnectionDto | null;
};

type PropertiesSummaryDto = {
  count: number;
  files: number;
  directories: number;
  totalSize: number;
  totalSizeLabel: string;
  lines: string[];
};

type SummaryOptionsPayload = {
  maxDepth: number | null;
};

type ExtractionOptionsPayload = {
  individualFolders: boolean;
  destinationMode: "same" | "custom";
  destinationPath: string | null;
  deleteArchives: boolean;
  overwrite: boolean;
};

type ExtractionPreviewRow = {
  archivePath: string;
  destinationPath: string;
};

type ChdSourceDto = {
  sourcePath: string;
  containerDir: string;
  command: string;
  displayExtensions: string[];
  requiredPaths: string[];
};

type SelectionAnalysisDto = {
  archives: string[];
  chdSources: ChdSourceDto[];
  restorableChds: string[];
  hasDirectories: boolean;
  hasFiles: boolean;
  uniqueExtensions: string[];
  chdMenuLabel: string | null;
  chdRestoreMenuLabel: string | null;
};

type ChdConversionOptionsPayload = {
  deleteOriginals: boolean;
  nameAsContainer: boolean;
  depositToParent: boolean;
  deleteOriginalSubfolders: boolean;
  overwrite: boolean;
};

type ChdRestoreOptionsPayload = {
  individualFolders: boolean;
  destinationMode: "same" | "custom";
  destinationPath: string | null;
  deleteChd: boolean;
  overwrite: boolean;
  splitBin: boolean;
};

type AppMetadataDto = {
  appVersion: string;
  chdman: Record<string, unknown>;
};

type JobProgressDto = {
  jobId: string;
  progress: number;
  message: string;
};

type JobLogDto = {
  jobId: string;
  line: string;
};

type JobFinishedDto = {
  jobId: string;
  success: boolean;
  message: string;
};

type ClipboardState = {
  paths: string[];
  operation: "copy" | "cut";
};

type ContextAction = {
  id: string;
  label: string;
  icon: string;
  disabled?: boolean;
  danger?: boolean;
};

type ContextMenuState = {
  x: number;
  y: number;
  actions: ContextAction[];
  paths: string[];
  destinationPath: string | null;
};

type ProgressState = {
  jobId: string;
  title: string;
  progress: number;
  message: string;
  logs: string[];
  done: boolean;
  success: boolean;
  resultMessage: string;
  showDialog?: boolean;
  statusMessageOnSuccess?: string | null;
  statusMessageOnFailure?: string | null;
};

type ActivateLocationEvent = {
  rootPath: string;
};

type NotificationKind = "error" | "info";

type NotificationEntry = {
  id: string;
  kind: NotificationKind;
  text: string;
  createdAt: number;
};

type ExtractionDialogState = {
  paths: string[];
  options: ExtractionOptionsPayload;
  previewRows: ExtractionPreviewRow[];
};

type ChdDialogState = {
  paths: string[];
  analysis: SelectionAnalysisDto;
  options: ChdConversionOptionsPayload;
};

type ChdRestoreDialogState = {
  paths: string[];
  analysis: SelectionAnalysisDto;
  options: ChdRestoreOptionsPayload;
};

type RenameState = {
  path: string;
  value: string;
};

type ViewMode = "list" | "grid";
type ThemeMode = "light" | "dark";
type ContentSortKey = "name" | "type" | "size" | "modified";
type SortDirection = "asc" | "desc";
type ContentColumnWidths = Record<ContentSortKey, number>;

type AppSettings = {
  theme: ThemeMode;
  fontScale: number;
  compactUi: boolean;
  defaultViewMode: ViewMode;
  defaultContentZoom: number;
  defaultTreeZoom: number;
  localLocations: string[];
  confirmDelete: boolean;
  hideDeleteProgressPopup: boolean;
  selectionWeightMaxDepth: number;
  selectionWeightCalculateAll: boolean;
  contentSortKey: ContentSortKey;
  contentSortDirection: SortDirection;
  contentColumnWidths: ContentColumnWidths;
};

type AppState = {
  localLocations: string[];
  rootPath: string | null;
  currentPath: string | null;
  pathInput: string;
  currentEntries: EntryDto[];
  selectedContentPaths: string[];
  selectedTreePath: string | null;
  treeChildren: Map<string, EntryDto[]>;
  expandedTreePaths: Set<string>;
  anchorPath: string | null;
  clipboard: ClipboardState | null;
  silentBusyCount: number;
  silentBusyLabel: string;
  topSearchQuery: string;
  localSearchOpen: boolean;
  localSearchQuery: string;
  localSearchRecursive: boolean;
  mode: "browse" | "global-search" | "local-search";
  modeLabel: string;
  contextMenu: ContextMenuState | null;
  progress: ProgressState | null;
  propertiesDialog: PropertiesSummaryDto | null;
  extractionDialog: ExtractionDialogState | null;
  chdDialog: ChdDialogState | null;
  chdRestoreDialog: ChdRestoreDialogState | null;
  metadata: AppMetadataDto | null;
  aboutOpen: boolean;
  menuOpen: string | null;
  notice: { kind: "error" | "info"; text: string } | null;
  typeSelectBuffer: string;
  typeSelectTimestamp: number;
  viewMode: ViewMode;
  zoom: number;
  treeZoom: number;
  treeWidth: number;
  treeScrollTop: number;
  contentScrollTop: number;
  contentScrollLeft: number;
  selectionSummaryLabel: string | null;
  selectionSummaryToken: number;
  rename: RenameState | null;
  settings: AppSettings;
  settingsOpen: boolean;
  statusFlash: { kind: "info" | "error"; text: string } | null;
  notifications: NotificationEntry[];
  notificationsOpen: boolean;
  activeNotificationId: string | null;
  expandedNotificationIds: Set<string>;
  connectionProfiles: ConnectionProfileDto[];
  activeConnections: ActiveConnectionDto[];
  editingConnectionProfileId: string | null;
  connectionDraft: ConnectionProfilePayload;
};

const maybeAppRoot = document.querySelector<HTMLDivElement>("#app");
if (!maybeAppRoot) {
  throw new Error("No se encontro el contenedor principal.");
}
const appRoot = maybeAppRoot;
const STORAGE_KEY = "dogu.settings";
const CONNECTIONS_WINDOW_HASH = "#connections-manager";
const CONNECTIONS_WINDOW_LABEL = "connections-manager";
const MAIN_WINDOW_LABEL = "main";
const CONNECTIONS_CHANGED_EVENT = "connections-changed";
const ACTIVATE_LOCATION_EVENT = "activate-location";
const ACTIVE_NOTIFICATION_DURATION_MS = 5000;
const DEFAULT_CONTENT_COLUMN_WIDTHS: ContentColumnWidths = {
  name: 320,
  type: 140,
  size: 120,
  modified: 148,
};
const CONTENT_COLUMN_MIN_WIDTHS: ContentColumnWidths = {
  name: 180,
  type: 96,
  size: 92,
  modified: 124,
};
const defaultSettings: AppSettings = {
  theme: "light",
  fontScale: 100,
  compactUi: true,
  defaultViewMode: "list",
  defaultContentZoom: 50,
  defaultTreeZoom: 50,
  localLocations: [],
  confirmDelete: true,
  hideDeleteProgressPopup: false,
  selectionWeightMaxDepth: 1,
  selectionWeightCalculateAll: false,
  contentSortKey: "name",
  contentSortDirection: "asc",
  contentColumnWidths: { ...DEFAULT_CONTENT_COLUMN_WIDTHS },
};

function createEmptyConnectionDraft(): ConnectionProfilePayload {
  return {
    id: null,
    label: "",
    protocol: "smb",
    host: "",
    port: 445,
    username: "",
    password: "",
    share: "",
    workgroup: "",
    startPath: "/",
    sshMode: "sftp",
    ftpMode: "passive",
    ftpSecureImplicit: false,
    ftpAcceptInvalidCertificates: false,
    ftpAcceptInvalidHostnames: false,
  };
}
function cloneDefaultSettings(): AppSettings {
  return {
    ...defaultSettings,
    contentColumnWidths: { ...DEFAULT_CONTENT_COLUMN_WIDTHS },
  };
}

const initialSettings = loadSettings();
let draggingContentPaths: string[] = [];
let activeDropTargetElement: HTMLElement | null = null;
let activeDragGhost: HTMLDivElement | null = null;
let suppressContentClick = false;
let pendingRenameCommitTimer: number | null = null;
let tooltipSystemBound = false;
let tooltipElement: HTMLDivElement | null = null;
let tooltipShowTimer: number | null = null;
let tooltipPendingTarget: HTMLElement | null = null;
let tooltipPendingText: string | null = null;
let tooltipPointerX = 0;
let tooltipPointerY = 0;
let activeNotificationTimeout: number | null = null;
let activeNotificationStartedAt = 0;
let activeNotificationRemainingMs = 0;
let activeNotificationPaused = false;

const state: AppState = {
  localLocations: [...initialSettings.localLocations],
  rootPath: null,
  currentPath: null,
  pathInput: "",
  currentEntries: [],
  selectedContentPaths: [],
  selectedTreePath: null,
  treeChildren: new Map<string, EntryDto[]>(),
  expandedTreePaths: new Set<string>(),
  anchorPath: null,
  clipboard: null,
  silentBusyCount: 0,
  silentBusyLabel: "",
  topSearchQuery: "",
  localSearchOpen: false,
  localSearchQuery: "",
  localSearchRecursive: true,
  mode: "browse",
  modeLabel: "",
  contextMenu: null,
  progress: null,
  propertiesDialog: null,
  extractionDialog: null,
  chdDialog: null,
  chdRestoreDialog: null,
  metadata: null,
  aboutOpen: false,
  menuOpen: null,
  notice: null,
  statusFlash: null,
  notifications: [],
  notificationsOpen: false,
  activeNotificationId: null,
  expandedNotificationIds: new Set<string>(),
  typeSelectBuffer: "",
  typeSelectTimestamp: 0,
  settings: initialSettings,
  settingsOpen: false,
  viewMode: initialSettings.defaultViewMode,
  zoom: initialSettings.defaultContentZoom,
  treeZoom: initialSettings.defaultTreeZoom,
  treeWidth: 320,
  treeScrollTop: 0,
  contentScrollTop: 0,
  contentScrollLeft: 0,
  selectionSummaryLabel: null,
  selectionSummaryToken: 0,
  rename: null,
  connectionProfiles: [],
  activeConnections: [],
  editingConnectionProfileId: null,
  connectionDraft: createEmptyConnectionDraft(),
};

void bootstrap();

async function bootstrap(): Promise<void> {
  await Promise.all([
    listen<JobProgressDto>("job-progress", (event) => {
      if (!state.progress || state.progress.jobId !== event.payload.jobId) {
        return;
      }
      state.progress.progress = event.payload.progress;
      state.progress.message = event.payload.message;
      render();
    }),
    listen<JobLogDto>("job-log", (event) => {
      if (!state.progress || state.progress.jobId !== event.payload.jobId) {
        return;
      }
      state.progress.logs = [...state.progress.logs, event.payload.line];
      render();
    }),
    listen<JobFinishedDto>("job-finished", async (event) => {
      if (!state.progress || state.progress.jobId !== event.payload.jobId) {
        return;
      }
      state.progress.done = true;
      state.progress.success = event.payload.success;
      state.progress.resultMessage = event.payload.message;
      if (event.payload.success) {
        await refreshVisibleData();
      }
      const finishedProgress = state.progress;
      if (finishedProgress.showDialog === false) {
        if (event.payload.success) {
          showStatusFlash(finishedProgress.statusMessageOnSuccess ?? event.payload.message, "info");
        } else {
          showStatusFlash(finishedProgress.statusMessageOnFailure ?? event.payload.message, "error");
        }
        state.progress = null;
      }
      render();
    }),
    listen(CONNECTIONS_CHANGED_EVENT, async () => {
      await Promise.all([loadConnectionProfiles(), loadActiveConnections()]);
      render();
    }),
    listen<ActivateLocationEvent>(ACTIVATE_LOCATION_EVENT, async (event) => {
      if (isConnectionsManagerWindow()) {
        return;
      }
      const rootPath = event.payload?.rootPath?.trim();
      if (!rootPath) {
        return;
      }
      await setRootAndNavigate(rootPath);
      render();
    }),
  ]);

  initTooltipSystem();

  document.addEventListener("click", () => {
    if (state.contextMenu || state.menuOpen || state.notificationsOpen) {
      state.contextMenu = null;
      state.menuOpen = null;
      state.notificationsOpen = false;
      render();
    }
  });
  window.addEventListener(
    "pointerdown",
    (event) => {
      const activeRenameInput = getActiveRenameInput();
      if (!state.rename || !activeRenameInput || isTargetInsideRenameInput(event.target, activeRenameInput)) {
        return;
      }
      scheduleRenameCommit(state.rename.path, activeRenameInput.value);
    },
    true,
  );
  document.addEventListener("keydown", handleGlobalKeyDown);

  try {
    state.metadata = await invoke<AppMetadataDto>("get_app_metadata");
  } catch (error) {
    showNotice("error", formatError(error));
  }

  try {
    await Promise.all([loadConnectionProfiles(), loadActiveConnections()]);
  } catch (error) {
    showNotice("error", formatError(error));
  }

  applyVisualSettings();
  render();
}

function render(): void {
  if (isConnectionsManagerWindow()) {
    document.title = "Conexiones remotas | Dogu";
    appRoot.innerHTML = renderConnectionsManagerWindow();
    bindUi();
    return;
  }

  document.title = "Dogu";
  appRoot.innerHTML = `
    <div class="shell">
      <nav class="menu-bar">
        ${renderMenu("Archivo", [
          ["select-root", "folder-plus", "Seleccionar carpeta base"],
          ["go-up", "up", "Subir un nivel"],
          ["refresh-current", "refresh", "Refrescar vista"],
          ["create-folder", "folder-plus", "Crear carpeta"],
          ["create-file", "file-plus", "Crear fichero"],
          ["open-settings", "settings", "Opciones"],
        ])}
        ${renderMenu("Herramientas", [
          ["open-local-search", "filter", "Busqueda local (Ctrl+F)"],
          ["paste-current", "paste", "Pegar en carpeta actual"],
        ])}
        ${renderMenu("Ayuda", [["about", "info", "About"]])}
      </nav>

      <header class="titlebar">
        <div class="titlebox">
          <div class="titlemark">${renderIcon("app")}</div>
          <div class="titlecopy">
            <div class="apptitle">Dogu</div>
            <div class="subtitle">Explorador, extraccion y conversiones para colecciones ROM</div>
          </div>
        </div>
        <div class="searchbox">
          <div class="searchfield">
            <button id="top-search-run" class="search-inline-button" ${renderTooltipAttr("Buscar")}>${renderIcon("search")}</button>
            <input id="top-search-input" type="text" placeholder="Buscar en todo el arbol..." value="${escapeAttr(
              state.topSearchQuery,
            )}" />
            <button id="top-search-clear" class="search-inline-button search-clear-button${
              state.topSearchQuery.trim() ? " visible" : ""
            }" ${renderTooltipAttr("Limpiar")}${state.topSearchQuery.trim() ? "" : " disabled"}>${renderIcon("x")}</button>
          </div>
        </div>
      </header>

      <section class="pathbar">
        <div class="pathbar-actions">
          <button id="go-up" class="toolbar-button"${!state.currentPath ? " disabled" : ""}>${renderIcon("up")}Subir</button>
          <button
            id="refresh-current"
            class="toolbar-button compact icon-only"
            ${renderTooltipAttr("Actualizar carpeta actual")}
            ${!state.currentPath ? " disabled" : ""}
          >
            ${renderIcon("refresh")}
          </button>
        </div>
        <div class="pathbar-route">
          ${renderActiveLocationBadge()}
          <div class="path-input-wrap route-input-wrap">
            <span class="field-icon">${renderIcon("route")}</span>
            <input id="path-input" type="text" placeholder="Ruta absoluta..." value="${escapeAttr(state.pathInput)}" />
          </div>
          <button id="path-go" class="toolbar-button primary"${!state.pathInput.trim() ? " disabled" : ""}>${renderIcon(
            "arrow-right",
          )}Ir</button>
        </div>
      </section>

      ${
        state.localSearchOpen
          ? `
            <section class="local-search-row">
              <div class="path-input-wrap">
                <span class="field-icon">${renderIcon("filter")}</span>
                <input id="local-search-input" type="search" placeholder="Buscar dentro de la carpeta actual..." value="${escapeAttr(
                  state.localSearchQuery,
                )}" />
              </div>
              <label class="checkbox-line toolbar-check">
                <input id="local-search-recursive" type="checkbox"${
                  state.localSearchRecursive ? " checked" : ""
                } />
                <span>Recursiva</span>
              </label>
              <button id="local-search-run" class="toolbar-button primary">${renderIcon("search")}Buscar</button>
              <button id="local-search-close" class="toolbar-button">${renderIcon("x")}Cerrar</button>
            </section>
          `
          : ""
      }

      <main class="workspace" style="grid-template-columns:${state.treeWidth}px 6px minmax(0, 1fr);">
        <aside class="tree-panel">
          ${renderLocationStrip()}
          <div class="section-head">
            <strong>${renderIcon("tree")}Arbol</strong>
            <label class="zoom-line panel-zoom-line tree-zoom-line">
              <span>${renderIcon("zoom")}</span>
              <input id="tree-zoom-range" type="range" min="1" max="100" step="1" value="${state.treeZoom}" />
              <span id="tree-zoom-value" class="zoom-bubble">${state.treeZoom}%</span>
            </label>
          </div>
          <div class="tree-scroll">${renderFolderTree()}</div>
        </aside>

        <div id="splitter" class="splitter" aria-hidden="true"></div>

        <section class="content-panel">
          <div class="section-head">
            <strong>${renderIcon(state.viewMode === "list" ? "list" : "grid")}Contenido</strong>
            <div class="section-tools">
              ${renderContentQuickActions()}
              <span class="toolbar-separator" aria-hidden="true"></span>
              <div class="view-toggle">
                <button id="view-list" class="toolbar-button compact${
                  state.viewMode === "list" ? " active" : ""
                }" ${renderTooltipAttr("Vista lista")}>${renderIcon("list")}</button>
                <button id="view-grid" class="toolbar-button compact${
                  state.viewMode === "grid" ? " active" : ""
                }" ${renderTooltipAttr("Vista rejilla")}>${renderIcon("grid")}</button>
              </div>
              <label class="zoom-line panel-zoom-line">
                <span>${renderIcon("zoom")}</span>
                <input id="zoom-range" type="range" min="1" max="100" step="1" value="${state.zoom}" />
                <span id="content-zoom-value" class="zoom-bubble">${state.zoom}%</span>
              </label>
            </div>
          </div>
          ${state.viewMode === "list" ? renderListView() : renderGridView()}
        </section>
      </main>

      <footer class="statusbar">
        <div class="status-main">
          <div class="status-chips">${renderStatusChips()}</div>
          <div class="status-flash-slot"></div>
        </div>
        <div class="status-side">
          ${renderNotificationsIndicator()}
          ${renderClipboardIndicator()}
          ${renderActivityIndicator()}
          ${renderNotificationsDropdown()}
        </div>
      </footer>

      ${renderNotificationToast()}
      ${state.contextMenu ? renderContextMenu() : ""}
      ${state.progress && state.progress.showDialog !== false ? renderProgressDialog() : ""}
      ${state.propertiesDialog ? renderPropertiesDialog() : ""}
      ${state.extractionDialog ? renderExtractionDialog() : ""}
      ${state.chdDialog ? renderChdDialog() : ""}
      ${state.chdRestoreDialog ? renderChdRestoreDialog() : ""}
      ${state.settingsOpen ? renderSettingsDialog() : ""}
      ${state.aboutOpen ? renderAboutDialog() : ""}
    </div>
  `;

  bindUi();
  window.requestAnimationFrame(() => {
    restoreScrollState();
  });
}

function isConnectionsManagerWindow(): boolean {
  return window.location.hash === CONNECTIONS_WINDOW_HASH;
}

function getConnectionsWindowUrl(): string {
  const url = new URL(window.location.href);
  url.hash = CONNECTIONS_WINDOW_HASH.slice(1);
  return url.toString();
}

async function openConnectionsManagerWindow(): Promise<void> {
  try {
    if (isConnectionsManagerWindow()) {
      return;
    }
    const existing = await WebviewWindow.getByLabel(CONNECTIONS_WINDOW_LABEL);
    if (existing) {
      await existing.show();
      await existing.unminimize();
      await existing.setFocus();
      return;
    }

    const connectionsWindow = new WebviewWindow(CONNECTIONS_WINDOW_LABEL, {
      url: getConnectionsWindowUrl(),
      title: "Conexiones remotas",
      width: 1100,
      height: 760,
      minWidth: 920,
      minHeight: 680,
      center: true,
      resizable: true,
    });
    void connectionsWindow.once("tauri://created", async () => {
      await connectionsWindow.setFocus();
    });
    void connectionsWindow.once("tauri://error", (event) => {
      reportConnectionsWindowOpenError(event.payload);
    });
  } catch (error) {
    reportConnectionsWindowOpenError(error);
  }
}

async function focusMainWindow(): Promise<void> {
  const mainWindow = await WebviewWindow.getByLabel(MAIN_WINDOW_LABEL);
  if (!mainWindow) {
    return;
  }
  await mainWindow.show();
  await mainWindow.unminimize();
  await mainWindow.setFocus();
}

function reportConnectionsWindowOpenError(error: unknown): void {
  const message = `No se pudo abrir el gestor remoto: ${formatError(error)}`;
  showNotice("error", message);
}

function renderMenu(label: string, items: Array<[string, string, string]>): string {
  const openMenu = state.menuOpen === label;
  return `
    <div class="menu-group">
      <button class="menu-trigger${openMenu ? " open" : ""}" data-menu-trigger="${escapeAttr(label)}">${escapeHtml(
        label,
      )}</button>
      ${
        openMenu
          ? `
            <div class="menu-dropdown">
              ${items
                .map(
                  ([id, icon, text]) => `
                    <button class="menu-item" data-menu-action="${escapeAttr(id)}">
                      <span class="menu-item-icon">${renderIcon(icon)}</span>
                      <span>${escapeHtml(text)}</span>
                    </button>
                  `,
                )
                .join("")}
            </div>
          `
          : ""
      }
    </div>
  `;
}

function renderActiveLocationBadge(): string {
  const location = getActiveLocationSummary();
  if (!location) {
    return "";
  }
  return `
    <div class="location-badge" ${renderTooltipAttr(location.tooltip)}>
      ${renderIcon(location.icon)}
      <span>${escapeHtml(location.label)}</span>
    </div>
  `;
}

function renderLocationStrip(): string {
  const hasLocal = state.localLocations.length > 0;
  const hasRemote = state.activeConnections.length > 0;
  return `
      <div class="location-strip">
      <div class="location-strip-head">
        <div class="location-strip-title">
          <strong>${renderIcon("locations")}Ubicaciones</strong>
        </div>
      </div>
      <div class="location-group">
        <div class="location-group-head">
          <div class="location-group-label">Local</div>
          <button id="add-local-root-inline" class="toolbar-button compact icon-only" ${renderTooltipAttr("Agregar ubicacion local")}>
            ${renderIcon("plus")}
          </button>
        </div>
        <div class="location-list">
          ${
            hasLocal
              ? state.localLocations
                  .map(
                    (locationPath) => `
                      <button
                        class="location-item location-item-compact${state.rootPath === locationPath ? " active" : ""}"
                        data-location-root="${escapeAttr(locationPath)}"
                        ${renderTooltipAttr(locationPath)}
                      >
                        <span class="location-item-mark local">${renderIcon("folder")}</span>
                        <span class="location-item-copy">
                          <strong>${escapeHtml(leafName(locationPath) || locationPath)}</strong>
                        </span>
                        <span class="location-item-tail">${renderIcon("arrow-right")}</span>
                      </button>
                    `,
                  )
                  .join("")
              : `
                <button id="pick-local-root-inline" class="location-item ghost">
                  <span class="location-item-mark local">${renderIcon("folder-plus")}</span>
                  <span class="location-item-copy">
                    <strong>Elegir base local</strong>
                    <small>Selecciona una carpeta para explorarla</small>
                  </span>
                </button>
              `
          }
        </div>
      </div>
      <div class="location-group">
        <div class="location-group-head">
          <div class="location-group-label">Remotas</div>
          <button id="open-connections-inline" class="toolbar-button compact icon-only" ${renderTooltipAttr("Gestionar conexiones remotas")}>
            ${renderIcon("settings")}
          </button>
        </div>
        <div class="location-list">
          ${
            hasRemote
              ? state.activeConnections
                  .map(
                    (connection) => `
                      <button
                        class="location-item${state.rootPath === connection.rootPath ? " active" : ""}"
                        data-location-root="${escapeAttr(connection.rootPath)}"
                        ${renderTooltipAttr(connection.detail)}
                      >
                        <span class="location-item-mark remote">${renderIcon(protocolIcon(connection.protocol))}</span>
                        <span class="location-item-copy">
                          <strong>${escapeHtml(connection.label)}</strong>
                          <small>${escapeHtml(connection.detail)}</small>
                        </span>
                        <span class="connection-state-dot active location-inline-dot" ${renderTooltipAttr("Sesion activa")}></span>
                      </button>
                    `,
                  )
                  .join("")
              : `
                <div class="location-empty">
                  ${renderIcon("network")}
                  <span>No hay conexiones remotas activas.</span>
                </div>
              `
          }
        </div>
      </div>
    </div>
  `;
}

function renderConnectionsManagerWindow(): string {
  return `
    <div class="connections-window-shell">
      <header class="connections-window-head">
        <div class="connections-modal-copy">
          <h3>Conexiones remotas</h3>
          <div class="help-line">Perfiles reutilizables para SMB, SSH, FTP, SFTP y FTPS.</div>
          <div class="connections-summary-row">
            <span class="section-pill">${state.connectionProfiles.length} perfiles</span>
            <span class="section-pill accent">${state.activeConnections.length} sesiones activas</span>
            <span class="section-pill">${state.connectionDraft.protocol.toUpperCase()}</span>
          </div>
        </div>
        <button id="connections-close" class="toolbar-button compact icon-only" ${renderTooltipAttr("Cerrar")}>${renderIcon("x")}</button>
      </header>
      <div class="connections-window-body">${renderConnectionsDialog()}</div>
      <footer class="statusbar connections-statusbar">
        <div class="status-main">
          <div class="status-chips">${renderConnectionsStatusText()}</div>
          <div class="status-flash-slot"></div>
        </div>
        <div class="status-side">
          ${renderNotificationsIndicator()}
          ${renderActivityIndicator()}
          ${renderNotificationsDropdown()}
        </div>
      </footer>
    </div>
  `;
}

function renderConnectionsStatusText(): string {
  const notification = getActiveNotification() ?? state.notifications[0] ?? null;
  if (!notification) {
    return "";
  }
  return `
    <span class="status-chip plain connections-status-text ${notification.kind}">
      <span>${escapeHtml(notification.text)}</span>
    </span>
  `;
}

function renderConnectionsDialog(): string {
  const draft = state.connectionDraft;
  const selectedProfileId = state.editingConnectionProfileId;
  return `
    <div class="connections-layout">
      <section class="connections-sidebar">
        <div class="connections-pane">
          <div class="connections-section-head">
            <strong>Perfiles</strong>
            <button id="connection-new" class="toolbar-button compact icon-only" ${renderTooltipAttr("Nuevo perfil")}>${renderIcon("plus")}</button>
          </div>
          <div class="connections-profile-list">
            ${
              state.connectionProfiles.length
                ? state.connectionProfiles
                    .map((profile) => {
                      const connected = state.activeConnections.some((item) => item.profileId === profile.id);
                      return `
                        <div class="connection-profile-item${selectedProfileId === profile.id ? " active" : ""}">
                          <button class="connection-profile-main-button" data-connection-profile="${escapeAttr(profile.id)}">
                            <span class="connection-profile-main">
                              <span class="location-item-mark remote">${renderIcon(protocolIcon(profile.protocol))}</span>
                              <span class="connection-profile-copy">
                                <strong>${escapeHtml(profile.label)}</strong>
                                <small>${escapeHtml(renderProfileShortLabel(profile))}</small>
                              </span>
                            </span>
                          </button>
                          <span class="connection-row-actions">
                            <button
                              class="toolbar-button compact icon-only"
                              data-profile-test="${escapeAttr(profile.id)}"
                              ${renderTooltipAttr("Probar conexion")}
                            >
                              ${renderIcon("pulse")}
                            </button>
                            <button
                              class="toolbar-button compact icon-only"
                              data-profile-connect="${escapeAttr(profile.id)}"
                              ${renderTooltipAttr(connected ? "Ya conectado" : "Conectar")}
                              ${connected ? "disabled" : ""}
                            >
                              ${renderIcon("plug")}
                            </button>
                          </span>
                        </div>
                      `;
                    })
                    .join("")
                : `<div class="location-empty">No hay perfiles guardados.</div>`
            }
          </div>
        </div>
        <div class="connections-pane">
          <div class="connections-section-head">
            <strong>Sesiones activas</strong>
            <span class="section-pill accent">${state.activeConnections.length}</span>
          </div>
          <div class="connections-session-list">
            ${
              state.activeConnections.length
                ? state.activeConnections
                    .map(
                      (connection) => `
                        <div class="connection-session-card">
                          <button class="connection-session-main" data-session-root="${escapeAttr(connection.rootPath)}">
                            <span class="location-item-mark remote">${renderIcon(protocolIcon(connection.protocol))}</span>
                            <span class="connection-profile-copy">
                              <strong>${escapeHtml(connection.label)}</strong>
                              <small>${escapeHtml(connection.detail)}</small>
                            </span>
                          </button>
                          <button
                            class="toolbar-button compact icon-only"
                            data-disconnect-session="${escapeAttr(connection.sessionId)}"
                            ${renderTooltipAttr("Desconectar")}
                          >
                            ${renderIcon("plug-off")}
                          </button>
                        </div>
                      `,
                    )
                    .join("")
                : `<div class="location-empty">Ninguna sesion abierta.</div>`
            }
          </div>
        </div>
      </section>

      <section class="connections-editor">
        <div class="connection-card-section connection-form-section">
          <div class="connection-form-head">
            <div class="connections-section-head">
              <strong>Perfil</strong>
              ${draft.label.trim() ? `<span class="section-pill">${escapeHtml(draft.label.trim())}</span>` : ""}
            </div>
            <div class="dialog-actions compact-actions">
              ${state.editingConnectionProfileId ? `<button id="connection-delete" class="toolbar-button danger">${renderIcon("trash")}Eliminar</button>` : ""}
              <button id="connection-test" class="toolbar-button">${renderIcon("pulse")}Probar</button>
              <button id="connection-save" class="toolbar-button primary">${renderIcon("save")}Guardar</button>
              ${state.editingConnectionProfileId ? `<button id="connection-connect" class="toolbar-button">${renderIcon("plug")}Conectar</button>` : ""}
            </div>
          </div>
          <div class="settings-grid connection-grid">
            <label for="connection-label">Etiqueta</label>
            <input id="connection-label" type="text" value="${escapeAttr(draft.label)}" />

            <label for="connection-protocol">Protocolo</label>
            <select id="connection-protocol">
              ${(["smb", "ssh", "ftp", "ftps"] as const)
                .map(
                  (protocol) =>
                    `<option value="${protocol}"${draft.protocol === protocol ? " selected" : ""}>${escapeHtml(
                      protocol.toUpperCase(),
                    )}</option>`,
                )
                .join("")}
            </select>

            <label for="connection-host">Host</label>
            <input id="connection-host" type="text" value="${escapeAttr(draft.host)}" placeholder="192.168.1.20" />

            <label for="connection-port">Puerto</label>
            <input id="connection-port" type="number" min="1" max="65535" value="${draft.port ?? ""}" />

            <label for="connection-username">Usuario</label>
            <input id="connection-username" type="text" value="${escapeAttr(draft.username)}" />

            <label for="connection-password">Contrasena</label>
            <input id="connection-password" type="password" value="${escapeAttr(draft.password)}" />

            <label for="connection-start-path">Ruta inicial</label>
            <input id="connection-start-path" type="text" value="${escapeAttr(draft.startPath)}" placeholder="/" />

            ${
              draft.protocol === "smb"
                ? `
                  <label for="connection-share">Recurso compartido</label>
                  <input id="connection-share" type="text" value="${escapeAttr(draft.share)}" placeholder="roms" />

                  <label for="connection-workgroup">Grupo de trabajo</label>
                  <input id="connection-workgroup" type="text" value="${escapeAttr(draft.workgroup)}" placeholder="WORKGROUP" />
                `
                : ""
            }

            ${
              draft.protocol === "ssh"
                ? `
                  <label for="connection-ssh-mode">Modo SSH</label>
                  <select id="connection-ssh-mode">
                    <option value="sftp"${draft.sshMode === "sftp" ? " selected" : ""}>SFTP</option>
                    <option value="scp"${draft.sshMode === "scp" ? " selected" : ""}>SCP</option>
                  </select>
                `
                : ""
            }

            ${
              draft.protocol === "ftp" || draft.protocol === "ftps"
                ? `
                  <label for="connection-ftp-mode">Modo de datos</label>
                  <select id="connection-ftp-mode">
                    <option value="passive"${draft.ftpMode === "passive" ? " selected" : ""}>Pasivo</option>
                    <option value="active"${draft.ftpMode === "active" ? " selected" : ""}>Activo</option>
                  </select>
                `
                : ""
            }
          </div>
        </div>

        ${
          draft.protocol === "ftps"
            ? `
              <div class="settings-section connection-card-section">
                <h4>Seguridad FTPS</h4>
                <label class="checkbox-line">
                  <input id="connection-ftps-implicit" type="checkbox"${draft.ftpSecureImplicit ? " checked" : ""} />
                  <span>Usar FTPS implicito</span>
                </label>
                <label class="checkbox-line">
                  <input id="connection-ftps-invalid-cert" type="checkbox"${
                    draft.ftpAcceptInvalidCertificates ? " checked" : ""
                  } />
                  <span>Aceptar certificados invalidos</span>
                </label>
                <label class="checkbox-line">
                  <input id="connection-ftps-invalid-host" type="checkbox"${
                    draft.ftpAcceptInvalidHostnames ? " checked" : ""
                  } />
                  <span>Aceptar nombres de host invalidos</span>
                </label>
              </div>
            `
            : ""
        }

        <div class="settings-section connection-card-section">
          <h4>Notas</h4>
          <div class="help-line">
            SMB navega recursos de red sin depender de montar rutas. SSH usa el canal de transferencia del host remoto y
            puede trabajar con equipos que solo exponen SSH. FTP y FTPS conservan sus opciones tipicas.
          </div>
        </div>
      </section>
    </div>
  `;
}

function renderTree(): string {
  if (!state.rootPath) {
    return `<div class="empty-state">Selecciona una carpeta base para cargar el arbol.</div>`;
  }
  const rootChildren = (state.treeChildren.get(state.rootPath) ?? []).filter((entry) => entry.isDir);
  return `
    <div class="tree-node${state.selectedTreePath === state.rootPath ? " selected" : ""}">
      <div class="tree-node-row">
        <button class="tree-expander" data-tree-toggle="${escapeAttr(state.rootPath)}"${
          rootChildren.length === 0 ? " disabled" : ""
        }>${state.expandedTreePaths.has(state.rootPath) ? "−" : "+"}</button>
        <button class="tree-select" data-tree-select="${escapeAttr(state.rootPath)}" ${renderTooltipAttr(
          leafName(state.rootPath) || state.rootPath,
        )}>
          <span class="icon-wrap">${renderIcon("folder")}</span>
          <span class="tree-label">${escapeHtml(leafName(state.rootPath) || state.rootPath)}</span>
        </button>
        </div>
      ${
        state.expandedTreePaths.has(state.rootPath)
          ? `<div class="tree-children">${renderTreeChildren(state.rootPath)}</div>`
          : ""
      }
    </div>
  `;
}

function renderTreeChildren(parentPath: string): string {
  const children = (state.treeChildren.get(parentPath) ?? []).filter((entry) => entry.isDir);
  return children
    .map((entry) => {
      const expanded = state.expandedTreePaths.has(entry.path);
      return `
        <div class="tree-node${state.selectedTreePath === entry.path ? " selected" : ""}">
          <div class="tree-node-row">
            <button class="tree-expander" data-tree-toggle="${escapeAttr(entry.path)}"${
              entry.isDir ? "" : " disabled"
            }>${entry.isDir ? (expanded ? "−" : "+") : ""}</button>
            <button class="tree-select" data-tree-select="${escapeAttr(entry.path)}" ${renderTooltipAttr(entry.name)}>
              <span class="icon-wrap">${renderIcon(entry.isDir ? "folder" : "file")}</span>
              <span class="tree-label">${escapeHtml(entry.name)}</span>
            </button>
          </div>
          ${
            entry.isDir && expanded
              ? `<div class="tree-children">${renderTreeChildren(entry.path)}</div>`
              : ""
          }
        </div>
      `;
    })
    .join("");
}

void renderTree;

function renderFolderTree(): string {
  if (!state.rootPath) {
    return `<div class="empty-state">Selecciona una carpeta base para cargar el arbol.</div>`;
  }
  const rootChildren = (state.treeChildren.get(state.rootPath) ?? []).filter((entry) => entry.isDir);
  const rootExpanded = state.expandedTreePaths.has(state.rootPath);
  return `
    <div class="tree-node${state.selectedTreePath === state.rootPath ? " selected" : ""}">
      <div class="tree-node-row">
        ${
          rootChildren.length > 0
            ? `<button class="tree-expander" data-tree-toggle="${escapeAttr(state.rootPath)}">${rootExpanded ? "-" : "+"}</button>`
            : `<span class="tree-expander-spacer" aria-hidden="true"></span>`
        }
        <button class="tree-select" data-tree-select="${escapeAttr(state.rootPath)}" ${renderTooltipAttr(
          leafName(state.rootPath) || state.rootPath,
          { delayMs: 450, overflowSelector: ".tree-label" },
        )}>
          <span class="icon-wrap">${renderIcon("folder")}</span>
          <span class="tree-label" ${renderTooltipAttr(leafName(state.rootPath) || state.rootPath, { delayMs: 450, overflowTarget: "self" })}>${escapeHtml(
            leafName(state.rootPath) || state.rootPath,
          )}</span>
        </button>
      </div>
      ${rootExpanded ? `<div class="tree-children">${renderFolderTreeChildren(state.rootPath)}</div>` : ""}
    </div>
  `;
}

function renderFolderTreeChildren(parentPath: string): string {
  const children = (state.treeChildren.get(parentPath) ?? []).filter((entry) => entry.isDir);
  return children
    .map((entry) => {
      const expanded = state.expandedTreePaths.has(entry.path);
      return `
        <div class="tree-node${state.selectedTreePath === entry.path ? " selected" : ""}">
          <div class="tree-node-row">
            ${
              entry.hasDirectoryChildren
                ? `<button class="tree-expander" data-tree-toggle="${escapeAttr(entry.path)}">${expanded ? "-" : "+"}</button>`
                : `<span class="tree-expander-spacer" aria-hidden="true"></span>`
            }
            <button class="tree-select" data-tree-select="${escapeAttr(entry.path)}" ${renderTooltipAttr(entry.name, {
              delayMs: 450,
              overflowSelector: ".tree-label",
            })}>
              <span class="icon-wrap">${renderIcon("folder")}</span>
              <span class="tree-label" ${renderTooltipAttr(entry.name, { delayMs: 450, overflowTarget: "self" })}>${escapeHtml(
                entry.name,
              )}</span>
            </button>
          </div>
          ${expanded ? `<div class="tree-children">${renderFolderTreeChildren(entry.path)}</div>` : ""}
        </div>
      `;
    })
    .join("");
}

function renderContentColumnStyle(): string {
  const widths = state.settings.contentColumnWidths;
  return [
    `--content-col-name:${widths.name}px`,
    `--content-col-type:${widths.type}px`,
    `--content-col-size:${widths.size}px`,
    `--content-col-modified:${widths.modified}px`,
  ].join(";");
}

function renderContentColumnHeader(key: ContentSortKey, label: string): string {
  const active = state.settings.contentSortKey === key;
  const direction = active ? state.settings.contentSortDirection : null;
  const glyph = direction === "desc" ? "↓" : "↑";
  const tooltip = active
    ? `${label}: orden ${direction === "desc" ? "descendente" : "ascendente"}`
    : `Ordenar por ${label.toLowerCase()}`;
  void glyph;
  return `
    <div class="content-column-header">
      <button class="content-column-button${active ? " active" : ""}" data-sort-key="${escapeAttr(key)}" ${renderTooltipAttr(tooltip)}>
        <span>${escapeHtml(label)}</span>
        ${active ? `<span class="sort-indicator active" aria-hidden="true">${direction === "desc" ? "▼" : "▲"}</span>` : ""}
      </button>
      <span class="column-resizer" data-resize-key="${escapeAttr(key)}" aria-hidden="true"></span>
    </div>
  `;
}

function parseDirectoryItemCount(entry: EntryDto): number {
  if (!entry.isDir) {
    return 0;
  }
  const match = entry.sizeLabel.match(/^\d+/);
  return match ? Number(match[0]) : 0;
}

function compareEntryNames(left: EntryDto, right: EntryDto): number {
  return left.name.localeCompare(right.name, "es", { sensitivity: "base", numeric: true });
}

function applyContentOrdering(entries: EntryDto[]): EntryDto[] {
  const key = state.settings.contentSortKey;
  const directionFactor = state.settings.contentSortDirection === "desc" ? -1 : 1;
  return [...entries].sort((left, right) => {
    let result = 0;
    switch (key) {
      case "name":
        result = compareEntryNames(left, right);
        break;
      case "type": {
        if (left.isDir !== right.isDir) {
          result = left.isDir ? -1 : 1;
          break;
        }
        const leftType = left.isDir ? "carpeta" : left.extension || "fichero";
        const rightType = right.isDir ? "carpeta" : right.extension || "fichero";
        result = leftType.localeCompare(rightType, "es", { sensitivity: "base", numeric: true });
        if (result === 0) {
          result = compareEntryNames(left, right);
        }
        break;
      }
      case "size": {
        if (left.isDir !== right.isDir) {
          result = left.isDir ? -1 : 1;
          break;
        }
        const leftMetric = left.isDir ? parseDirectoryItemCount(left) : left.size;
        const rightMetric = right.isDir ? parseDirectoryItemCount(right) : right.size;
        result = leftMetric - rightMetric;
        if (result === 0) {
          result = compareEntryNames(left, right);
        }
        break;
      }
      case "modified":
        result = left.modifiedTs - right.modifiedTs;
        if (result === 0) {
          result = compareEntryNames(left, right);
        }
        break;
      default:
        result = compareEntryNames(left, right);
        break;
    }
    return result * directionFactor;
  });
}

function reapplyContentOrdering(shouldRender = true): void {
  state.currentEntries = applyContentOrdering(state.currentEntries);
  if (shouldRender) {
    render();
  }
}

function toggleContentSort(key: ContentSortKey): void {
  if (state.settings.contentSortKey === key) {
    state.settings.contentSortDirection = state.settings.contentSortDirection === "asc" ? "desc" : "asc";
  } else {
    state.settings.contentSortKey = key;
    state.settings.contentSortDirection = "asc";
  }
  reapplyContentOrdering();
  saveSettings();
}

function renderListView(): string {
  const allSelected =
    state.currentEntries.length > 0 && state.currentEntries.every((entry) => state.selectedContentPaths.includes(entry.path));
  const parent = getParentEntryPath();
  return `
    <div class="content-body list-content-body" style="${renderContentColumnStyle()}">
      <div class="content-header list-row">
        <div class="cell cell-pad-start" aria-hidden="true"></div>
        <div class="cell cell-check">
          <input id="content-select-all" type="checkbox"${allSelected ? " checked" : ""} />
        </div>
        <div class="list-main list-main-header">
          ${renderContentColumnHeader("name", "Nombre")}
          ${renderContentColumnHeader("type", "Tipo")}
          ${renderContentColumnHeader("size", "Tamano")}
          ${renderContentColumnHeader("modified", "Modificado")}
        </div>
        <div class="cell cell-pad-end" aria-hidden="true"></div>
      </div>
      <div class="content-scroll list-scroll" id="content-scroll">
        ${
          parent
            ? renderListParentEntry(parent)
            : ""
        }
        ${
          state.currentEntries.length === 0
            ? `<div class="empty-state">No hay elementos para mostrar.</div>`
            : state.currentEntries.map((entry) => renderListEntry(entry)).join("")
        }
      </div>
    </div>
  `;
}

function renderListParentEntry(path: string): string {
  return `
    <div class="content-row list-row parent-entry" data-parent-entry="true" data-open-path="${escapeAttr(path)}" data-drop-destination="${escapeAttr(path)}" ${renderTooltipAttr(
      "Directorio superior",
    )}>
      <div class="cell cell-pad-start" aria-hidden="true"></div>
      <div class="cell cell-check"></div>
      <div class="list-main list-main-surface">
        <div class="cell name-cell">
          <span class="icon-wrap">${renderIcon("up")}</span>
          <span class="entry-name">..</span>
        </div>
        <div class="cell">Directorio superior</div>
        <div class="cell">-</div>
        <div class="cell">-</div>
      </div>
      <div class="cell cell-pad-end" aria-hidden="true"></div>
    </div>
  `;
}

function renderListEntry(entry: EntryDto): string {
  const selected = state.selectedContentPaths.includes(entry.path);
  const renaming = state.rename?.path === entry.path;
  return `
    <div class="content-row list-row${selected ? " selected" : ""}" data-content-path="${escapeAttr(entry.path)}" data-content-kind="${
      entry.isDir ? "dir" : "file"
    }"${entry.isDir ? ` data-drop-destination="${escapeAttr(entry.path)}"` : ""} ${renderTooltipAttr(entry.name, {
      delayMs: 450,
      overflowSelector: ".entry-name",
    })}>
      <div class="cell cell-pad-start" aria-hidden="true"></div>
      <div class="cell cell-check">
        <input type="checkbox" data-entry-check="${escapeAttr(entry.path)}"${selected ? " checked" : ""} />
      </div>
      <div class="list-main list-main-surface">
        <div class="cell name-cell">
          <span class="icon-wrap">${renderIcon(entry.isDir ? "folder" : "file")}</span>
          ${
            renaming
              ? `<input class="inline-rename" data-rename-input="${escapeAttr(entry.path)}" value="${escapeAttr(
                  state.rename?.value ?? entry.name,
                )}" />`
              : `<span class="entry-name" ${renderTooltipAttr(entry.name, { delayMs: 450, overflowTarget: "self" })}>${escapeHtml(
                  entry.name,
                )}</span>`
          }
        </div>
        <div class="cell">${escapeHtml(entry.isDir ? "Carpeta" : entry.extension || "Fichero")}</div>
        <div class="cell">${escapeHtml(entry.sizeLabel || "-")}</div>
        <div class="cell">${escapeHtml(entry.modifiedLabel)}</div>
      </div>
      <div class="cell cell-pad-end" aria-hidden="true"></div>
    </div>
  `;
}

function renderGridView(): string {
  const parent = getParentEntryPath();
  return `
    <div class="content-body">
      <div class="content-scroll grid-scroll" id="content-scroll">
        ${
          state.currentEntries.length === 0 && !parent
            ? `<div class="empty-state">No hay elementos para mostrar.</div>`
            : `<div class="card-grid">${parent ? renderGridParentEntry(parent) : ""}${state.currentEntries
                .map((entry) => renderGridEntry(entry))
                .join("")}</div>${state.currentEntries.length === 0 ? `<div class="empty-state">No hay elementos para mostrar.</div>` : ""}`
        }
      </div>
    </div>
  `;
}

function renderGridParentEntry(path: string): string {
  return `
    <div class="grid-card parent-entry" data-parent-entry="true" data-open-path="${escapeAttr(path)}" data-drop-destination="${escapeAttr(path)}" ${renderTooltipAttr(
      "Directorio superior",
    )}>
      <div class="grid-card-icon">${renderIcon("up")}</div>
      <div class="grid-card-name">..</div>
      <div class="grid-card-meta">Directorio superior</div>
    </div>
  `;
}

function renderGridEntry(entry: EntryDto): string {
  const selected = state.selectedContentPaths.includes(entry.path);
  const renaming = state.rename?.path === entry.path;
  return `
    <div class="grid-card${selected ? " selected" : ""}" data-content-path="${escapeAttr(entry.path)}" data-content-kind="${
      entry.isDir ? "dir" : "file"
    }"${entry.isDir ? ` data-drop-destination="${escapeAttr(entry.path)}"` : ""} ${renderTooltipAttr(entry.name, {
      delayMs: 450,
      overflowSelector: ".grid-card-name",
    })}>
      <div class="grid-card-check">
        <input type="checkbox" data-entry-check="${escapeAttr(entry.path)}"${selected ? " checked" : ""} />
      </div>
      <div class="grid-card-icon">${renderIcon(entry.isDir ? "folder" : "file")}</div>
      <div class="grid-card-name">
        ${
          renaming
            ? `<input class="inline-rename" data-rename-input="${escapeAttr(entry.path)}" value="${escapeAttr(
                state.rename?.value ?? entry.name,
              )}" />`
            : `<span ${renderTooltipAttr(entry.name, { delayMs: 450, overflowTarget: "self" })}>${escapeHtml(entry.name)}</span>`
        }
      </div>
      <div class="grid-card-meta">${escapeHtml(entry.isDir ? "Carpeta" : entry.extension || "Fichero")}</div>
      <div class="grid-card-meta">${escapeHtml(entry.sizeLabel || "—")}</div>
    </div>
  `;
}

function renderStatusChips(): string {
  if (state.selectedContentPaths.length === 0) {
    return `<span class="status-chip muted">${escapeHtml(state.currentPath ? displayPathForInput(state.currentPath) : "Sin carpeta activa")}</span>`;
  }
  const selected = state.currentEntries.filter((entry) => state.selectedContentPaths.includes(entry.path));
  const summary = state.selectionSummaryLabel
    ? `<span class="status-meta">${escapeHtml(state.selectionSummaryLabel)}</span>`
    : "";
  if (selected.length <= 1) {
    const entry = selected[0];
    if (!entry) {
      return `<span class="status-chip muted">${escapeHtml(state.currentPath ? displayPathForInput(state.currentPath) : "Sin carpeta activa")}</span>`;
    }
    return `
      <span class="status-chip plain">
        <span class="icon-wrap">${renderIcon(entry.isDir ? "folder" : "file")}</span>
        <span>${escapeHtml(entry.name)}</span>
        ${summary}
      </span>
    `;
  }
  return `
    <span class="status-chip plain">
      <span class="icon-wrap">${renderIcon("selection")}</span>
      <span>${escapeHtml(`${selected.length} elementos seleccionados`)}</span>
      ${summary}
    </span>
  `;
}

function renderClipboardIndicator(): string {
  if (!state.clipboard) {
    return `
      <div class="status-indicator" ${renderTooltipAttr("Portapapeles vacio")}>
        ${renderIcon("clipboard")}
      </div>
    `;
  }
  const icon = state.clipboard.operation === "cut" ? "cut" : "copy";
  const label = `Portapapeles: ${state.clipboard.operation === "cut" ? "mover" : "copiar"} ${state.clipboard.paths.length} elemento(s)`;
  return `
    <div class="status-indicator active" ${renderTooltipAttr(label)}>
      ${renderIcon(icon)}
      <span class="status-indicator-badge">${state.clipboard.paths.length}</span>
    </div>
  `;
}

function renderActivityIndicator(): string {
  const active = state.silentBusyCount > 0;
  const title = active
    ? `Proceso en segundo plano: ${state.silentBusyLabel || "Trabajando..."}` 
    : "Sin tareas en segundo plano";
  return `
    <div class="status-indicator activity-indicator${active ? " active" : ""}" ${renderTooltipAttr(title)}>
      ${renderIcon(active ? "refresh" : "check")}
    </div>
  `;
}

function renderNotificationsIndicator(): string {
  const count = state.notifications.length;
  const tooltip = count
    ? `${count} notificacion${count === 1 ? "" : "es"} en el historial`
    : "Sin notificaciones";
  return `
    <button
      id="notifications-toggle"
      class="status-indicator${state.notificationsOpen ? " active" : ""}"
      ${renderTooltipAttr(tooltip)}
    >
      ${renderIcon("bell")}
      ${count ? `<span class="status-indicator-badge">${count > 99 ? "99+" : count}</span>` : ""}
    </button>
  `;
}

function renderNotificationsDropdown(): string {
  if (!state.notificationsOpen) {
    return "";
  }
  return `
    <div class="notifications-dropdown">
      <div class="notifications-dropdown-head">
        <strong>Notificaciones</strong>
        ${
          state.notifications.length
            ? `<button id="notifications-clear" class="toolbar-button compact icon-only" ${renderTooltipAttr("Vaciar historial")}>${renderIcon("trash")}</button>`
            : ""
        }
      </div>
      <div class="notifications-list">
        ${
          state.notifications.length
            ? state.notifications.map((notification) => renderNotificationHistoryCard(notification)).join("")
            : `<div class="location-empty">No hay notificaciones registradas.</div>`
        }
      </div>
    </div>
  `;
}

function renderNotificationHistoryCard(notification: NotificationEntry): string {
  const expanded = state.expandedNotificationIds.has(notification.id);
  return `
    <div
      class="notification-card ${notification.kind}${expanded ? " expanded" : ""}"
    >
      <button
        class="notification-card-dismiss"
        data-notification-remove="${escapeAttr(notification.id)}"
        ${renderTooltipAttr("Eliminar")}
      >
        ${renderIcon("x")}
      </button>
      <div
        class="notification-card-content"
        data-notification-card="${escapeAttr(notification.id)}"
        ${renderTooltipAttr(expanded ? "Notificacion expandida" : "Expandir")}
        tabindex="0"
      >
        <div class="notification-card-head">
          <span class="notification-card-kind ${notification.kind}">${escapeHtml(notification.kind === "error" ? "Error" : "Info")}</span>
          <span class="notification-card-time">${escapeHtml(formatNotificationTime(notification.createdAt))}</span>
        </div>
        <div class="notification-card-body">${escapeHtml(notification.text)}</div>
        ${
          expanded
            ? `
              <button
                class="notification-card-collapse"
                data-notification-collapse="${escapeAttr(notification.id)}"
                ${renderTooltipAttr("Plegar")}
              >
                ${renderIcon("chevron-up")}
              </button>
            `
            : ""
        }
      </div>
    </div>
  `;
}

function renderNotificationToast(): string {
  const active = getActiveNotification();
  if (!active) {
    return "";
  }
  const remaining = getActiveNotificationRemainingMs();
  const elapsed = Math.max(0, ACTIVE_NOTIFICATION_DURATION_MS - remaining);
  return `
    <div
      id="notification-toast"
      class="notification-toast ${active.kind}"
      data-notification-toast="${escapeAttr(active.id)}"
    >
      <div class="notification-toast-head">
        <strong>${escapeHtml(active.kind === "error" ? "Error" : "Informacion")}</strong>
        <span>${escapeHtml(formatNotificationTime(active.createdAt))}</span>
      </div>
      <div class="notification-toast-body">${escapeHtml(active.text)}</div>
      <div class="notification-toast-progress-track">
        <div
          class="notification-toast-progress${activeNotificationPaused ? " paused" : ""}"
          style="animation-duration:${ACTIVE_NOTIFICATION_DURATION_MS}ms; animation-delay:-${elapsed}ms; animation-play-state:${activeNotificationPaused ? "paused" : "running"};"
        ></div>
      </div>
    </div>
  `;
}

function renderContentQuickActions(): string {
  const hasSelection = state.selectedContentPaths.length > 0;
  const singleSelection = state.selectedContentPaths.length === 1;
  const canPaste = Boolean(state.clipboard && state.currentPath);
  const actions: Array<{ id: string; icon: string; title: string; disabled: boolean; danger?: boolean }> = [
    { id: "open", icon: "open", title: "Abrir", disabled: !singleSelection },
    { id: "rename", icon: "rename", title: "Renombrar", disabled: !singleSelection },
    { id: "copy", icon: "copy", title: "Copiar", disabled: !hasSelection },
    { id: "cut", icon: "cut", title: "Cortar", disabled: !hasSelection },
    { id: "paste", icon: "paste", title: "Pegar", disabled: !canPaste },
    { id: "delete", icon: "trash", title: "Eliminar", disabled: !hasSelection, danger: true },
    { id: "properties", icon: "info", title: "Propiedades", disabled: !hasSelection },
  ];
  return `
    <div class="quick-actions">
      ${actions
        .map(
          (action) => `
            <button
              class="toolbar-button compact icon-only quick-action${action.danger ? " danger" : ""}"
              data-quick-action="${escapeAttr(action.id)}"
              ${renderTooltipAttr(action.title)}
              ${action.disabled ? "disabled" : ""}
            >
              ${renderIcon(action.icon)}
            </button>
          `,
        )
        .join("")}
    </div>
  `;
}

function renderContextMenu(): string {
  const menu = state.contextMenu;
  if (!menu) {
    return "";
  }
  return `
    <div class="context-menu" style="left:${menu.x}px; top:${menu.y}px;">
      ${menu.actions
        .map(
          (action) => `
            <button class="context-item${action.danger ? " danger" : ""}" data-context-action="${escapeAttr(
              action.id,
            )}"${action.disabled ? " disabled" : ""}>
              <span class="menu-item-icon">${renderIcon(action.icon)}</span>
              <span>${escapeHtml(action.label)}</span>
            </button>
          `,
        )
        .join("")}
    </div>
  `;
}

function renderProgressDialog(): string {
  const progress = state.progress!;
  return `
    <div class="modal-backdrop">
      <div class="modal progress-modal">
        <div class="modal-head">
          <h3>${escapeHtml(progress.title)}</h3>
        </div>
        <div class="progress-track">
          <div class="progress-fill" style="width:${Math.round(progress.progress * 100)}%;"></div>
        </div>
        <div class="progress-meta">${Math.round(progress.progress * 100)}% | ${escapeHtml(progress.message)}</div>
        <div class="log-view">${progress.logs.map((line) => escapeHtml(line)).join("<br />")}</div>
        ${
          progress.done
            ? `<div class="dialog-actions">
                <div class="result-line ${progress.success ? "ok" : "error"}">${escapeHtml(progress.resultMessage)}</div>
                <button id="progress-close" class="toolbar-button primary">Cerrar</button>
              </div>`
            : ""
        }
      </div>
    </div>
  `;
}

function renderPropertiesDialog(): string {
  const summary = state.propertiesDialog!;
  return `
    <div class="modal-backdrop">
      <div class="modal">
        <div class="modal-head">
          <h3>Propiedades</h3>
          <button id="properties-close" class="toolbar-button">Cerrar</button>
        </div>
        <div class="properties-grid">
          <div>Elementos seleccionados</div><div>${summary.count}</div>
          <div>Ficheros contenidos</div><div>${summary.files}</div>
          <div>Subcarpetas contenidas</div><div>${summary.directories}</div>
          <div>Tamano total</div><div>${escapeHtml(summary.totalSizeLabel)}</div>
        </div>
        <div class="log-view">${summary.lines.map((line) => escapeHtml(line)).join("<br />")}</div>
      </div>
    </div>
  `;
}

function renderExtractionDialog(): string {
  const dialog = state.extractionDialog!;
  return `
    <div class="modal-backdrop">
      <div class="modal">
        <div class="modal-head">
          <h3>Descomprimir...</h3>
          <button id="extract-close" class="toolbar-button">Cerrar</button>
        </div>
        <div class="form-grid">
          <label class="checkbox-line"><input id="extract-individual" type="checkbox"${
            dialog.options.individualFolders ? " checked" : ""
          } /><span>En carpeta/as individuales de mismo nombre</span></label>
          <label class="checkbox-line"><input id="extract-custom-destination" type="checkbox"${
            dialog.options.destinationMode === "custom" ? " checked" : ""
          } /><span>En otra ruta...</span></label>
          <div class="path-picker">
            <input id="extract-destination-path" type="text" value="${escapeAttr(
              dialog.options.destinationPath ?? "",
            )}" placeholder="Ruta alternativa" ${
              dialog.options.destinationMode === "custom" ? "" : "disabled"
            } />
            <button id="extract-pick-path" class="toolbar-button"${
              dialog.options.destinationMode === "custom" ? "" : " disabled"
            }>Seleccionar</button>
          </div>
          <label class="checkbox-line"><input id="extract-delete-archives" type="checkbox"${
            dialog.options.deleteArchives ? " checked" : ""
          } /><span>Eliminar ficheros comprimidos al terminar</span></label>
          <label class="checkbox-line"><input id="extract-overwrite" type="checkbox"${
            dialog.options.overwrite ? " checked" : ""
          } /><span>Permitir sobreescribir</span></label>
        </div>
        <div class="dialog-actions">
          <button id="extract-preview" class="toolbar-button">Previsualizar resultado</button>
          <button id="extract-run" class="toolbar-button primary">Descomprimir</button>
        </div>
        ${
          dialog.previewRows.length > 0
            ? `
              <div class="preview-columns">
                <div>
                  <h4>Estado actual</h4>
                  <div class="log-view compact">${dialog.previewRows.map((row) => escapeHtml(row.archivePath)).join("<br />")}</div>
                </div>
                <div>
                  <h4>Resultado esperado</h4>
                  <div class="log-view compact">${dialog.previewRows.map((row) => escapeHtml(row.destinationPath)).join("<br />")}</div>
                </div>
              </div>
            `
            : ""
        }
      </div>
    </div>
  `;
}

function renderChdDialog(): string {
  const dialog = state.chdDialog!;
  const directoryMode = dialog.paths.some((path) => pathIsDirectorySelection(path));
  return `
    <div class="modal-backdrop">
      <div class="modal">
        <div class="modal-head">
          <h3>${escapeHtml(dialog.analysis.chdMenuLabel ?? "Convertir a CHD")}</h3>
          <button id="chd-close" class="toolbar-button">Cerrar</button>
        </div>
        <div class="log-view compact">${dialog.analysis.chdSources
          .map((source) => escapeHtml(source.sourcePath))
          .join("<br />")}</div>
        <div class="form-grid">
          <label class="checkbox-line"><input id="chd-delete-originals" type="checkbox"${
            dialog.options.deleteOriginals ? " checked" : ""
          } /><span>Eliminar ficheros originales al terminar</span></label>
          <label class="checkbox-line"><input id="chd-name-as-container" type="checkbox"${
            dialog.options.nameAsContainer ? " checked" : ""
          } /><span>Nombrar fichero final como carpeta contenedora</span></label>
          <p class="help-line">Util cuando la ROM esta dentro de una subcarpeta y quieres heredar ese nombre.</p>
          ${
            directoryMode
              ? `
                <label class="checkbox-line"><input id="chd-deposit-parent" type="checkbox"${
                  dialog.options.depositToParent ? " checked" : ""
                } /><span>Depositar todo en la carpeta padre</span></label>
                <p class="help-line">Solo afecta a ROMs que viven dentro de subcarpetas del directorio seleccionado.</p>
              `
              : ""
          }
          ${
            directoryMode && dialog.options.depositToParent
              ? `
                <label class="checkbox-line"><input id="chd-delete-subfolders" type="checkbox"${
                  dialog.options.deleteOriginalSubfolders ? " checked" : ""
                } /><span>Eliminar subcarpetas originales al terminar</span></label>
              `
              : ""
          }
          <label class="checkbox-line"><input id="chd-overwrite" type="checkbox"${
            dialog.options.overwrite ? " checked" : ""
          } /><span>Permitir sobreescribir</span></label>
        </div>
        <div class="dialog-actions">
          <button id="chd-run" class="toolbar-button primary"${
            dialog.analysis.chdSources.length === 0 ? " disabled" : ""
          }>Convertir</button>
        </div>
      </div>
    </div>
  `;
}

function renderChdRestoreDialog(): string {
  const dialog = state.chdRestoreDialog!;
  return `
    <div class="modal-backdrop">
      <div class="modal">
        <div class="modal-head">
          <h3>${escapeHtml(dialog.analysis.chdRestoreMenuLabel ?? "Recuperar desde CHD")}</h3>
          <button id="chd-restore-close" class="toolbar-button">Cerrar</button>
        </div>
        <div class="log-view compact">${dialog.analysis.restorableChds.map((item) => escapeHtml(item)).join("<br />")}</div>
        <p class="help-line">Basado en las ordenes oficiales de chdman: extractcd recupera salida CUE/BIN y extractdvd recupera salida ISO.</p>
        <div class="form-grid">
          <label class="checkbox-line"><input id="restore-individual" type="checkbox"${
            dialog.options.individualFolders ? " checked" : ""
          } /><span>En carpeta/as individuales de mismo nombre</span></label>
          <label class="checkbox-line"><input id="restore-custom-destination" type="checkbox"${
            dialog.options.destinationMode === "custom" ? " checked" : ""
          } /><span>En otra ruta...</span></label>
          <div class="path-picker">
            <input id="restore-destination-path" type="text" value="${escapeAttr(
              dialog.options.destinationPath ?? "",
            )}" placeholder="Ruta alternativa" ${
              dialog.options.destinationMode === "custom" ? "" : "disabled"
            } />
            <button id="restore-pick-path" class="toolbar-button"${
              dialog.options.destinationMode === "custom" ? "" : " disabled"
            }>Seleccionar</button>
          </div>
          <label class="checkbox-line"><input id="restore-delete-chd" type="checkbox"${
            dialog.options.deleteChd ? " checked" : ""
          } /><span>Eliminar ficheros .chd al terminar</span></label>
          <label class="checkbox-line"><input id="restore-overwrite" type="checkbox"${
            dialog.options.overwrite ? " checked" : ""
          } /><span>Permitir sobreescribir</span></label>
          <label class="checkbox-line"><input id="restore-split-bin" type="checkbox"${
            dialog.options.splitBin ? " checked" : ""
          } /><span>Dividir BIN por pistas cuando aplique</span></label>
        </div>
        <div class="dialog-actions">
          <button id="restore-run" class="toolbar-button primary"${
            dialog.analysis.restorableChds.length === 0 ? " disabled" : ""
          }>Recuperar</button>
        </div>
      </div>
    </div>
  `;
}

function renderSettingsDialog(): string {
  return `
    <div class="modal-backdrop">
      <div class="modal">
        <div class="modal-head">
          <h3>Opciones</h3>
          <button id="settings-close" class="toolbar-button compact icon-only" ${renderTooltipAttr("Cerrar")}>${renderIcon("x")}</button>
        </div>

        <div class="settings-section">
          <h4>Apariencia</h4>
          <div class="settings-grid">
            <label>Tema</label>
            <select id="settings-theme">
              <option value="light"${state.settings.theme === "light" ? " selected" : ""}>Claro</option>
              <option value="dark"${state.settings.theme === "dark" ? " selected" : ""}>Oscuro</option>
            </select>
            <label>Escala tipografica</label>
            <div class="settings-inline">
              <input id="settings-font-scale" type="range" min="90" max="125" step="5" value="${state.settings.fontScale}" />
              <span id="settings-font-scale-value">${state.settings.fontScale}%</span>
            </div>
            <label>Interfaz compacta</label>
            <label class="checkbox-line"><input id="settings-compact-ui" type="checkbox"${
              state.settings.compactUi ? " checked" : ""
            } /><span>Reducir altura de barras y controles</span></label>
          </div>
        </div>

        <div class="settings-section">
          <h4>Explorador</h4>
          <div class="settings-grid">
            <label>Vista por defecto</label>
            <select id="settings-default-view">
              <option value="list"${state.settings.defaultViewMode === "list" ? " selected" : ""}>Lista</option>
              <option value="grid"${state.settings.defaultViewMode === "grid" ? " selected" : ""}>Rejilla</option>
            </select>
            <label>Zoom contenido por defecto</label>
            <div class="settings-inline">
              <input id="settings-default-content-zoom" type="range" min="1" max="100" step="1" value="${state.settings.defaultContentZoom}" />
              <span id="settings-default-content-zoom-value">${state.settings.defaultContentZoom}%</span>
            </div>
            <label>Zoom arbol por defecto</label>
            <div class="settings-inline">
              <input id="settings-default-tree-zoom" type="range" min="1" max="100" step="1" value="${state.settings.defaultTreeZoom}" />
              <span id="settings-default-tree-zoom-value">${state.settings.defaultTreeZoom}%</span>
            </div>
            <label>Calculo de peso</label>
            <div class="settings-stack">
              <label class="checkbox-line"><input id="settings-selection-weight-all" type="checkbox"${
                state.settings.selectionWeightCalculateAll ? " checked" : ""
              } /><span>Calcular todo sin limite de subniveles</span></label>
              <div class="settings-inline">
                <span>Subniveles a tener en cuenta</span>
                <input id="settings-selection-weight-depth" type="number" min="1" max="99" step="1" value="${state.settings.selectionWeightMaxDepth}"${
                  state.settings.selectionWeightCalculateAll ? " disabled" : ""
                } />
              </div>
            </div>
          </div>
        </div>

        <div class="settings-section">
          <h4>Seguridad</h4>
          <div class="settings-grid">
            <label>Confirmar eliminacion</label>
            <label class="checkbox-line"><input id="settings-confirm-delete" type="checkbox"${
              state.settings.confirmDelete ? " checked" : ""
            } /><span>Pedir confirmacion antes de borrar</span></label>
            <label>Popup de borrado</label>
            <label class="checkbox-line"><input id="settings-hide-delete-progress-popup" type="checkbox"${
              state.settings.hideDeleteProgressPopup ? " checked" : ""
            } /><span>No mostrar nunca el popup de progreso al eliminar</span></label>
          </div>
        </div>

        <div class="dialog-actions">
          <button id="settings-reset" class="toolbar-button">${renderIcon("refresh")}Restablecer</button>
          <button id="settings-apply" class="toolbar-button">${renderIcon("check")}Aplicar</button>
          <button id="settings-save" class="toolbar-button primary">${renderIcon("save")}Guardar</button>
        </div>
      </div>
    </div>
  `;
}

function renderAboutDialog(): string {
  const metadata = state.metadata?.chdman ?? {};
  const version = pickMetadataString(metadata, ["version", "release", "release_tag"]) ?? "No disponible";
  const source = pickMetadataString(metadata, ["source"]) ?? "No disponible";
  const author = pickMetadataString(metadata, ["author"]) ?? "MAMEdev and contributors";
  return `
    <div class="modal-backdrop">
      <div class="modal about-modal">
        <div class="modal-head">
          <h3>About</h3>
          <button id="about-close" class="toolbar-button">Cerrar</button>
        </div>
        <div class="about-block"><strong>Dogu</strong><br />Version app: ${escapeHtml(
          state.metadata?.appVersion ?? "0.2.0",
        )}</div>
        <div class="about-block"><strong>CHDMAN integrado</strong><br />Version: ${escapeHtml(
          version,
        )}<br />Autoria / source: ${escapeHtml(author)}<br />Referencia: ${escapeHtml(source)}</div>
      </div>
    </div>
  `;
}

function bindUi(): void {
  bindMenu();
  bindPathbar();
  bindTree();
  bindContent();
  bindDialogs();
  bindSplitter();
  bindNotifications();
  adjustContextMenuPosition();

  appRoot.querySelector<HTMLButtonElement>("#top-search-run")?.addEventListener("click", () => {
    void runGlobalSearch();
  });
  appRoot.querySelector<HTMLButtonElement>("#top-search-clear")?.addEventListener("click", () => {
    void clearSearchMode();
  });
  appRoot.querySelector<HTMLInputElement>("#top-search-input")?.addEventListener("input", (event) => {
    state.topSearchQuery = (event.currentTarget as HTMLInputElement).value;
    syncTopSearchUi();
  });
  appRoot.querySelector<HTMLInputElement>("#top-search-input")?.addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      void runGlobalSearch();
    }
  });
}

function bindNotifications(): void {
  appRoot.querySelector<HTMLButtonElement>("#notifications-toggle")?.addEventListener("click", (event) => {
    event.stopPropagation();
    state.notificationsOpen = !state.notificationsOpen;
    render();
  });
  appRoot.querySelector<HTMLElement>(".notifications-dropdown")?.addEventListener("click", (event) => {
    event.stopPropagation();
  });
  appRoot.querySelector<HTMLButtonElement>("#notifications-clear")?.addEventListener("click", (event) => {
    event.stopPropagation();
    clearNotifications();
  });
  appRoot.querySelectorAll<HTMLElement>("[data-notification-card]").forEach((card) => {
    card.addEventListener("click", (event) => {
      event.stopPropagation();
      const notificationId = (event.currentTarget as HTMLElement).dataset.notificationCard;
      if (notificationId && !state.expandedNotificationIds.has(notificationId)) {
        toggleNotificationExpansion(notificationId);
      }
    });
    card.addEventListener("keydown", (event) => {
      if (event.key !== "Enter" && event.key !== " ") {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
      const notificationId = (event.currentTarget as HTMLElement).dataset.notificationCard;
      if (notificationId && !state.expandedNotificationIds.has(notificationId)) {
        toggleNotificationExpansion(notificationId);
      }
    });
  });
  appRoot.querySelectorAll<HTMLElement>("[data-notification-collapse]").forEach((button) => {
    button.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      const notificationId = (event.currentTarget as HTMLElement).dataset.notificationCollapse;
      if (notificationId && state.expandedNotificationIds.has(notificationId)) {
        toggleNotificationExpansion(notificationId);
      }
    });
  });
  appRoot.querySelectorAll<HTMLElement>("[data-notification-remove]").forEach((button) => {
    button.addEventListener("click", (event) => {
      event.stopPropagation();
      const notificationId = (event.currentTarget as HTMLElement).dataset.notificationRemove;
      if (notificationId) {
        removeNotification(notificationId);
      }
    });
  });
  appRoot.querySelector<HTMLElement>("#notification-toast")?.addEventListener("mouseenter", () => {
    pauseActiveNotificationTimer();
  });
  appRoot.querySelector<HTMLElement>("#notification-toast")?.addEventListener("mouseleave", () => {
    resumeActiveNotificationTimer();
  });
}

function bindMenu(): void {
  appRoot.querySelectorAll<HTMLElement>("[data-menu-trigger]").forEach((button) => {
    button.addEventListener("click", (event) => {
      event.stopPropagation();
      const label = (event.currentTarget as HTMLElement).dataset.menuTrigger ?? null;
      state.menuOpen = state.menuOpen === label ? null : label;
      render();
    });
  });
  appRoot.querySelectorAll<HTMLElement>("[data-menu-action]").forEach((button) => {
    button.addEventListener("click", (event) => {
      event.stopPropagation();
      state.menuOpen = null;
      void handleMenuAction((event.currentTarget as HTMLElement).dataset.menuAction ?? "");
    });
  });
}

function bindPathbar(): void {
  appRoot.querySelector<HTMLButtonElement>("#go-up")?.addEventListener("click", () => {
    void navigateUp();
  });
  appRoot.querySelector<HTMLButtonElement>("#refresh-current")?.addEventListener("click", () => {
    void refreshVisibleData();
  });
  appRoot.querySelector<HTMLInputElement>("#path-input")?.addEventListener("input", (event) => {
    state.pathInput = (event.currentTarget as HTMLInputElement).value;
  });
  appRoot.querySelector<HTMLInputElement>("#path-input")?.addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      void goToPathInput();
    }
  });
  appRoot.querySelector<HTMLButtonElement>("#path-go")?.addEventListener("click", () => {
    void goToPathInput();
  });

  appRoot.querySelectorAll<HTMLButtonElement>("[data-quick-action]").forEach((button) => {
    button.addEventListener("click", () => {
      void handleContextAction(button.dataset.quickAction ?? "");
    });
  });
  appRoot.querySelector<HTMLButtonElement>("#view-list")?.addEventListener("click", () => {
    state.viewMode = "list";
    render();
  });
  appRoot.querySelector<HTMLButtonElement>("#view-grid")?.addEventListener("click", () => {
    state.viewMode = "grid";
    render();
  });
  appRoot.querySelector<HTMLInputElement>("#zoom-range")?.addEventListener("input", (event) => {
    setContentZoomPreference(Number((event.currentTarget as HTMLInputElement).value));
  });
  bindZoomPreview("#zoom-range");
  appRoot.querySelector<HTMLInputElement>("#tree-zoom-range")?.addEventListener("input", (event) => {
    setTreeZoomPreference(Number((event.currentTarget as HTMLInputElement).value));
  });
  bindZoomPreview("#tree-zoom-range");

  appRoot.querySelector<HTMLInputElement>("#local-search-input")?.addEventListener("input", (event) => {
    state.localSearchQuery = (event.currentTarget as HTMLInputElement).value;
  });
  appRoot.querySelector<HTMLInputElement>("#local-search-input")?.addEventListener("keydown", (event) => {
    if (event.key === "Enter") {
      void runLocalSearch();
    }
  });
  appRoot.querySelector<HTMLInputElement>("#local-search-recursive")?.addEventListener("change", (event) => {
    state.localSearchRecursive = (event.currentTarget as HTMLInputElement).checked;
  });
  appRoot.querySelector<HTMLButtonElement>("#local-search-run")?.addEventListener("click", () => {
    void runLocalSearch();
  });
  appRoot.querySelector<HTMLButtonElement>("#local-search-close")?.addEventListener("click", () => {
    state.localSearchOpen = false;
    render();
  });
  appRoot.querySelectorAll<HTMLElement>("[data-location-root]").forEach((button) => {
    button.addEventListener("click", (event) => {
      const root = (event.currentTarget as HTMLElement).dataset.locationRoot;
      if (root) {
        void setRootAndNavigate(root);
      }
    });
  });
  appRoot.querySelector<HTMLButtonElement>("#pick-local-root-inline")?.addEventListener("click", () => {
    void chooseRootDirectory();
  });
  appRoot.querySelector<HTMLButtonElement>("#add-local-root-inline")?.addEventListener("click", () => {
    void chooseRootDirectory();
  });
  appRoot.querySelector<HTMLButtonElement>("#open-connections-inline")?.addEventListener("click", () => {
    void openConnectionsManagerWindow();
  });
}

function bindTree(): void {
  const treeScroll = appRoot.querySelector<HTMLDivElement>(".tree-scroll");
  treeScroll?.addEventListener("scroll", (event) => {
    state.treeScrollTop = (event.currentTarget as HTMLDivElement).scrollTop;
  });
  appRoot.querySelectorAll<HTMLElement>("[data-tree-toggle]").forEach((button) => {
    button.addEventListener("pointerdown", (event) => {
      event.preventDefault();
    });
    button.addEventListener("click", (event) => {
      event.stopPropagation();
      const path = (event.currentTarget as HTMLElement).dataset.treeToggle;
      if (path) {
        void toggleTreePath(path);
      }
    });
  });
  appRoot.querySelectorAll<HTMLElement>("[data-tree-select]").forEach((button) => {
    button.addEventListener("pointerdown", (event) => {
      event.preventDefault();
    });
    button.addEventListener("click", (event) => {
      event.stopPropagation();
      const path = (event.currentTarget as HTMLElement).dataset.treeSelect;
      if (path) {
        void selectTreePath(path);
      }
    });
    button.addEventListener("contextmenu", (event) => {
      event.preventDefault();
      event.stopPropagation();
      const path = (event.currentTarget as HTMLElement).dataset.treeSelect;
      if (!path) {
        return;
      }
      const treeEntry = findTreeEntry(path);
      void openContextMenu(event.clientX, event.clientY, [path], treeEntry?.isDir ? path : null);
    });
  });
  bindTreeDropTargets(treeScroll);
}

function bindContent(): void {
  appRoot.querySelectorAll<HTMLElement>("[data-sort-key]").forEach((button) => {
    button.addEventListener("click", (event) => {
      event.preventDefault();
      event.stopPropagation();
      const key = (event.currentTarget as HTMLElement).dataset.sortKey as ContentSortKey | undefined;
      if (key) {
        toggleContentSort(key);
      }
    });
  });

  appRoot.querySelectorAll<HTMLElement>("[data-resize-key]").forEach((handle) => {
    handle.addEventListener("pointerdown", (event) => {
      event.preventDefault();
      event.stopPropagation();
      const key = (event.currentTarget as HTMLElement).dataset.resizeKey as ContentSortKey | undefined;
      if (!key) {
        return;
      }
      const startWidth = state.settings.contentColumnWidths[key];
      const startX = event.clientX;
      const pointerId = event.pointerId;
      const target = event.currentTarget as HTMLElement;
      target.setPointerCapture(pointerId);
      document.body.classList.add("dragging-column-resize");

      const onPointerMove = (moveEvent: PointerEvent) => {
        if (moveEvent.pointerId !== pointerId) {
          return;
        }
        state.settings.contentColumnWidths[key] = Math.max(
          CONTENT_COLUMN_MIN_WIDTHS[key],
          Math.round(startWidth + (moveEvent.clientX - startX)),
        );
        applyContentColumnWidths();
      };

      const finish = () => {
        document.body.classList.remove("dragging-column-resize");
        target.releasePointerCapture(pointerId);
        target.removeEventListener("pointermove", onPointerMove);
        target.removeEventListener("pointerup", onPointerUp);
        target.removeEventListener("pointercancel", onPointerCancel);
        saveSettings();
      };

      const onPointerUp = (upEvent: PointerEvent) => {
        if (upEvent.pointerId !== pointerId) {
          return;
        }
        finish();
      };

      const onPointerCancel = (cancelEvent: PointerEvent) => {
        if (cancelEvent.pointerId !== pointerId) {
          return;
        }
        finish();
      };

      target.addEventListener("pointermove", onPointerMove);
      target.addEventListener("pointerup", onPointerUp);
      target.addEventListener("pointercancel", onPointerCancel);
    });
  });

  appRoot.querySelectorAll<HTMLElement>("[data-parent-entry]").forEach((row) => {
    row.addEventListener("click", (event) => {
      if ((event.target as HTMLElement).closest(".cell-pad-start, .cell-check, .cell-pad-end")) {
        return;
      }
      setContentSelection([], null, true);
    });
    row.addEventListener("dblclick", (event) => {
      if ((event.target as HTMLElement).closest(".cell-pad-start, .cell-check, .cell-pad-end")) {
        return;
      }
      const path = row.dataset.openPath;
      if (path) {
        void navigateToPath(path);
      }
    });
  });

  appRoot.querySelectorAll<HTMLElement>("[data-content-path]").forEach((row) => {
    row.addEventListener("pointerdown", (event) => {
      beginContentPointerDrag(event, row);
    });
    row.addEventListener("click", (event) => {
      if (suppressContentClick) {
        suppressContentClick = false;
        event.preventDefault();
        event.stopPropagation();
        return;
      }
      if ((event.target as HTMLElement).closest("[data-entry-check], .cell-pad-start, .cell-check, .cell-pad-end, .inline-rename")) {
        return;
      }
      void handleContentClick(event as MouseEvent, row);
    });
    row.addEventListener("dblclick", (event) => {
      if ((event.target as HTMLElement).closest(".cell-pad-start, .cell-check, .cell-pad-end, .inline-rename")) {
        return;
      }
      void openPathFromRow(row);
    });
    row.addEventListener("contextmenu", (event) => {
      event.preventDefault();
      const path = row.dataset.contentPath;
      if (!path) {
        return;
      }
      selectByContext(path);
      const entry = state.currentEntries.find((item) => item.path === path);
      void openContextMenu(event.clientX, event.clientY, state.selectedContentPaths, entry?.isDir ? path : state.currentPath);
    });
  });

  appRoot.querySelectorAll<HTMLInputElement>("[data-entry-check]").forEach((checkbox) => {
    checkbox.addEventListener("click", (event) => {
      event.stopPropagation();
    });
    checkbox.addEventListener("change", (event) => {
      const path = (event.currentTarget as HTMLInputElement).dataset.entryCheck;
      if (!path) {
        return;
      }
      setContentSelection(toggleSelection(state.selectedContentPaths, path), path, true);
    });
  });

  const selectAll = appRoot.querySelector<HTMLInputElement>("#content-select-all");
  if (selectAll) {
    selectAll.indeterminate =
      state.selectedContentPaths.length > 0 && state.selectedContentPaths.length < state.currentEntries.length;
    selectAll.addEventListener("click", (event) => {
      event.stopPropagation();
    });
    selectAll.addEventListener("change", (event) => {
      const checked = (event.currentTarget as HTMLInputElement).checked;
      const paths = checked ? state.currentEntries.map((entry) => entry.path) : [];
      setContentSelection(paths, paths[0] ?? null, true);
    });
  }

  const contentScroll = appRoot.querySelector<HTMLDivElement>("#content-scroll");
  contentScroll?.addEventListener("scroll", (event) => {
    const element = event.currentTarget as HTMLDivElement;
    state.contentScrollTop = element.scrollTop;
    state.contentScrollLeft = element.scrollLeft;
  });
  contentScroll?.addEventListener("contextmenu", (event) => {
    if ((event.target as HTMLElement).closest("[data-content-path]")) {
      return;
    }
    event.preventDefault();
    state.selectedContentPaths = [];
    state.anchorPath = null;
    state.selectionSummaryLabel = null;
    state.selectionSummaryToken += 1;
    state.rename = null;
    void openContextMenu(event.clientX, event.clientY, [], state.currentPath, true);
  });
  bindContentDropTargets(contentScroll);
  bindContentMarqueeSelection(contentScroll);

  appRoot.querySelectorAll<HTMLElement>("[data-context-action]").forEach((item) => {
    item.addEventListener("click", (event) => {
      event.stopPropagation();
      void handleContextAction((event.currentTarget as HTMLElement).dataset.contextAction ?? "");
    });
  });

  appRoot.querySelectorAll<HTMLInputElement>("[data-rename-input]").forEach((input) => {
    input.addEventListener("pointerdown", (event) => {
      event.stopPropagation();
    });
    input.addEventListener("click", (event) => {
      event.stopPropagation();
    });
    input.addEventListener("input", (event) => {
      if (!state.rename) {
        return;
      }
      state.rename.value = (event.currentTarget as HTMLInputElement).value;
    });
    input.addEventListener("keydown", (event) => {
      if (event.key === "Enter") {
        event.preventDefault();
        if (state.rename) {
          scheduleRenameCommit(state.rename.path, input.value, true);
        }
      }
      if (event.key === "Escape") {
        event.preventDefault();
        cancelRename();
      }
    });
    window.setTimeout(() => {
      input.focus();
      input.select();
    }, 0);
  });
}

function bindDialogs(): void {
  appRoot.querySelector<HTMLButtonElement>("#progress-close")?.addEventListener("click", () => {
    state.progress = null;
    render();
  });
  appRoot.querySelector<HTMLButtonElement>("#properties-close")?.addEventListener("click", () => {
    state.propertiesDialog = null;
    render();
  });
  appRoot.querySelector<HTMLButtonElement>("#extract-close")?.addEventListener("click", () => {
    state.extractionDialog = null;
    render();
  });
  appRoot.querySelector<HTMLInputElement>("#extract-individual")?.addEventListener("change", (event) => {
    if (!state.extractionDialog) {
      return;
    }
    state.extractionDialog.options.individualFolders = (event.currentTarget as HTMLInputElement).checked;
  });
  appRoot.querySelector<HTMLInputElement>("#extract-custom-destination")?.addEventListener("change", (event) => {
    if (!state.extractionDialog) {
      return;
    }
    state.extractionDialog.options.destinationMode = (event.currentTarget as HTMLInputElement).checked
      ? "custom"
      : "same";
    render();
  });
  appRoot.querySelector<HTMLInputElement>("#extract-destination-path")?.addEventListener("input", (event) => {
    if (state.extractionDialog) {
      state.extractionDialog.options.destinationPath = (event.currentTarget as HTMLInputElement).value;
    }
  });
  appRoot.querySelector<HTMLButtonElement>("#extract-pick-path")?.addEventListener("click", () => {
    void pickExtractionDestination();
  });
  appRoot.querySelector<HTMLInputElement>("#extract-delete-archives")?.addEventListener("change", (event) => {
    if (state.extractionDialog) {
      state.extractionDialog.options.deleteArchives = (event.currentTarget as HTMLInputElement).checked;
    }
  });
  appRoot.querySelector<HTMLInputElement>("#extract-overwrite")?.addEventListener("change", (event) => {
    if (state.extractionDialog) {
      state.extractionDialog.options.overwrite = (event.currentTarget as HTMLInputElement).checked;
    }
  });
  appRoot.querySelector<HTMLButtonElement>("#extract-preview")?.addEventListener("click", () => {
    void loadExtractionPreview();
  });
  appRoot.querySelector<HTMLButtonElement>("#extract-run")?.addEventListener("click", () => {
    void startExtractionJob();
  });

  appRoot.querySelector<HTMLButtonElement>("#chd-close")?.addEventListener("click", () => {
    state.chdDialog = null;
    render();
  });
  appRoot.querySelector<HTMLInputElement>("#chd-delete-originals")?.addEventListener("change", (event) => {
    if (state.chdDialog) {
      state.chdDialog.options.deleteOriginals = (event.currentTarget as HTMLInputElement).checked;
    }
  });
  appRoot.querySelector<HTMLInputElement>("#chd-name-as-container")?.addEventListener("change", (event) => {
    if (state.chdDialog) {
      state.chdDialog.options.nameAsContainer = (event.currentTarget as HTMLInputElement).checked;
    }
  });
  appRoot.querySelector<HTMLInputElement>("#chd-deposit-parent")?.addEventListener("change", (event) => {
    if (!state.chdDialog) {
      return;
    }
    state.chdDialog.options.depositToParent = (event.currentTarget as HTMLInputElement).checked;
    render();
  });
  appRoot.querySelector<HTMLInputElement>("#chd-delete-subfolders")?.addEventListener("change", (event) => {
    if (state.chdDialog) {
      state.chdDialog.options.deleteOriginalSubfolders = (event.currentTarget as HTMLInputElement).checked;
    }
  });
  appRoot.querySelector<HTMLInputElement>("#chd-overwrite")?.addEventListener("change", (event) => {
    if (state.chdDialog) {
      state.chdDialog.options.overwrite = (event.currentTarget as HTMLInputElement).checked;
    }
  });
  appRoot.querySelector<HTMLButtonElement>("#chd-run")?.addEventListener("click", () => {
    void startChdJob();
  });

  appRoot.querySelector<HTMLButtonElement>("#chd-restore-close")?.addEventListener("click", () => {
    state.chdRestoreDialog = null;
    render();
  });
  appRoot.querySelector<HTMLInputElement>("#restore-individual")?.addEventListener("change", (event) => {
    if (state.chdRestoreDialog) {
      state.chdRestoreDialog.options.individualFolders = (event.currentTarget as HTMLInputElement).checked;
    }
  });
  appRoot.querySelector<HTMLInputElement>("#restore-custom-destination")?.addEventListener("change", (event) => {
    if (!state.chdRestoreDialog) {
      return;
    }
    state.chdRestoreDialog.options.destinationMode = (event.currentTarget as HTMLInputElement).checked
      ? "custom"
      : "same";
    render();
  });
  appRoot.querySelector<HTMLInputElement>("#restore-destination-path")?.addEventListener("input", (event) => {
    if (state.chdRestoreDialog) {
      state.chdRestoreDialog.options.destinationPath = (event.currentTarget as HTMLInputElement).value;
    }
  });
  appRoot.querySelector<HTMLButtonElement>("#restore-pick-path")?.addEventListener("click", () => {
    void pickRestoreDestination();
  });
  appRoot.querySelector<HTMLInputElement>("#restore-delete-chd")?.addEventListener("change", (event) => {
    if (state.chdRestoreDialog) {
      state.chdRestoreDialog.options.deleteChd = (event.currentTarget as HTMLInputElement).checked;
    }
  });
  appRoot.querySelector<HTMLInputElement>("#restore-overwrite")?.addEventListener("change", (event) => {
    if (state.chdRestoreDialog) {
      state.chdRestoreDialog.options.overwrite = (event.currentTarget as HTMLInputElement).checked;
    }
  });
  appRoot.querySelector<HTMLInputElement>("#restore-split-bin")?.addEventListener("change", (event) => {
    if (state.chdRestoreDialog) {
      state.chdRestoreDialog.options.splitBin = (event.currentTarget as HTMLInputElement).checked;
    }
  });
  appRoot.querySelector<HTMLButtonElement>("#restore-run")?.addEventListener("click", () => {
    void startRestoreJob();
  });

  appRoot.querySelector<HTMLButtonElement>("#about-close")?.addEventListener("click", () => {
    state.aboutOpen = false;
    render();
  });

  appRoot.querySelector<HTMLButtonElement>("#settings-close")?.addEventListener("click", () => {
    state.settingsOpen = false;
    render();
  });
  appRoot.querySelector<HTMLInputElement>("#settings-font-scale")?.addEventListener("input", (event) => {
    state.settings.fontScale = Number((event.currentTarget as HTMLInputElement).value);
    updateSettingsLabels();
    applyVisualSettings();
  });
  appRoot.querySelector<HTMLInputElement>("#settings-default-content-zoom")?.addEventListener("input", (event) => {
    state.settings.defaultContentZoom = Number((event.currentTarget as HTMLInputElement).value);
    updateSettingsLabels();
  });
  appRoot.querySelector<HTMLInputElement>("#settings-default-tree-zoom")?.addEventListener("input", (event) => {
    state.settings.defaultTreeZoom = Number((event.currentTarget as HTMLInputElement).value);
    updateSettingsLabels();
  });
  appRoot.querySelector<HTMLInputElement>("#settings-selection-weight-all")?.addEventListener("change", (event) => {
    const checked = (event.currentTarget as HTMLInputElement).checked;
    state.settings.selectionWeightCalculateAll = checked;
    const depthInput = appRoot.querySelector<HTMLInputElement>("#settings-selection-weight-depth");
    if (depthInput) {
      depthInput.disabled = checked;
    }
  });
  appRoot.querySelector<HTMLButtonElement>("#settings-reset")?.addEventListener("click", () => {
    state.settings = cloneDefaultSettings();
    state.viewMode = state.settings.defaultViewMode;
    state.zoom = state.settings.defaultContentZoom;
    state.treeZoom = state.settings.defaultTreeZoom;
    reapplyContentOrdering(false);
    void refreshSelectionSummary([...state.selectedContentPaths]);
    applyVisualSettings();
    render();
  });
  appRoot.querySelector<HTMLButtonElement>("#settings-apply")?.addEventListener("click", () => {
    applySettingsFromDialog();
    saveSettings();
    void refreshSelectionSummary([...state.selectedContentPaths]);
    applyVisualSettings();
    render();
  });
  appRoot.querySelector<HTMLButtonElement>("#settings-save")?.addEventListener("click", () => {
    applySettingsFromDialog();
    saveSettings();
    void refreshSelectionSummary([...state.selectedContentPaths]);
    state.settingsOpen = false;
    applyVisualSettings();
    render();
  });

  appRoot.querySelector<HTMLButtonElement>("#connections-close")?.addEventListener("click", () => {
    void WebviewWindow.getCurrent().close();
  });
  appRoot.querySelector<HTMLButtonElement>("#connection-new")?.addEventListener("click", () => {
    startNewConnectionDraft();
  });
  appRoot.querySelectorAll<HTMLElement>("[data-connection-profile]").forEach((button) => {
    button.addEventListener("click", (event) => {
      const profileId = (event.currentTarget as HTMLElement).dataset.connectionProfile;
      if (profileId) {
        selectConnectionProfile(profileId);
      }
    });
  });
  appRoot.querySelectorAll<HTMLElement>("[data-disconnect-session]").forEach((button) => {
    button.addEventListener("click", (event) => {
      event.stopPropagation();
      const sessionId = (event.currentTarget as HTMLElement).dataset.disconnectSession;
      if (sessionId) {
        void disconnectSession(sessionId);
      }
    });
  });
  appRoot.querySelectorAll<HTMLElement>("[data-session-root]").forEach((button) => {
    button.addEventListener("click", (event) => {
      event.stopPropagation();
      const rootPath = (event.currentTarget as HTMLElement).dataset.sessionRoot;
      if (!rootPath) {
        return;
      }
      void emit(ACTIVATE_LOCATION_EVENT, { rootPath });
      void focusMainWindow();
    });
  });
  appRoot.querySelectorAll<HTMLElement>("[data-profile-test]").forEach((button) => {
    button.addEventListener("click", (event) => {
      event.stopPropagation();
      const profileId = (event.currentTarget as HTMLElement).dataset.profileTest;
      if (profileId) {
        void testProfile(profileId);
      }
    });
  });
  appRoot.querySelectorAll<HTMLElement>("[data-profile-connect]").forEach((button) => {
    button.addEventListener("click", (event) => {
      event.stopPropagation();
      const profileId = (event.currentTarget as HTMLElement).dataset.profileConnect;
      if (profileId) {
        void connectProfile(profileId);
      }
    });
  });
  appRoot.querySelector<HTMLButtonElement>("#connection-save")?.addEventListener("click", () => {
    void saveConnectionDraft();
  });
  appRoot.querySelector<HTMLButtonElement>("#connection-test")?.addEventListener("click", () => {
    void testConnectionDraft();
  });
  appRoot.querySelector<HTMLButtonElement>("#connection-connect")?.addEventListener("click", () => {
    if (state.editingConnectionProfileId) {
      void connectProfile(state.editingConnectionProfileId);
    }
  });
  appRoot.querySelector<HTMLButtonElement>("#connection-delete")?.addEventListener("click", () => {
    if (state.editingConnectionProfileId) {
      void deleteConnectionProfileById(state.editingConnectionProfileId);
    }
  });
  bindConnectionDraftInputs();
}

function bindSplitter(): void {
  const splitter = appRoot.querySelector<HTMLDivElement>("#splitter");
  if (!splitter) {
    return;
  }
  splitter.addEventListener("pointerdown", (event) => {
    event.preventDefault();
    document.body.classList.add("dragging-splitter");
    splitter.setPointerCapture(event.pointerId);
    const moveHandler = (moveEvent: PointerEvent) => {
      const min = 220;
      const max = Math.max(min + 80, window.innerWidth - 260);
      state.treeWidth = clamp(moveEvent.clientX, min, max);
      const workspace = appRoot.querySelector<HTMLElement>(".workspace");
      if (workspace) {
        workspace.style.gridTemplateColumns = `${state.treeWidth}px 6px minmax(0, 1fr)`;
      }
    };
    const upHandler = () => {
      document.body.classList.remove("dragging-splitter");
      window.removeEventListener("pointermove", moveHandler);
      window.removeEventListener("pointerup", upHandler);
      window.removeEventListener("pointercancel", upHandler);
    };
    window.addEventListener("pointermove", moveHandler);
    window.addEventListener("pointerup", upHandler);
    window.addEventListener("pointercancel", upHandler);
  });
}

async function chooseRootDirectory(): Promise<void> {
  const selected = await open({
    directory: true,
    multiple: false,
    title: "Selecciona la carpeta base de tus ROMs",
  });
  if (!selected || Array.isArray(selected)) {
    return;
  }
  await setRootAndNavigate(selected);
}

function ensureLocalLocation(path: string): void {
  if (isRemotePath(path)) {
    return;
  }
  if (!state.localLocations.includes(path)) {
    state.localLocations = [...state.localLocations, path];
    state.settings.localLocations = [...state.localLocations];
    saveSettings();
  }
}

async function setRootAndNavigate(rootPath: string, explicitSelection: string[] = []): Promise<void> {
  if (!isRemotePath(rootPath)) {
    ensureLocalLocation(rootPath);
  }
  state.rootPath = rootPath;
  state.treeScrollTop = 0;
  state.contentScrollTop = 0;
  state.contentScrollLeft = 0;
  state.treeChildren.clear();
  state.expandedTreePaths = new Set([rootPath]);
  state.selectedTreePath = rootPath;
  await loadTreeChildren(rootPath);
  await loadContent(rootPath, explicitSelection);
}

async function navigateToPath(path: string): Promise<void> {
  const trimmed = resolveInputPath(path.trim());
  if (!trimmed) {
    return;
  }
  const entry = await withBusy("Abriendo ruta...", () => invoke<EntryDto>("inspect_path", { path: trimmed }));
  const targetDir = entry.isDir ? entry.path : parentPath(entry.path);
  if (!targetDir) {
    showNotice("error", "No se pudo navegar a la ruta indicada.");
    return;
  }

  if (!state.rootPath || !isSubPath(targetDir, state.rootPath)) {
    await setRootAndNavigate(targetDir, entry.isDir ? [] : [entry.path]);
    return;
  }

  await expandTreeToPath(targetDir);
  state.selectedTreePath = targetDir;
  await loadContent(targetDir, entry.isDir ? [] : [entry.path]);
}

async function goToPathInput(): Promise<void> {
  try {
    await navigateToPath(state.pathInput);
  } catch (error) {
    showNotice("error", formatError(error));
  }
}

async function navigateUp(): Promise<void> {
  if (!state.currentPath) {
    return;
  }
  const parent = parentPath(state.currentPath);
  if (!parent) {
    return;
  }
  await navigateToPath(parent);
}

async function expandTreeToPath(targetPath: string): Promise<void> {
  if (!state.rootPath || !isSubPath(targetPath, state.rootPath)) {
    return;
  }
  const chain = buildPathChain(state.rootPath, targetPath);
  for (const path of chain) {
    state.expandedTreePaths.add(path);
    await loadTreeChildren(path, false);
  }
}

async function toggleTreePath(path: string): Promise<void> {
  captureScrollState();
  if (state.expandedTreePaths.has(path)) {
    state.expandedTreePaths.delete(path);
    render();
    return;
  }
  state.expandedTreePaths.add(path);
  await loadTreeChildren(path);
}

async function selectTreePath(path: string): Promise<void> {
  captureScrollState();
  state.selectedTreePath = path;
  const entry = findTreeEntry(path);
  if (entry?.isDir || path === state.rootPath) {
    await loadTreeChildren(path);
    await loadContent(path);
    return;
  }
  const parent = parentPath(path);
  if (parent) {
    await loadContent(parent, [path]);
  }
}

async function loadTreeChildren(path: string, shouldRender = true): Promise<void> {
  const children = await withBusy("Cargando arbol...", () => invoke<EntryDto[]>("list_children", { path }));
  state.treeChildren.set(path, children);
  if (shouldRender) {
    render();
  }
}

async function loadContent(path: string, selection: string[] = []): Promise<void> {
  const shouldResetScroll = state.currentPath !== path || state.mode !== "browse";
  const entries = await withBusy("Cargando carpeta...", () => invoke<EntryDto[]>("list_children", { path }));
  state.currentPath = path;
  state.pathInput = displayPathForInput(path);
  state.currentEntries = applyContentOrdering(entries);
  if (shouldResetScroll) {
    state.contentScrollTop = 0;
    state.contentScrollLeft = 0;
  }
  state.selectedContentPaths = selection.filter((item) => entries.some((entry) => entry.path === item));
  state.anchorPath = state.selectedContentPaths[0] ?? null;
  void refreshSelectionSummary([...state.selectedContentPaths]);
  state.rename = null;
  state.mode = "browse";
  state.modeLabel = "";
  render();
}

async function runGlobalSearch(): Promise<void> {
  if (!state.rootPath) {
    showNotice("error", "Selecciona primero una carpeta base.");
    return;
  }
  const query = state.topSearchQuery.trim();
  if (!query) {
    showNotice("info", "Escribe un texto para la busqueda global.");
    return;
  }
  const results = await withBusy("Buscando en todo el arbol...", () =>
    invoke<EntryDto[]>("search_entries", { path: state.rootPath, query, recursive: true }),
  );
  state.currentEntries = applyContentOrdering(results);
  state.contentScrollTop = 0;
  state.contentScrollLeft = 0;
  state.selectedContentPaths = [];
  state.anchorPath = null;
  void refreshSelectionSummary([]);
  state.rename = null;
  state.mode = "global-search";
  state.modeLabel = `Busqueda global: ${query}`;
  render();
}

async function runLocalSearch(): Promise<void> {
  if (!state.currentPath) {
    showNotice("error", "No hay carpeta activa para la busqueda local.");
    return;
  }
  const query = state.localSearchQuery.trim();
  if (!query) {
    showNotice("info", "Escribe un texto para la busqueda local.");
    return;
  }
  const results = await withBusy("Buscando dentro de la carpeta actual...", () =>
    invoke<EntryDto[]>("search_entries", {
      path: state.currentPath,
      query,
      recursive: state.localSearchRecursive,
    }),
  );
  state.currentEntries = applyContentOrdering(results);
  state.contentScrollTop = 0;
  state.contentScrollLeft = 0;
  state.selectedContentPaths = [];
  state.anchorPath = null;
  void refreshSelectionSummary([]);
  state.rename = null;
  state.mode = "local-search";
  state.modeLabel = `Busqueda local: ${query}${state.localSearchRecursive ? " (recursiva)" : ""}`;
  render();
}

async function clearSearchMode(): Promise<void> {
  state.topSearchQuery = "";
  if (!state.currentPath) {
    state.mode = "browse";
    state.modeLabel = "";
    void refreshSelectionSummary([]);
    render();
    return;
  }
  await loadContent(state.currentPath);
}

async function handleMenuAction(action: string): Promise<void> {
  switch (action) {
    case "select-root":
      await chooseRootDirectory();
      return;
    case "go-up":
      await navigateUp();
      return;
    case "refresh-current":
      await refreshVisibleData();
      return;
    case "create-folder":
      await createFolder();
      return;
    case "create-file":
      await createFile();
      return;
    case "open-settings":
      state.settingsOpen = true;
      render();
      return;
    case "open-local-search":
      state.localSearchOpen = true;
      render();
      return;
    case "paste-current":
      await pasteInto(state.currentPath);
      return;
    case "about":
      state.aboutOpen = true;
      render();
      return;
    default:
      return;
  }
}

async function handleContentClick(event: MouseEvent, row: HTMLElement): Promise<void> {
  const path = row.dataset.contentPath;
  if (!path) {
    return;
  }
  if (event.shiftKey && state.anchorPath) {
    setContentSelection(computeRangeSelection(state.anchorPath, path), state.anchorPath, true);
  } else if (event.ctrlKey || event.metaKey) {
    setContentSelection(toggleSelection(state.selectedContentPaths, path), path, true);
  } else {
    setContentSelection([path], path, true);
  }
}

function bindContentMarqueeSelection(contentScroll: HTMLDivElement | null): void {
  if (!contentScroll) {
    return;
  }

  let dragPointerId: number | null = null;
  let dragging = false;
  let startClientX = 0;
  let startClientY = 0;
  let startScrollLeft = 0;
  let startScrollTop = 0;
  let lastClientX = 0;
  let lastClientY = 0;
  let baseSelection = new Set<string>();
  let marquee: HTMLDivElement | null = null;
  let autoScrollFrame = 0;

  const cleanup = (shouldRender: boolean) => {
    dragPointerId = null;
    if (marquee) {
      marquee.remove();
      marquee = null;
    }
    if (autoScrollFrame) {
      window.cancelAnimationFrame(autoScrollFrame);
      autoScrollFrame = 0;
    }
    contentScroll.classList.remove("marquee-active");
    document.body.classList.remove("dragging-selection");
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
    window.removeEventListener("pointercancel", onPointerUp);
    if (shouldRender) {
      render();
    }
  };

  const ensureMarquee = () => {
    if (marquee) {
      return marquee;
    }
    marquee = document.createElement("div");
    marquee.className = "selection-marquee";
    contentScroll.appendChild(marquee);
    contentScroll.classList.add("marquee-active");
    document.body.classList.add("dragging-selection");
    return marquee;
  };

  const updatePreviewSelection = (selection: string[]) => {
    const selectedSet = new Set(selection);
    state.selectedContentPaths = state.currentEntries
      .filter((entry) => selectedSet.has(entry.path))
      .map((entry) => entry.path);
    state.anchorPath = state.selectedContentPaths[0] ?? null;
    contentScroll.querySelectorAll<HTMLElement>("[data-content-path]").forEach((row) => {
      const path = row.dataset.contentPath ?? "";
      row.classList.toggle("selected", selectedSet.has(path));
    });
    contentScroll.querySelectorAll<HTMLInputElement>("[data-entry-check]").forEach((checkbox) => {
      const path = checkbox.dataset.entryCheck ?? "";
      checkbox.checked = selectedSet.has(path);
    });
    const selectAll = appRoot.querySelector<HTMLInputElement>("#content-select-all");
    if (selectAll) {
      selectAll.checked = state.selectedContentPaths.length > 0 && state.selectedContentPaths.length === state.currentEntries.length;
      selectAll.indeterminate =
        state.selectedContentPaths.length > 0 && state.selectedContentPaths.length < state.currentEntries.length;
    }
  };

  const updateDrag = () => {
    const scrollRect = contentScroll.getBoundingClientRect();
    const currentX = lastClientX - scrollRect.left + contentScroll.scrollLeft;
    const currentY = lastClientY - scrollRect.top + contentScroll.scrollTop;
    const startX = startClientX - scrollRect.left + startScrollLeft;
    const startY = startClientY - scrollRect.top + startScrollTop;
    const left = Math.min(startX, currentX);
    const top = Math.min(startY, currentY);
    const width = Math.abs(currentX - startX);
    const height = Math.abs(currentY - startY);
    const box = ensureMarquee();
    box.style.left = `${left}px`;
    box.style.top = `${top}px`;
    box.style.width = `${width}px`;
    box.style.height = `${height}px`;

    const preview = new Set(baseSelection);
    const selectionRect = {
      left,
      top,
      right: left + width,
      bottom: top + height,
    };

    contentScroll.querySelectorAll<HTMLElement>("[data-content-path]").forEach((row) => {
      const path = row.dataset.contentPath;
      if (!path) {
        return;
      }
      const rect = row.getBoundingClientRect();
      const rowRect = {
        left: rect.left - scrollRect.left + contentScroll.scrollLeft,
        top: rect.top - scrollRect.top + contentScroll.scrollTop,
        right: rect.right - scrollRect.left + contentScroll.scrollLeft,
        bottom: rect.bottom - scrollRect.top + contentScroll.scrollTop,
      };
      const intersects =
        rowRect.right >= selectionRect.left &&
        rowRect.left <= selectionRect.right &&
        rowRect.bottom >= selectionRect.top &&
        rowRect.top <= selectionRect.bottom;
      if (intersects) {
        preview.add(path);
      }
    });

    updatePreviewSelection([...preview]);
  };

  const onPointerMove = (event: PointerEvent) => {
    if (dragPointerId !== event.pointerId) {
      return;
    }
    lastClientX = event.clientX;
    lastClientY = event.clientY;
    if (!dragging) {
      const threshold = Math.abs(lastClientX - startClientX) + Math.abs(lastClientY - startClientY);
      if (threshold < 4) {
        return;
      }
      dragging = true;
      if (!autoScrollFrame) {
        autoScrollFrame = window.requestAnimationFrame(autoScrollTick);
      }
    }
    event.preventDefault();
    updateDrag();
  };

  const onScrollDuringSelection = () => {
    state.contentScrollTop = contentScroll.scrollTop;
    state.contentScrollLeft = contentScroll.scrollLeft;
    if (dragPointerId !== null) {
      updateDrag();
    }
  };

  const autoScrollTick = () => {
    if (!dragging || dragPointerId === null) {
      autoScrollFrame = 0;
      return;
    }
    const rect = contentScroll.getBoundingClientRect();
    const edge = 52;
    let deltaY = 0;
    if (lastClientY < rect.top + edge) {
      deltaY = -Math.max(8, Math.min(24, Math.ceil((rect.top + edge - lastClientY) / 2)));
    } else if (lastClientY > rect.bottom - edge) {
      deltaY = Math.max(8, Math.min(24, Math.ceil((lastClientY - (rect.bottom - edge)) / 2)));
    }
    if (deltaY !== 0) {
      const nextTop = clamp(contentScroll.scrollTop + deltaY, 0, contentScroll.scrollHeight - contentScroll.clientHeight);
      if (nextTop !== contentScroll.scrollTop) {
        contentScroll.scrollTop = nextTop;
        state.contentScrollTop = nextTop;
        updateDrag();
      }
    }
    autoScrollFrame = window.requestAnimationFrame(autoScrollTick);
  };

  const onPointerUp = (event: PointerEvent) => {
    if (dragPointerId !== event.pointerId) {
      return;
    }
    const didDrag = dragging;
    dragging = false;
    if (didDrag) {
      void refreshSelectionSummary([...state.selectedContentPaths]);
    }
    cleanup(didDrag);
  };

  contentScroll.addEventListener("scroll", onScrollDuringSelection);

  contentScroll.addEventListener("pointerdown", (event) => {
    const target = event.target as HTMLElement;
    if (event.button !== 0) {
      return;
    }
    const gutterStart =
      (target.closest(".cell-pad-start") || target.closest(".cell-check") || target.closest(".cell-pad-end")) &&
      !target.closest("[data-entry-check], button, input, label");
    if (
      !gutterStart &&
      target.closest("[data-content-path], [data-parent-entry], [data-entry-check], .inline-rename, button, input, label")
    ) {
      return;
    }
    dragPointerId = event.pointerId;
    startClientX = event.clientX;
    startClientY = event.clientY;
    startScrollLeft = contentScroll.scrollLeft;
    startScrollTop = contentScroll.scrollTop;
    lastClientX = event.clientX;
    lastClientY = event.clientY;
    dragging = false;
    baseSelection = new Set(event.ctrlKey || event.metaKey ? state.selectedContentPaths : []);
    contentScroll.setPointerCapture(event.pointerId);
    window.addEventListener("pointermove", onPointerMove);
    window.addEventListener("pointerup", onPointerUp);
    window.addEventListener("pointercancel", onPointerUp);
  });
}

async function openPathFromRow(row: HTMLElement): Promise<void> {
  const path = row.dataset.contentPath;
  const kind = row.dataset.contentKind;
  if (!path) {
    return;
  }
  if (kind === "dir") {
    await navigateToPath(path);
    return;
  }
  await invoke("open_path", { path });
}

function handleGlobalKeyDown(event: KeyboardEvent): void {
  const target = event.target as HTMLElement | null;
  if (target && ["INPUT", "TEXTAREA"].includes(target.tagName)) {
    return;
  }
  if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "f") {
    event.preventDefault();
    state.localSearchOpen = true;
    render();
    return;
  }
  if (event.key === "F2" && state.selectedContentPaths.length === 1) {
    event.preventDefault();
    void beginRename(state.selectedContentPaths[0]);
    return;
  }
  if (!state.currentEntries.length || event.key.length !== 1 || event.altKey || event.ctrlKey || event.metaKey) {
    return;
  }
  const now = Date.now();
  state.typeSelectBuffer = now - state.typeSelectTimestamp > 900 ? event.key : `${state.typeSelectBuffer}${event.key}`;
  state.typeSelectTimestamp = now;
  const found = state.currentEntries.find((entry) =>
    entry.name.toLowerCase().startsWith(state.typeSelectBuffer.toLowerCase()),
  );
  if (found) {
    setContentSelection([found.path], found.path, true);
  }
}

function selectByContext(path: string): void {
  if (!state.selectedContentPaths.includes(path)) {
    setContentSelection([path], path, true);
  }
}

async function openContextMenu(
  x: number,
  y: number,
  requestedPaths: string[],
  destinationPath: string | null,
  preferRequestedPaths = false,
): Promise<void> {
  const paths = preferRequestedPaths
    ? requestedPaths
    : requestedPaths.length > 0
      ? requestedPaths
      : state.selectedContentPaths;
  const canUseLocalFileTools = paths.length > 0 && paths.every((path) => getEntryLocationKind(path) === "local");
  const analysis = canUseLocalFileTools ? await invoke<SelectionAnalysisDto>("scan_selection", { paths }) : null;

  const actions: ContextAction[] = [
    { id: "open", label: "Abrir", icon: "open", disabled: paths.length !== 1 },
    { id: "open-with", label: "Abrir con editor", icon: "edit-open", disabled: paths.length !== 1 },
    { id: "copy", label: "Copiar", icon: "copy", disabled: paths.length === 0 },
    { id: "cut", label: "Cortar", icon: "cut", disabled: paths.length === 0 },
    { id: "paste", label: "Pegar", icon: "paste", disabled: !state.clipboard || !(destinationPath ?? state.currentPath) },
    { id: "rename", label: "Renombrar", icon: "rename", disabled: paths.length !== 1 },
    { id: "delete", label: "Eliminar", icon: "trash", disabled: paths.length === 0, danger: true },
    { id: "properties", label: "Propiedades", icon: "info", disabled: paths.length === 0 },
  ];

  if (paths.length === 0 && destinationPath) {
    actions.push({ id: "create-folder", label: "Crear carpeta", icon: "folder-plus" });
    actions.push({ id: "create-file", label: "Crear fichero", icon: "file-plus" });
  }

  if (analysis && analysis.archives.length > 0) {
    actions.push({ id: "extract-here", label: "Descomprimir aqui", icon: "unarchive" });
    actions.push({ id: "extract-advanced", label: "Descomprimir...", icon: "unarchive" });
  }
  if (analysis?.chdMenuLabel) {
    actions.push({ id: "convert-chd", label: analysis.chdMenuLabel, icon: "disc" });
  }
  if (analysis?.chdRestoreMenuLabel) {
    actions.push({ id: "restore-chd", label: analysis.chdRestoreMenuLabel, icon: "restore-disc" });
  }

  state.contextMenu = { x, y, actions, paths, destinationPath };
  render();
}

async function handleContextAction(action: string): Promise<void> {
  const menu = state.contextMenu;
  state.contextMenu = null;
  render();
  const paths = menu?.paths ?? state.selectedContentPaths;
  switch (action) {
    case "open":
      await openSingleSelection(paths);
      return;
    case "open-with":
      if (paths.length === 1) {
        await invoke("open_with_dialog", { path: paths[0] });
      }
      return;
    case "copy":
      state.clipboard = { paths: [...paths], operation: "copy" };
      render();
      return;
    case "cut":
      state.clipboard = { paths: [...paths], operation: "cut" };
      render();
      return;
    case "paste":
      await pasteInto(menu?.destinationPath ?? state.currentPath);
      return;
    case "create-folder":
      await createFolder();
      return;
    case "create-file":
      await createFile();
      return;
    case "rename":
      await beginRename(paths[0] ?? null);
      return;
    case "delete":
      await deleteSelection(paths);
      return;
    case "properties":
      await openPropertiesDialog(paths);
      return;
    case "extract-here":
    case "extract-advanced": {
      if (!paths.every((path) => getEntryLocationKind(path) === "local")) {
        showNotice("info", "Las herramientas de extraccion solo estan disponibles sobre ficheros locales.");
        return;
      }
      const analysis = await invoke<SelectionAnalysisDto>("scan_selection", { paths });
      state.extractionDialog = {
        paths: analysis.archives,
        options: {
          individualFolders: false,
          destinationMode: "same",
          destinationPath: null,
          deleteArchives: false,
          overwrite: false,
        },
        previewRows: [],
      };
      if (action === "extract-here") {
        await startExtractionJob();
      } else {
        render();
      }
      return;
    }
    case "convert-chd":
      await openChdDialog(paths);
      return;
    case "restore-chd":
      await openChdRestoreDialog(paths);
      return;
    default:
      return;
  }
}

async function openSingleSelection(paths: string[] = state.selectedContentPaths): Promise<void> {
  if (paths.length !== 1) {
    return;
  }
  try {
    await navigateToPath(paths[0]);
    const entry = state.currentEntries.find((item) => item.path === paths[0]);
    if (!entry || entry.isDir) {
      return;
    }
    await invoke("open_path", { path: paths[0] });
  } catch {
    await invoke("open_path", { path: paths[0] });
  }
}

async function pasteInto(destination: string | null): Promise<void> {
  if (!state.clipboard || !destination) {
    showNotice("error", "No hay una carpeta destino valida para pegar.");
    return;
  }
  await startTransferJob(state.clipboard.paths, destination, state.clipboard.operation);
  state.clipboard = null;
}

async function moveDraggedSelection(destination: string | null): Promise<void> {
  if (!destination || draggingContentPaths.length === 0) {
    return;
  }
  await startTransferJob(draggingContentPaths, destination, "cut");
  draggingContentPaths = [];
}

async function startTransferJob(paths: string[], destination: string, operation: "copy" | "cut"): Promise<void> {
  const jobId = crypto.randomUUID();
  state.progress = {
    jobId,
    title: operation === "cut" ? "Moviendo elementos" : "Copiando elementos",
    progress: 0,
    message: "Preparando...",
    logs: [],
    done: false,
    success: false,
    resultMessage: "",
  };
  render();
  await invoke("start_copy_or_move_paths", {
    jobId,
    paths,
    destination,
    operation,
    overwrite: true,
  });
}

async function deleteSelection(paths: string[]): Promise<void> {
  if (paths.length === 0) {
    return;
  }
  if (state.settings.confirmDelete && !window.confirm(`Se eliminaran ${paths.length} elemento(s). Continuar?`)) {
    return;
  }
  const deletionBehavior = await determineDeleteBehavior(paths);
  const jobId = crypto.randomUUID();
  state.progress = {
    jobId,
    title: "Eliminando elementos",
    progress: 0,
    message: "Preparando...",
    logs: [],
    done: false,
    success: false,
    resultMessage: "",
    showDialog: deletionBehavior.showDialog,
    statusMessageOnSuccess: deletionBehavior.successMessage,
    statusMessageOnFailure: deletionBehavior.failureMessage,
  };
  render();
  await invoke("start_delete_paths", { jobId, paths });
}

async function determineDeleteBehavior(
  paths: string[],
): Promise<{ showDialog: boolean; successMessage: string; failureMessage: string }> {
  if (paths.length !== 1) {
    return {
      showDialog: !state.settings.hideDeleteProgressPopup,
      successMessage: `${paths.length} elementos eliminados.`,
      failureMessage: "No se pudieron eliminar algunos elementos.",
    };
  }

  const path = paths[0];
  let entry =
    state.currentEntries.find((item) => item.path === path) ??
    findTreeEntry(path);
  if (!entry) {
    try {
      entry = await invoke<EntryDto>("inspect_path", { path });
    } catch {
      entry = undefined;
    }
  }

  const simpleDelete = Boolean(entry && (!entry.isDir || !entry.hasChildren));
  const showDialog = state.settings.hideDeleteProgressPopup ? false : !simpleDelete;
  const label = entry?.name ?? leafName(path) ?? "Elemento";
  return {
    showDialog,
    successMessage: entry?.isDir ? `Carpeta eliminada: ${label}` : `Fichero eliminado: ${label}`,
    failureMessage: `No se pudo eliminar: ${label}`,
  };
}

async function openPropertiesDialog(paths: string[]): Promise<void> {
  if (paths.length === 0) {
    return;
  }
  const requestId = Date.now();
  state.propertiesDialog = await withBusy("Calculando propiedades...", () =>
    invoke<PropertiesSummaryDto>("summarize_paths", {
      requestId,
      paths,
      options: {
        maxDepth: state.settings.selectionWeightCalculateAll ? null : state.settings.selectionWeightMaxDepth,
      } satisfies SummaryOptionsPayload,
    }),
  );
  render();
}

async function createProvisionalEntry(kind: "folder" | "file"): Promise<string | null> {
  if (!state.currentPath) {
    return null;
  }
  const existingNames = new Set(state.currentEntries.map((entry) => entry.name.toLowerCase()));
  const baseName = kind === "folder" ? "Nueva carpeta" : "Nuevo fichero";
  const extension = kind === "file" ? ".txt" : "";
  for (let attempt = 1; attempt <= 500; attempt += 1) {
    const suffix = attempt === 1 ? "" : ` (${attempt})`;
    const candidate = `${baseName}${suffix}${extension}`;
    if (existingNames.has(candidate.toLowerCase())) {
      continue;
    }
    try {
      if (kind === "folder") {
        return await invoke<string>("create_folder", { parent: state.currentPath, name: candidate });
      }
      return await invoke<string>("create_file", { parent: state.currentPath, name: candidate });
    } catch {
      continue;
    }
  }
  showNotice("error", `No se pudo crear ${kind === "folder" ? "la carpeta" : "el fichero"} nuevo.`);
  return null;
}

async function createFolder(): Promise<void> {
  if (!state.currentPath) {
    showNotice("error", "No hay carpeta activa.");
    return;
  }
  const createdPath = await createProvisionalEntry("folder");
  if (!createdPath) {
    return;
  }
  await refreshVisibleData();
  await beginRename(createdPath);
}

async function createFile(): Promise<void> {
  if (!state.currentPath) {
    showNotice("error", "No hay carpeta activa.");
    return;
  }
  const createdPath = await createProvisionalEntry("file");
  if (!createdPath) {
    return;
  }
  await refreshVisibleData();
  await beginRename(createdPath);
}

async function beginRename(path: string | null): Promise<void> {
  if (!path) {
    return;
  }
  clearPendingRenameCommit();
  if (!state.currentEntries.some((entry) => entry.path === path)) {
    const parent = parentPath(path);
    if (!parent) {
      return;
    }
    await navigateToPath(parent);
    setContentSelection([path], path, false);
  }
  const entry = state.currentEntries.find((item) => item.path === path);
  if (!entry) {
    return;
  }
  setContentSelection([path], path, false);
  state.rename = { path, value: entry.name };
  render();
}

function cancelRename(): void {
  clearPendingRenameCommit();
  state.rename = null;
  render();
}

function clearPendingRenameCommit(): void {
  if (pendingRenameCommitTimer !== null) {
    window.clearTimeout(pendingRenameCommitTimer);
    pendingRenameCommitTimer = null;
  }
}

function scheduleRenameCommit(path: string, rawValue: string, immediate = false): void {
  clearPendingRenameCommit();
  const run = () => {
    pendingRenameCommitTimer = null;
    void finalizeRename(path, rawValue);
  };
  if (immediate) {
    run();
    return;
  }
  pendingRenameCommitTimer = window.setTimeout(run, 0);
}

async function finalizeRename(path: string, rawValue: string): Promise<void> {
  const currentName = leafName(path);
  const newName = rawValue.trim();
  if (state.rename?.path === path) {
    state.rename = null;
    render();
  }
  if (!newName || newName === currentName) {
    return;
  }
  try {
    const renamedPath = await invoke<string>("rename_path", { path, newName });
    const currentDirectory = state.currentPath;
    if (currentDirectory && parentPath(renamedPath) === currentDirectory) {
      await loadContent(currentDirectory, [renamedPath]);
      return;
    }
    await refreshVisibleData();
  } catch (error) {
    showNotice("error", formatError(error));
    await refreshVisibleData();
  }
}

async function openChdDialog(paths: string[]): Promise<void> {
  if (!paths.every((path) => getEntryLocationKind(path) === "local")) {
    showNotice("info", "La conversion CHD solo esta disponible sobre selecciones locales.");
    return;
  }
  const analysis = await invoke<SelectionAnalysisDto>("scan_selection", { paths });
  state.chdDialog = {
    paths,
    analysis,
    options: {
      deleteOriginals: false,
      nameAsContainer: false,
      depositToParent: false,
      deleteOriginalSubfolders: false,
      overwrite: false,
    },
  };
  render();
}

async function openChdRestoreDialog(paths: string[]): Promise<void> {
  if (!paths.every((path) => getEntryLocationKind(path) === "local")) {
    showNotice("info", "La recuperacion CHD solo esta disponible sobre selecciones locales.");
    return;
  }
  const analysis = await invoke<SelectionAnalysisDto>("scan_selection", { paths });
  state.chdRestoreDialog = {
    paths,
    analysis,
    options: {
      individualFolders: false,
      destinationMode: "same",
      destinationPath: null,
      deleteChd: false,
      overwrite: false,
      splitBin: false,
    },
  };
  render();
}

async function pickExtractionDestination(): Promise<void> {
  if (!state.extractionDialog) {
    return;
  }
  const selected = await open({ directory: true, multiple: false, title: "Selecciona la carpeta destino" });
  if (!selected || Array.isArray(selected)) {
    return;
  }
  state.extractionDialog.options.destinationPath = selected;
  render();
}

async function pickRestoreDestination(): Promise<void> {
  if (!state.chdRestoreDialog) {
    return;
  }
  const selected = await open({ directory: true, multiple: false, title: "Selecciona la carpeta destino" });
  if (!selected || Array.isArray(selected)) {
    return;
  }
  state.chdRestoreDialog.options.destinationPath = selected;
  render();
}

async function loadExtractionPreview(): Promise<void> {
  if (!state.extractionDialog) {
    return;
  }
  state.extractionDialog.previewRows = await invoke<ExtractionPreviewRow[]>("build_extraction_preview", {
    archives: state.extractionDialog.paths,
    options: state.extractionDialog.options,
  });
  render();
}

async function startExtractionJob(): Promise<void> {
  if (!state.extractionDialog) {
    return;
  }
  const { paths, options } = state.extractionDialog;
  const jobId = crypto.randomUUID();
  state.progress = {
    jobId,
    title: "Descomprimiendo archivos",
    progress: 0,
    message: "Preparando...",
    logs: [],
    done: false,
    success: false,
    resultMessage: "",
  };
  state.extractionDialog = null;
  render();
  await invoke("start_extract_archives", { jobId, archives: paths, options });
}

async function startChdJob(): Promise<void> {
  if (!state.chdDialog) {
    return;
  }
  const { paths, options } = state.chdDialog;
  const jobId = crypto.randomUUID();
  state.progress = {
    jobId,
    title: "Convirtiendo a CHD",
    progress: 0,
    message: "Preparando...",
    logs: [],
    done: false,
    success: false,
    resultMessage: "",
  };
  state.chdDialog = null;
  render();
  await invoke("start_convert_to_chd", { jobId, paths, options });
}

async function startRestoreJob(): Promise<void> {
  if (!state.chdRestoreDialog) {
    return;
  }
  const { paths, options } = state.chdRestoreDialog;
  const jobId = crypto.randomUUID();
  state.progress = {
    jobId,
    title: "Recuperando contenido desde CHD",
    progress: 0,
    message: "Preparando...",
    logs: [],
    done: false,
    success: false,
    resultMessage: "",
  };
  state.chdRestoreDialog = null;
  render();
  await invoke("start_restore_from_chd", { jobId, paths, options });
}

async function refreshVisibleData(): Promise<void> {
  if (!state.currentPath) {
    return;
  }
  const currentPath = state.currentPath;
  const expanded = [...state.expandedTreePaths];
  state.treeChildren.clear();
  for (const path of expanded) {
    try {
      const children = await invoke<EntryDto[]>("list_children", { path });
      state.treeChildren.set(path, children);
    } catch {
      continue;
    }
  }
  await loadContent(currentPath, state.selectedContentPaths);
}

async function loadConnectionProfiles(): Promise<void> {
  state.connectionProfiles = await invoke<ConnectionProfileDto[]>("list_connection_profiles");
  if (state.editingConnectionProfileId) {
    const matching = state.connectionProfiles.find((profile) => profile.id === state.editingConnectionProfileId);
    if (matching) {
      state.connectionDraft = profileToDraft(matching);
    }
  }
}

async function loadActiveConnections(): Promise<void> {
  state.activeConnections = await invoke<ActiveConnectionDto[]>("list_active_connections");
  if (state.rootPath && isRemotePath(state.rootPath)) {
    const activeRoots = new Set(state.activeConnections.map((connection) => connection.rootPath));
    if (!activeRoots.has(state.rootPath)) {
      state.rootPath = state.localLocations[0] ?? null;
      state.currentPath = state.localLocations[0] ?? null;
      if (state.localLocations[0]) {
        await setRootAndNavigate(state.localLocations[0]);
      } else {
        state.currentEntries = [];
      }
    }
  }
}

function startNewConnectionDraft(): void {
  state.editingConnectionProfileId = null;
  state.connectionDraft = createEmptyConnectionDraft();
  render();
}

function selectConnectionProfile(profileId: string): void {
  const profile = state.connectionProfiles.find((item) => item.id === profileId);
  if (!profile) {
    return;
  }
  state.editingConnectionProfileId = profile.id;
  state.connectionDraft = profileToDraft(profile);
  render();
}

function profileToDraft(profile: ConnectionProfileDto): ConnectionProfilePayload {
  return {
    id: profile.id,
    label: profile.label,
    protocol: profile.protocol,
    host: profile.host,
    port: profile.port,
    username: profile.username,
    password: profile.password,
    share: profile.share,
    workgroup: profile.workgroup,
    startPath: profile.startPath,
    sshMode: profile.sshMode,
    ftpMode: profile.ftpMode,
    ftpSecureImplicit: profile.ftpSecureImplicit,
    ftpAcceptInvalidCertificates: profile.ftpAcceptInvalidCertificates,
    ftpAcceptInvalidHostnames: profile.ftpAcceptInvalidHostnames,
  };
}

function buildConnectionDraftPayload(): ConnectionProfilePayload {
  return {
    ...state.connectionDraft,
    label: state.connectionDraft.label.trim(),
    host: state.connectionDraft.host.trim(),
    username: state.connectionDraft.username.trim(),
    share: state.connectionDraft.share.trim(),
    workgroup: state.connectionDraft.workgroup.trim(),
    startPath: state.connectionDraft.startPath.trim() || "/",
  };
}

function bindConnectionDraftInputs(): void {
  const bindText = (selector: string, key: keyof ConnectionProfilePayload) => {
    appRoot.querySelector<HTMLInputElement>(selector)?.addEventListener("input", (event) => {
      state.connectionDraft[key] = (event.currentTarget as HTMLInputElement).value as never;
    });
  };
  bindText("#connection-label", "label");
  bindText("#connection-host", "host");
  bindText("#connection-username", "username");
  bindText("#connection-password", "password");
  bindText("#connection-share", "share");
  bindText("#connection-workgroup", "workgroup");
  bindText("#connection-start-path", "startPath");
  appRoot.querySelector<HTMLInputElement>("#connection-port")?.addEventListener("input", (event) => {
    const value = Number((event.currentTarget as HTMLInputElement).value);
    state.connectionDraft.port = Number.isFinite(value) && value > 0 ? value : null;
  });
  appRoot.querySelector<HTMLSelectElement>("#connection-protocol")?.addEventListener("change", (event) => {
    state.connectionDraft.protocol = (event.currentTarget as HTMLSelectElement).value as ConnectionProfilePayload["protocol"];
    if (!state.connectionDraft.id) {
      state.connectionDraft.port = defaultPortForProtocol(state.connectionDraft.protocol);
    }
    render();
  });
  appRoot.querySelector<HTMLSelectElement>("#connection-ssh-mode")?.addEventListener("change", (event) => {
    state.connectionDraft.sshMode = (event.currentTarget as HTMLSelectElement).value as ConnectionProfilePayload["sshMode"];
  });
  appRoot.querySelector<HTMLSelectElement>("#connection-ftp-mode")?.addEventListener("change", (event) => {
    state.connectionDraft.ftpMode = (event.currentTarget as HTMLSelectElement).value as ConnectionProfilePayload["ftpMode"];
  });
  appRoot.querySelector<HTMLInputElement>("#connection-ftps-implicit")?.addEventListener("change", (event) => {
    state.connectionDraft.ftpSecureImplicit = (event.currentTarget as HTMLInputElement).checked;
  });
  appRoot.querySelector<HTMLInputElement>("#connection-ftps-invalid-cert")?.addEventListener("change", (event) => {
    state.connectionDraft.ftpAcceptInvalidCertificates = (event.currentTarget as HTMLInputElement).checked;
  });
  appRoot.querySelector<HTMLInputElement>("#connection-ftps-invalid-host")?.addEventListener("change", (event) => {
    state.connectionDraft.ftpAcceptInvalidHostnames = (event.currentTarget as HTMLInputElement).checked;
  });
}

async function saveConnectionDraft(): Promise<void> {
  try {
    const payload = buildConnectionDraftPayload();
    const saved = await withBusy("Guardando perfil remoto...", () =>
      invoke<ConnectionProfileDto>("save_connection_profile", { profile: payload }),
    );
    await loadConnectionProfiles();
    state.editingConnectionProfileId = saved.id;
    state.connectionDraft = profileToDraft(saved);
    showStatusFlash(`Perfil guardado: ${saved.label}`);
    await emit(CONNECTIONS_CHANGED_EVENT);
    render();
  } catch {
    // withBusy ya informa el error al usuario.
  }
}

async function deleteConnectionProfileById(profileId: string): Promise<void> {
  const profile = state.connectionProfiles.find((item) => item.id === profileId);
  if (profile && !window.confirm(`Se eliminara el perfil "${profile.label}". Continuar?`)) {
    return;
  }
  try {
    await withBusy("Eliminando perfil remoto...", () => invoke("delete_connection_profile", { profileId }));
    await loadConnectionProfiles();
    startNewConnectionDraft();
    showStatusFlash("Perfil eliminado.");
    await emit(CONNECTIONS_CHANGED_EVENT);
  } catch {
    // withBusy ya informa el error al usuario.
  }
}

async function connectProfile(profileId: string, trustCurrentFingerprint = false): Promise<void> {
  try {
    const result = await withBusy("Abriendo conexion remota...", () =>
      invoke<ConnectionOpenResultDto>("connect_connection_profile", {
        profileId,
        trustCurrentFingerprint,
      }),
    );
    if (result.requiresTrust && result.fingerprint) {
      const accepted = window.confirm(
        `La huella SSH del host no esta guardada.\n\n${result.fingerprint}\n\nDeseas confiar en ella y continuar?`,
      );
      if (accepted) {
        await connectProfile(profileId, true);
      }
      return;
    }
    await loadActiveConnections();
    if (result.connection) {
      if (isConnectionsManagerWindow()) {
        await emit(ACTIVATE_LOCATION_EVENT, { rootPath: result.connection.rootPath });
        await focusMainWindow();
      } else {
        await setRootAndNavigate(result.connection.rootPath);
      }
      showStatusFlash(result.message ?? `Conexion abierta: ${result.connection.label}`);
    } else if (result.message) {
      showStatusFlash(result.message);
    }
    await emit(CONNECTIONS_CHANGED_EVENT);
    render();
  } catch {
    // withBusy ya informa el error al usuario.
  }
}

async function testConnectionDraft(trustCurrentFingerprint = false): Promise<void> {
  try {
    const result = await withBusy("Probando borrador remoto...", () =>
      invoke<ConnectionOpenResultDto>("test_connection_profile_payload", {
        profile: buildConnectionDraftPayload(),
        trustCurrentFingerprint,
      }),
    );
    if (result.requiresTrust && result.fingerprint) {
      const accepted = window.confirm(
        `La huella SSH del host no esta guardada.\n\n${result.fingerprint}\n\nDeseas confiar en ella para completar la prueba?`,
      );
      if (accepted) {
        await testConnectionDraft(true);
      }
      return;
    }
    showStatusFlash(result.message ?? "Conexion verificada correctamente.");
  } catch {
    // withBusy ya informa el error al usuario.
  }
}

async function testProfile(profileId: string, trustCurrentFingerprint = false): Promise<void> {
  try {
    const result = await withBusy("Probando conexion remota...", () =>
      invoke<ConnectionOpenResultDto>("test_connection_profile", {
        profileId,
        trustCurrentFingerprint,
      }),
    );
    if (result.requiresTrust && result.fingerprint) {
      const accepted = window.confirm(
        `La huella SSH del host no esta guardada.\n\n${result.fingerprint}\n\nDeseas confiar en ella para completar la prueba?`,
      );
      if (accepted) {
        await testProfile(profileId, true);
      }
      return;
    }
    showStatusFlash(result.message ?? "Conexion verificada correctamente.");
  } catch {
    // withBusy ya informa el error al usuario.
  }
}

async function disconnectSession(sessionId: string): Promise<void> {
  const session = state.activeConnections.find((item) => item.sessionId === sessionId);
  try {
    await withBusy("Cerrando conexion remota...", () => invoke("disconnect_connection", { sessionId }));
    await loadActiveConnections();
    if (state.rootPath && parseRemoteVirtualPath(state.rootPath)?.sessionId === sessionId) {
      if (state.localLocations[0]) {
        await setRootAndNavigate(state.localLocations[0]);
      } else {
        state.rootPath = null;
        state.currentPath = null;
        state.pathInput = "";
        state.currentEntries = [];
        state.selectedContentPaths = [];
        state.treeChildren.clear();
        state.expandedTreePaths.clear();
        render();
      }
    } else {
      render();
    }
    showStatusFlash(`Conexion cerrada: ${session?.label ?? sessionId}`);
    await emit(CONNECTIONS_CHANGED_EVENT);
  } catch {
    // withBusy ya informa el error al usuario.
  }
}

function applySettingsFromDialog(): void {
  const theme = appRoot.querySelector<HTMLSelectElement>("#settings-theme")?.value as ThemeMode | undefined;
  const defaultView = appRoot.querySelector<HTMLSelectElement>("#settings-default-view")?.value as ViewMode | undefined;
  const fontScale = appRoot.querySelector<HTMLInputElement>("#settings-font-scale")?.value;
  const defaultContentZoom = appRoot.querySelector<HTMLInputElement>("#settings-default-content-zoom")?.value;
  const defaultTreeZoom = appRoot.querySelector<HTMLInputElement>("#settings-default-tree-zoom")?.value;
  const selectionWeightDepth = appRoot.querySelector<HTMLInputElement>("#settings-selection-weight-depth")?.value;
  const selectionWeightAll = appRoot.querySelector<HTMLInputElement>("#settings-selection-weight-all")?.checked;
  const compactUi = appRoot.querySelector<HTMLInputElement>("#settings-compact-ui")?.checked;
  const confirmDelete = appRoot.querySelector<HTMLInputElement>("#settings-confirm-delete")?.checked;
  const hideDeleteProgressPopup = appRoot.querySelector<HTMLInputElement>("#settings-hide-delete-progress-popup")?.checked;

  state.settings.theme = theme ?? state.settings.theme;
  state.settings.defaultViewMode = defaultView ?? state.settings.defaultViewMode;
  state.settings.fontScale = fontScale ? Number(fontScale) : state.settings.fontScale;
  state.settings.defaultContentZoom = defaultContentZoom
    ? Number(defaultContentZoom)
    : state.settings.defaultContentZoom;
  state.settings.defaultTreeZoom = defaultTreeZoom ? Number(defaultTreeZoom) : state.settings.defaultTreeZoom;
  state.settings.selectionWeightMaxDepth = selectionWeightDepth
    ? clamp(Number(selectionWeightDepth), 1, 99)
    : state.settings.selectionWeightMaxDepth;
  state.settings.selectionWeightCalculateAll = selectionWeightAll ?? state.settings.selectionWeightCalculateAll;
  state.settings.compactUi = compactUi ?? state.settings.compactUi;
  state.settings.confirmDelete = confirmDelete ?? state.settings.confirmDelete;
  state.settings.hideDeleteProgressPopup = hideDeleteProgressPopup ?? state.settings.hideDeleteProgressPopup;

  state.viewMode = state.settings.defaultViewMode;
  state.zoom = state.settings.defaultContentZoom;
  state.treeZoom = state.settings.defaultTreeZoom;
  reapplyContentOrdering(false);
}

function updateSettingsLabels(): void {
  const fontValue = appRoot.querySelector<HTMLElement>("#settings-font-scale-value");
  const contentValue = appRoot.querySelector<HTMLElement>("#settings-default-content-zoom-value");
  const treeValue = appRoot.querySelector<HTMLElement>("#settings-default-tree-zoom-value");
  if (fontValue) {
    fontValue.textContent = `${state.settings.fontScale}%`;
  }
  if (contentValue) {
    contentValue.textContent = `${state.settings.defaultContentZoom}%`;
  }
  if (treeValue) {
    treeValue.textContent = `${state.settings.defaultTreeZoom}%`;
  }
}

function applyVisualSettings(): void {
  document.body.dataset.theme = state.settings.theme;
  document.body.classList.toggle("compact-ui", state.settings.compactUi);
  appRoot.style.setProperty("--app-font-scale", `${state.settings.fontScale / 100}`);
  applyContentZoom();
  applyContentColumnWidths();
  applyTreeZoom();
  const workspace = appRoot.querySelector<HTMLElement>(".workspace");
  if (workspace) {
    workspace.style.gridTemplateColumns = `${state.treeWidth}px 6px minmax(0, 1fr)`;
  }
}

function setContentZoomPreference(value: number, persist = true): void {
  const normalized = normalizeZoomPercent(value);
  state.zoom = normalized;
  state.settings.defaultContentZoom = normalized;
  applyContentZoom();
  if (persist) {
    saveSettings();
  }
}

function setTreeZoomPreference(value: number, persist = true): void {
  const normalized = normalizeZoomPercent(value);
  state.treeZoom = normalized;
  state.settings.defaultTreeZoom = normalized;
  applyTreeZoom();
  if (persist) {
    saveSettings();
  }
}

function applyContentZoom(): void {
  appRoot.style.setProperty("--content-scale", `${zoomPercentToScale(state.zoom)}`);
  appRoot.querySelectorAll<HTMLElement>("#content-zoom-value").forEach((node) => {
    node.textContent = `${state.zoom}%`;
  });
  const zoomRange = appRoot.querySelector<HTMLInputElement>("#zoom-range");
  if (zoomRange) {
    zoomRange.value = String(state.zoom);
  }
}

function applyContentColumnWidths(): void {
  const body = appRoot.querySelector<HTMLElement>(".list-content-body");
  if (!body) {
    return;
  }
  const widths = state.settings.contentColumnWidths;
  body.style.setProperty("--content-col-name", `${widths.name}px`);
  body.style.setProperty("--content-col-type", `${widths.type}px`);
  body.style.setProperty("--content-col-size", `${widths.size}px`);
  body.style.setProperty("--content-col-modified", `${widths.modified}px`);
}

function applyTreeZoom(): void {
  appRoot.style.setProperty("--tree-scale", `${zoomPercentToScale(state.treeZoom)}`);
  const treeLabel = appRoot.querySelector<HTMLElement>("#tree-zoom-value");
  if (treeLabel) {
    treeLabel.textContent = `${state.treeZoom}%`;
  }
  const range = appRoot.querySelector<HTMLInputElement>("#tree-zoom-range");
  if (range) {
    range.value = String(state.treeZoom);
  }
}

function syncTopSearchUi(): void {
  const clearButton = appRoot.querySelector<HTMLButtonElement>("#top-search-clear");
  if (clearButton) {
    const hasQuery = state.topSearchQuery.trim().length > 0;
    clearButton.disabled = !hasQuery;
    clearButton.classList.toggle("visible", hasQuery);
  }
}

function bindZoomPreview(selector: string): void {
  const input = appRoot.querySelector<HTMLInputElement>(selector);
  const container = input?.closest<HTMLElement>(".zoom-line");
  if (!input || !container) {
    return;
  }
  const stopDragging = () => {
    container.classList.remove("dragging");
    window.removeEventListener("pointerup", stopDragging);
    window.removeEventListener("pointercancel", stopDragging);
  };
  input.addEventListener("pointerdown", () => {
    container.classList.add("dragging");
    window.addEventListener("pointerup", stopDragging);
    window.addEventListener("pointercancel", stopDragging);
  });
}

function captureScrollState(): void {
  const treeScroll = appRoot.querySelector<HTMLDivElement>(".tree-scroll");
  if (treeScroll) {
    state.treeScrollTop = treeScroll.scrollTop;
  }
  const contentScroll = appRoot.querySelector<HTMLDivElement>("#content-scroll");
  if (contentScroll) {
    state.contentScrollTop = contentScroll.scrollTop;
    state.contentScrollLeft = contentScroll.scrollLeft;
  }
}

function restoreScrollState(): void {
  const treeScroll = appRoot.querySelector<HTMLDivElement>(".tree-scroll");
  if (treeScroll) {
    treeScroll.scrollTop = state.treeScrollTop;
  }
  const contentScroll = appRoot.querySelector<HTMLDivElement>("#content-scroll");
  if (contentScroll) {
    contentScroll.scrollTop = state.contentScrollTop;
    contentScroll.scrollLeft = state.contentScrollLeft;
  }
}

function adjustContextMenuPosition(): void {
  const menu = appRoot.querySelector<HTMLDivElement>(".context-menu");
  if (!menu) {
    return;
  }
  const margin = 8;
  const rect = menu.getBoundingClientRect();
  const left = Math.max(margin, Math.min(rect.left, window.innerWidth - rect.width - margin));
  const top = Math.max(margin, Math.min(rect.top, window.innerHeight - rect.height - margin));
  menu.style.left = `${left}px`;
  menu.style.top = `${top}px`;
}

function setContentSelection(paths: string[], anchor: string | null = null, shouldRender = true): void {
  state.selectedContentPaths = paths;
  state.anchorPath = anchor ?? paths[0] ?? null;
  void refreshSelectionSummary([...paths]);
  if (shouldRender) {
    captureScrollState();
    render();
  }
}

async function refreshSelectionSummary(paths: string[]): Promise<void> {
  const token = state.selectionSummaryToken + 1;
  state.selectionSummaryToken = token;
  if (paths.length === 0) {
    state.selectionSummaryLabel = null;
    if (!state.progress && !state.contextMenu) {
      render();
    }
    return;
  }
  try {
    const summary = await invoke<PropertiesSummaryDto>("summarize_paths", {
      requestId: token,
      paths,
      options: {
        maxDepth: state.settings.selectionWeightCalculateAll ? null : state.settings.selectionWeightMaxDepth,
      } satisfies SummaryOptionsPayload,
    });
    if (state.selectionSummaryToken !== token) {
      return;
    }
    state.selectionSummaryLabel = summary.totalSizeLabel;
    render();
  } catch {
    if (state.selectionSummaryToken !== token) {
      return;
    }
    state.selectionSummaryLabel = null;
    render();
  }
}

function clearDropTargets(): void {
  activeDropTargetElement?.classList.remove("drop-target");
  activeDropTargetElement = null;
}

function removeDragGhost(): void {
  activeDragGhost?.remove();
  activeDragGhost = null;
}

function updateDragGhostPosition(clientX: number, clientY: number): void {
  if (!activeDragGhost) {
    return;
  }
  activeDragGhost.style.left = `${clientX + 14}px`;
  activeDragGhost.style.top = `${clientY + 14}px`;
}

function createDragGhost(): void {
  removeDragGhost();
  const ghost = document.createElement("div");
  ghost.className = "drag-ghost";
  const count = draggingContentPaths.length;
  const primaryPath = draggingContentPaths[0] ?? "";
  const primaryEntry = state.currentEntries.find((entry) => entry.path === primaryPath);
  const iconName = "move";
  const label = count > 1 ? `Moviendo ${count} elementos` : `Moviendo: ${primaryEntry?.name ?? leafName(primaryPath) ?? "Elemento"}`;
  ghost.innerHTML = `
    <span class="icon-wrap">${renderIcon(iconName)}</span>
    <span>${escapeHtml(label)}</span>
  `;
  document.body.appendChild(ghost);
  activeDragGhost = ghost;
}

function setActiveDropTarget(element: HTMLElement | null): void {
  if (activeDropTargetElement === element) {
    return;
  }
  clearDropTargets();
  activeDropTargetElement = element;
  activeDropTargetElement?.classList.add("drop-target");
}

function updateDragSourceState(): void {
  const dragging = new Set(draggingContentPaths);
  appRoot.querySelectorAll<HTMLElement>("[data-content-path]").forEach((element) => {
    const path = element.dataset.contentPath ?? "";
    element.classList.toggle("drag-source", dragging.has(path));
  });
}

function resetDragInteractionState(): void {
  draggingContentPaths = [];
  clearDropTargets();
  updateDragSourceState();
  removeDragGhost();
  document.body.classList.remove("dragging-content-items");
}

function beginContentPointerDrag(event: PointerEvent, row: HTMLElement): void {
  const target = event.target as HTMLElement;
  if (event.button !== 0) {
    return;
  }
  if (target.closest("[data-entry-check], .cell-pad-start, .cell-check, .cell-pad-end, .inline-rename, button, input, label")) {
    return;
  }
  const path = row.dataset.contentPath;
  if (!path) {
    return;
  }

  event.preventDefault();
  document.getSelection()?.removeAllRanges();

  const startX = event.clientX;
  const startY = event.clientY;
  let dragActive = false;

  const updateDropTargetFromPoint = (clientX: number, clientY: number) => {
    const pointTarget = document.elementFromPoint(clientX, clientY) as HTMLElement | null;
    const resolution = resolveTreeDropTarget(pointTarget) ?? resolveContentDropTarget(pointTarget);
    setActiveDropTarget(resolution?.element ?? null);
    return resolution?.destination ?? null;
  };

  const finish = (shouldSuppressClick: boolean) => {
    if (shouldSuppressClick) {
      suppressContentClick = true;
      window.setTimeout(() => {
        suppressContentClick = false;
      }, 0);
    }
    window.removeEventListener("pointermove", onPointerMove);
    window.removeEventListener("pointerup", onPointerUp);
    window.removeEventListener("pointercancel", onPointerCancel);
    resetDragInteractionState();
  };

  const onPointerMove = (moveEvent: PointerEvent) => {
    const deltaX = moveEvent.clientX - startX;
    const deltaY = moveEvent.clientY - startY;
    if (!dragActive) {
      if (Math.hypot(deltaX, deltaY) < 6) {
        return;
      }
      if (!state.selectedContentPaths.includes(path)) {
        setContentSelection([path], path, false);
      }
      draggingContentPaths = state.selectedContentPaths.includes(path) ? [...state.selectedContentPaths] : [path];
      dragActive = draggingContentPaths.length > 0;
      if (!dragActive) {
        return;
      }
      document.getSelection()?.removeAllRanges();
      updateDragSourceState();
      createDragGhost();
      document.body.classList.add("dragging-content-items");
    }
    moveEvent.preventDefault();
    updateDragGhostPosition(moveEvent.clientX, moveEvent.clientY);
    updateDropTargetFromPoint(moveEvent.clientX, moveEvent.clientY);
  };

  const onPointerUp = (upEvent: PointerEvent) => {
    if (!dragActive) {
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", onPointerUp);
      window.removeEventListener("pointercancel", onPointerCancel);
      return;
    }
    upEvent.preventDefault();
    const destination = updateDropTargetFromPoint(upEvent.clientX, upEvent.clientY);
    const draggedPaths = [...draggingContentPaths];
    finish(true);
    if (destination && draggedPaths.length > 0) {
      void startTransferJob(draggedPaths, destination, "cut");
    }
  };

  const onPointerCancel = () => {
    if (!dragActive) {
      window.removeEventListener("pointermove", onPointerMove);
      window.removeEventListener("pointerup", onPointerUp);
      window.removeEventListener("pointercancel", onPointerCancel);
      return;
    }
    finish(true);
  };

  window.addEventListener("pointermove", onPointerMove);
  window.addEventListener("pointerup", onPointerUp);
  window.addEventListener("pointercancel", onPointerCancel);
}

function isValidDropDestination(destination: string): boolean {
  const normalizedDestination = normalizePath(destination);
  return draggingContentPaths.every((sourcePath) => {
    const normalizedSource = normalizePath(sourcePath);
    if (normalizedSource === normalizedDestination) {
      return false;
    }
    return !pathIsDirectorySelection(sourcePath) || !isSubPath(destination, sourcePath);
  });
}

function resolveTreeDropTarget(target: HTMLElement | null): { element: HTMLElement; destination: string } | null {
  const treeTarget = target?.closest<HTMLElement>("[data-tree-select]");
  const destination = treeTarget?.dataset.treeSelect ?? null;
  if (!treeTarget || !destination || !isValidDropDestination(destination)) {
    return null;
  }
  return { element: treeTarget, destination };
}

function resolveContentDropTarget(target: HTMLElement | null): { element: HTMLElement; destination: string } | null {
  const directTarget = target?.closest<HTMLElement>("[data-drop-destination]");
  const directDestination = directTarget?.dataset.dropDestination ?? null;
  if (directTarget && directDestination) {
    if (!isValidDropDestination(directDestination)) {
      return null;
    }
    return { element: directTarget, destination: directDestination };
  }
  if (target?.closest("[data-content-path]")) {
    return null;
  }
  if (!state.currentPath || state.mode === "global-search" || !isValidDropDestination(state.currentPath)) {
    return null;
  }
  const fallbackTarget = appRoot.querySelector<HTMLElement>("#content-scroll");
  if (!fallbackTarget) {
    return null;
  }
  return { element: fallbackTarget, destination: state.currentPath };
}

function bindTreeDropTargets(treeScroll: HTMLDivElement | null): void {
  treeScroll?.addEventListener("dragover", (event) => {
    if (draggingContentPaths.length === 0) {
      return;
    }
    const resolution = resolveTreeDropTarget(event.target as HTMLElement | null);
    if (!resolution) {
      setActiveDropTarget(null);
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    if (event.dataTransfer) {
      event.dataTransfer.dropEffect = "move";
    }
    setActiveDropTarget(resolution.element);
  });
  treeScroll?.addEventListener("dragleave", (event) => {
    const related = event.relatedTarget as Node | null;
    if (!related || !treeScroll.contains(related)) {
      clearDropTargets();
    }
  });
  treeScroll?.addEventListener("drop", (event) => {
    if (draggingContentPaths.length === 0) {
      return;
    }
    const resolution = resolveTreeDropTarget(event.target as HTMLElement | null);
    if (!resolution) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    clearDropTargets();
    document.body.classList.remove("dragging-content-items");
    void moveDraggedSelection(resolution.destination);
  });
}

function bindContentDropTargets(contentScroll: HTMLDivElement | null): void {
  contentScroll?.addEventListener("dragover", (event) => {
    if (draggingContentPaths.length === 0) {
      return;
    }
    const resolution = resolveContentDropTarget(event.target as HTMLElement | null);
    if (!resolution) {
      setActiveDropTarget(null);
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    if (event.dataTransfer) {
      event.dataTransfer.dropEffect = "move";
    }
    setActiveDropTarget(resolution.element);
  });
  contentScroll?.addEventListener("dragleave", (event) => {
    const related = event.relatedTarget as Node | null;
    if (!related || !contentScroll.contains(related)) {
      clearDropTargets();
    }
  });
  contentScroll?.addEventListener("drop", (event) => {
    if (draggingContentPaths.length === 0) {
      return;
    }
    const resolution = resolveContentDropTarget(event.target as HTMLElement | null);
    if (!resolution) {
      return;
    }
    event.preventDefault();
    event.stopPropagation();
    clearDropTargets();
    document.body.classList.remove("dragging-content-items");
    void moveDraggedSelection(resolution.destination);
  });
}

function getParentEntryPath(): string | null {
  if (!state.currentPath || state.mode === "global-search") {
    return null;
  }
  return parentPath(state.currentPath);
}

function saveSettings(): void {
  localStorage.setItem(STORAGE_KEY, JSON.stringify(state.settings));
}

function loadSettings(): AppSettings {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) {
      return cloneDefaultSettings();
    }
    const parsedRaw = JSON.parse(raw) as Partial<AppSettings>;
    const parsed = {
      ...cloneDefaultSettings(),
      ...parsedRaw,
      contentColumnWidths: {
        ...DEFAULT_CONTENT_COLUMN_WIDTHS,
        ...(parsedRaw.contentColumnWidths ?? {}),
      },
    };
    parsed.defaultContentZoom = normalizeZoomPercent(parsed.defaultContentZoom);
    parsed.defaultTreeZoom = normalizeZoomPercent(parsed.defaultTreeZoom);
    parsed.localLocations = Array.isArray(parsed.localLocations)
      ? parsed.localLocations.filter((value): value is string => typeof value === "string" && value.trim().length > 0)
      : [];
    parsed.fontScale = clamp(parsed.fontScale, 85, 130);
    parsed.selectionWeightMaxDepth = clamp(parsed.selectionWeightMaxDepth ?? defaultSettings.selectionWeightMaxDepth, 1, 99);
    parsed.selectionWeightCalculateAll = Boolean(parsed.selectionWeightCalculateAll);
    parsed.contentSortKey = (["name", "type", "size", "modified"] as ContentSortKey[]).includes(parsed.contentSortKey)
      ? parsed.contentSortKey
      : "name";
    parsed.contentSortDirection = parsed.contentSortDirection === "desc" ? "desc" : "asc";
    parsed.contentColumnWidths = {
      name: Math.max(CONTENT_COLUMN_MIN_WIDTHS.name, Number(parsed.contentColumnWidths.name) || DEFAULT_CONTENT_COLUMN_WIDTHS.name),
      type: Math.max(CONTENT_COLUMN_MIN_WIDTHS.type, Number(parsed.contentColumnWidths.type) || DEFAULT_CONTENT_COLUMN_WIDTHS.type),
      size: Math.max(CONTENT_COLUMN_MIN_WIDTHS.size, Number(parsed.contentColumnWidths.size) || DEFAULT_CONTENT_COLUMN_WIDTHS.size),
      modified: Math.max(
        CONTENT_COLUMN_MIN_WIDTHS.modified,
        Number(parsed.contentColumnWidths.modified) || DEFAULT_CONTENT_COLUMN_WIDTHS.modified,
      ),
    };
    return parsed;
  } catch {
    return cloneDefaultSettings();
  }
}

function normalizeZoomPercent(value: number): number {
  if (!Number.isFinite(value)) {
    return 50;
  }
  if (value >= 1 && value <= 100) {
    return Math.round(value);
  }
  return clamp(Math.round(value - 50), 1, 100);
}

function zoomPercentToScale(value: number): number {
  return 0.5 + normalizeZoomPercent(value) / 100;
}

async function withBusy<T>(label: string, action: () => Promise<T>): Promise<T> {
  state.silentBusyCount += 1;
  state.silentBusyLabel = label;
  render();
  try {
    return await action();
  } catch (error) {
    showNotice("error", formatError(error));
    throw error;
  } finally {
    state.silentBusyCount = Math.max(0, state.silentBusyCount - 1);
    if (state.silentBusyCount === 0) {
      state.silentBusyLabel = "";
    }
    render();
  }
}

function showNotice(kind: "error" | "info", text: string): void {
  pushNotification(kind, text);
  state.notice = { kind, text };
  render();
  window.setTimeout(() => {
    if (state.notice?.text === text) {
      state.notice = null;
      render();
    }
  }, 5000);
}

function showStatusFlash(text: string, kind: "info" | "error" = "info"): void {
  pushNotification(kind, text);
  state.statusFlash = { text, kind };
  render();
  window.setTimeout(() => {
    if (state.statusFlash?.text === text) {
      state.statusFlash = null;
      render();
    }
  }, 5000);
}

function pushNotification(kind: NotificationKind, text: string): void {
  const notification: NotificationEntry = {
    id: crypto.randomUUID(),
    kind,
    text,
    createdAt: Date.now(),
  };
  state.notifications = [notification, ...state.notifications].slice(0, 80);
  state.activeNotificationId = notification.id;
  state.expandedNotificationIds.delete(notification.id);
  startActiveNotificationTimer(notification.id, ACTIVE_NOTIFICATION_DURATION_MS);
}

function startActiveNotificationTimer(notificationId: string, durationMs: number): void {
  if (activeNotificationTimeout !== null) {
    window.clearTimeout(activeNotificationTimeout);
  }
  state.activeNotificationId = notificationId;
  activeNotificationStartedAt = Date.now();
  activeNotificationRemainingMs = durationMs;
  activeNotificationPaused = false;
  activeNotificationTimeout = window.setTimeout(() => {
    dismissActiveNotification(notificationId);
  }, durationMs);
}

function dismissActiveNotification(notificationId: string): void {
  if (state.activeNotificationId !== notificationId) {
    return;
  }
  if (activeNotificationTimeout !== null) {
    window.clearTimeout(activeNotificationTimeout);
    activeNotificationTimeout = null;
  }
  state.activeNotificationId = null;
  activeNotificationRemainingMs = 0;
  activeNotificationPaused = false;
  render();
}

function pauseActiveNotificationTimer(): void {
  if (!state.activeNotificationId || activeNotificationPaused) {
    return;
  }
  if (activeNotificationTimeout !== null) {
    window.clearTimeout(activeNotificationTimeout);
    activeNotificationTimeout = null;
  }
  const elapsed = Date.now() - activeNotificationStartedAt;
  activeNotificationRemainingMs = Math.max(0, activeNotificationRemainingMs - elapsed);
  activeNotificationPaused = true;
  render();
}

function resumeActiveNotificationTimer(): void {
  if (!state.activeNotificationId || !activeNotificationPaused) {
    return;
  }
  activeNotificationStartedAt = Date.now();
  activeNotificationPaused = false;
  if (activeNotificationTimeout !== null) {
    window.clearTimeout(activeNotificationTimeout);
  }
  activeNotificationTimeout = window.setTimeout(() => {
    if (state.activeNotificationId) {
      dismissActiveNotification(state.activeNotificationId);
    }
  }, activeNotificationRemainingMs);
  render();
}

function getActiveNotification(): NotificationEntry | null {
  if (!state.activeNotificationId) {
    return null;
  }
  return state.notifications.find((notification) => notification.id === state.activeNotificationId) ?? null;
}

function getActiveNotificationRemainingMs(): number {
  if (!state.activeNotificationId) {
    return 0;
  }
  if (activeNotificationPaused) {
    return activeNotificationRemainingMs;
  }
  return Math.max(0, activeNotificationRemainingMs - (Date.now() - activeNotificationStartedAt));
}

function formatNotificationTime(timestamp: number): string {
  return new Intl.DateTimeFormat("es-ES", {
    hour: "2-digit",
    minute: "2-digit",
    second: "2-digit",
  }).format(timestamp);
}

function removeNotification(notificationId: string): void {
  state.notifications = state.notifications.filter((notification) => notification.id !== notificationId);
  state.expandedNotificationIds.delete(notificationId);
  if (state.activeNotificationId === notificationId) {
    dismissActiveNotification(notificationId);
    return;
  }
  render();
}

function clearNotifications(): void {
  state.notifications = [];
  state.expandedNotificationIds.clear();
  if (state.activeNotificationId) {
    dismissActiveNotification(state.activeNotificationId);
    return;
  }
  render();
}

function toggleNotificationExpansion(notificationId: string): void {
  if (state.expandedNotificationIds.has(notificationId)) {
    state.expandedNotificationIds.delete(notificationId);
  } else {
    state.expandedNotificationIds.add(notificationId);
  }
  render();
}

function findTreeEntry(path: string): EntryDto | undefined {
  for (const items of state.treeChildren.values()) {
    const entry = items.find((item) => item.path === path);
    if (entry) {
      return entry;
    }
  }
  return undefined;
}

function toggleSelection(items: string[], path: string): string[] {
  return items.includes(path) ? items.filter((item) => item !== path) : [...items, path];
}

function computeRangeSelection(startPath: string, endPath: string): string[] {
  const startIndex = state.currentEntries.findIndex((entry) => entry.path === startPath);
  const endIndex = state.currentEntries.findIndex((entry) => entry.path === endPath);
  if (startIndex < 0 || endIndex < 0) {
    return [endPath];
  }
  const [from, to] = startIndex < endIndex ? [startIndex, endIndex] : [endIndex, startIndex];
  return state.currentEntries.slice(from, to + 1).map((entry) => entry.path);
}

function defaultPortForProtocol(protocol: ConnectionProfilePayload["protocol"]): number {
  switch (protocol) {
    case "ssh":
      return 22;
    case "smb":
      return 445;
    default:
      return 21;
  }
}

function protocolIcon(protocol: ConnectionProfileDto["protocol"] | ActiveConnectionDto["protocol"]): string {
  switch (protocol) {
    case "ssh":
      return "terminal";
    case "smb":
      return "network";
    case "ftps":
      return "shield";
    default:
      return "globe";
  }
}

function renderProfileShortLabel(profile: ConnectionProfileDto): string {
  switch (profile.protocol) {
    case "smb":
      return `${profile.host}\\${profile.share}`;
    case "ssh":
      return `${profile.username}@${profile.host}:${profile.port} · ${profile.sshMode.toUpperCase()}`;
    case "ftps":
      return `${profile.host}:${profile.port} · ${profile.ftpSecureImplicit ? "Implicito" : "Explicito"}`;
    default:
      return `${profile.host}:${profile.port} · FTP`;
  }
}

function getActiveLocationSummary(): { icon: string; label: string; tooltip: string } | null {
  if (!state.rootPath) {
    return null;
  }
  if (isRemotePath(state.rootPath)) {
    const parsed = parseRemoteVirtualPath(state.rootPath);
    const connection = parsed ? findConnectionBySessionId(parsed.sessionId) : null;
    if (!connection) {
      return { icon: "network", label: "Remoto", tooltip: "Ubicacion remota activa" };
    }
    return {
      icon: protocolIcon(connection.protocol),
      label: connection.label,
      tooltip: `${connection.detail}\n${displayPathForInput(state.rootPath)}`,
    };
  }
  return {
    icon: "folder",
    label: "Local",
    tooltip: state.rootPath,
  };
}

function findConnectionBySessionId(sessionId: string): ActiveConnectionDto | null {
  return state.activeConnections.find((connection) => connection.sessionId === sessionId) ?? null;
}

function isRemotePath(path: string): boolean {
  return path.startsWith("remote://");
}

function normalizeLogicalRemotePath(path: string): string {
  const normalized = path.trim().replace(/\\/g, "/");
  if (!normalized || normalized === "/") {
    return "/";
  }
  return `/${normalized.replace(/^\/+/, "").replace(/\/+$/, "")}`;
}

function parseRemoteVirtualPath(path: string): { sessionId: string; logicalPath: string } | null {
  if (!isRemotePath(path)) {
    return null;
  }
  const rest = path.slice("remote://".length);
  const splitIndex = rest.indexOf("/");
  if (splitIndex < 0) {
    return null;
  }
  return {
    sessionId: rest.slice(0, splitIndex),
    logicalPath: normalizeLogicalRemotePath(rest.slice(splitIndex)),
  };
}

function buildRemoteVirtualPath(sessionId: string, logicalPath: string): string {
  const normalized = normalizeLogicalRemotePath(logicalPath);
  return normalized === "/" ? `remote://${sessionId}/` : `remote://${sessionId}${normalized}`;
}

function displayPathForInput(path: string): string {
  const remote = parseRemoteVirtualPath(path);
  if (!remote) {
    return path;
  }
  return remote.logicalPath;
}

function resolveInputPath(value: string): string {
  if (!value) {
    return value;
  }
  if (isRemotePath(value)) {
    return value;
  }
  const remoteBase =
    (state.currentPath ? parseRemoteVirtualPath(state.currentPath) : null) ??
    (state.rootPath ? parseRemoteVirtualPath(state.rootPath) : null);
  if (!remoteBase) {
    return value;
  }
  if (/^[A-Za-z]:[\\/]/.test(value) || value.startsWith("//") || value.startsWith("\\\\")) {
    return value;
  }
  if (value.startsWith("/")) {
    return buildRemoteVirtualPath(remoteBase.sessionId, value);
  }
  return buildRemoteVirtualPath(remoteBase.sessionId, joinLogicalRemotePath(remoteBase.logicalPath, value));
}

function joinLogicalRemotePath(parentLogicalPath: string, childName: string): string {
  const parent = normalizeLogicalRemotePath(parentLogicalPath);
  const cleanChild = childName.replace(/\\/g, "/").replace(/^\/+/, "").replace(/\/+$/, "");
  if (!cleanChild) {
    return parent;
  }
  if (parent === "/") {
    return `/${cleanChild}`;
  }
  return `${parent}/${cleanChild}`;
}

function leafName(path: string): string {
  const remote = parseRemoteVirtualPath(path);
  if (remote) {
    if (remote.logicalPath === "/") {
      return findConnectionBySessionId(remote.sessionId)?.label ?? "/";
    }
    const parts = remote.logicalPath.split("/").filter(Boolean);
    return parts[parts.length - 1] ?? remote.logicalPath;
  }
  const normalized = path.replace(/[\\/]+$/, "");
  const parts = normalized.split(/[\\/]/);
  return parts[parts.length - 1] ?? normalized;
}

function parentPath(path: string): string | null {
  const remote = parseRemoteVirtualPath(path);
  if (remote) {
    if (remote.logicalPath === "/") {
      return null;
    }
    const trimmed = remote.logicalPath.replace(/\/+$/, "");
    const splitIndex = trimmed.lastIndexOf("/");
    if (splitIndex <= 0) {
      return buildRemoteVirtualPath(remote.sessionId, "/");
    }
    return buildRemoteVirtualPath(remote.sessionId, trimmed.slice(0, splitIndex));
  }
  const normalized = path.replace(/[\\/]+$/, "");
  const splitIndex = Math.max(normalized.lastIndexOf("/"), normalized.lastIndexOf("\\"));
  if (splitIndex <= 0) {
    return null;
  }
  return normalized.slice(0, splitIndex);
}

function normalizePath(path: string): string {
  const remote = parseRemoteVirtualPath(path);
  if (remote) {
    return `remote://${remote.sessionId}${normalizeLogicalRemotePath(remote.logicalPath).toLowerCase()}`;
  }
  return path.replace(/\\/g, "/").replace(/\/+$/, "").toLowerCase();
}

function isSubPath(path: string, root: string): boolean {
  const remotePath = parseRemoteVirtualPath(path);
  const remoteRoot = parseRemoteVirtualPath(root);
  if (remotePath || remoteRoot) {
    if (!remotePath || !remoteRoot || remotePath.sessionId !== remoteRoot.sessionId) {
      return false;
    }
    const normalizedPath = normalizeLogicalRemotePath(remotePath.logicalPath).toLowerCase();
    const normalizedRoot = normalizeLogicalRemotePath(remoteRoot.logicalPath).toLowerCase();
    if (normalizedRoot === "/") {
      return normalizedPath.startsWith("/");
    }
    return normalizedPath === normalizedRoot || normalizedPath.startsWith(`${normalizedRoot}/`);
  }
  const normalizedPath = normalizePath(path);
  const normalizedRoot = normalizePath(root);
  if (normalizedRoot === "/") {
    return normalizedPath.startsWith("/");
  }
  return normalizedPath === normalizedRoot || normalizedPath.startsWith(`${normalizedRoot}/`);
}

function buildPathChain(root: string, target: string): string[] {
  const remoteRoot = parseRemoteVirtualPath(root);
  const remoteTarget = parseRemoteVirtualPath(target);
  if (remoteRoot || remoteTarget) {
    if (!remoteRoot || !remoteTarget || remoteRoot.sessionId !== remoteTarget.sessionId) {
      return [root];
    }
    const rootParts = normalizeLogicalRemotePath(remoteRoot.logicalPath).split("/").filter(Boolean);
    const targetParts = normalizeLogicalRemotePath(remoteTarget.logicalPath).split("/").filter(Boolean);
    if (targetParts.length <= rootParts.length) {
      return [buildRemoteVirtualPath(remoteRoot.sessionId, remoteRoot.logicalPath)];
    }
    const chain = [buildRemoteVirtualPath(remoteRoot.sessionId, remoteRoot.logicalPath)];
    let current = remoteRoot.logicalPath;
    for (const part of targetParts.slice(rootParts.length)) {
      current = joinLogicalRemotePath(current, part);
      chain.push(buildRemoteVirtualPath(remoteRoot.sessionId, current));
    }
    return chain;
  }
  if (!isSubPath(target, root)) {
    return [root];
  }
  const separator = root.includes("\\") ? "\\" : "/";
  const rootParts = root.replace(/[\\/]+$/, "").split(/[\\/]+/).filter(Boolean);
  const targetParts = target.replace(/[\\/]+$/, "").split(/[\\/]+/).filter(Boolean);
  if (targetParts.length <= rootParts.length) {
    return [root];
  }
  const rootRaw = root.replace(/[\\/]+$/, "");
  const chain = [rootRaw];
  let current = rootRaw;
  for (const part of targetParts.slice(rootParts.length)) {
    current = `${current}${separator}${part}`;
    chain.push(current);
  }
  return chain;
}

function pathIsDirectorySelection(path: string): boolean {
  return Boolean(findTreeEntry(path)?.isDir || state.currentEntries.find((entry) => entry.path === path)?.isDir);
}

function getEntryLocationKind(path: string): "local" | "remote" {
  const currentEntry = state.currentEntries.find((entry) => entry.path === path);
  if (currentEntry) {
    return currentEntry.locationKind === "remote" ? "remote" : "local";
  }
  const treeEntry = findTreeEntry(path);
  if (treeEntry) {
    return treeEntry.locationKind === "remote" ? "remote" : "local";
  }
  return isRemotePath(path) ? "remote" : "local";
}

function pickMetadataString(metadata: Record<string, unknown>, keys: string[]): string | null {
  for (const key of keys) {
    const value = metadata[key];
    if (typeof value === "string" && value.trim()) {
      return value;
    }
  }
  return null;
}

function clamp(value: number, min: number, max: number): number {
  return Math.max(min, Math.min(max, value));
}

function renderIcon(name: string): string {
  const icons: Record<string, string> = {
    app: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 6h7l2 2h7v10H4z"/><path d="M8 11h8M8 15h8"/></svg>`,
    folder: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 7h6l2 2h10v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><path d="M3 7a2 2 0 0 1 2-2h4l2 2"/></svg>`,
    file: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 3h7l5 5v13H7z"/><path d="M14 3v5h5"/></svg>`,
    search: `<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="11" cy="11" r="6"/><path d="M20 20l-4.2-4.2"/></svg>`,
    x: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 6l12 12M18 6l-12 12"/></svg>`,
    up: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 5l-6 6M12 5l6 6"/><path d="M12 5v14"/></svg>`,
    "chevron-up": `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 14l6-6 6 6"/></svg>`,
    refresh: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M20 6v5h-5"/><path d="M4 18v-5h5"/><path d="M19 11a7 7 0 0 0-12-3L4 11"/><path d="M5 13a7 7 0 0 0 12 3l3-3"/></svg>`,
    "arrow-right": `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12h14"/><path d="M13 6l6 6-6 6"/></svg>`,
    route: `<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="6" cy="18" r="2"/><circle cx="18" cy="6" r="2"/><path d="M8 18h5a5 5 0 0 0 5-5V8"/></svg>`,
    tree: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 5h6v4H5zM13 15h6v4h-6zM5 15h6v4H5z"/><path d="M11 7h2v10h2"/></svg>`,
    list: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 7h11M8 12h11M8 17h11"/><circle cx="4" cy="7" r="1"/><circle cx="4" cy="12" r="1"/><circle cx="4" cy="17" r="1"/></svg>`,
    grid: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 4h7v7H4zM13 4h7v7h-7zM4 13h7v7H4zM13 13h7v7h-7z"/></svg>`,
    zoom: `<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="10" cy="10" r="6"/><path d="M20 20l-4.2-4.2"/><path d="M10 7v6M7 10h6"/></svg>`,
    open: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M14 4h6v6"/><path d="M10 14L20 4"/><path d="M20 14v5H4V5h5"/></svg>`,
    "edit-open": `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 19h6"/><path d="M13 5l6 6"/><path d="M12 6L6 12v6h6l6-6-6-6z"/></svg>`,
    copy: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M9 9h10v11H9z"/><path d="M5 15H4V4h11v1"/></svg>`,
    cut: `<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="6" cy="18" r="2"/><circle cx="6" cy="6" r="2"/><path d="M20 4L8 16"/><path d="M14 14l6 6"/><path d="M8 8l4 4"/></svg>`,
    paste: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M9 4h6v3H9z"/><path d="M7 7h10v13H7z"/><path d="M9 11h6M9 15h4"/></svg>`,
    clipboard: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M9 4h6v3H9z"/><path d="M7 7h10v13H7z"/><path d="M9 11h6M9 15h6"/></svg>`,
    bell: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 17h12"/><path d="M8 17V11a4 4 0 1 1 8 0v6"/><path d="M10 20a2 2 0 0 0 4 0"/></svg>`,
    rename: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 20h4l10-10-4-4L4 16z"/><path d="M13 5l4 4"/></svg>`,
    trash: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 7h16"/><path d="M9 7V4h6v3"/><path d="M7 7l1 13h8l1-13"/><path d="M10 11v6M14 11v6"/></svg>`,
    info: `<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M12 11v5"/><circle cx="12" cy="8" r="1"/></svg>`,
    "folder-plus": `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 7h6l2 2h10v8a2 2 0 0 1-2 2H5a2 2 0 0 1-2-2z"/><path d="M16 12v4M14 14h4"/></svg>`,
    "file-plus": `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M7 3h7l5 5v13H7z"/><path d="M14 3v5h5"/><path d="M12 11v6M9 14h6"/></svg>`,
    filter: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 6h16l-6 7v5l-4 2v-7z"/></svg>`,
    settings: `<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="3"/><path d="M19 12a7 7 0 0 0-.1-1l2-1.5-2-3.4-2.3.8a7 7 0 0 0-1.7-1L14.5 3h-5l-.4 2.9a7 7 0 0 0-1.7 1L5 6.1l-2 3.4 2 1.5a7 7 0 0 0 0 2l-2 1.5 2 3.4 2.4-.8a7 7 0 0 0 1.6 1l.5 2.9h5l.4-2.9a7 7 0 0 0 1.7-1l2.3.8 2-3.4-2-1.5c.1-.3.1-.7.1-1z"/></svg>`,
    check: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 12l4.2 4.2L19 6.5"/></svg>`,
    move: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M8 12h10"/><path d="M14 8l4 4-4 4"/><path d="M4 6h6v12H4z"/></svg>`,
    plus: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 5v14M5 12h14"/></svg>`,
    network: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 18h5v-4H4zM15 18h5v-4h-5zM9 10h6V6H9z"/><path d="M12 10v2M6.5 14v-2h11v2"/></svg>`,
    locations: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M6 19h12"/><path d="M12 4l7 4v4c0 4-3 6.5-7 8-4-1.5-7-4-7-8V8z"/></svg>`,
    terminal: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 6h16v12H4z"/><path d="M8 10l2 2-2 2"/><path d="M12 16h4"/></svg>`,
    globe: `<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="9"/><path d="M3 12h18"/><path d="M12 3a14 14 0 0 1 0 18"/><path d="M12 3a14 14 0 0 0 0 18"/></svg>`,
    shield: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M12 3l7 3v5c0 4.6-2.8 7.7-7 10-4.2-2.3-7-5.4-7-10V6z"/><path d="M9 12l2 2 4-4"/></svg>`,
    plug: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M9 8V4M15 8V4"/><path d="M8 8h8v4a4 4 0 0 1-4 4v4"/><path d="M10 20h4"/></svg>`,
    "plug-off": `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M9 8V4M15 8V6"/><path d="M8 8h5"/><path d="M16 11v1a4 4 0 0 1-4 4v4"/><path d="M10 20h4"/><path d="M4 4l16 16"/></svg>`,
    pulse: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M3 12h4l2-4 4 8 2-4h6"/></svg>`,
    save: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 4h11l3 3v13H5z"/><path d="M8 4v6h8V4"/><path d="M9 17h6"/></svg>`,
    unarchive: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M4 5h16v4H4z"/><path d="M5 9h14v10H5z"/><path d="M12 12v5"/><path d="M9 14l3 3 3-3"/></svg>`,
    disc: `<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="8"/><circle cx="12" cy="12" r="2"/></svg>`,
    "restore-disc": `<svg viewBox="0 0 24 24" aria-hidden="true"><circle cx="12" cy="12" r="8"/><circle cx="12" cy="12" r="2"/><path d="M7 9H4V6"/><path d="M4 9a8 8 0 0 1 14-3"/></svg>`,
    selection: `<svg viewBox="0 0 24 24" aria-hidden="true"><path d="M5 7h10"/><path d="M5 12h14"/><path d="M5 17h9"/><circle cx="18" cy="7" r="1"/><circle cx="18" cy="17" r="1"/></svg>`,
  };
  return `<span class="svg-icon">${icons[name] ?? icons.file}</span>`;
}

function escapeHtml(text: string): string {
  return text
    .replaceAll("&", "&amp;")
    .replaceAll("<", "&lt;")
    .replaceAll(">", "&gt;")
    .replaceAll('"', "&quot;");
}

function escapeAttr(text: string): string {
  return escapeHtml(text).replaceAll("'", "&#39;");
}

function formatError(error: unknown): string {
  if (typeof error === "string") {
    return error;
  }
  if (error instanceof Error) {
    return error.message;
  }
  return "Se produjo un error inesperado.";
}

function renderTooltipAttr(
  text: string,
  options: { delayMs?: number; overflowTarget?: "self"; overflowSelector?: string } = {},
): string {
  const attrs = [`data-tooltip="${escapeAttr(text)}"`];
  if (options.delayMs !== undefined) {
    attrs.push(`data-tooltip-delay="${options.delayMs}"`);
  }
  if (options.overflowTarget) {
    attrs.push(`data-tooltip-overflow="${options.overflowTarget}"`);
  }
  if (options.overflowSelector) {
    attrs.push(`data-tooltip-overflow-selector="${escapeAttr(options.overflowSelector)}"`);
  }
  return attrs.join(" ");
}

function ensureTooltipElement(): HTMLDivElement {
  if (tooltipElement) {
    return tooltipElement;
  }
  const element = document.createElement("div");
  element.className = "app-tooltip";
  document.body.appendChild(element);
  tooltipElement = element;
  return element;
}

function hideTooltip(): void {
  tooltipElement?.classList.remove("visible");
}

function clearTooltipTimer(): void {
  if (tooltipShowTimer !== null) {
    window.clearTimeout(tooltipShowTimer);
    tooltipShowTimer = null;
  }
  tooltipPendingTarget = null;
  tooltipPendingText = null;
}

function positionTooltip(clientX: number, clientY: number): void {
  if (!tooltipElement) {
    return;
  }
  const margin = 10;
  const offsetX = 14;
  const offsetY = 18;
  const width = tooltipElement.offsetWidth;
  const height = tooltipElement.offsetHeight;
  let left = clientX + offsetX;
  let top = clientY + offsetY;
  if (left + width > window.innerWidth - margin) {
    left = Math.max(margin, window.innerWidth - width - margin);
  }
  if (top + height > window.innerHeight - margin) {
    top = Math.max(margin, clientY - height - 14);
  }
  tooltipElement.style.left = `${left}px`;
  tooltipElement.style.top = `${top}px`;
}

function showTooltip(text: string, clientX: number, clientY: number): void {
  clearTooltipTimer();
  const element = ensureTooltipElement();
  if (element.textContent !== text) {
    element.textContent = text;
  }
  element.classList.add("visible");
  positionTooltip(clientX, clientY);
}

function tooltipDelayForTarget(target: HTMLElement): number {
  const baseDelay = Number(target.dataset.tooltipDelay ?? "0");
  const overflowElement =
    target.dataset.tooltipOverflow === "self"
      ? target
      : target.dataset.tooltipOverflowSelector
        ? target.querySelector<HTMLElement>(target.dataset.tooltipOverflowSelector)
        : null;
  if (overflowElement && isElementTextTruncated(overflowElement)) {
    return 0;
  }
  return Math.max(0, baseDelay);
}

function isElementTextTruncated(element: HTMLElement): boolean {
  return element.scrollWidth > element.clientWidth + 1 || element.scrollHeight > element.clientHeight + 1;
}

function queueTooltip(target: HTMLElement, text: string, clientX: number, clientY: number): void {
  tooltipPointerX = clientX;
  tooltipPointerY = clientY;
  const delay = tooltipDelayForTarget(target);
  if (delay === 0) {
    showTooltip(text, clientX, clientY);
    return;
  }
  if (tooltipPendingTarget === target && tooltipPendingText === text) {
    return;
  }
  clearTooltipTimer();
  hideTooltip();
  tooltipPendingTarget = target;
  tooltipPendingText = text;
  tooltipShowTimer = window.setTimeout(() => {
    if (tooltipPendingTarget === target && tooltipPendingText === text) {
      showTooltip(text, tooltipPointerX, tooltipPointerY);
    }
  }, delay);
}

function initTooltipSystem(): void {
  if (tooltipSystemBound) {
    return;
  }
  tooltipSystemBound = true;

  appRoot.addEventListener("pointermove", (event) => {
    const target = (event.target as HTMLElement | null)?.closest<HTMLElement>("[data-tooltip]");
    const text = target?.dataset.tooltip?.trim();
    if (!target || !text) {
      clearTooltipTimer();
      hideTooltip();
      return;
    }
    const element = ensureTooltipElement();
    if (element.classList.contains("visible") && element.textContent === text) {
      tooltipPointerX = event.clientX;
      tooltipPointerY = event.clientY;
      positionTooltip(event.clientX, event.clientY);
      return;
    }
    queueTooltip(target, text, event.clientX, event.clientY);
  });

  appRoot.addEventListener("pointerleave", () => {
    clearTooltipTimer();
    hideTooltip();
  });

  appRoot.addEventListener("pointerdown", () => {
    clearTooltipTimer();
    hideTooltip();
  });

  appRoot.addEventListener("focusin", (event) => {
    const target = (event.target as HTMLElement | null)?.closest<HTMLElement>("[data-tooltip]");
    const text = target?.dataset.tooltip?.trim();
    if (!target || !text) {
      return;
    }
    const rect = target.getBoundingClientRect();
    showTooltip(text, rect.left + rect.width / 2, rect.bottom);
  });

  appRoot.addEventListener("focusout", () => {
    hideTooltip();
  });

  window.addEventListener("scroll", hideTooltip, true);
}

function getActiveRenameInput(): HTMLInputElement | null {
  return appRoot.querySelector<HTMLInputElement>(".inline-rename");
}

function isTargetInsideRenameInput(target: EventTarget | null, input: HTMLInputElement): boolean {
  return target instanceof Node && (target === input || input.contains(target));
}
