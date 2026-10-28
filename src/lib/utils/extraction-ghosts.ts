import type { ExtractionOptionsPayload, ExtractionPreviewRow, GhostEntry } from "../types/index.js";
import { basenameOf, buildGhost, dirnameOf, joinPath, normalizePath, stemOf } from "./ghosts.js";

/** The execution backend gives an explicit remote destination priority. */
export function extractionDestination(archive: string, options: ExtractionOptionsPayload): string {
  return options.remoteDestination || (options.destinationMode === "custom" && options.destinationPath
    ? options.destinationPath : dirnameOf(archive));
}

/** Rebase a local archive listing onto its real local or virtual destination. */
export function extractionGhosts(archive: string, options: ExtractionOptionsPayload, opId: string, row?: ExtractionPreviewRow): GhostEntry[] {
  const base = extractionDestination(archive, options);
  const target = options.individualFolders ? joinPath(base, stemOf(basenameOf(archive))) : base;
  if (!row) return options.individualFolders ? [buildGhost(target, true, opId, true)] : [];
  const root = row.destinationRoot.replace(/\\/g, "/").replace(/\/+$/, "");
  const ghosts: GhostEntry[] = [];
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
