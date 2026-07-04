// Ghost-file prediction utilities.
//
// A "ghost" is a file or folder that a queued operation will create once it
// runs, shown in the explorer before it exists so the user can queue further
// operations on top of it. For deterministic operations the predicted path is
// exactly where the real file lands; approximate predictions (CHD restore) are
// reconciled against reality at execution time.

import type {
  GhostEntry,
  EntryDto,
  ChdConversionOptionsPayload,
  ChdRestoreOptionsPayload,
  CompressionOptionsPayload,
} from "../types/index.js";

// ── Path helpers (handle Windows '\', POSIX '/', and remote:// paths) ──────

const REMOTE_PREFIX = "remote://";

/** Splits a `remote://session/logical/path` into [`remote://session`, `/logical/path`]. */
function splitRemote(path: string): [string, string] | null {
  if (!path.startsWith(REMOTE_PREFIX)) return null;
  const rest = path.slice(REMOTE_PREFIX.length);
  const slash = rest.indexOf("/");
  if (slash === -1) return [REMOTE_PREFIX + rest, "/"];
  return [REMOTE_PREFIX + rest.slice(0, slash), rest.slice(slash)];
}

/** Preferred separator for a given path (backslash for Windows-style paths). */
function sepOf(path: string): string {
  if (path.startsWith(REMOTE_PREFIX)) return "/";
  return path.includes("\\") ? "\\" : "/";
}

export function basenameOf(path: string): string {
  const remote = splitRemote(path);
  const logical = remote ? remote[1] : path;
  const trimmed = logical.replace(/[/\\]+$/, "");
  const parts = trimmed.split(/[/\\]/);
  return parts[parts.length - 1] || trimmed;
}

export function dirnameOf(path: string): string {
  const remote = splitRemote(path);
  if (remote) {
    const [session, logical] = remote;
    const trimmed = logical.replace(/\/+$/, "");
    const slash = trimmed.lastIndexOf("/");
    if (slash <= 0) return `${session}/`;
    return `${session}${trimmed.slice(0, slash)}`;
  }
  const trimmed = path.replace(/[/\\]+$/, "");
  const idx = Math.max(trimmed.lastIndexOf("/"), trimmed.lastIndexOf("\\"));
  if (idx < 0) return trimmed;
  // Preserve a trailing slash for filesystem roots ("C:\" or "/").
  if (idx === 0) return trimmed.slice(0, 1) || "/";
  if (/^[a-zA-Z]:$/.test(trimmed.slice(0, idx))) return trimmed.slice(0, idx + 1);
  return trimmed.slice(0, idx);
}

export function joinPath(dir: string, name: string): string {
  const sep = sepOf(dir);
  const trimmed = dir.replace(/[/\\]+$/, "");
  return `${trimmed}${sep}${name}`;
}

/** File name without its final extension. */
export function stemOf(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(0, dot) : name;
}

/** Lower-case extension without the dot ('' when there is none). */
export function extOf(name: string): string {
  const dot = name.lastIndexOf(".");
  return dot > 0 ? name.slice(dot + 1).toLowerCase() : "";
}

/** Best-effort guess of whether a path denotes a directory (no file extension). */
function looksLikeDir(name: string): boolean {
  return extOf(name) === "";
}

// ── Normalisation for overlay/dependency matching ──────────────────────────

/** Case-insensitive, separator-agnostic, trailing-slash-free key for a path. */
export function normalizePath(path: string): string {
  return path.replace(/[/\\]+$/, "").replace(/\\/g, "/").toLowerCase();
}

export function sameDir(a: string, b: string): boolean {
  return normalizePath(a) === normalizePath(b);
}

/**
 * Mirrors the backend `unique_path`: if `desired` is already taken, returns the
 * first "stem (N)" variant (N≥2, extension preserved) not present in `taken`
 * (a set of normalized paths). Used to display the name a renamed output will get.
 */
export function uniqueDisplayPath(desired: string, taken: Set<string>): string {
  if (!taken.has(normalizePath(desired))) return desired;
  const dir = dirnameOf(desired);
  const name = basenameOf(desired);
  const dot = name.lastIndexOf(".");
  const stem = dot > 0 ? name.slice(0, dot) : name;
  const ext = dot > 0 ? name.slice(dot) : ""; // includes the leading dot
  let n = 2;
  // eslint-disable-next-line no-constant-condition
  while (true) {
    const candidate = joinPath(dir, `${stem} (${n})${ext}`);
    if (!taken.has(normalizePath(candidate))) return candidate;
    n++;
  }
}

// ── Ghost construction ─────────────────────────────────────────────────────

function makeGhost(path: string, isDir: boolean, opId: string, approximate: boolean): GhostEntry {
  const name = basenameOf(path);
  return {
    path,
    name,
    isDir,
    parentDir: dirnameOf(path),
    format: isDir ? "" : extOf(name),
    approximate,
    producedByOpId: opId,
  };
}

/** Public constructor for a single ghost (e.g. from an extraction preview). */
export function buildGhost(path: string, isDir: boolean, opId: string, approximate = false): GhostEntry {
  return makeGhost(path, isDir, opId, approximate);
}

/** Turns a ghost into a display EntryDto so it can flow through the file list. */
export function ghostToEntry(g: GhostEntry): EntryDto {
  return {
    path: g.path,
    name: g.name,
    isDir: g.isDir,
    size: 0,
    sizeLabel: "—",
    modifiedTs: 0,
    modifiedLabel: "—",
    // Real entries carry a dot-prefixed extension (".cue"); match that.
    extension: g.format ? `.${g.format}` : "",
    hasChildren: false,
    hasDirectoryChildren: false,
    locationKind: "ghost",
    displayPath: g.path,
    rootLabel: null,
    isGhost: true,
    ghostOpId: g.producedByOpId,
    ghostApproximate: g.approximate,
  };
}

// ── Predictors ─────────────────────────────────────────────────────────────

/** Copy/move: each source reappears at `destDir/basename(source)`. */
export function predictCopyMove(sources: string[], destDir: string, opId: string): GhostEntry[] {
  return sources.map((src) => {
    const name = basenameOf(src);
    return makeGhost(joinPath(destDir, name), looksLikeDir(name), opId, false);
  });
}

/** Compress: a single archive at `destDir/archiveName.format`. */
export function predictCompress(opts: CompressionOptionsPayload, destDir: string, opId: string): GhostEntry[] {
  const name = `${opts.archiveName}.${opts.format}`;
  return [makeGhost(joinPath(destDir, name), false, opId, false)];
}

/**
 * CHD convert: one `.chd` per source. Ported from `build_chd_output_path`
 * (src-tauri/src/ops.rs). `containerDir` is the folder holding the source disc
 * image; here we approximate it as the source's parent directory.
 *
 * The `depositToParent` option can relocate the output to the originally
 * selected root, which we can't know at prediction time — in that case the
 * ghost is flagged approximate so the overlay doesn't over-promise its folder.
 */
export function predictChdConvert(
  sources: string[],
  opts: ChdConversionOptionsPayload,
  opId: string,
): GhostEntry[] {
  const isSingle = sources.length === 1;
  return sources.map((src) => {
    const srcDir = dirnameOf(src);
    const containerName = basenameOf(srcDir);

    let stem: string;
    const custom = opts.customName?.trim();
    if (isSingle && custom) {
      stem = custom.replace(/\.chd$/i, "");
    } else if (opts.nameAsContainer) {
      stem = containerName;
    } else {
      stem = stemOf(basenameOf(src));
    }

    const fileName = `${stem}.chd`;

    if (opts.destinationMode === "custom" && opts.destinationPath) {
      return makeGhost(joinPath(opts.destinationPath, fileName), false, opId, false);
    }
    if (opts.remoteDestination) {
      return makeGhost(joinPath(opts.remoteDestination, fileName), false, opId, false);
    }
    // depositToParent relocates to the selected root — unknown here.
    return makeGhost(joinPath(srcDir, fileName), false, opId, opts.depositToParent);
  });
}

/**
 * CHD restore: each `.chd` yields either a `.cue`+`.bin` pair (CD) or an `.iso`
 * (DVD) — Dogu only knows which by trying at runtime. Predictions are therefore
 * always approximate. We surface the most common CD case (`.cue` + `.bin`).
 */
export function predictChdRestore(
  chdPaths: string[],
  opts: ChdRestoreOptionsPayload,
  opId: string,
): GhostEntry[] {
  const ghosts: GhostEntry[] = [];
  for (const chd of chdPaths) {
    const stem = stemOf(basenameOf(chd));
    let destDir: string;
    if (opts.destinationMode === "custom" && opts.destinationPath) {
      destDir = opts.destinationPath;
    } else if (opts.remoteDestination) {
      destDir = opts.remoteDestination;
    } else {
      destDir = dirnameOf(chd);
    }
    if (opts.individualFolders) {
      const folderName =
        opts.folderNamingMode === "custom" && opts.customFolderName ? opts.customFolderName : stem;
      destDir = joinPath(destDir, folderName);
      ghosts.push(makeGhost(destDir, true, opId, true));
    }
    const outName =
      opts.outputNamingMode === "custom" && opts.customOutputName ? opts.customOutputName : stem;
    ghosts.push(makeGhost(joinPath(destDir, `${outName}.cue`), false, opId, true));
    ghosts.push(makeGhost(joinPath(destDir, `${outName}.bin`), false, opId, true));
  }
  return ghosts;
}
