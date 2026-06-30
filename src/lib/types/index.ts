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
};

export type AppMetadataDto = {
  appVersion: string;
  chdman: Record<string, unknown>;
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
  destinationMode: "same" | "custom";
  destinationPath: string | null;
  deleteArchives: boolean;
  overwrite: boolean;
};

export type ExtractionPreviewRow = {
  archivePath: string;
  destinationPath: string;
};

export type ChdSourceDto = {
  sourcePath: string;
  containerDir: string;
  command: string;
  displayExtensions: string[];
  requiredPaths: string[];
};

export type SelectionAnalysisDto = {
  archives: string[];
  chdSources: ChdSourceDto[];
  restorableChds: string[];
  hasDirectories: boolean;
  hasFiles: boolean;
  uniqueExtensions: string[];
  chdMenuLabel: string | null;
  chdRestoreMenuLabel: string | null;
};

export type ChdConversionOptionsPayload = {
  deleteOriginals: boolean;
  nameAsContainer: boolean;
  depositToParent: boolean;
  deleteOriginalSubfolders: boolean;
  overwrite: boolean;
};

export type ChdRestoreOptionsPayload = {
  individualFolders: boolean;
  destinationMode: "same" | "custom";
  destinationPath: string | null;
  deleteChd: boolean;
  overwrite: boolean;
  splitBin: boolean;
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

export type AppSettings = {
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

export type NotificationKind = "error" | "info" | "success";

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
