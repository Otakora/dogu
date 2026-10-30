/** Select in the order actually rendered, including queued ghost entries. */
export function visibleRange(paths: string[], from: string, to: string): string[] {
  const a = paths.indexOf(from), b = paths.indexOf(to);
  return a < 0 || b < 0 ? [] : paths.slice(Math.min(a, b), Math.max(a, b) + 1);
}

/** Keep the initial selection fixed while the area changes size. */
export function mergeAreaSelection(base: Iterable<string>, inside: Iterable<string>): string[] {
  return [...new Set([...base, ...inside])];
}
