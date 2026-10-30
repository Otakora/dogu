import type { ExtractionOptionsPayload, ExtractionPreviewRow, GhostEntry } from "../types/index.js";
import { basenameOf, buildGhost, dirnameOf, joinPath, normalizePath, stemOf } from "./ghosts.js";

/** The execution backend gives an explicit remote destination priority. */
export function extractionDestination(archive: string, options: ExtractionOptionsPayload): string {
  let base = options.remoteDestination || (options.destinationMode === "custom" && options.destinationPath
    ? options.destinationPath : dirnameOf(archive));
  if (!options.remoteDestination && options.destinationMode === "parent") base = dirnameOf(base);
  return options.sharedFolderName ? joinPath(base, options.sharedFolderName) : base;
}

/** Rebase a local archive listing onto its real local or virtual destination. */
export function extractionGhosts(archive: string, options: ExtractionOptionsPayload, opId: string, row?: ExtractionPreviewRow): GhostEntry[] {
  const base = extractionDestination(archive, options);
  const target = options.individualFolders ? joinPath(base, stemOf(basenameOf(archive))) : base;
  const shared = options.sharedFolderName ? [{ ...buildGhost(base, true, opId), sharedDirectory: true }] : [];
  if (!row) return [...shared, ...(options.individualFolders ? [buildGhost(target, true, opId, true)] : [])];
  const root = row.destinationRoot.replace(/\\/g, "/").replace(/\/+$/, "");
  const ghosts: GhostEntry[] = [...shared];
  for (const entry of row.entries) {
    const path = entry.destinationPath.replace(/\\/g, "/");
    const rootKey = normalizePath(row.destinationRoot);
    const pathKey = normalizePath(entry.destinationPath);
    if (pathKey !== rootKey && !pathKey.startsWith(rootKey.endsWith("/") ? rootKey : `${rootKey}/`)) continue;
    const relative = pathKey === rootKey ? "" : path.slice(root.length).replace(/^\/+/, "");
    const finalPath = relative.split("/").filter(Boolean).reduce((parent, name) => joinPath(parent, name), target);
    ghosts.push(buildGhost(finalPath, entry.isDir, opId));
  }
  return ghosts;
}

/** Remove a trailing disc marker while preserving region and other archive tags. */
export function stripDiscMarker(name: string): string {
  return name.replace(/\s*(?:\(\s*(?:disc|disk|cd)\s*[-_.]?\s*\d+\s*\)|\[\s*(?:disc|disk|cd)\s*[-_.]?\s*\d+\s*\])\s*$/i, "").trim();
}
