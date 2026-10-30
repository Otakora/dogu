import type { QueuedOp, QueueConflict, ConflictKind, ConflictSeverity, GhostEntry } from "../types/index.js";
import { normalizePath, uniqueDisplayPath, basenameOf, dirnameOf, pathContains, joinPath, stemOf } from "./ghosts.js";

export type QueuePlan = {
  total: number;
  invalidDependencies: boolean;
  hasDependencies: boolean;
  hasOrderingConstraints: boolean;
  dependencyEdges: number;
  orderingEdges: number;
  maxParallelWidth: number;
  maxConcurrent: number;
  firstWave: number;
  levels: string[][];
  blockingConflicts: QueueConflict[];
  hardPredecessors: Map<string, Set<string>>;
  hardSuccessors: Map<string, Set<string>>;
  orderPredecessors: Map<string, Set<string>>;
};

// ─── Queue conflict detection ──────────────────────────────
function pathContainsOther(parent: string, child: string): boolean {
  const p = normalizePath(parent);
  const c = normalizePath(child);
  return c === p || c.startsWith(p.endsWith("/") ? p : p + "/");
}

function pathsOverlap(a: string, b: string): boolean {
  return pathContainsOther(a, b) || pathContainsOther(b, a);
}

/** Whether the backend's "name (N).ext" policy can pick `candidate`. */
function renamedPathCanEqual(requested: string, candidate: string): boolean {
  if (normalizePath(dirnameOf(requested)) !== normalizePath(dirnameOf(candidate))) return false;
  const requestedName = basenameOf(normalizePath(requested));
  const candidateName = basenameOf(normalizePath(candidate));
  const dot = requestedName.lastIndexOf(".");
  const stem = dot > 0 ? requestedName.slice(0, dot) : requestedName;
  const ext = dot > 0 ? requestedName.slice(dot) : "";
  if (!candidateName.startsWith(`${stem} (`) || !candidateName.endsWith(`)${ext}`)) return false;
  const number = candidateName.slice(stem.length + 2, candidateName.length - ext.length - 1);
  return /^[0-9]+$/.test(number) && Number(number) >= 1 && String(Number(number)) === number;
}

/** Simplify only the drawing: keep execution edges unchanged in the plan. */
export function simplifyRelationLinks<T extends { fromId: string; toId: string; kind: string }>(links: T[]): T[] {
  const unique = links.filter((link, index) => links.findIndex(other => other.kind === link.kind && other.fromId === link.fromId && other.toId === link.toId) === index);
  return unique.filter(link => {
    if (link.kind === "blocking") return true;
    const visited = new Set([link.fromId]);
    const todo = [link.fromId];
    while (todo.length) {
      const current = todo.pop()!;
      for (const edge of unique) {
        if (edge === link || edge.kind !== link.kind || edge.fromId !== current || visited.has(edge.toId)) continue;
        if (edge.toId === link.toId) return false;
        visited.add(edge.toId);
        todo.push(edge.toId);
      }
    }
    return true;
  });
}

function deleteVsSourceSeverity(deletePath: string, sourcePath: string): ConflictSeverity | null {
  // Deleting the source itself (or one of its ancestors) makes a later consumer
  // impossible even in sequential order.
  if (pathContainsOther(deletePath, sourcePath)) return "blocking";
  // Deleting something *inside* a source directory just changes that tree's
  // contents; queue order still makes sequential execution safe.
  if (pathContainsOther(sourcePath, deletePath)) return "parallel-only";
  return null;
}

/**
 * Resolves the final output paths of every queued op against the outputs of
 * earlier ops and currently known real entries, applying rename-on-conflict.
 * These are predictions, not reservations: the filesystem may change before
 * execution. Dependent paths are checked before and after their producer runs.
 */
export function resolveQueueOutputs(ops: QueuedOp[], knownPaths: string[] = []): Map<string, GhostEntry[]> {
  const taken = new Set(knownPaths.map(normalizePath));
  const map = new Map<string, GhostEntry[]>();
  for (const op of ops) {
    const resolved: GhostEntry[] = [];
    const directoryMappings: Array<{ original: string; actual: string }> = [];
    // CHD restore names a related set (CUE/BIN/ISO/GDI) with one common
    // stem; choosing each extension independently would produce an invalid pair.
    if (op.kind === "chd-restore" && !op.fixedOutputs && op.sources.length === 1 && op.produces.length) {
      const folder = op.produces.find(g => g.isDir);
      const desiredFolder = folder?.path;
      const actualFolder = folder && !op.overwrite && op.renameOnConflict ? uniqueDisplayPath(folder.path, taken) : desiredFolder;
      const firstFile = op.produces.find(g => !g.isDir);
      const outputDir = actualFolder ?? (firstFile ? dirnameOf(firstFile.path) : "");
      const originalStem = firstFile ? stemOf(basenameOf(firstFile.path)) : "";
      let chosenStem = originalStem;
      if (firstFile && !op.overwrite && op.renameOnConflict) {
        let number = 1;
        while ([".cue", ".bin", ".iso", ".gdi"].some(ext => taken.has(normalizePath(joinPath(outputDir, chosenStem + ext))))) {
          chosenStem = `${originalStem} (${number++})`;
        }
      }
      for (const ghost of op.produces) {
        const path = ghost.isDir ? (actualFolder ?? ghost.path) : joinPath(outputDir, `${chosenStem}.${ghost.format}`);
        const mapped = path === ghost.path ? ghost : { ...ghost, path, name: basenameOf(path), parentDir: dirnameOf(path) };
        taken.add(normalizePath(path));
        resolved.push(mapped);
      }
      map.set(op.id, resolved);
      continue;
    }
    for (const [index, g] of op.produces.entries()) {
      const fixed = op.fixedOutputs?.[index];
      const mapping = directoryMappings.filter(dir => normalizePath(g.path).startsWith(normalizePath(dir.original) + "/"))
        .sort((a, b) => b.original.length - a.original.length)[0];
      let path = fixed?.path ?? (mapping ? mapping.actual + g.path.slice(mapping.original.length) : g.path);
      if (!fixed && !g.sharedDirectory && taken.has(normalizePath(path)) && !op.overwrite && op.renameOnConflict) {
        path = uniqueDisplayPath(path, taken);
      }
      if (g.isDir && normalizePath(path) !== normalizePath(g.path)) directoryMappings.push({ original: g.path, actual: path });
      taken.add(normalizePath(path));
      resolved.push(path === g.path ? g : { ...g, path, name: basenameOf(path), parentDir: dirnameOf(path) });
    }
    map.set(op.id, resolved);
  }
  return map;
}

export function detectConflicts(ops: QueuedOp[], knownPaths: string[] = []): QueueConflict[] {
  const conflicts: QueueConflict[] = [];
  // Use projected outputs for read/delete relationships; reserve requested
  // output names separately because runtime renaming is not atomic.
  const resolvedOutputs = resolveQueueOutputs(ops, knownPaths);

  function push(opAId: string, opBId: string, kind: ConflictKind, severity: ConflictSeverity, pathA: string, pathB: string) {
    if (!conflicts.some(c => c.opAId === opAId && c.opBId === opBId && c.kind === kind)) {
      conflicts.push({ opAId, opBId, kind, severity, pathA, pathB });
    }
  }

  // Conflicts are computed against the actual output FILES each op creates
  // (`produces`), not the destination folders — deleting or writing a different
  // file inside a shared folder is not a conflict.
  const outputsOf = (op: QueuedOp): string[] =>
    (resolvedOutputs.get(op.id) ?? op.produces).map((g) => g.path);

  for (let i = 0; i < ops.length; i++) {
    for (let j = i + 1; j < ops.length; j++) {
      const a = ops[i], b = ops[j];
      const outA = outputsOf(a);
      const outB = outputsOf(b);

      // Reserve the requested names, not only their predicted renamed names:
      // the backend chooses unique names at execution time and concurrent
      // existence checks are not atomic.
      for (const pa of a.produces.filter(g => !g.sharedDirectory).map(g => g.path)) {
        for (const pb of b.produces.filter(g => !g.sharedDirectory).map(g => g.path)) {
          const sameName = normalizePath(pa) === normalizePath(pb);
          // Only reserve names that the other operation can actually choose.
          // Sharing a folder does not make unrelated filenames collide.
          const renameCollision = (a.renameOnConflict && !a.overwrite && renamedPathCanEqual(pa, pb))
            || (b.renameOnConflict && !b.overwrite && renamedPathCanEqual(pb, pa));
          if (sameName || renameCollision) push(a.id, b.id, "dest-collision", "parallel-only", pa, pb);
        }
      }
      // No preview means extraction outputs are unknown. Be conservative when
      // another operation writes into that same tree, regardless of renaming.
      const unknownA = a.kind === "extract" && !a.produces.some(g => !g.sharedDirectory);
      const unknownB = b.kind === "extract" && !b.produces.some(g => !g.sharedDirectory);
      if (unknownA || unknownB) {
        const pathsA = unknownA ? a.destinations : a.produces.map(g => g.path);
        const pathsB = unknownB ? b.destinations : b.produces.map(g => g.path);
        for (const pa of pathsA) for (const pb of pathsB) {
          if (pathsOverlap(pa, pb)) push(a.id, b.id, "dest-collision", "parallel-only", pa, pb);
        }
      }
      // A reader must finish before a later writer changes its input.
      for (const source of a.sources) {
        for (const output of outB) {
          if (pathsOverlap(source, output)) push(a.id, b.id, "source-written", "parallel-only", source, output);
        }
      }

      // `a` deletes something `b` needs as input, or is going to create.
      for (const d of a.deletes) {
        for (const s of b.sources) {
          const severity = deleteVsSourceSeverity(d, s);
          if (severity) push(a.id, b.id, "source-deleted", severity, d, s);
        }
        for (const p of outB)      { if (pathsOverlap(d, p)) push(a.id, b.id, "dest-deleted",   "parallel-only", d, p); }
      }

      for (const d of b.deletes) {
        for (const s of a.sources) {
          if (deleteVsSourceSeverity(d, s)) push(b.id, a.id, "source-deleted", "parallel-only", d, s);
        }
        for (const p of outA)      { if (pathsOverlap(d, p)) push(b.id, a.id, "dest-deleted",   "parallel-only", d, p); }
      }
    }
  }

  return conflicts;
}

// ─── Ghost dependency helpers ──────────────────────────────
/** Returns the id of the queued op that produces `path` (or contains it), if any. */
function ghostProducerOf(path: string, ops: QueuedOp[], knownPaths: string[]): string | null {
  const np = normalizePath(path);
  const resolvedOutputs = resolveQueueOutputs(ops, knownPaths);
  for (const op of ops) {
    for (const g of resolvedOutputs.get(op.id) ?? op.produces) {
      const gp = normalizePath(g.path);
      if (np === gp || (g.isDir && np.startsWith(gp.endsWith("/") ? gp : gp + "/"))) return op.id;
    }
  }
  return null;
}

function ghostProducersTouchingPath(path: string, ops: QueuedOp[], includeDescendantGhosts: boolean, knownPaths: string[]): string[] {
  const np = normalizePath(path);
  const producers = new Set<string>();
  const resolvedOutputs = resolveQueueOutputs(ops, knownPaths);
  for (const op of ops) {
    for (const g of resolvedOutputs.get(op.id) ?? op.produces) {
      const gp = normalizePath(g.path);
      const pathConsumesGhost = np === gp || (g.isDir && np.startsWith(gp.endsWith("/") ? gp : gp + "/"));
      const pathContainsGhost = includeDescendantGhosts && gp.startsWith(np.endsWith("/") ? np : np + "/");
      if (pathConsumesGhost || pathContainsGhost) producers.add(op.id);
    }
  }
  return [...producers];
}

/** Computes which already-queued ops an op depends on (consumes their outputs). */
export function linkDependencies(op: QueuedOp, existing: QueuedOp[], knownPaths: string[] = []): string[] {
  const deps = new Set<string>();
  for (const input of [...op.sources, ...op.deletes]) {
    for (const producer of ghostProducersTouchingPath(input, existing, true, knownPaths)) {
      if (producer !== op.id) deps.add(producer);
    }
  }
  for (const input of op.destinations) {
    const producer = ghostProducerOf(input, existing, knownPaths);
    if (producer && producer !== op.id) deps.add(producer);
  }
  // Recreating a path freed by an earlier delete requires that delete to
  // succeed, not merely finish: otherwise an overwrite could destroy it.
  for (const prior of existing) {
    if (prior.deletes.some(deleted => op.produces.some(output => pathsOverlap(deleted, output.path)))) deps.add(prior.id);
  }
  return [...deps];
}

/** Transitive closure of ops that (directly or indirectly) depend on any of `ids`. */
export function collectDependents(ids: string[], ops: QueuedOp[]): Set<string> {
  const result = new Set(ids);
  let changed = true;
  while (changed) {
    changed = false;
    for (const op of ops) {
      if (result.has(op.id)) continue;
      if (op.dependsOn.some((d) => result.has(d))) {
        result.add(op.id);
        changed = true;
      }
    }
  }
  return result;
}

/** True when every op appears after all the ops it depends on. */
export function orderRespectsDeps(ops: QueuedOp[]): boolean {
  const indexOf = new Map(ops.map((o, i) => [o.id, i]));
  return ops.every((op, i) => op.dependsOn.every((d) => indexOf.has(d) && indexOf.get(d)! < i));
}

export function buildQueuePlan(ops: QueuedOp[], conflicts: QueueConflict[], maxConcurrent: number): QueuePlan {
  const ids = new Set(ops.map((o) => o.id));
  let invalidDependencies = ids.size !== ops.length || ops.some(op => op.dependsOn.some(id => !ids.has(id) || id === op.id));
  const indexOf = new Map(ops.map((o, i) => [o.id, i]));
  const hardPredecessors = new Map<string, Set<string>>();
  const hardSuccessors = new Map<string, Set<string>>();
  const orderPredecessors = new Map<string, Set<string>>();
  const allSuccessors = new Map<string, Set<string>>();

  for (const op of ops) {
    hardPredecessors.set(op.id, new Set());
    hardSuccessors.set(op.id, new Set());
    orderPredecessors.set(op.id, new Set());
    allSuccessors.set(op.id, new Set());
  }

  let dependencyEdges = 0;
  let orderingEdges = 0;

  const addEdge = (from: string, to: string, hard: boolean) => {
    if (from === to || !ids.has(from) || !ids.has(to)) return;
    const predecessors = hard ? hardPredecessors : orderPredecessors;
    const predSet = predecessors.get(to);
    const succSet = allSuccessors.get(from);
    if (!predSet || !succSet || predSet.has(from)) return;
    predSet.add(from);
    succSet.add(to);
    if (hard) {
      hardSuccessors.get(from)?.add(to);
      dependencyEdges += 1;
    } else {
      orderingEdges += 1;
    }
  };

  for (const op of ops) {
    for (const dep of op.dependsOn) addEdge(dep, op.id, true);
  }

  for (const conflict of conflicts) {
    if (conflict.severity !== "parallel-only") continue;
    const aIndex = indexOf.get(conflict.opAId);
    const bIndex = indexOf.get(conflict.opBId);
    if (aIndex === undefined || bIndex === undefined) continue;
    const earlier = ops[Math.min(aIndex, bIndex)];
    const later = ops[Math.max(aIndex, bIndex)];
    addEdge(earlier.id, later.id, false);
  }

  const remainingPreds = new Map<string, Set<string>>();
  for (const op of ops) {
    remainingPreds.set(op.id, new Set([
      ...(hardPredecessors.get(op.id) ?? []),
      ...(orderPredecessors.get(op.id) ?? []),
    ]));
  }

  const pending = new Set(ops.map((o) => o.id));
  const levels: string[][] = [];
  while (pending.size > 0) {
    const ready = ops
      .filter((op) => pending.has(op.id) && (remainingPreds.get(op.id)?.size ?? 0) === 0)
      .map((op) => op.id);
    if (ready.length === 0) {
      invalidDependencies = true;
      break;
    }
    levels.push(ready);
    for (const id of ready) {
      pending.delete(id);
      for (const child of allSuccessors.get(id) ?? []) {
        remainingPreds.get(child)?.delete(id);
      }
    }
  }

  return {
    total: ops.length,
    invalidDependencies,
    hasDependencies: dependencyEdges > 0,
    hasOrderingConstraints: orderingEdges > 0,
    dependencyEdges,
    orderingEdges,
    maxParallelWidth: Math.max(0, ...levels.map((level) => level.length)),
    maxConcurrent: Math.min(maxConcurrent, ops.length),
    firstWave: levels[0]?.length ?? 0,
    levels,
    blockingConflicts: conflicts.filter((c) => c.severity === "blocking"),
    hardPredecessors,
    hardSuccessors,
    orderPredecessors,
  };
}


/** Paths a producer must create for the inputs of its queued consumers. */
export function requiredOutputs(ops: QueuedOp[], knownPaths: string[] = []): Map<string, Array<{ requested: string; expected: string; allowExisting?: boolean }>> {
  const resolved = resolveQueueOutputs(ops, knownPaths);
  const result = new Map<string, Array<{ requested: string; expected: string; allowExisting?: boolean }>>();
  for (const producer of ops) {
    if (producer.fixedOutputs?.length) {
      result.set(producer.id, producer.fixedOutputs.flatMap((output, index) => output.approximate ? [] : [{ requested: producer.produces[index].path, expected: output.path, ...(output.sharedDirectory ? { allowExisting: true } : {}) }]));
    }
  }
  for (const consumer of ops) {
    const inputs = [...consumer.sources, ...consumer.deletes, ...consumer.destinations];
    for (const depId of consumer.dependsOn) {
      const producer = ops.find(op => op.id === depId);
      if (!producer) continue;
      const outputs = resolved.get(depId) ?? [];
      for (let index = 0; index < outputs.length; index++) {
        const output = outputs[index];
        const outputKey = normalizePath(output.path);
        const used = inputs.some(input => {
          const inputKey = normalizePath(input);
          return inputKey === outputKey || (output.isDir && pathContains(output.path, input)) || pathContains(input, output.path);
        });
        if (!used) continue;
        const entries = result.get(depId) ?? [];
        if (!entries.some(entry => normalizePath(entry.expected) === outputKey)) {
          entries.push({ requested: producer.produces[index].path, expected: output.path, ...(output.sharedDirectory ? { allowExisting: true } : {}) });
          result.set(depId, entries);
        }
      }
    }
  }
  return result;
}

/** A dependency must still refer to the output that was visible when queued. */
export function hasUnresolvedOutputs(ops: QueuedOp[], knownPaths: string[] = []): boolean {
  const resolved = resolveQueueOutputs(ops, knownPaths);
  return ops.some(consumer => consumer.dependsOn.some(depId => {
    const outputs = resolved.get(depId) ?? [];
    // A dependency can free a destination without supplying a ghost input.
    const producer = ops.find(op => op.id === depId);
    if (producer?.deletes.some(deleted => consumer.produces.some(output => pathsOverlap(deleted, output.path)))) return false;
    const inputs = [...consumer.sources, ...consumer.deletes, ...consumer.destinations];
    return !outputs.some(output => inputs.some(input => {
      const inputKey = normalizePath(input);
      const outputKey = normalizePath(output.path);
      return inputKey === outputKey || (output.isDir && pathContains(output.path, input)) || pathContains(input, output.path);
    }));
  }));
}
