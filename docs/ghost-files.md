# Ghost files — queueing operations on outputs that don't exist yet

Ghost files let you build a whole pipeline of operations in the queue even when a
later step depends on files produced by an earlier one. Extract an archive and,
without waiting for it to run, queue the CHD conversion of the files it *will*
produce. Then run the whole batch and walk away.

## The idea

Every queued operation declares the files it will create — its **ghost outputs**.
The explorer overlays those predicted files (dimmed, dashed icon, `pending`
badge) in the folders where they will land. You can select a ghost and queue
further operations on it exactly as you would a real file. When the queue runs,
operations execute in dependency order so the real files exist by the time a
consumer needs them.

For deterministic operations the predicted path *is* the real path, so a consumer
simply records that path plus a dependency link on its producer. Only CHD restore
is inherently unpredictable (a `.chd` extracts to either `.cue`+`.bin` or `.iso`,
decided at runtime), so its ghosts are flagged **approximate** (`≈ pending`).

## What predicts what

| Operation      | Ghost output                                   | Exact? |
|----------------|------------------------------------------------|--------|
| Copy / Move    | `dest/<name>` per source                       | ✅ |
| Compress       | `dest/<archiveName>.<format>`                  | ✅ |
| CHD convert    | one `.chd` per source (see `build_chd_output_path`) | ✅ |
| Extract        | the archive's **entire** tree — files and folders at every level (via deep backend preview) | ✅ |
| CHD restore    | `<stem>.cue` + `<stem>.bin` (or `.iso`)        | ≈ approximate |
| M3U            | one `.m3u` per selected playlist group         | ✅ |
| Delete         | — (removes files)                              | — |

Extraction outputs are predicted by calling `build_extraction_preview_deep` when
the op is queued — it enumerates the whole archive tree (files and folders at
every level, synthesising intermediate directories), so ghost folders are
navigable and you can queue operations on nested files. Remote and ghost
archives are not previewed, so they produce no ghosts (the extraction still runs
normally).

**Known limitation:** copying or moving a *folder* predicts a ghost for the
folder itself but not its contents, so you can't yet drill into a moved/copied
ghost folder to act on its children. Extraction is the fully-supported deep case.
Relocating a ghost subtree on copy/move is a planned follow-up.

## Execution & dependencies

- When you queue an op whose input is another op's ghost, the store links them
  (`dependsOn`) automatically by matching paths.
- A batch with dependencies **can only run sequentially** — the parallel button
  is disabled and each chained op shows a `⛓ after step N` badge.
- Removing an op also removes everything downstream that depended on it (with a
  notification), since those consumers would be left pointing at a ghost that
  will never appear.
- Reordering that would place a consumer before its producer is rejected.

## Where it lives

- `src/lib/utils/ghosts.ts` — path helpers, predictors, `ghostToEntry`.
- `src/lib/types/index.ts` — `GhostEntry`, `QueuedOp.produces` / `.dependsOn`.
- `src/lib/stores/app.svelte.ts` — dependency linking, `ghostsForDir`,
  cascade removal, ordered execution.
- `src/lib/components/shell/Shell.svelte` — op builders populate `produces`;
  `openChdDialog` synthesises a CHD analysis for ghost sources.
- `src/lib/components/content/ContentPanel.svelte` + `FileRow.svelte` — overlay,
  styling, ghost-aware context menu, ghost-directory navigation.
- `src/lib/components/dialogs/JobsPanel.svelte` — dependency badges, parallel gating.

## Manual test checklist

Queue mode must be ON (the queue-mode toolbar button) for ghosts to appear.

1. **Extract → convert (the flagship).**
   - Put a `game.zip` containing `game.cue` + `game.bin` in a folder.
   - Enable queue mode. Right-click `game.zip` → *Extract here*.
   - `game.cue` and `game.bin` appear as ghosts (dimmed, `pending`) in the folder.
   - Select the ghost `game.cue` → right-click → *Convert to CHD…*, confirm.
   - A ghost `game.chd` appears; the queue shows the convert op with `⛓ after step 1`.
   - The parallel button is disabled. Run *sequentially*.
   - Extraction runs, then conversion; `game.chd` becomes real.

2. **Extract to folder.** Right-click an archive → *Extract to "name"*. Navigate
   into the (ghost) folder — its predicted contents show. Queue a conversion there.

3. **Copy → compress.** Queue a copy of files into another folder; navigate there;
   select the ghost copies; queue *Compress to ZIP*. Run sequentially.

4. **Cascade removal.** Build a 2-step chain, then remove the first op — the
   dependent op is removed too, with a notification.

5. **No false chaining.** Queue two independent operations (different files/dirs).
   The parallel button stays enabled; no dependency badges.

6. **Overwrite dedup.** Queue an op whose output name matches an existing real
   file. The list must not show a duplicate row (no Svelte key error).

7. **Ghost guards.** Ghost files can't be opened, renamed, or inspected
   (Properties); those items are absent from a ghost's context menu.
