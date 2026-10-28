import type { QueuedOp } from "../types/index.js";

export type QueueStepStatus = "pending" | "running" | "succeeded" | "failed" | "skipped";
export type QueueExecutionPlan = {
  hardPredecessors: Map<string, Set<string>>;
  orderPredecessors: Map<string, Set<string>>;
};

/** A queued failure must be retried through the queue so its dependents stay linked. */
export function canRetryFailedJob(job: { done: boolean; success: boolean; retried: boolean; retryQueuedOp: QueuedOp | null }, queue: QueuedOp[]): boolean {
  return job.done && !job.success && !job.retried && !!job.retryQueuedOp && !job.retryQueuedOp.requiresReview && !queue.some(op => op.id === job.retryQueuedOp?.id);
}

/** Run a fixed snapshot. A failed predecessor blocks consumers and conflicting
 * operations: a failed operation may have left the shared paths half-written. */
export async function runQueue(
  ops: QueuedOp[],
  plan: QueueExecutionPlan,
  concurrency: number,
  execute: (op: QueuedOp) => Promise<boolean>,
  changed: (status: ReadonlyMap<string, QueueStepStatus>) => void,
  onError: (op: QueuedOp, error: unknown) => void,
): Promise<Map<string, QueueStepStatus>> {
  const status = new Map<string, QueueStepStatus>(ops.map(op => [op.id, "pending"]));
  const running = new Map<string, Promise<void>>();
  const limit = Math.max(1, Math.floor(concurrency));
  const predecessors = (id: string) => new Set([
    ...(plan.hardPredecessors.get(id) ?? []),
    ...(plan.orderPredecessors.get(id) ?? []),
  ]);
  const publish = () => changed(new Map(status));
  publish();

  while ([...status.values()].some(state => state === "pending") || running.size) {
    // Repeat because one skipped step can block another further down the graph.
    let changedSkip: boolean;
    do {
      changedSkip = false;
      for (const op of ops) {
        if (status.get(op.id) !== "pending") continue;
        if ([...predecessors(op.id)].some(id => !status.has(id) || status.get(id) === "failed" || status.get(id) === "skipped")) {
          status.set(op.id, "skipped");
          changedSkip = true;
        }
      }
      if (changedSkip) publish();
    } while (changedSkip);

    for (const op of ops) {
      if (running.size >= limit) break;
      if (status.get(op.id) !== "pending") continue;
      if (![...predecessors(op.id)].every(id => status.get(id) === "succeeded")) continue;
      status.set(op.id, "running");
      // Defer execution so the promise is in the map even for synchronous errors.
      const task = Promise.resolve().then(() => execute(op)).then(
        ok => { status.set(op.id, ok ? "succeeded" : "failed"); },
        error => { status.set(op.id, "failed"); onError(op, error); },
      ).finally(() => { running.delete(op.id); publish(); });
      running.set(op.id, task);
      publish();
    }
    if (running.size) {
      await Promise.race(running.values());
    } else {
      // Invalid or cyclic graph: never silently run or lose remaining steps.
      for (const op of ops) if (status.get(op.id) === "pending") status.set(op.id, "skipped");
      publish();
      break;
    }
  }
  return status;
}

/** Retain failed and blocked steps, including links between them, in their original order. */
export function restoreUnfinished(ops: QueuedOp[], status: ReadonlyMap<string, QueueStepStatus>): QueuedOp[] {
  const unfinished = ops.filter(op => status.get(op.id) !== "succeeded");
  const ids = new Set(unfinished.map(op => op.id));
  return unfinished.map(op => ({ ...op, dependsOn: op.dependsOn.filter(id => ids.has(id)) }));
}
