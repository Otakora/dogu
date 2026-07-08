import { dirnameOf, normalizePath } from "./ghosts.js";

export type FileTransferItem = {
  path: string;
  isDir: boolean;
};

export type FileTransferOperation = "copy" | "move";

export function remoteSessionIdOf(path: string): string | null {
  const match = path.match(/^remote:\/\/([^/]+)(?:\/|$)/);
  return match?.[1] ?? null;
}

export function localDriveOf(path: string): string | null {
  const match = path.match(/^([A-Za-z]:)[\\/]/);
  return match?.[1]?.toUpperCase() ?? null;
}

export function inferFileTransferOperation(
  items: FileTransferItem[],
  dest: string,
  preferredOperation?: FileTransferOperation | null,
): FileTransferOperation {
  if (preferredOperation) return preferredOperation;

  const sourceRemoteSessions = new Set(
    items.map((item) => remoteSessionIdOf(item.path)).filter((value): value is string => !!value),
  );
  const hasRemoteSources = sourceRemoteSessions.size > 0;
  const hasLocalSources = items.some((item) => !remoteSessionIdOf(item.path));
  const destRemoteSession = remoteSessionIdOf(dest);

  if (!hasRemoteSources && !destRemoteSession) {
    const sourceDrives = new Set(
      items.map((item) => localDriveOf(item.path)).filter((value): value is string => !!value),
    );
    const destDrive = localDriveOf(dest);
    return sourceDrives.size === 1 && !!destDrive && sourceDrives.has(destDrive) ? "move" : "copy";
  }

  if (!hasLocalSources && !!destRemoteSession) {
    return sourceRemoteSessions.size === 1 && sourceRemoteSessions.has(destRemoteSession) ? "move" : "copy";
  }

  return "copy";
}

export function canDropFileItemsToPath(items: FileTransferItem[], dest: string): boolean {
  const destKey = normalizePath(dest);
  return !items.some((item) => {
    const itemKey = normalizePath(item.path);
    return itemKey === destKey || (item.isDir && destKey.startsWith(itemKey + "/"));
  });
}

export function isNoOpFileTransfer(
  items: FileTransferItem[],
  dest: string,
  operation: FileTransferOperation,
): boolean {
  return operation === "move" && items.every((item) => normalizePath(dirnameOf(item.path)) === normalizePath(dest));
}

export function isRecursiveFileTransfer(items: FileTransferItem[], dest: string): boolean {
  const destKey = normalizePath(dest);
  return items.some((item) => {
    if (!item.isDir) return false;
    const itemKey = normalizePath(item.path);
    return destKey === itemKey || destKey.startsWith(itemKey + "/");
  });
}
