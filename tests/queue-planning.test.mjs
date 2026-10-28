import test from "node:test";
import assert from "node:assert/strict";
import { readFileSync } from "node:fs";
import vm from "node:vm";
import ts from "typescript";

// Exercise the planner itself without mounting the Svelte application.
function compile(source) {
  return ts.transpile(source, { target: ts.ScriptTarget.ES2022, module: ts.ModuleKind.CommonJS });
}
const ghostsSource = readFileSync(new URL("../src/lib/utils/ghosts.ts", import.meta.url), "utf8");
const ghostModule = { exports: {} };
vm.runInNewContext(compile(ghostsSource), { exports: ghostModule.exports, module: ghostModule });
const plannerSource = readFileSync(new URL("../src/lib/utils/queue-plan.ts", import.meta.url), "utf8");
const plannerExports = {};
vm.runInNewContext(compile(plannerSource), {
  exports: plannerExports,
  require: name => {
    if (name === "./ghosts.js") return ghostModule.exports;
    throw new Error(`Unexpected import: ${name}`);
  },
});
const planner = plannerExports;
const extractionExports = {};
vm.runInNewContext(compile(readFileSync(new URL("../src/lib/utils/extraction-ghosts.ts", import.meta.url), "utf8")), {
  exports: extractionExports,
  require: name => { if (name === "./ghosts.js") return ghostModule.exports; throw new Error(`Unexpected import: ${name}`); },
});
const chdGhostExports = {};
vm.runInNewContext(compile(readFileSync(new URL("../src/lib/utils/chd-ghosts.ts", import.meta.url), "utf8")), {
  exports: chdGhostExports,
  require: name => { if (name === "./ghosts.js") return ghostModule.exports; throw new Error(`Unexpected import: ${name}`); },
});
const storyExports = {};
vm.runInNewContext(compile(readFileSync(new URL("../src/lib/utils/queue-story.ts", import.meta.url), "utf8")), {
  exports: storyExports,
  require: name => { if (name === "./ghosts.js") return ghostModule.exports; throw new Error(`Unexpected import: ${name}`); },
});
const runnerExports = {};
vm.runInNewContext(compile(readFileSync(new URL("../src/lib/utils/queue-runner.ts", import.meta.url), "utf8")), { exports: runnerExports });
const ghost = (path, isDir = false) => ({ path, isDir, name: path.split("/").pop(), parentDir: "/", format: "", approximate: false, producedByOpId: "" });
const op = (id, data = {}) => ({ id, sources: [], destinations: [], deletes: [], produces: [], dependsOn: [], overwrite: false, renameOnConflict: false, ...data });

test("the queue story distinguishes an old file from its renamed successor", () => {
  const path = "remote://session-1/psx/Tekken.chd";
  const interim = "remote://session-1/psx/Tekken (1).chd";
  const ops = [op("delete", { kind: "delete", deletes: [path] }), op("extract", { kind: "extract" }),
    op("convert", { kind: "chd-convert", produces: [ghost(interim)] }),
    op("rename", { kind: "rename", sources: [interim], deletes: [interim], produces: [ghost(path)] })];
  const outputs = new Map(ops.map(item => [item.id, item.produces]));
  const final = storyExports.queueStory(path, false, undefined, ops, outputs);
  assert.equal(final.finalExists, true);
  assert.deepEqual(Array.from(final.events, e => `${e.step}:${e.action}`), ["1:remove", "3:create", "4:rename"]);
  const intermediate = storyExports.queueStory(interim, true, "convert", ops, outputs);
  assert.equal(intermediate.finalExists, false);
  assert.equal(intermediate.renamedTo, path);
  const removedAgain = storyExports.queueStory(path, false, undefined, [...ops, op("last", { kind: "delete", deletes: [path] })], outputs);
  assert.equal(removedAgain.finalExists, false);
});

test("a recreated ghost reports the final generation instead of only its removal", () => {
  const path = "remote://s/out/game.iso";
  const ops = [op("first", { kind: "cso-restore", produces: [ghost(path)] }),
    op("delete", { kind: "delete", deletes: [path] }),
    op("second", { kind: "cso-restore", produces: [ghost(path)] })];
  const story = storyExports.queueStory(path, true, "first", ops, new Map(ops.map(item => [item.id, item.produces])));
  assert.equal(story.finalExists, true);
  assert.deepEqual(Array.from(story.events, event => event.action), ["create", "remove", "create"]);
});

test("an earlier deletion must succeed before a later step recreates its path", () => {
  const path = "remote://session-1/psx/Tekken.chd";
  const removal = op("remove", { deletes: [path] });
  const conversion = op("convert", { produces: [ghost(path)], overwrite: true });
  conversion.dependsOn = Array.from(planner.linkDependencies(conversion, [removal], [path]));
  assert.deepEqual(conversion.dependsOn, ["remove"]);
  const conflicts = planner.detectConflicts([removal, conversion], [path]);
  const plan = planner.buildQueuePlan([removal, conversion], conflicts, 2);
  assert.equal(plan.blockingConflicts.length, 0);
  assert.equal(plan.hardPredecessors.get("convert").has("remove"), true);
  assert.equal(planner.hasUnresolvedOutputs([removal, conversion], [path]), false);
});

test("a fixed CHD name remains stable after its original name disappears", () => {
  const root = "remote://session-1/psx/Tekken.chd";
  const fixed = "remote://session-1/psx/Tekken (1).chd";
  const conversion = op("convert", { kind: "chd-convert", produces: [ghost(root)], fixedOutputs: [ghost(fixed)], renameOnConflict: true });
  assert.equal(planner.resolveQueueOutputs([conversion], [root]).get("convert")[0].path, fixed);
  assert.equal(planner.resolveQueueOutputs([conversion], []).get("convert")[0].path, fixed);
  assert.equal(planner.requiredOutputs([conversion], []).get("convert")[0].expected, fixed);
});

test("individual queued transfers keep their reserved names after earlier work frees the original", () => {
  const target = "remote://session-1/psx/game.chd";
  const fixed = "remote://session-1/psx/game (1).chd";
  const transfer = op("move", { kind: "move", sources: ["E:/roms/game.chd"], produces: [ghost(target)], fixedOutputs: [ghost(fixed)], renameOnConflict: true });
  assert.equal(planner.resolveQueueOutputs([transfer], [target]).get("move")[0].path, fixed);
  assert.equal(planner.resolveQueueOutputs([transfer], []).get("move")[0].path, fixed);
});

test("renaming an extraction directory rebases all of its child ghosts", () => {
  const root = "remote://session-1/psx/Game";
  const extraction = op("extract", { kind: "extract", produces: [ghost(root, true), ghost(`${root}/Game.cue`), ghost(`${root}/Game.bin`)], renameOnConflict: true });
  const outputs = planner.resolveQueueOutputs([extraction], [root]).get("extract");
  assert.equal(outputs[0].path, `${root} (1)`);
  assert.equal(outputs[1].path, `${root} (1)/Game.cue`);
  assert.equal(outputs[1].parentDir, `${root} (1)`);
  assert.equal(outputs[2].path, `${root} (1)/Game.bin`);
});

test("CHD restore reserves one common stem when any related extension is occupied", () => {
  const base = "C:/out/game";
  const restore = op("restore", { kind: "chd-restore", sources: ["C:/out/game.chd"], produces: [
    { ...ghost(`${base}.cue`), format: "cue", approximate: true },
    { ...ghost(`${base}.bin`), format: "bin", approximate: true },
  ], renameOnConflict: true });
  const outputs = planner.resolveQueueOutputs([restore], [`${base}.iso`]).get("restore");
  assert.equal(outputs[0].path, `${base} (1).cue`);
  assert.equal(outputs[1].path, `${base} (1).bin`);
});

test("fixed deterministic archive and disc outputs remain stable after earlier deletion", () => {
  for (const kind of ["compress", "cso-convert", "cso-restore", "xiso-pack", "xiso-unpack", "rvz-convert", "rvz-restore"]) {
    const original = "remote://session-1/games/file.iso";
    const fixed = "remote://session-1/games/file (1).iso";
    const operation = op(kind, { kind, produces: [ghost(original)], fixedOutputs: [ghost(fixed)], renameOnConflict: true });
    assert.equal(planner.resolveQueueOutputs([operation], []).get(kind)[0].path, fixed);
    assert.equal(planner.requiredOutputs([operation], []).get(kind)[0].expected, fixed);
  }
});

test("rename waits for both its producer and the earlier deletion that frees its destination", () => {
  const original = "remote://session-1/psx/Tekken.chd";
  const suffixed = "remote://session-1/psx/Tekken (1).chd";
  const removal = op("delete", { deletes: [original] });
  const conversion = op("convert", { kind: "chd-convert", produces: [ghost(original)], fixedOutputs: [ghost(suffixed)], renameOnConflict: true });
  const rename = op("rename", { kind: "rename", sources: [suffixed], deletes: [suffixed], produces: [ghost(original)] });
  rename.dependsOn = Array.from(planner.linkDependencies(rename, [removal, conversion], [original]));
  assert.ok(rename.dependsOn.includes("delete"));
  assert.ok(rename.dependsOn.includes("convert"));
  assert.equal(planner.hasUnresolvedOutputs([removal, conversion, rename], [original]), false);
});

test("numbered outputs include the first suffix in conflict detection", () => {
  const a = op("a", { produces: [ghost("C:/out/game.chd")], renameOnConflict: true });
  const b = op("b", { produces: [ghost("C:/out/game (1).chd")] });
  assert.equal(planner.detectConflicts([a, b]).some(c => c.kind === "dest-collision"), true);
});

test("renamed outputs with the same requested path are ordered", () => {
  const ops = [op("a", { produces: [ghost("C:/out/a.iso")], renameOnConflict: true }), op("b", { produces: [ghost("C:/out/a.iso")], renameOnConflict: true })];
  const conflicts = planner.detectConflicts(ops);
  assert.equal(conflicts.some(c => c.kind === "dest-collision"), true);
  const plan = planner.buildQueuePlan(ops, conflicts, 3);
  assert.equal(plan.orderPredecessors.get("b").has("a"), true);
});

test("a later writer waits for a reader of the same source", () => {
  const ops = [op("read", { sources: ["C:/a.iso"] }), op("write", { produces: [ghost("C:/a.iso")] })];
  const conflicts = planner.detectConflicts(ops);
  assert.equal(conflicts.some(c => c.kind === "source-written"), true);
  assert.equal(planner.buildQueuePlan(ops, conflicts, 3).orderPredecessors.get("write").has("read"), true);
});

test("ghost files are not treated as parent directories", () => {
  const file = op("file", { produces: [ghost("C:/output/file.iso")] });
  const folder = op("folder", { produces: [ghost("C:/output/dir", true)] });
  const consumer = op("consumer", { sources: ["C:/output/file.iso/nested", "C:/output/dir/nested"] });
  assert.deepEqual(Array.from(planner.linkDependencies(consumer, [file, folder])), ["folder"]);
});

test("rename-on-conflict reserves names another step can choose", () => {
  const ops = [op("a", { produces: [ghost("C:/out/a.iso")], renameOnConflict: true }), op("b", { produces: [ghost("C:/out/a (2).iso")] })];
  const conflicts = planner.detectConflicts(ops);
  assert.equal(planner.buildQueuePlan(ops, conflicts, 3).orderPredecessors.get("b").has("a"), true);
});

test("independent archive outputs in one folder can start together", async () => {
  const regions = ["Europe", "Japan", "USA"];
  const ops = regions.map(region => op(region, {
    kind: "extract", sources: [`C:/roms/game (${region}).zip`], destinations: ["C:/roms"], renameOnConflict: true,
    produces: [ghost(`C:/roms/game (${region}).cue`), ghost(`C:/roms/game (${region}).bin`)],
  }));
  const conflicts = planner.detectConflicts(ops);
  assert.equal(conflicts.length, 0);
  const executionPlan = planner.buildQueuePlan(ops, conflicts, 3);
  assert.equal(executionPlan.firstWave, 3);
  const started = [];
  const states = await runnerExports.runQueue(ops, executionPlan, 3, async item => {
    started.push(item.id); return item.id !== "Europe";
  }, () => {}, () => {});
  assert.equal(started.length, 3);
  assert.equal(states.get("Europe"), "failed");
  assert.equal(states.get("Japan"), "succeeded");
  assert.equal(states.get("USA"), "succeeded");
});

test("unknown extraction outputs remain ordered when destinations overlap", () => {
  const ops = [op("a", { kind: "extract", destinations: ["C:/roms"] }), op("b", { kind: "extract", destinations: ["C:/roms"] })];
  const plan = planner.buildQueuePlan(ops, planner.detectConflicts(ops), 3);
  assert.equal(plan.orderPredecessors.get("b").has("a"), true);
});

test("relation drawing removes redundant paths without hiding direct blocking edges", () => {
  const edge = (fromId, toId, kind = "dependency") => ({ fromId, toId, kind });
  const links = [edge("a", "b"), edge("b", "c"), edge("a", "c"), edge("a", "c", "blocking")];
  const visible = planner.simplifyRelationLinks(links);
  assert.equal(visible.length, 3);
  assert(visible.some(item => item.fromId === "a" && item.toId === "c" && item.kind === "blocking"));
  assert(!visible.some(item => item.fromId === "a" && item.toId === "c" && item.kind === "dependency"));
});

test("missing and circular dependencies invalidate the plan", () => {
  const missing = [op("a", { dependsOn: ["missing"] })];
  assert.equal(planner.buildQueuePlan(missing, [], 2).invalidDependencies, true);
  assert.equal(planner.orderRespectsDeps(missing), false);
  const cyclic = [op("a", { dependsOn: ["b"] }), op("b", { dependsOn: ["a"] })];
  assert.equal(planner.buildQueuePlan(cyclic, [], 2).invalidDependencies, true);
});

test("tracks the original and expected outputs used by consumers", () => {
  const producer = op("a", { produces: [ghost("C:/out/folder", true)] });
  const consumer = op("b", { sources: ["C:/out/folder/disc.cue"], dependsOn: ["a"] });
  const outputs = planner.requiredOutputs([producer, consumer]).get("a");
  assert.equal(outputs.length, 1);
  assert.equal(outputs[0].requested, "C:/out/folder");
  assert.equal(outputs[0].expected, "C:/out/folder");
});

test("a visible renamed ghost links to its producer when a real file already exists", () => {
  const producer = op("a", { produces: [ghost("C:/out/a.iso")], renameOnConflict: true });
  const consumer = op("b", { sources: ["C:/out/a (1).iso"] });
  const known = ["C:/out/a.iso"];
  assert.equal(planner.resolveQueueOutputs([producer], known).get("a")[0].path, "C:/out/a (1).iso");
  assert.deepEqual(Array.from(planner.linkDependencies(consumer, [producer], known)), ["a"]);
});

test("a ghost CUE predicts all matching tracks without including unrelated files", () => {
  const folder = "remote://session-1/psx/Tekken 3 (USA)";
  const cue = `${folder}/Tekken 3 (USA).cue`;
  const tracks = [1, 2, 3].map(n => `${folder}/Tekken 3 (USA) (Track ${n}).bin`);
  const actual = chdGhostExports.predictedCueTracks(cue, [cue, ...tracks, `${folder}/Otro.bin`, `remote://session-1/else/Tekken 3 (USA) (Track 4).bin`]);
  assert.deepEqual(Array.from(actual), tracks);
});

test("a ghost container is removed only when all tracks are removed and output is elsewhere", () => {
  const dir = "remote://session-1/psx/Tekken 3 (USA)";
  const files = ["Tekken 3 (USA).cue", ...[1, 2, 3].map(n => `Tekken 3 (USA) (Track ${n}).bin`)].map(name => `${dir}/${name}`);
  const ghosts = [{ path: dir, isDir: true }, ...files.map(path => ({ path, isDir: false }))];
  assert.equal(chdGhostExports.predictsEmptyGhostContainer(dir, ghosts, files, ["remote://session-1/psx"]), true);
  assert.equal(chdGhostExports.predictsEmptyGhostContainer(dir, ghosts, files.slice(0, 3), ["remote://session-1/psx"]), false);
  assert.equal(chdGhostExports.predictsEmptyGhostContainer(dir, ghosts, files, [dir]), false);
  assert.equal(chdGhostExports.predictsEmptyGhostContainer(dir, [...ghosts, { path: `${dir}/readme.txt`, isDir: false }], files, ["remote://session-1/psx"]), false);
});

test("queued local and remote copies show the next available numbered ghost", () => {
  for (const root of ["C:/out", "remote://session-1/out"]) {
    const path = `${root}/game.iso`;
    const producer = op("copy", { produces: [ghost(path)], renameOnConflict: true });
    const known = [path, `${root}/game (1).iso`];
    assert.equal(planner.resolveQueueOutputs([producer], known).get("copy")[0].path, `${root}/game (2).iso`);
  }
});

test("detects when a ghost path changes after the consumer was queued", () => {
  const producer = op("a", { produces: [ghost("C:/out/a.iso")], renameOnConflict: true });
  const consumer = op("b", { sources: ["C:/out/a.iso"], dependsOn: ["a"] });
  assert.equal(planner.hasUnresolvedOutputs([producer, consumer]), false);
  assert.equal(planner.hasUnresolvedOutputs([producer, consumer], ["C:/out/a.iso"]), true);
});

test("mixed batches preserve ghost dependencies and resume only unfinished chains", async () => {
  const ops = [];
  for (let i = 0; i < 40; i++) {
    const dir = `C:/out/game${i}`;
    const add = item => { item.dependsOn = Array.from(planner.linkDependencies(item, ops)); ops.push(item); };
    add(op(`extract-${i}`, { sources: [`C:/archives/game${i}.zip`], produces: [ghost(`${dir}/extra.txt`), ghost(`${dir}/disc.iso`)] }));
    add(op(`delete-${i}`, { deletes: [`${dir}/extra.txt`] }));
    add(op(`move-${i}`, { sources: [`${dir}/disc.iso`], deletes: [`${dir}/disc.iso`], produces: [ghost(`C:/ready/game${i}.iso`)] }));
    add(op(`chd-${i}`, { sources: [`C:/ready/game${i}.iso`], produces: [ghost(`C:/ready/game${i}.chd`)] }));
  }
  assert.deepEqual(ops[1].dependsOn, ["extract-0"]);
  assert.deepEqual(ops[2].dependsOn, ["extract-0"]);
  assert.deepEqual(ops[3].dependsOn, ["move-0"]);
  const plan = planner.buildQueuePlan(ops, planner.detectConflicts(ops), 4);
  assert.equal(plan.invalidDependencies, false);
  assert.equal(plan.blockingConflicts.length, 0);
  const history = [];
  const first = await runnerExports.runQueue(ops, plan, 4, async item => item.id !== "extract-17", state => history.push(new Map(state)), () => {});
  for (const prefix of ["delete", "move", "chd"]) assert.equal(first.get(`${prefix}-17`), "skipped");
  assert.equal(first.get("chd-16"), "succeeded");
  assert(history.some(state => state.get("extract-17") === "failed"));
  const remaining = Array.from(runnerExports.restoreUnfinished(ops, first));
  assert.deepEqual(remaining.map(item => item.id), ["extract-17", "delete-17", "move-17", "chd-17"]);
  const retryPlan = planner.buildQueuePlan(remaining, planner.detectConflicts(remaining), 4);
  const order = [];
  const retried = await runnerExports.runQueue(remaining, retryPlan, 4, async item => { order.push(item.id); return true; }, () => {}, () => {});
  assert.equal([...retried.values()].every(state => state === "succeeded"), true);
  assert(order.indexOf("extract-17") < order.indexOf("move-17"));
  assert(order.indexOf("move-17") < order.indexOf("chd-17"));
});

test("ghost predictions retain paths and producers across local and remote moves", () => {
  const api = ghostModule.exports;
  const source = api.buildGhost("C:\\juegos\\disco.iso", false, "extract");
  const moved = api.predictCopyMove([source.path], "remote://nas/library", "move")[0];
  const entry = api.ghostToEntry(moved);
  assert.equal(entry.path, "remote://nas/library/disco.iso");
  assert.equal(entry.extension, ".iso");
  assert.equal(entry.isGhost, true);
  assert.equal(entry.ghostOpId, "move");
  assert.equal(entry.isDir, false);
  const compressed = api.predictCompress({ archiveName: "colección", format: "7z" }, "C:\\salida", "compress")[0];
  assert.equal(compressed.path, "C:\\salida\\colección.7z");
  assert.equal(compressed.producedByOpId, "compress");
});

test("Linux and remote paths preserve case while Windows paths remain case insensitive", () => {
  const api = ghostModule.exports;
  assert.notEqual(api.normalizePath("/games/Title.iso"), api.normalizePath("/games/title.iso"));
  assert.notEqual(api.normalizePath("remote://nas/Games"), api.normalizePath("remote://nas/games"));
  assert.equal(api.normalizePath("C:\\Games\\TITLE.iso"), api.normalizePath("c:/games/title.iso"));
  assert.equal(api.pathContains("/", "/games/title.iso"), true);
  assert.equal(api.pathContains("/games/Foo", "/games/foo/file"), false);
  const producer = op("producer", { produces: [ghost("/games/Game.iso")] });
  const consumer = op("consumer", { sources: ["/games/game.iso"] });
  assert.deepEqual(Array.from(planner.linkDependencies(consumer, [producer])), []);
  assert.equal(planner.detectConflicts([producer, op("writer", { produces: [ghost("/games/game.iso")] })]).length, 0);
});

test("archive previews map temporary paths to their real remote and local destinations", () => {
  const options = { individualFolders: true, splitEntries: false, destinationMode: "same", destinationPath: null, remoteDestination: "remote://target/output" };
  const row = { archivePath: "C:/temp/game.zip", destinationRoot: "C:/temp/game", entries: [
    { name: "game", isDir: true, destinationPath: "C:/temp/game" },
    { name: "disc.cue", isDir: false, destinationPath: "C:/temp/game/disc.cue" },
    { name: "track.bin", isDir: false, destinationPath: "C:/temp/game/sub/track.bin" },
  ] };
  const ghosts = extractionExports.extractionGhosts("remote://s/roms/game.zip", options, "extract", row);
  assert.deepEqual(Array.from(ghosts, item => item.path), ["remote://target/output/game", "remote://target/output/game/disc.cue", "remote://target/output/game/sub/track.bin"]);
  assert.equal(ghosts[1].producedByOpId, "extract");
  const pending = extractionExports.extractionGhosts("remote://s/roms/game.zip", { ...options, remoteDestination: null }, "extract");
  assert.equal(pending[0].path, "remote://s/roms/game");
  assert.equal(pending[0].approximate, true);
  const local = extractionExports.extractionGhosts("remote://s/roms/game.zip", { ...options, remoteDestination: null, individualFolders: false, destinationMode: "custom", destinationPath: "C:/local" }, "extract", row);
  assert.equal(local[1].path, "C:/local/disc.cue");
});

test("disc and CHD predictions honor explicit remote destinations", () => {
  const api = ghostModule.exports;
  const opts = { destinationMode: "custom", destinationPath: "C:/unused", remoteDestination: "remote://other/target" };
  for (const fn of [api.predictCsoConvert, api.predictCsoRestore, api.predictRvzConvert, api.predictRvzRestore, api.predictXisoPack, api.predictXisoUnpack]) {
    assert(fn(["remote://nas/source/game.iso"], opts, "op")[0].path.startsWith("remote://other/target/"));
  }
  const chd = api.predictChdConvert(["remote://nas/dir/game.cue"], { ...opts, customName: null, nameAsContainer: false, depositToParent: false }, "op");
  assert.equal(chd[0].path, "remote://other/target/game.chd");
});
