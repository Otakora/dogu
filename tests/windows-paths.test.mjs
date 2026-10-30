import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";
const code = ts.transpile(readFileSync(new URL("../src/lib/utils/windows-paths.ts", import.meta.url), "utf8"), { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS });
const exports = {};
vm.runInNewContext(code, { exports });
const { windowsPathIssue, isLongPathError } = exports;

test("checks Windows paths in UTF-16 units and individual components", () => {
  assert.equal(windowsPathIssue("C:\\" + "a".repeat(255)), false);
  assert.equal(windowsPathIssue("C:\\" + "a".repeat(256)), true);
  assert.equal(windowsPathIssue("C:\\dir\\" + "x".repeat(252)), false);
  assert.equal(windowsPathIssue("C:\\dir\\" + "x".repeat(253)), true);
  assert.equal(windowsPathIssue("C:\\dir\\" + "😀".repeat(127)), true);
  assert.equal(windowsPathIssue("\\\\server\\share\\" + "a".repeat(250)), true);
  assert.equal(windowsPathIssue("remote://nas/" + "x".repeat(300)), false);
  assert.equal(windowsPathIssue("/home/" + "x".repeat(300)), false);
});

test("recognizes path length errors without confusing unrelated errors", () => {
  for (const error of ["[PATH_TOO_LONG] failure", "os error 206", "The filename or extension is too long", "path is too long"]) assert.equal(isLongPathError(error), true);
  for (const error of ["Permission denied", "os error 20", "disk full"]) assert.equal(isLongPathError(error), false);
});
