// Non-ASCII (accent) handling for flows whose external tool can't cope with them.
//
// Verified empirically (Rust harness, the real spawn path): the ONLY bundled tool
// that mishandles non-ASCII paths on Windows is chdman (MAME). 7-Zip and DolphinTool
// handle them fine, and native Rust ops always do. chdman is worked around for
// accented *folders* (cwd + relative names, see run_chdman_convert/restore in the
// backend), so only accented *file names* remain a problem — those are what this
// module detects and offers to de-accent.

import { basenameOf, dirnameOf, joinPath } from "./ghosts.js";

/** True if the string contains any character outside the ASCII range. */
export function hasNonAscii(s: string): boolean {
  return /[^\x00-\x7f]/.test(s);
}

/** Strips diacritics via Unicode NFD decomposition (á→a, ñ→n, ü→u, …). Characters
 *  with no ASCII base (CJK, emoji) are left as-is, so the result may still be
 *  non-ASCII — callers must check with `hasNonAscii`. */
export function deaccent(s: string): string {
  return s.normalize("NFD").replace(/[̀-ͯ]/g, "");
}

/** Op kinds whose external tool cannot handle non-ASCII file names (chdman only). */
const ACCENT_UNSAFE_KINDS = new Set(["chd-convert", "chd-restore"]);

export function isAccentUnsafeKind(kind: string): boolean {
  return ACCENT_UNSAFE_KINDS.has(kind);
}

export type AccentIssue = {
  /** Full path of the offending file (its base name carries the accents). */
  path: string;
  /** Current base name. */
  oldName: string;
  /** De-accented base name to rename it to. */
  newName: string;
  /** New full path after renaming (dir + newName). */
  newPath: string;
  /** True when `newName` is pure ASCII (a rename would actually fix it). */
  fixable: boolean;
};

/**
 * Finds files whose BASE NAME has accents among an op's source paths. Only the
 * leaf name matters — accented parent folders are handled by the backend cwd
 * workaround, so they are deliberately NOT flagged.
 */
export function collectAccentIssues(paths: string[]): AccentIssue[] {
  const issues: AccentIssue[] = [];
  const seen = new Set<string>();
  for (const p of paths) {
    if (seen.has(p)) continue;
    seen.add(p);
    const oldName = basenameOf(p);
    if (!hasNonAscii(oldName)) continue;
    const newName = deaccent(oldName);
    issues.push({
      path: p,
      oldName,
      newName,
      newPath: joinPath(dirnameOf(p), newName),
      fixable: !hasNonAscii(newName),
    });
  }
  return issues;
}
