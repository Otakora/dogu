import type { QueuedOp } from "../types/index.js";

export type RequiredOutput = { requested: string; expected: string };

/** Do not let a dependent step consume a stale or guessed output path. */
export async function executeWithOutputChecks(
  op: QueuedOp,
  outputs: RequiredOutput[],
  exists: (path: string) => Promise<boolean>,
  messages: { uncertain: (path: string) => string; missing: (path: string) => string },
  execute: () => Promise<boolean>,
): Promise<boolean> {
  if (op.fixedOutputs && !op.overwrite) {
    for (const output of outputs) {
      if (await exists(output.expected)) throw new Error(messages.uncertain(output.expected));
    }
  } else if (op.renameOnConflict && !op.overwrite) {
    for (const output of outputs) {
      if (await exists(output.requested)) throw new Error(messages.uncertain(output.requested));
    }
  }
  if (!await execute()) return false;
  for (const output of outputs) {
    if (!await exists(output.expected)) throw new Error(messages.missing(output.expected));
  }
  return true;
}
