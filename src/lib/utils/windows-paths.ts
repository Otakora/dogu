/** Conservative Win32 compatibility limit, including the terminating NUL. */
export function windowsPathIssue(path: string): boolean {
  if (!/^(?:[a-z]:[\\/]|\\\\)/i.test(path) || path.startsWith("remote://")) return false;
  return path.length >= 260 || path.split(/[\\/]/).some(component => component.length > 255);
}

export function firstLongWindowsPath(paths: string[]): string | undefined {
  return paths.find(windowsPathIssue);
}

export function isLongPathError(message: string): boolean {
  return /\[PATH_TOO_LONG\]|os error (?:206|111)\b|(?:error|winerror)\s*(?:206|111)\b|file\s*name (?:or (?:the )?extension )?(?:is )?too long|path (?:is |name is )?too long|file name, or both,? (?:are|is) too long|nombre de archivo (?:o la extensi[oó]n )?(?:es )?demasiado largo/i.test(message);
}
