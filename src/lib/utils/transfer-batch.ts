import type { QueuedOp } from "../types/index.js";
import { buildQueuePlan, detectConflicts } from "./queue-plan.js";
import { restoreUnfinished, runQueue } from "./queue-runner.js";

/** Direct batches use the same conflict ordering and concurrency as the queue. */
export async function runTransferBatch(
  items: QueuedOp[],
  concurrency: number,
  execute: (item: QueuedOp) => Promise<boolean>,
  retain: (items: QueuedOp[]) => void,
  report: (error: unknown) => void,
): Promise<void> {
  const plan = buildQueuePlan(items, detectConflicts(items), concurrency);
  if (plan.invalidDependencies || plan.blockingConflicts.length) {
    retain(items);
    return;
  }
  const status = await runQueue(items, plan, concurrency, execute, () => {}, (_item, error) => report(error));
  const unfinished = restoreUnfinished(items, status);
  if (unfinished.length) retain(unfinished);
}
