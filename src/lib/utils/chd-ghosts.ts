import { basenameOf, dirnameOf, normalizePath, stemOf } from "./ghosts.js";

/** Predict only BIN names that unambiguously follow this CUE's common track naming. */
export function predictedCueTracks(cuePath: string, candidates: string[]): string[] {
  const dir = normalizePath(dirnameOf(cuePath));
  const stem = stemOf(basenameOf(cuePath));
  const escaped = stem.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
  const pattern = new RegExp(`^${escaped}(?: \\(Track [0-9]+\\))?\\.bin$`, "i");
  return candidates.filter(path => normalizePath(dirnameOf(path)) === dir && pattern.test(basenameOf(path)));
}

/** A queued container can disappear only if all known files are removed and no output stays inside it. */
export function predictsEmptyGhostContainer(
  dir: string,
  ghosts: { path: string; isDir: boolean }[],
  requiredPaths: string[],
  outputDirs: string[],
): boolean {
  const key = normalizePath(dir);
  if (!ghosts.some(ghost => ghost.isDir && normalizePath(ghost.path) === key)) return false;
  const required = new Set(requiredPaths.map(normalizePath));
  const files = ghosts.filter(ghost => !ghost.isDir && normalizePath(ghost.path).startsWith(key + "/"));
  const outputInside = outputDirs.some(output => {
    const outputKey = normalizePath(output);
    return outputKey === key || outputKey.startsWith(key + "/");
  });
  return !outputInside && files.length > 0 && files.every(file => required.has(normalizePath(file.path)));
}
