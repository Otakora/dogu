export type EntryDto = {
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
  /** Ghost overlay markers — set only for predicted (not-yet-existing) entries. */
  isGhost?: boolean;
  ghostOpId?: string;
  /** True when the prediction is only approximate (e.g. CHD restore output). */
  ghostApproximate?: boolean;
  /** Set on an entry that a queued op will overwrite. */
  willBeReplaced?: boolean;
  /** Set on an entry that a queued op will remove from its current location. */
  willBeRemoved?: boolean;
  /** Set on a real entry that a queued op wants to write over but can neither
   *  overwrite nor rename around — the op will fail at run time. */
  willConflict?: boolean;
};

export type ConnectionProfileDto = {
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

export type ConnectionProfilePayload = {
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

export type ActiveConnectionDto = {
  sessionId: string;
  profileId: string;
  label: string;
  protocol: "smb" | "ssh" | "ftp" | "ftps";
  host: string;
  rootPath: string;
  displayPath: string;
  detail: string;
};

export type ConnectionOpenResultDto = {
  connected: boolean;
  requiresTrust: boolean;
  fingerprint: string | null;
  message: string | null;
  connection: ActiveConnectionDto | null;
  writeAccess: boolean | null;
};

export type ToolRuntimeDto = {
  available: boolean;
  path: string | null;
  version: string | null;
  error: string | null;
};

export type AppMetadataDto = {
  appVersion: string;
  platform: string;
  chdman: Record<string, unknown>;
  chdmanRuntime: ToolRuntimeDto;
};

/** Startup status of one external tool + the capabilities it unlocks. */
export type ToolStatusDto = {
  id: "chdman" | "sevenZip" | "dolphinTool";
  displayName: string;
  /** When false, missing = a core feature set is unavailable. */
  optional: boolean;
  /** "bundled" (shipped with Dogu) or "manual" (user installs; Dogu detects). */
  provisioning: "bundled" | "manual";
  runtime: ToolRuntimeDto;
  /** Capability ids unlocked when available, e.g. "chd", "archives", "rvzDolphin". */
  enables: string[];
  guidanceUrl: string | null;
};

export type JobProgressDto = {
  jobId: string;
  progress: number;
  message: string;
};

export type JobLogDto = {
  jobId: string;
  line: string;
};

export type JobFinishedDto = {
  jobId: string;
  success: boolean;
  message: string;
};

export type PropertiesSummaryDto = {
  count: number;
  files: number;
  directories: number;
  totalSize: number;
  totalSizeLabel: string;
  lines: string[];
};

export type SummaryOptionsPayload = {
  maxDepth: number | null;
};

export type ExtractionOptionsPayload = {
  individualFolders: boolean;
  splitEntries: boolean;
  destinationMode: 'same' | 'custom';
  destinationPath: string | null;
  deleteArchives: boolean;
  overwrite: boolean;
  /** When not overwriting and a name collides, auto-rename instead of failing/skipping. */
  renameOnConflict?: boolean;
  /** Virtual remote path. When set output is written to temp then uploaded. */
  remoteDestination?: string | null;
  remoteTransfer?: RemoteTransferPolicy | null;
};

export type ExtractionPreviewEntry = {
  name: string;
  isDir: boolean;
  destinationPath: string;
};

export type ExtractionPreviewRow = {
  archivePath: string;
  destinationRoot: string;
  entries: ExtractionPreviewEntry[];
};

export type ChdSourceDto = {
  sourcePath: string;
  containerDir: string;
  command: string;
  displayExtensions: string[];
  requiredPaths: string[];
  /** Filenames that are referenced but missing from disk. Empty = all OK. */
  missingFiles: string[];
};

/** Content-based classification of a `.iso`, from `detect_disc_kinds`.
 * "xiso" = trimmed XDVDFS (ready for xemu / can be unpacked),
 * "redump" = full Xbox disc dump (can be trimmed into a playable XISO),
 * "gc-wii" = GameCube/Wii disc (RVZ candidate),
 * "iso" = not a recognised console disc (only generic CSO/CHD treatments apply). */
export type DiscKind = "xiso" | "redump" | "gc-wii" | "iso";

/** RVZ engine identifier. "nod" = native Rust library, "dolphin" = DolphinTool sidecar. */
export type RvzEngine = "nod" | "dolphin";
export type DiscKindDto = { path: string; kind: DiscKind };

export type SelectionAnalysisDto = {
  archives: string[];
  chdSources: ChdSourceDto[];
  restorableChds: string[];
  hasDirectories: boolean;
  hasFiles: boolean;
  uniqueExtensions: string[];
  chdMenuLabel: string | null;
  chdRestoreMenuLabel: string | null;
  hasRemoteDirectories: boolean;
  /** .bin files found with no matching .cue anywhere in the same folder. */
  orphanBins: string[];
};

// ── Remote transfer ─────────────────────────────────────────────

/** "abort" | "skip" | "pause" — what the backend does on a recoverable error */
export type RemoteTransferOnError = 'abort' | 'skip' | 'pause';

export type RemoteTransferPolicy = {
  onError: RemoteTransferOnError;
};

export type PreflightWarningKind = 'lowSpace' | 'criticalSpace' | 'noSpaceCheck';

export type PreflightWarning = {
  kind: PreflightWarningKind;
  detail: string;
};

export type PreflightCheckResult = {
  ok: boolean;
  warnings: PreflightWarning[];
};

/** Payload of the `job-paused` Tauri event */
export type JobPausedDto = {
  jobId: string;
  error: string;
  fileName: string;
  isRecoverable: boolean;
};

// ── CHD ─────────────────────────────────────────────────────────

export type ChdConversionOptionsPayload = {
  deleteOriginals: boolean;
  nameAsContainer: boolean;
  depositToParent: boolean;
  deleteOriginalSubfolders: boolean;
  overwrite: boolean;
  renameOnConflict?: boolean;
  customName: string | null;
  /** "same" = alongside source (default), "parent" = parent folder, "custom" = use destinationPath */
  destinationMode?: 'same' | 'parent' | 'custom' | null;
  /** Local destination folder when destinationMode == "custom". */
  destinationPath?: string | null;
  /** Virtual remote path. When set the CHD is written to temp then uploaded. */
  remoteDestination?: string | null;
  remoteTransfer?: RemoteTransferPolicy | null;
};

export type ChdRestoreNamingMode = 'chdStem' | 'custom';

export type ChdRestoreOptionsPayload = {
  individualFolders: boolean;
  destinationMode: "same" | "custom";
  destinationPath: string | null;
  /** How to name the extracted output files (.cue/.bin/.iso) */
  outputNamingMode: ChdRestoreNamingMode;
  /** Only used when outputNamingMode == "custom" and single CHD */
  customOutputName: string | null;
  /** How to name the individual subfolder (only when individualFolders) */
  folderNamingMode: ChdRestoreNamingMode;
  /** Only used when folderNamingMode == "custom" and single CHD */
  customFolderName: string | null;
  deleteChd: boolean;
  overwrite: boolean;
  renameOnConflict?: boolean;
  splitBin: boolean;
  /** Virtual remote path. When set output is written to temp then uploaded. */
  remoteDestination?: string | null;
  remoteTransfer?: RemoteTransferPolicy | null;
};

// ── Compression ─────────────────────────────────────────────────

export type DiscImageOptionsPayload = {
  destinationMode: 'same' | 'custom';
  destinationPath: string | null;
  deleteOriginals: boolean;
  overwrite: boolean;
  renameOnConflict?: boolean;
  compressionLevel: number;
  remoteDestination?: string | null;
  remoteTransfer?: RemoteTransferPolicy | null;
  /** RVZ engine to try first. Only used by RVZ modes. */
  rvzPrimaryEngine?: RvzEngine;
  /** RVZ engine to fall back to when the primary fails. */
  rvzFallbackEngine?: RvzEngine;
  /** Whether to try the fallback engine when the primary one fails. */
  rvzEnableFallback?: boolean;
};

export type CompressionFormat = 'zip' | '7z' | 'rar';

export type CompressionOptionsPayload = {
  format: CompressionFormat;
  archiveName: string;
  destinationMode: 'same' | 'custom';
  destinationPath: string | null;
  compressionLevel: number; // 0=store 3=fast 5=normal 9=max
  deleteOriginals: boolean;
  overwrite: boolean;
  renameOnConflict?: boolean;
  remoteDestination?: string | null;
  remoteTransfer?: RemoteTransferPolicy | null;
};

export type CompressionCapabilitiesDto = {
  canCompressZip: boolean;
  canCompress7z: boolean;
  canCompressRar: boolean;
};

export type ClipboardState = {
  paths: string[];
  operation: "copy" | "cut";
};

export type ViewMode = "list" | "grid";
export type ThemeMode = "light" | "dark";
export type ContentSortKey = "name" | "type" | "size" | "modified";
export type SortDirection = "asc" | "desc";
export type ContentColumnWidths = Record<ContentSortKey, number>;
export type Locale = "en" | "es";

export type VolumeDto = {
  path: string;
  name: string;
  label: string;
  totalBytes: number;
  freeBytes: number;
  isRemovable: boolean;
};

export type RemoteDiskUsageMethod = "sftpStatvfs" | "sshDf" | "unsupported" | "unavailable";

export type RemoteDiskUsageDto = {
  scopePath: string;
  totalBytes: number | null;
  freeBytes: number | null;
  usedBytes: number | null;
  method: RemoteDiskUsageMethod;
  note: string | null;
};

export type KnownFoldersDto = {
  home: string | null;
  desktop: string | null;
  documents: string | null;
  downloads: string | null;
  pictures: string | null;
  music: string | null;
  videos: string | null;
};

export type AppSettings = {
  theme: ThemeMode;
  locale: Locale;
  fontScale: number;
  compactUi: boolean;
  defaultViewMode: ViewMode;
  defaultContentZoom: number;
  defaultTreeZoom: number;
  favoriteLocations: string[];
  confirmDelete: boolean;
  hideDeleteProgressPopup: boolean;
  selectionWeightMaxDepth: number;
  selectionWeightCalculateAll: boolean;
  contentSortKey: ContentSortKey;
  contentSortDirection: SortDirection;
  contentColumnWidths: ContentColumnWidths;
  chdScanDepth: number;
  /** Max queued operations Dogu may run at once in smart execution mode. */
  queueMaxConcurrent: number;
  /** Whether queue mode starts enabled when Dogu launches. */
  defaultQueueMode: boolean;
  /** Show dependency/conflict relation lines in the queue panel. */
  showQueueRelationMap: boolean;
  /** Default "replace existing" behavior for operations without a dialog (copy/move)
   *  and the default state of the replace checkbox in operation dialogs. */
  defaultOverwriteOnConflict: boolean;
  /** When an operation does NOT overwrite and hits a name collision, append a
   *  numeric suffix " (2)", " (3)"… instead of failing/skipping. */
  renameOnConflict: boolean;
  /** RVZ engine tried first for GameCube/Wii conversions. Default "nod". */
  rvzPrimaryEngine: RvzEngine;
  /** Try the other engine when the primary one fails. Default true. */
  rvzEnableFallback: boolean;
  /** Engine used as fallback when enabled. Default "dolphin". */
  rvzFallbackEngine: RvzEngine;
};

export type NotificationKind = "error" | "warn" | "info" | "success";

export type NotificationEntry = {
  id: string;
  kind: NotificationKind;
  text: string;
  createdAt: number;
};

export type ProgressState = {
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

export type RenameState = {
  path: string;
  value: string;
};

export type AvailableShell = {
  name: string;
  path: string;
};

export type TerminalTabInfo = {
  id: string;
  title: string;
  cwd: string | null;
};

// ── M3U generation ──────────────────────────────────────────────

export type M3uOutputLocation = 'sameFolder' | 'scanRoot' | 'currentDir' | 'custom';

export type M3uScanOptions = {
  multiDiscOnly: boolean;
  recursive: boolean;
  outputLocation: M3uOutputLocation;
  customOutputPath: string | null;
  useRelativePaths: boolean;
};

export type M3uEntryDto = {
  absolutePath: string;
  m3uPath: string;
  discNumber: number;
  format: string;
};

export type M3uWarningDto = {
  kind: 'missingDisc' | 'duplicateDisc' | 'mixedFormats' | 'm3uExists';
  detail: string | null;
};

export type M3uGroupDto = {
  id: string;
  baseName: string;
  outputPath: string;
  m3uExists: boolean;
  entries: M3uEntryDto[];
  warnings: M3uWarningDto[];
};

export type M3uGenerateGroupPayload = {
  outputPath: string;
  baseName: string;
  entries: string[];
};

export type M3uGeneratePayload = {
  groups: M3uGenerateGroupPayload[];
  overwrite: boolean;
  renameOnConflict?: boolean;
};

export type M3uFailureDto = {
  path: string;
  error: string;
};

export type M3uGenerateResultDto = {
  created: string[];
  skipped: string[];
  failed: M3uFailureDto[];
};

// ── Operation queue ─────────────────────────────────────────────

export type DeaccentRenameResult = {
  oldPath: string;
  newPath: string;
};

export type QueuedOpKind =
  | 'copy'
  | 'move'
  | 'delete'
  | 'extract'
  | 'compress'
  | 'chd-convert'
  | 'chd-restore'
  | 'cso-convert'
  | 'cso-restore'
  | 'xiso-pack'
  | 'xiso-unpack'
  | 'rvz-convert'
  | 'rvz-restore'
  | 'm3u';

/**
 * A predicted file/folder that a queued operation will create once it runs.
 * For deterministic operations the `path` is exactly where the real file will
 * land, so a later operation can target it directly. For approximate outputs
 * (CHD restore) the path is a best guess and is resolved against reality at
 * execution time.
 */
export type GhostEntry = {
  /** Predicted concrete path (equals the real path once produced, for deterministic ops). */
  path: string;
  name: string;
  isDir: boolean;
  /** Parent directory of `path`, used to place the ghost in the explorer overlay. */
  parentDir: string;
  /** Lower-case extension without the dot ('' for directories). */
  format: string;
  /** True when the exact name/format/type isn't guaranteed until the op runs. */
  approximate: boolean;
  /** Id of the queued op that will produce this entry. */
  producedByOpId: string;
};

export type QueuedOp = {
  id: string;
  title: string;
  kind: QueuedOpKind;
  batchId?: string;
  batchTitle?: string;
  batchIndex?: number;
  batchTotal?: number;
  sources: string[];
  destinations: string[];
  deletes: string[];
  /** Files/folders this op will create (ghost outputs). */
  produces: GhostEntry[];
  /** Ids of queued ops whose ghost outputs this op consumes as input. */
  dependsOn: string[];
  /** Effective conflict policy — drives the overlay's replace/rename/conflict rendering. */
  overwrite: boolean;
  renameOnConflict: boolean;
  execute: (jobId?: string, retryQueuedOp?: QueuedOp | null) => Promise<boolean>;
  /** For accent-unsafe ops: rebuilds a fresh op with source paths remapped after a
   *  de-accent rename (oldPath → newPath). Absent for ops that never need it. */
  rebuildWithRenames?: (renamed: Map<string, string>) => QueuedOp;
};

export type ConflictKind = 'source-deleted' | 'dest-deleted' | 'dest-collision';

// 'blocking'      → dangerous in sequential AND parallel (deleter runs first in queue)
// 'parallel-only' → safe in sequential (order in queue protects it), dangerous in parallel
export type ConflictSeverity = 'blocking' | 'parallel-only';

export type QueueConflict = {
  opAId: string;
  opBId: string;
  kind: ConflictKind;
  severity: ConflictSeverity;
  pathA: string;
  pathB: string;
};
