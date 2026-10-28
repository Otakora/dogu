import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

const code = ts.transpile(readFileSync(new URL("../src/lib/utils/queue-runner.ts", import.meta.url), "utf8"), { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS });
const exports = {};
vm.runInNewContext(code, { exports });
const { runQueue } = exports;
const ghostsExports = {};
vm.runInNewContext(ts.transpile(readFileSync(new URL("../src/lib/utils/ghosts.ts", import.meta.url), "utf8"), { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS }), { exports: ghostsExports });
const plannerExports = {};
vm.runInNewContext(ts.transpile(readFileSync(new URL("../src/lib/utils/queue-plan.ts", import.meta.url), "utf8"), { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS }), { exports: plannerExports, require: name => { if (name === "./ghosts.js") return ghostsExports; throw Error(name); } });
const transferExports = {};
vm.runInNewContext(ts.transpile(readFileSync(new URL("../src/lib/utils/transfer-batch.ts", import.meta.url), "utf8"), { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS }), { exports: transferExports, require: name => name === "./queue-plan.js" ? plannerExports : name === "./queue-runner.js" ? exports : (() => { throw Error(name); })() });
const op = id => ({ id });
const plan = (hard = [], order = []) => ({ hardPredecessors: new Map(hard), orderPredecessors: new Map(order) });
const deferred = () => { let resolve; const promise = new Promise(r => { resolve = r; }); return { promise, resolve }; };

test("direct batch retains each failed member and all later members without repeating successes", async () => {
  const items = Array.from({ length: 7 }, (_, i) => ({ id: String(i), kind: "copy", sources: [`/source/${i}`], destinations: ["/out"], deletes: [], produces: [{ path: "/out/shared", isDir: false }], dependsOn: [], overwrite: false, renameOnConflict: false }));
  for (let failure = 0; failure < items.length; failure++) {
    const started = [], saved = [];
    await transferExports.runTransferBatch(items, 3, async item => { started.push(item); return item !== items[failure]; },
      remaining => saved.push(...remaining), () => assert.fail("unexpected exception"));
    assert.deepEqual(started, items.slice(0, failure + 1));
    assert.deepEqual(saved.map(item => item.id), items.slice(failure).map(item => item.id));
  }
});

test("direct batch retains work when execution throws", async () => {
  const saved = [], errors = [];
  const items = [1, 2, 3].map(id => ({ id: String(id), kind: "copy", sources: [`/source/${id}`], destinations: ["/out"], deletes: [], produces: [{ path: "/out/shared", isDir: false }], dependsOn: [] }));
  await transferExports.runTransferBatch(items, 2, async item => {
    if (item.id === "2") throw new Error("cut");
    return true;
  }, remaining => saved.push(...remaining), error => errors.push(error.message));
  assert.deepEqual(saved.map(item => item.id), ["2", "3"]);
  assert.deepEqual(errors, ["cut"]);
});

test("direct transfers use the configured concurrency and retain only failures", async () => {
  const items = Array.from({ length: 6 }, (_, id) => ({ id: String(id), kind: "copy", sources: [`/source/${id}`], destinations: ["/out"], deletes: [], produces: [{ path: `/out/${id}`, isDir: false }], dependsOn: [] }));
  let running = 0, peak = 0;
  const saved = [];
  await transferExports.runTransferBatch(items, 3, async item => {
    peak = Math.max(peak, ++running);
    await new Promise(resolve => setTimeout(resolve, 2));
    running--;
    return item.id !== "2";
  }, remaining => saved.push(...remaining), error => assert.fail(String(error)));
  assert.equal(peak, 3);
  assert.deepEqual(saved.map(item => item.id), ["2"]);
});

test("runs independent work concurrently and starts a consumer after its producer", async () => {
  const first = deferred(), second = deferred();
  const started = [];
  const result = runQueue([op("a"), op("b"), op("c")], plan([["c", new Set(["a"])]]), 2, async operation => {
    started.push(operation.id);
    if (operation.id === "a") return first.promise;
    if (operation.id === "b") return second.promise;
    return true;
  }, () => {}, () => {});
  await new Promise(resolve => setTimeout(resolve, 0));
  assert.deepEqual(started, ["a", "b"]);
  first.resolve(true);
  await new Promise(resolve => setTimeout(resolve, 0));
  assert.deepEqual(started, ["a", "b", "c"]);
  second.resolve(true);
  const status = await result;
  assert.deepEqual([...status.values()], ["succeeded", "succeeded", "succeeded"]);
});

test("failure blocks dependent and conflicting steps but not independent work", async () => {
  const started = [];
  const status = await runQueue([op("a"), op("b"), op("c"), op("d")], plan([["b", new Set(["a"])]], [["c", new Set(["a"])]]), 3, async operation => {
    started.push(operation.id);
    return operation.id !== "a";
  }, () => {}, () => {});
  assert.deepEqual(started, ["a", "d"]);
  assert.equal(status.get("a"), "failed");
  assert.equal(status.get("b"), "skipped");
  assert.equal(status.get("c"), "skipped");
  assert.equal(status.get("d"), "succeeded");
});

test("exceptions are reported and cycles are retained as skipped", async () => {
  const errors = [];
  const ops = [op("a"), op("b"), op("c")];
  const status = await runQueue(ops, plan([["b", new Set(["c"])], ["c", new Set(["b"])]]), 2, async operation => {
    if (operation.id === "a") throw new Error("disk failure");
    return true;
  }, () => {}, (_op, error) => errors.push(error.message));
  assert.deepEqual(errors, ["disk failure"]);
  assert.equal(status.get("a"), "failed");
  assert.equal(status.get("b"), "skipped");
  assert.equal(status.get("c"), "skipped");
});

test("a large mixed queue respects concurrency and completes every independent step", async () => {
  const ops = Array.from({ length: 80 }, (_, i) => op(`step-${i}`));
  const hard = ops.filter((_, i) => i % 4 === 1).map((item, i) => [item.id, new Set([`step-${i * 4}`])]);
  let running = 0, peak = 0;
  const status = await runQueue(ops, plan(hard), 3, async () => {
    running++;
    peak = Math.max(peak, running);
    await new Promise(resolve => setTimeout(resolve, 1));
    running--;
    return true;
  }, () => {}, () => {});
  assert.equal(peak, 3);
  assert.equal([...status.values()].filter(value => value === "succeeded").length, 80);
});


test("a failed extraction preserves its delete, move and CHD chain for retry", async () => {
  const ops = [
    { id: "extract", dependsOn: [] }, { id: "delete", dependsOn: ["extract"] },
    { id: "move", dependsOn: ["extract", "delete"] }, { id: "chd", dependsOn: ["move"] },
    { id: "other", dependsOn: [] },
  ];
  const hard = ops.map(item => [item.id, new Set(item.dependsOn)]);
  const called = [];
  const status = await runQueue(ops, plan(hard), 3, async item => { called.push(item.id); return item.id !== "extract"; }, () => {}, () => {});
  assert.equal(status.get("extract"), "failed");
  for (const id of ["delete", "move", "chd"]) assert.equal(status.get(id), "skipped");
  assert.equal(status.get("other"), "succeeded");
  const pending = exports.restoreUnfinished(ops, status);
  assert.deepEqual(Array.from(pending, item => item.id), ["extract", "delete", "move", "chd"]);
  assert.deepEqual(Array.from(pending[2].dependsOn), ["extract", "delete"]);
  assert.deepEqual(Array.from(pending[3].dependsOn), ["move"]);
});

test("a dependent stays pending during an automatic retry and starts after success", async () => {
  const release = deferred(), seen = [], started = [];
  const running = runQueue([op("extract"), op("convert")], plan([["convert", new Set(["extract"])]]), 2, async item => {
    started.push(item.id);
    if (item.id === "extract") { await release.promise; }
    return true;
  }, state => seen.push(new Map(state)), () => {});
  await new Promise(resolve => setTimeout(resolve, 0));
  assert.deepEqual(started, ["extract"]);
  assert.equal(seen.at(-1).get("extract"), "running");
  assert.equal(seen.at(-1).get("convert"), "pending");
  release.resolve();
  await running;
  assert.deepEqual(started, ["extract", "convert"]);
  assert.equal(seen.at(-1).get("convert"), "succeeded");
});

test("manual retry is offered only for failed jobs outside the pending queue", () => {
  const failed = { done: true, success: false, retried: false, retryQueuedOp: { id: "extract" } };
  const eligible = exports.canRetryFailedJob;
  assert.equal(eligible(failed, []), true);
  assert.equal(eligible(failed, [{ id: "extract" }]), false);
  assert.equal(eligible({ ...failed, retried: true }, []), false);
  assert.equal(eligible({ ...failed, success: true }, []), false);
  assert.equal(eligible({ ...failed, done: false }, []), false);
  assert.equal(eligible({ ...failed, retryQueuedOp: null }, []), false);
  assert.equal(eligible({ ...failed, retryQueuedOp: { id: "extract", requiresReview: "uncertain publication" } }, []), false);
});

test("a failed queue chain can be retried repeatedly until it succeeds", async () => {
  let pending = [{ id: "extract", dependsOn: [] }, { id: "convert", dependsOn: ["extract"] }];
  for (let attempt = 0; attempt < 4; attempt++) {
    const executionPlan = plan(pending.map(item => [item.id, new Set(item.dependsOn)]));
    const started = [];
    const states = await runQueue(pending, executionPlan, 2, async item => {
      started.push(item.id);
      return item.id !== "extract" || attempt === 3;
    }, () => {}, () => {});
    if (attempt < 3) {
      assert.deepEqual(started, ["extract"]);
      assert.equal(states.get("convert"), "skipped");
    } else {
      assert.deepEqual(started, ["extract", "convert"]);
    }
    pending = Array.from(exports.restoreUnfinished(pending, states));
  }
  assert.equal(pending.length, 0);
});

test("failed deletion blocks conversion and rename that reuse its name", async () => {
  const ops = [op("delete"), op("extract"), op("convert"), op("rename")];
  const hard = [["convert", new Set(["delete", "extract"])], ["rename", new Set(["delete", "convert"])]];
  const started = [];
  const status = await runQueue(ops, plan(hard), 2, async item => { started.push(item.id); return item.id !== "delete"; }, () => {}, () => {});
  assert.equal(status.get("delete"), "failed");
  assert.equal(status.get("extract"), "succeeded");
  assert.equal(status.get("convert"), "skipped");
  assert.equal(status.get("rename"), "skipped");
  assert.equal(started.includes("rename"), false);
});

test("interrupted multi-file move retains only unfinished units and blocks their consumers", async () => {
  const earlier = Array.from({ length: 18 }, (_, i) => ({ id: `previous-${i}`, dependsOn: [] }));
  const files = Array.from({ length: 7 }, (_, i) => ({ id: `move-${i}`, dependsOn: [] }));
  const consumer = { id: "consumer", dependsOn: ["move-1"] };
  const ops = [...earlier, ...files, consumer];
  // The second move fails after the first succeeded. Later members wait for
  // the failed transfer in this conservative sequential execution.
  const order = files.slice(1).map((item, i) => [item.id, new Set([files[i].id])]);
  const hard = [[consumer.id, new Set(consumer.dependsOn)]];
  const status = await runQueue(ops, plan(hard, order), 1, async item => item.id !== "move-1", () => {}, () => {});
  assert.equal(status.get("move-0"), "succeeded");
  assert.equal(status.get("move-1"), "failed");
  assert.equal(status.get("consumer"), "skipped");
  const pending = Array.from(exports.restoreUnfinished(ops, status), item => item.id);
  assert.deepEqual(pending, ["move-1", "move-2", "move-3", "move-4", "move-5", "move-6", "consumer"]);
});
