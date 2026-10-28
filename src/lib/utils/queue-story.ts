import type { GhostEntry, QueuedOp } from "../types/index.js";
import { normalizePath, pathContains } from "./ghosts.js";

export type QueueStoryEvent = {
  step: number;
  kind: QueuedOp["kind"];
  action: "remove" | "create" | "replace" | "rename";
  path: string;
  source?: string;
};
export type QueueStory = { events: QueueStoryEvent[]; finalExists: boolean; renamedTo?: string };

/** Describe the sequence for one visible generation, without changing the queue. */
export function queueStory(path: string, isGhost: boolean, producerId: string | undefined, ops: QueuedOp[], outputs: Map<string, GhostEntry[]>): QueueStory {
  const key = normalizePath(path);
  const producerIndex = isGhost ? ops.findIndex(op => op.id === producerId) : -1;
  let exists = !isGhost;
  let renamedTo: string | undefined;
  const events: QueueStoryEvent[] = [];
  const add = (event: QueueStoryEvent) => {
    if (!events.some(e => e.step === event.step && e.action === event.action && normalizePath(e.path) === normalizePath(event.path))) events.push(event);
  };
  ops.forEach((op, index) => {
    if (isGhost && (producerIndex < 0 || index < producerIndex)) return;
    const removed = op.deletes.some(deleted => pathContains(deleted, path));
    const output = (outputs.get(op.id) ?? op.produces).find(ghost => normalizePath(ghost.path) === key);
    const renameSource = op.kind === "rename" && op.sources.some(source => normalizePath(source) === key);
    if (removed && exists) {
      if (renameSource) {
        const destination = (outputs.get(op.id) ?? op.produces)[0]?.path;
        if (destination) { renamedTo = destination; add({ step: index + 1, kind: op.kind, action: "rename", path: destination, source: path }); }
      } else add({ step: index + 1, kind: op.kind, action: "remove", path });
      exists = false;
    }
    if (output) {
      if (op.kind === "rename" && op.sources[0]) {
        const source = op.sources[0];
        // Include the immediately preceding producer of the renamed ghost.
        for (let i = index - 1; i >= 0; i--) {
          if ((outputs.get(ops[i].id) ?? ops[i].produces).some(g => normalizePath(g.path) === normalizePath(source))) {
            add({ step: i + 1, kind: ops[i].kind, action: "create", path: source });
            break;
          }
        }
        add({ step: index + 1, kind: op.kind, action: "rename", path, source });
      } else add({ step: index + 1, kind: op.kind, action: exists ? "replace" : "create", path });
      exists = true;
      renamedTo = undefined;
    }
  });
  return { events: events.sort((a, b) => a.step - b.step), finalExists: exists, renamedTo };
}
