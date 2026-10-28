import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";
const code = ts.transpile(readFileSync(new URL("../src/lib/utils/queue-validation.ts", import.meta.url), "utf8"), { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS });
const exports = {};
vm.runInNewContext(code, { exports });
const { executeWithOutputChecks } = exports;
const msg = { uncertain: p => `uncertain ${p}`, missing: p => `missing ${p}` };
const op = { renameOnConflict: true, overwrite: false };
const outputs = [{ requested: "/file", expected: "/file" }];

test("an occupied destination stops a producer before it changes files", async () => {
  let executed = false;
  await assert.rejects(executeWithOutputChecks(op, outputs, async () => true, msg, async () => { executed = true; return true; }), /uncertain/);
  assert.equal(executed, false);
});

test("a missing output stops the dependent chain after the producer", async () => {
  let executed = false;
  await assert.rejects(executeWithOutputChecks(op, outputs, async () => false, msg, async () => { executed = true; return true; }), /missing/);
  assert.equal(executed, true);
});

test("a confirmed output allows the chain to proceed", async () => {
  let calls = 0;
  assert.equal(await executeWithOutputChecks(op, outputs, async () => ++calls > 1, msg, async () => true), true);
});

test("a fixed CHD checks the promised suffixed name instead of the original name", async () => {
  const fixed = { kind: "chd-convert", fixedOutputs: [{ path: "/file (1).chd" }], renameOnConflict: true, overwrite: false };
  const result = [{ requested: "/file.chd", expected: "/file (1).chd" }];
  let produced = false;
  assert.equal(await executeWithOutputChecks(fixed, result, async path => path === "/file.chd" || produced, msg, async () => { produced = true; return true; }), true);
});

test("a fixed CHD refuses a destination created externally before execution", async () => {
  const fixed = { kind: "chd-convert", fixedOutputs: [{ path: "/file (1).chd" }], renameOnConflict: true, overwrite: false };
  let executed = false;
  await assert.rejects(executeWithOutputChecks(fixed, [{ requested: "/file.chd", expected: "/file (1).chd" }], async () => true, msg, async () => { executed = true; return true; }), /uncertain/);
  assert.equal(executed, false);
});
