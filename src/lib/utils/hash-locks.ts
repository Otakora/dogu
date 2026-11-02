import type { QueuedOp } from "../types/index.js";
import { pathContains } from "./ghosts.js";

/** A checksum may only read a file that no pending or active operation touches. */
export function hashBlockedPaths(paths: string[], operations: QueuedOp[]): string[] {
  return paths.filter(path => operations.some(op => {
    const outputs = op.fixedOutputs ?? op.produces;
    const affected = [...op.sources, ...op.deletes, ...outputs.filter(output => !output.sharedDirectory).map(output => output.path)];
    // Unknown extraction contents may land anywhere in the destination tree.
    if (op.kind === "extract" && !outputs.some(output => !output.sharedDirectory && !output.approximate)) affected.push(...op.destinations);
    return affected.some(candidate => pathContains(candidate, path) || pathContains(path, candidate));
  }));
}
