# Smart queue: execution and worker design

Status: proposed implementation specification. No runtime changes are part of this document.
Date: 2026-11-02. English technical specification; Spanish product summary below.

## 1. Objective and boundaries

Dogu is a multipurpose file explorer and ROM preparation workbench. Its queue
orchestrates tools and file operations; conversion is one capability, not the model
around which all other work is organised. CHD, CSO, XISO and RVZ have no scheduling
privilege over extraction, compression, transfers, organisation or future capabilities.

Allow ready steps of any workflow to advance while independent work continues. An
extracted archive may be organised and transferred while other archives are unpacked;
a converted image may be published and renamed while independent work runs. Preserve
the user's intended file transformations, output names and order where order affects
meaning. Optimise eligible execution, never reinterpret inputs.

The scheduler must prefer explainable, safe progress over maximum process count.
Successful completion cannot be guaranteed after external edits, connection failures,
process crashes or insufficient space. The guarantees are: no internal dependency
violations, no knowingly stale inputs, no silent name substitution, controlled
publication, and retained recoverable work.

Scope includes all queued kinds: copy, move, rename, delete, extraction, compression,
CHD conversion/restoration, CSO, XISO, RVZ and M3U. Direct operations use the same
resource/lock admission service. Future capabilities integrate through the same adapter
contracts, without adding tool-specific scheduling branches. Checksums remain immediate
and non-queueable, as requested; their availability guard shares the same affected-artifact
information. Extensibility does not override an operation's explicit product policy.

## 2. Findings in the current implementation

Verified by reading the current workspace and running in-memory scheduler examples:

| Area | Current behaviour | Consequence |
| --- | --- | --- |
| `queue-runner.ts` | Starts the first ready items in array order up to one process limit | A newly ready move can lose its slot to another earlier-listed conversion |
| `queue-plan.ts` | Hard producer dependencies and order constraints for conflicts | Existing graph is a useful foundation; execution is not strictly FIFO |
| Shared folders | A destination can depend on the first ghost directory producer | Independent extractions into a reusable folder can become serial |
| CHD builder in `Shell.svelte` | `sources` contains descriptors; `requiredPaths` is known separately | Track reads can be absent from conflict/dependency declarations |
| Ghost resolution | Most deterministic outputs are fixed; checks before/after execution | Useful safeguards, but path existence alone does not prove artifact identity |
| Remote sessions | One mutex protects the session map; some streaming methods hold it throughout I/O | Independent remote operations may serialize behind an internal lock |
| Remote conversion | Download, conversion and upload are one running job | A process slot remains occupied through different resource demands |
| Pause | `PauseRegistry` blocks a job thread awaiting a user decision | Waiting work still occupies its scheduler slot |
| External tools | No general per-task thread budget | Multiple tools may each try to use most of the CPU |
| Queue representation | Closures capture arguments; running queue is a snapshot | Difficult to persist, revalidate or safely edit a running graph |
| Restoration | Unknown CHD output geometry can produce approximate ghosts | Approximate paths must not become promises for exact downstream inputs |

Source map: `src/lib/utils/{queue-plan,queue-runner,queue-validation,queue-story,ghosts}.ts`,
`src/lib/stores/app.svelte.ts`, `src/lib/components/shell/Shell.svelte`,
`src-tauri/src/{ops,remote,pause,commands}.rs`.

## 3. Product decisions

These defaults make the design implementable without requiring a new product decision.
They are conservative starting policies, not performance measurements.

- Keep **Sequential**: one operation at a time, in displayed order; internal safe
  staging is allowed, independent later operations do not overtake.
- Evolve **Smart** into main workers plus optional assistance.
- Rename the current concurrency setting to **Main workers** when assistance is
  enabled. Do not silently exceed a setting still labelled maximum operations.
- Migration: retain the existing numeric limit and initially set assistance **off**
  for existing installations. Offer a clear opt-in explaining additional background
  activity. New installations also start off until the new scheduler passes rollout.
- Assistance settings: **Off**, **Automatic**, **Manual**. Automatic allows up to
  two concurrent supporting phases. Manual allows 1-4. These are caps, not targets.
- With assistance off, the existing maximum remains a global operation ceiling.
  With assistance on, at most P main phases and S supporting phases execute, with
  an absolute cap of P+S. Waiting operations do not count as executing.
- Assistance off keeps ready-list order as the dispatch tie-break and uses a shared
  pool for both roles. It does not refuse a supporting operation merely because main
  work exists. Sequential additionally waits for the preceding operation's full
  configured contract (including cleanup) before starting the next operation.
- Main workers retain the current supported 1-6 range. Sequential always ignores
  assistance and uses one operation. Settings reductions drain existing work;
  they never kill a running phase.
- Keep displayed user order stable. Mark overtaking in the execution view rather
  than rearranging the user's list automatically.
- Add an explicit **Finish these steps before continuing** fence when strict local
  ordering is required inside Smart. A normal list position is a preference except
  where data/namespace hazards require it as a constraint.
- Queue additions during a run belong to the next run by default. They see the
  running plan's output reservations but do not join its snapshot automatically.
- Do not add process preemption, GPU scheduling, automatic filesystem changes,
  global OS priority changes or automatic destructive crash recovery.

## 4. Architecture and authority

Use one Rust execution coordinator for plans, artifact state, locks, admission,
process ownership and the recovery journal. Svelte is a client of that coordinator.
Do not build competing frontend and backend schedulers.

1. Frontend sends serializable operation requests and options.
2. An operation adapter inspects inputs and produces a declarative operation plan.
3. The backend builds a versioned logical plan: artifact references, names,
   dependencies, ordered hazards, resource estimates and discovery barriers.
4. Frontend renders the returned plan and ghosts. Execution uses that same version.
5. Backend scheduler dispatches ready phases subject to permits and leases.
6. A reconciler validates phase results, records commits and advances dependencies.

Keep existing staging/publication routines where their contracts suffice. Wrap them
in adapters; do not rewrite all file I/O in the initial scheduler change. Legacy
monolithic adapters have conservative resource declarations until split safely.

Proposed modules under `src-tauri/src/queue/`:

- `model.rs`: versioned request, plan, artifact and event schemas.
- `planner.rs`: manifest inspection, hazard edges, name reservation and discovery.
- `resources.rs`: CPU, memory, storage, endpoint and executor admission.
- `leases.rs`: content/namespace ownership, mutation quarantine and path aliases.
- `scheduler.rs`: deterministic ready selection and state transitions.
- `journal.rs`: durable phase/commit records and restart inspection.
- `reconcile.rs`: observed outputs, fingerprints, generation updates.
- `adapters/`: per-operation inspect/prepare/run/validate/publish/cleanup contracts.

Frontend utilities eventually become view projections of backend plans, not an
independent source of filesystem semantics. Share JSON fixtures and schema version
checks during migration. An unsupported plan version is rejected, never approximated.

### Capability registry and extension contract

Register versioned adapters by operation kind. The scheduler consumes phases, access
claims, dependencies and resource profiles; it must not branch on `chd`, `cso`, `xiso`
or any other tool/format identifier. Tool-specific arguments and semantics belong in
the adapter. UI descriptions/options come from typed capability metadata; dispatch
and validation remain backend-authoritative. This is an internal extension boundary,
not a requirement to load arbitrary third-party executable plugins.

Each adapter registration supplies:

- A stable capability ID, options schema and adapter/plan compatibility version.
- Supported input/output families, local/remote requirements and availability probes.
- Complete input groups, access claims, output manifests/certainty and discovery rules.
- Phase boundaries, conservative CPU/memory/storage/space profiles, verified thread
  controls and eligibility for supporting admission.
- Cancellation/checkpoint, validation, publication, retry, cleanup and recovery contracts.
- User-visible progress units, diagnostic mapping and owned process/staging information.

Persist the adapter contract version with the plan and journal. If an adapter is
missing or incompatible after an update, block the affected work with an explanation;
do not silently select another engine or reinterpret its outputs. An approved fallback
engine stays within the same declared output/integrity contract and uses an isolated
attempt. Version changes require an explicit compatibility check or pending-plan replan.

A new tool can begin with a conservative monolithic main phase. Supporting admission
and early publication require evidence that their boundaries satisfy the common safety
contracts. Admission profiles are reviewed capabilities, not arbitrary hints supplied
by the UI. Adding a capability requires adapter contract fixtures and fault scenarios;
it must not require edits to dependency, lease or priority algorithms.

## 5. Identity, artifacts and namespaces

A path is a location, not a durable file identity. Define:

- `ArtifactId`: an existing item or the output of one logical producer.
- `ContentGeneration`: a particular content version.
- `BindingId`: one path binding of an artifact, with namespace generation.
- `ArtifactRef`: the exact artifact/content generation a consumer expects.
- `ArtifactGroupId`: a coherent set, such as a descriptor plus tracks, a split archive,
  a generated directory tree or a playlist and its explicit reference relationships.
- `LocationKey`: endpoint + normalized root + path components, with comparison policy.
- `StorageKey`: physical contention domain, which can cover several filesystem volumes.

Existing items are inspected when admitted; record kind, size, mtime at available
precision and native file ID/version/etag when available. Metadata fingerprints are
change detectors, not cryptographic proof. Strict copy verification can use hashes;
never imply a size/mtime check proves equal bytes.

A copy creates a new artifact. Conversion/extraction creates new content artifacts.
Rename and move change bindings while preserving the logical content identity.
An overwrite consumes the prior binding generation and publishes a new one.
Delete consumes the chosen binding generation. Recreating the same pathname creates
a new generation; its consumers cannot accidentally read the old file.

Local identity resolves existing ancestors and detects known symlink/junction and
hard-link aliases. Unknown aliases are treated conservatively. Do not traverse
symlink trees blindly. Remote identity uses a stable endpoint/profile/root identity,
not an ephemeral session ID; reconnecting does not silently change the server/root.
Respect provider case behaviour; unknown behaviour gets conservative collision checks
without rewriting displayed names or conflating artifact IDs.

Resolve downstream paths from artifact/binding references at planning time. Never
rewrite every matching string at dispatch. A consumer intentionally reading the old
location must finish before relocation; a consumer targeting the moved ghost depends
on the relocation commit. Relative references inside GDI/CUE/M3U are explicit group
relationships. A rename/copy that would break them requires an adapter that rewrites
and validates the references, otherwise reject it with an actionable explanation.

## 6. Complete access declarations and dependencies

Every adapter must declare **all** required reads, writes, removals and bindings.
For GDI/CUE this includes every referenced BIN, RAW and ISO, whether selected or not.
Compression declares every input/subtree it will enumerate. Folder moves/deletes
cover descendants. M3U declares the artifact bindings used to construct its entries.

Build the logical filesystem in displayed order before choosing execution priority:

1. Bind reads to the generation visible at that logical position, including exact
   output reservations of previous operations.
2. Add producer-commit dependencies for produced inputs.
3. Add last-writer -> reader, reader -> next-writer, and writer -> next-writer edges.
4. Add relocation, delete/recreate, descriptor-group and explicit fence edges.
5. Detect cycles, unavailable references and impossible destructive orderings.
6. Freeze names and publish a plan revision.

Edges have semantics, not only a boolean hard/soft label:

| Edge | Meaning | Failure handling |
| --- | --- | --- |
| `requires-artifact` | Expected generation must be committed | Failed/uncertain producer blocks consumer |
| `requires-binding` | Relocation/name must be committed | Consumer cannot use earlier binding |
| `hazard-order` | Preserve user meaning for overlapping accesses | Release only after predecessor succeeded or reconciliation proves it made no changes |
| `fence` | Explicit user ordering barrier | Failure/cancellation stops the downstream fenced region |
| `resource-wait` | Runtime capacity unavailable | Not a persistent dependency edge |

Initially retain conservative failure propagation. Later reconciliation may release
an order-only successor after a proven no-mutation failure; never infer this merely
from a process exit code.

Shared-directory creation uses idempotent `ensure-directory` with a compatible
container-intent lease. Merely sharing an existing directory does not make one writer
consume every output of another writer. Precise file writes may overlap in time;
rename/delete/replace of the directory conflicts with all descendant access.

Opaque folder reads bind a directory manifest version. Any queued descendant mutation
that should affect that view is ordered before inspection/consumption; subsequent
mutations wait for its readers. Do not snapshot a folder early and omit files that an
earlier logical operation is expected to add. Track parent directory membership
changes as well as content changes, including empty directories.

## 7. Worker roles and resource profiles

A worker is an execution permit, not a permanently dedicated thread. Never spawn an
unbounded task for every ready phase and let tasks wait inside the executor pool.

- **Main**: general execution capacity for substantial computation, compression,
  extraction, any format transformation, expensive inspection/verification and opaque
  tools without a certified narrower resource profile.
- **Supporting**: additional capacity for declared streaming transfers, safe namespace
  changes, folder creation, publication and cleanup, each with its full resource cost.
  Supporting means eligible for this capacity, not cheap, unimportant or converter-owned.
- Roles apply to phases. A move that falls back from rename to copy is reclassified
  before copying. A compression phase still requires a main permit even when it
  belongs to the continuation of an earlier operation.
- Classification follows the phase contract, not its format, list position or whether
  it follows a conversion. A bulk copy keeps the same profile when standalone or part
  of a chain. Expensive verification is a separate main phase; it cannot hide inside
  a supporting transfer. An unknown future operation defaults to main, conservatively.
- Main work cannot borrow supporting permits. Supporting work may borrow a free main
  permit only when no eligible main work needs it, and remains counted under P+S.
- Supporting phases do not require an active main phase. Transfer/organisation-only
  workloads use the same scheduler and may use idle main capacity under that rule.
  Resource budgets still constrain their actual parallelism.
- One logical operation executes at most one phase at a time. Independent operations
  can use different roles. Moving an entire folder cannot consume partly produced
  contents unless its explicit manifest defines that complete subset.

All executing phases additionally request an all-or-nothing resource vector:

| Resource | Initial policy |
| --- | --- |
| CPU | C = max(1, logical CPUs - 1) tokens, bounded by known host/adapter capability |
| Main tool threads | At most max(1, floor(C / P)) per newly launched main phase; no live thread resizing |
| Memory | Sum of adapter bounds plus bounded buffers; reserve host headroom. An unbounded/unknown large profile excludes other large profiles until a validated bound is available |
| Local storage | Weighted credits per physical device: initially 4 on identified SSD/NVMe, 2 on HDD/unknown. A verified CPU-dominated phase costs 1; bulk transfer or I/O-heavy processing costs 2; same-device bulk copy requests the device's full budget |
| Metadata | At most two supporting metadata phases globally, subject to path locks; not blocked just because an unrelated bulk transfer uses that disk |
| Remote | One active data stream per session initially; separate sessions can run if endpoint/storage budgets allow |
| Remote endpoint | One stream per endpoint initially; configurable later if provider supports safe parallel sessions |
| Cross-storage copy | Acquire source and target credits together, at cost 2 per device capped at its full budget; same-disk copy is exclusive against other data phases |
| Temporary space | Byte reservations per volume, including staged output and overwrite backups |
| Executors | Bounded blocking I/O/process-control threads; async waits consume no worker/executor permit |

The CPU token count is a scheduling estimate, not an OS utilisation guarantee. Supply
actual `chdman -np` / 7-Zip thread options only through verified adapter capabilities.
If a tool cannot be constrained, do not pretend it respects the token count: mark its
CPU profile exclusive. P is a maximum; low CPU/storage capacity may result in fewer
main tasks. Raise defaults only after measurements.

Storage weights represent contention, not a measured MB/s guarantee. An adapter must
justify the CPU-dominated classification; unknown transformations cost 2 credits.
Known SSDs can therefore admit two CPU-dominated phases and one transfer if other
budgets fit. HDD/unknown policy can deliberately limit that overlap. Show the storage
wait instead of claiming the second worker is broken. These constants require benchmark
validation before public default activation.

Memory admission initially caps estimated queue working sets at 70% of currently
available memory at run start, rechecked before each admission; already-running work
is not killed on a later drop. Reject an estimated single-phase demand that cannot fit
with an actionable reason. Estimates cannot prevent unrelated apps causing pressure;
stop new admissions under pressure. Unknown tool bounds require a conservative tested
adapter ceiling or an explicit warning/exclusive profile, never unlimited admission.

Large transfers are not treated as cheap metadata work. Storage mapping distinguishes
filesystem atomic-rename boundaries from physical contention domains. Two partitions
on one device do not get independent full-speed budgets. Unknown mappings share a
conservative domain. Network shares which resolve to the same endpoint share network
budget even with different profile names where this can be established.

Space estimates include existing intermediates, staged results, rollback backups and
pending transfers. Unknown output expansion is explicit: probe when possible; otherwise
admit only under a conservative adapter policy and show uncertainty. Unsupported
free-space queries do not mean infinite free space. User may explicitly proceed with
an unknown-space warning; do not silently reserve a fabricated size. Recheck during
production, stop before destructive cleanup when space cannot be maintained.

## 8. Deadlock avoidance

Logical reservations (artifact names, expected generations, quarantine) are distinct
from execution leases (active data access) and resource tokens.

- The coordinator grants the complete current phase lease/token vector in one decision,
  including memory, executor and I/O capacity. A denied phase holds none of those.
- No task acquires storage A and blocks while holding it for storage B.
- Lock hierarchies use stable resource/location ordering. Descendant conflicts are
  checked by the central lease manager, not independent folder mutexes.
- Never hold the session registry mutex during streaming. Use per-session handles,
  with per-session access coordinated by the resource broker. Multi-session operations
  acquire both handles in a canonical order or use explicitly staged independent phases.
- A phase must release execution leases and tokens before waiting for the user, a
  retry delay or the next phase. It retains logical output reservations and owned staging.
- Inputs needed later retain logical generation pins. A blocked mutator is identified
  in the graph; it must not hold CPU/I/O permits while waiting for that pin.
- Pins release after the last declared consumption/conditional cleanup of that
  generation, not automatically at whole-run completion. A stable private input copy
  can end an original read early only if the adapter validates it and preserves any
  original-generation precondition needed by later delete-originals cleanup.
- Add pin and reservation wait edges to the wait-for graph on discovery, replan and
  recovery. Validate acyclicity before accepting the revision; reject an expansion
  that requires a task to release data only after its own descendant runs.
- Commit/rollback is bounded and cannot pause halfway for a user prompt. Fail into
  reconciliation/quarantine if it cannot finish safely.
- A phase must not retain borrowed capacity while trying to upgrade it. End the phase,
  preserve owned staging and request a new complete vector.
- If disk space makes all ready producers inadmissible, ready safe cleanup is preferred.
  If the cleanup itself depends on those producers, show insufficient space; never
  bypass its data dependency to force progress.

A long read or external tool may delay cancellation until a safe boundary. Timeout
policy must distinguish cancellation requested from cancellation completed. No forced
unlock while a process can still write.

## 9. Operation and phase contracts

Every adapter exposes `inspect`, `plan`, `preparePhase`, `executePhase`,
`validatePhase`, `commit`, `reconcile` and `cleanupOwned`. It may initially represent
an operation as a conservative monolithic phase. Phase transitions are explicit events.

| Kind | Logical phases | Important declarations |
| --- | --- | --- |
| Copy | inspect -> stage copy -> verify -> publish | Source generation and complete tree if a folder; target binding; backup requirements |
| Move, same filesystem | inspect -> guarded rename -> reconcile | Source and target namespace mutation, descriptor-group closure |
| Move, fallback | inspect -> stage copy -> verify -> publish -> delete originals | Source generation must remain unchanged until validated publication; re-admit for copy resources |
| Rename | inspect reference impact -> guarded namespace commit | All affected bindings and internal references; reject unsupported group rewrite |
| Delete | validate selected generations -> remove -> reconcile | Full affected subtree, source ownership; no rollback promise for removed user data |
| Extract | inspect index -> produce staged tree -> validate manifest -> publish -> optional original cleanup | Exact archive generation, path traversal rejection, output group and split-layout mapping |
| Compress | snapshot logical input manifest -> compress -> validate -> publish -> optional cleanup | Every input read, archive generation, codec/threads/space limits |
| CHD convert | inspect descriptor/track group -> materialize if remote -> convert -> validate -> publish -> optional cleanup | GDI/CUE plus ALL referenced tracks including ISO; referenced ISO is not a separate disc |
| CHD restore | inspect geometry -> materialize -> restore -> discover/validate group -> publish -> optional cleanup | Geometry-dependent manifest, GDI/CUE reference consistency, no guessed tracks |
| CSO/RVZ convert/restore | inspect -> materialize -> transform -> validate -> publish -> optional cleanup | Fallback engines share one logical output; each attempt isolated and budgeted |
| XISO pack/unpack | inspect source/tree -> materialize -> transform -> validate -> publish -> optional cleanup | Output tree manifests and safe path handling |
| M3U | resolve referenced bindings -> generate staged text -> publish | Final intended paths and playlist contents, no mutable string remapping at execution |
| Ensure directory | inspect -> create if absent -> validate directory | Compatible shared intent, fail if an incompatible file occupies the path |
| Future capability | adapter-defined inspect/produce/discover/validate/publish/cleanup phases as needed | Same artifact, access, resource, recovery and diagnostic contracts; no special scheduler path |

The table describes adapters, not a scheduler dispatch table. Every operation is a
first-class workflow, including one with no conversion at all. A batch is a UI grouping;
only declared dependencies/fences introduce execution barriers between its items.

Remote materialisation is normally a supporting streaming phase; substantial computation
uses main capacity; upload and publish are supporting. This applies equally to archive
work, format tools and future operations. Keep downloaded input and generated output
in owned staging between phases. Before reusing them after a wait, revalidate the bound
source generation.

A successful transform may release its main permit after its owned intermediate is
closed and validated. This lets conversion C begin while A uploads. A's final artifact
is not available until its publication commit. Where the adapter still transforms
straight into a final path, it cannot release/publish early; migrate to isolated staging
first. Waiting for publish retains disk-space reservation but not a CPU permit.

Validation is adapter-specific. A hash/format verification pass consumes resources
and must be declared, not hidden from budgets. Exit code plus file existence alone is
insufficient for manifest completeness. Use tool metadata checks, expected file kinds,
reference checks, byte counts and transfer integrity guarantees appropriate to each
backend. Record the verification level actually performed; no implied full cryptographic
verification. Before any delete-originals phase, satisfy the adapter's publication and
integrity contract, including every member of a descriptor group.

Validate actual staging and final path limits, descriptor references and traversal/
alias behaviour before each phase creates data. Cleanup targets only the declared
original generations and owned stages. Never recursively remove a source container
which now contains a valid output or unrelated/new user files; empty-folder cleanup
requires a fresh guarded emptiness check.

A user-visible operation succeeds only when its configured cleanup is complete.
Dependent consumers need the artifact's commit, not necessarily unrelated final
cleanup, provided access declarations prove that early consumption is safe. Cleanup
failure produces `completed-with-cleanup-error`; show that output is valid and source
removal is incomplete. Do not reconvert committed output when retrying that cleanup.

## 10. Names, publication and truthful ghosts

Reserve output names from logical order before scheduling. Requested and actual planned
names are explicit fields. Reserve families together (e.g. `game.cue` and `game.bin`).
An external collision never triggers a hidden suffix change under an existing consumer.
Stop and offer a new plan revision with all affected consumers/ghosts updated for review.
No actual placeholder file is created merely to display a reservation.

Artifacts have certainty states:

- `exact`: verified input manifest/format determines names and kinds.
- `container-known`: output container is known, members are unresolved.
- `approximate`: only illustrative candidate names are available.
- `observed`: committed actual manifest/fingerprints are available.

Exact path-bound downstream requests require exact or observed artifacts. A known
container can support a future **selector**, such as convert supported discs in this
container after discovery. A selector is not a fabricated exact file. Type-specific
operations on approximate members remain blocked until discovery resolves the choice.

Discovery barriers inspect unknown archive indexes/CHD geometry or observe a producer's
owned stage. They may add bounded suboperations in the original logical operation's
position, before any affected consumer is admitted. Expansion must preserve IDs of
already exact artifacts, reservations and all committed work. If a user decision changes
semantics (e.g. a CD conversion requested but actual output is a DVD), wait for review.
Do not execute a different transformation automatically just to keep the queue moving.

Shared folders have multiple contributing producers. A consumer of one exact member
waits only for that member's producer; a consumer of the full folder waits for all
producers whose earlier logical writes are part of its directory manifest. This is
how batching avoids both false serialisation and partial-folder compression/moves.

Publication establishes a logical commit record containing the actual output manifest,
fingerprints and verification level. Emit `artifact-committed` only after the backend
has closed/validated output and recorded the commit. An internal consumer cannot use
a merely visible partial file. A file move within one filesystem may publish with an
atomic primitive; multi-file and remote publication are not assumed filesystem-atomic.
Use owned staging, per-member journal, backups and guarded commit/rollback. External
programs may see intermediate namespace changes for a multi-file commit; Dogu's own
consumers are gated until the whole promised group is reconciled.

Keep planned, staging and published paths distinct. Ghosts derive from plan/artifact
state, not a second frontend prediction pass. Show uncertainty, changed-plan review,
producer failure and cancellation. Historical delete/recreate and rename stories refer
to artifact/binding generations, so identical path strings do not collapse different
files. Default overlay shows pending work, not a duplicate of committed real files.

## 11. Scheduling algorithm and priorities

Preserve one stable order key per logical operation; internal phases also have stable
IDs. Priorities do not alter the logical filesystem or output naming. Operation kind,
format popularity and choice of external executable confer no implicit priority.

On an event (completion, new phase, token release, user decision, resource change):

1. Reconcile completed phase attempts; commit observed manifests first.
2. Propagate failed/uncertain requirements and fence blocks.
3. Build the dependency-ready phase set. Discovery-incomplete is a separate wait reason.
4. Choose main and supporting candidates under their permit ceilings.
5. Reject candidates whose complete lease/resource vector cannot be granted now.
6. Dispatch selected candidates and repeat until no admissible phase fits.
7. If nothing runs, identify the reason: user decision, unavailable connection, space,
   unavailable resource, unsatisfied external input, or invalid graph. Do not spin.

Deterministic priority, in precedence order:

1. Complete an already-started commit/reconciliation that can safely progress.
2. A ready phase aged at least 30 seconds, oldest-ready first within its eligible role.
3. User priority tier (`high`, `normal`, `low`; default normal), never above safety/fences.
4. Ready cleanup that demonstrably frees constrained temporary space.
5. Ready continuation of an operation/chain whose producer has just committed.
6. More downstream work unlocked, using graph counts rather than invented ETA.
7. Original logical order, then stable phase ID.

Thirty seconds is a tuneable policy constant. Age accrues while dependency-ready,
including resource wait; preserve it across resource backoff. Aged work may reserve
priority for the next compatible capacity release, preventing new conflicting younger
admissions from consuming that capacity indefinitely. Existing running phases finish
normally. Deadlines are not guaranteed, and external waits do not count as scheduler
starvation. No preemption or automatic OS process priority adjustment is required.

Use duration/throughput history initially for explanations and estimates, not to make
correctness decisions. Do not claim an optimal makespan: the online workload has
unknown durations and externally shared resources. Optional later critical-path ranking
must retain deterministic tie-breaks and aging.

The key acceptance example with P=2, S=2:

- Convert A and B run on main permits when resource budgets allow.
- A finishes its transform; C may use the freed main permit.
- A's publish/move/rename chain uses supporting capacity independently of C and B.
- If A's move needs the disk that B is saturating under the configured disk policy,
  it waits with an explicit resource reason. No dependency on unrelated conversion C
  is invented. A metadata rename on an unrelated path may still proceed.
- If compression follows moving A, it competes for a main permit; it is not smuggled
  through the supporting pool merely because it is a continuation.

An equally required example contains no conversion: extract archives A and B, rename
A's committed output group, transfer that group, and generate its playlist while B
continues. The group moves only after its complete manifest is committed; the playlist
uses the final bindings. Independent compression can use freed main capacity. A pure
copy/move/rename workload uses the same admission and continuation rules without
inventing a transformation as its central step.

## 12. State machine, pauses, failures and cancellation

Phase states: `planned`, `waiting-discovery`, `waiting-dependency`, `ready`,
`waiting-resources`, `running`, `waiting-user`, `retry-backoff`, `committing`,
`reconciling`, `succeeded`, `failed`, `cancelled`, `blocked`.

Operation aggregate states distinguish executing from waiting and preserve a committed
output even when later cleanup fails. Progress is monotonic per phase/attempt with an
explicit attempt counter; phase weights are estimates, byte counts are observations.
Do not fabricate a complete percentage when future work size is unknown.

On failure:

- Proven no mutation: keep owned stage if useful, release execution leases/tokens,
  retain output reservations and record retry eligibility.
- Commit succeeded but cleanup failed: publish artifact state, retain cleanup task.
- Mutation uncertain/partial rollback: quarantine the affected bindings/subtree;
  block readers/writers there and retain backups/journal. Independent work continues.
- Lost remote connection: release capacity, pin expected endpoint/root/artifacts;
  reconnect and revalidate before retry. No automatic host-trust changes.

Retries belong to one logical operation, with new attempt IDs. Resume at the earliest
uncommitted safe phase; do not clone away its dependencies or generate another output
name. Automatic retry only repeats isolated idempotent preparation/transfer work with
bounded attempts and backoff. Never blindly repeat publication or destructive cleanup.

A user pause releases execution capacity and active leases at a safe boundary. Logical
reservations/pins remain and are displayed as the reason conflicting work cannot run.
Keep isolated outputs for retry. Resume revalidates inputs, target baselines and free
space, then goes through normal resource admission. Do not hold a blocking Rust worker
thread while the user is considering a prompt.

Cancellation requests cooperative stop, waits for owned processes/streams to terminate,
and cleans only provably owned temporary data. Commit-critical work first reaches a
safe commit/rollback boundary. Cancelled prerequisites block consumers; do not silently
rebind them to another artifact at the same path. User can explicitly cancel a subtree.
All reads/writes/retained recovery paths remain visible when cancellation is incomplete.

## 13. Persistence, process ownership and crash recovery

Replace queued execution closures with schema-versioned descriptors. Persist plans,
phase attempts and commit records in a local transactional journal (SQLite is the
proposed implementation). Credentials and access tokens never belong in that journal;
store profile IDs and request authentication through existing connection services.

Persist intent before filesystem mutations, then record their outcome. Use owned stage
IDs and commit manifests to reconcile crashes between a write and its completion record.
Never infer ownership from a filename prefix alone.

The process supervisor must know the child process tree, attempt ID and paths it may
write. Use supported platform process ownership/termination facilities. After a crash,
do not resume a conflicting task until surviving writers are stopped or identified as
requiring user action. If the coordinator restarts with no reliable ownership proof,
quarantine those paths and show recovery rather than trying to kill arbitrary processes.

At startup inspect unfinished journal records and preserve necessary recovery stages
**before** running orphan-temp cleanup. Existing `sweep_temp_dirs` must be made journal-
aware before durable recovery is enabled. Stale generation/name reservations survive
restart only as recovery reservations until revalidation succeeds.

Recovery offers resume safe remaining work, inspect paths, or discard provably owned
staging. It never automatically deletes originals or declares a partially published
artifact complete. App close offers wait or cancel at safe boundaries; no implied
exactly-once behaviour across arbitrary filesystem/server failures.

## 14. Plan editing and global coordination

A submitted run freezes its version. Reorder/priority/worker-setting commands carry
`expectedPlanVersion`; reject stale updates and return the latest snapshot.

- Reorder pending work: rebuild affected logical bindings, hazard edges and ghost
  projections. Present semantic/name changes before accepting. Keep committed outputs
  and active phase contracts unchanged.
- Priority changes: affect future dispatch only. They do not move list positions or
  invalidate namespaces.
- Remove work: explain downstream affected operations; invalidate only references to
  those producers. Never leave an exact ghost pointing at a removed producer.
- Queue additions during execution: next run; observe active output reservations and
  access conflicts. On completion/failure revalidate against observed outcomes.
- Direct operations: same coordinator admission and path ownership, so a direct move
  cannot race a queued conversion. Immediate UI does not mean permission to bypass locks.
- Checksums: remain immediate; block if a selected artifact is involved in queued or
  active operations, as already requested. They do not become supporting queue jobs.
  Check and acquire their read lease atomically. If a new queued operation later
  involves that artifact, request checksum cancellation and wait for its read lease
  to end before conflicting execution. Merely disabling a UI button is insufficient.

If live-append is later added, it requires a separate reviewed transaction that binds
new consumers to the active plan and extends reservations. It is not a side effect of
pushing an item into a frontend array.

## 15. UI and settings contract

The main list remains the user's logical workflow order with batch grouping. Provide a
separate execution view showing main/supporting lanes, current phase, resources and
actual events. Lanes are current assignments, not stable worker identities.

Each pending operation exposes one primary explanation and expandable detail:

- waiting for producer/member/group publication;
- waiting for a discovery result or user decision;
- waiting for CPU, storage, memory, temporary space or remote session;
- blocked by changed input, uncertain mutation, failed predecessor or fence;
- ready; executing; committed, cleaning originals; recovery required.

Show P/S configured capacities and current use; say “up to” rather than claiming all
will run. The assistance setting explains that additional transfers may compete for
I/O even with little CPU use. Advanced settings initially expose main count, assistance
mode/count and operation priority/fences. Avoid a large collection of resource knobs
until measurements justify them.

When a supporting step overtakes an earlier unrelated item, the execution view can
explain “input complete; transfer capacity available”. Resource recommendations cannot
change user settings automatically. Lowering capacity applies to the next admission.

Preserve exact ghost destinations. A plan revised after external changes clearly marks
which items changed and requires user review for any changed promised output. Do not
show guessed restoration members as if their exact paths were guaranteed.

## 16. Serializable interface sketch

Illustrative types; wire field naming and enums must be frozen with schema fixtures.
IDs are opaque. Paths preserve original display spelling alongside comparison keys.

```ts
type OperationSpec = {
  schemaVersion: 1;
  id: string;
  kind: OperationKind;
  adapterContractVersion: string;
  logicalOrder: string;
  batchId?: string;
  inputRefs: ArtifactRef[];
  options: OperationOptions; // tagged by kind, not executable closures
  priority: "high" | "normal" | "low";
  fenceAfter: boolean;
};

type ArtifactRef = {
  artifactId: string;
  contentGeneration: string;
  bindingId?: string;
  selection: "exact" | "container-selector";
};

type PhasePlan = {
  id: string;
  operationId: string;
  role: "main" | "support";
  requires: Edge[];
  accesses: AccessClaim[]; // artifact generation, namespace membership and subtree scopes
  resources: ResourceDemand;
  stageContract: string;
  retryClass: "isolated-idempotent" | "reconcile-first" | "never-automatic";
};

type PlanRevision = {
  schemaVersion: 1;
  planId: string;
  revision: number;
  operations: OperationSpec[];
  phases: PhasePlan[];
  artifacts: ArtifactRecord[];
  reservations: NameReservation[];
  diagnostics: PlanDiagnostic[];
  policy: SchedulerPolicy;
};

type ExecutionEvent = {
  runId: string;
  planRevision: number;
  sequence: number;
  operationId?: string;
  phaseId?: string;
  attemptId?: string;
  kind: EventKind;
  payload: EventPayload;
};
```

Proposed Tauri commands:

- `queue_preview(specs, baseRevision)` -> inspected plan + diagnostics + ghosts.
- `queue_submit(planId, expectedRevision, mode)` -> run ID; revalidate before acceptance.
- `queue_snapshot(runId)` -> state, last event sequence and revision.
- `queue_events(runId, afterSequence)` -> incremental events/resync request.
- `queue_set_policy`, `queue_set_priority`, `queue_edit_pending` -> versioned updates.
- `queue_pause`, `queue_resume`, `queue_cancel`, `queue_retry` -> operation/phase-aware actions.
- `queue_recovery_inspect` -> owned stages, observed commits and safe actions.

Persist before emitting mutation-completion events. Frontend ignores duplicates and
old sequence numbers and resynchronises after gaps. Progress events may be coalesced;
commit, failure and recovery events must not be lost or reordered. Avoid secrets in
logs/events. Validate user options and input/output paths again in the backend.

## 17. Invariants and acceptance scenarios

Invariant checks run in deterministic scheduler tests and debug assertions:

1. No phase starts without committed required generations and all current leases/tokens.
2. No incompatible accesses overlap, including aliases and directory membership changes.
3. A phase never holds a partial resource vector while waiting for the rest.
4. Main/supporting and absolute executing limits are never exceeded.
5. Every observed ghost/consumer refers to its planned producer and generation.
6. Committed names cannot change as a side effect of execution priority.
7. Destructive cleanup requires its validated publication prerequisites.
8. Failure/cancellation never reroutes consumers to stale or unrelated data.
9. Durable committed work is not repeated during recovery/retry without explicit reason.
10. Every nonterminal wait has a reason; scheduler starvation is detectable.
11. Owned cleanup never escapes the owned stage/group manifest.
12. Cross-phase progress cannot report a whole operation complete before its contract is met.

Required scenario matrix:

| Scenario | Expected result |
| --- | --- |
| A/B/C conversions + move/rename A | A's ready support chain can overlap unrelated main work when resource vector fits |
| Extract -> rename -> transfer alongside independent compression | Ready phases advance using the same contracts, without any conversion in the plan |
| Transfer/organisation-only workload | Support runs without active main work and may borrow idle main capacity; storage/session limits still apply |
| Mixed CHD/CSO/XISO/RVZ/archive operations | No format receives implicit priority; dependencies, explicit priority, aging and resources govern admission |
| New fake capability with unfamiliar kind | Scheduler executes its declared phases without tool-specific branches; incomplete contracts are rejected |
| Adapter unavailable/incompatible after restart | Affected plan blocks for supported recovery/replan; no silent engine substitution |
| A -> move A -> compress A | Binding remains stable; compress waits for move commit and a main permit |
| Several producers in shared folder | Exact-member consumers need their producer; full-folder consumers wait for all logical contributors |
| GDI + ISO/BIN/RAW tracks | One descriptor conversion, all tracks declared as reads; moving any required track is ordered |
| CUE/GDI name changes | Supported reference rewrite is verified; unsupported mutation is rejected |
| Repeated file name after deletion | Consumer reads the intended generation, never the old file |
| Concurrent rename-on-conflict | Names reserved in logical order and passed to the backend unchanged |
| Unexpected external destination | Producer stops for review; downstream ghosts do not silently change suffix |
| External input edit during staging | Invalidate/reinspect, preserve originals; no stale commit/cleanup |
| Approximate CHD restore -> convert | Discovery resolves geometry; exact operation waits or requests review |
| Nested archive created by prior op | Discovery barrier expands only the correct container/position |
| Same-filesystem move becomes cross-filesystem fallback | Release metadata admission; reacquire copy resources before transfer |
| CPU and network phases of remote conversion | CPU permit released before upload; consumer waits for final publication |
| Two remote endpoints | Registry mutex does not serialize full transfers; endpoint/session caps respected |
| Same endpoint via two profiles | Known common contention is budgeted together |
| User pause/retry | No occupied main slot; reservations retained; resume revalidates and re-admits |
| Remote upload interrupted | Isolated retry or quarantine; source retained; no duplicate final output |
| Partial multi-file publish and rollback failure | Group quarantined, backups retained; independent paths can continue |
| Output committed, source cleanup fails | Retry cleanup only, consumers can use valid output where safe |
| Space full with ready safe cleanup | Cleanup can free reserved pressure without violating read dependencies |
| Space full with cleanup dependent on producer | Actionable blocked state; no unsafe dependency bypass |
| P lowered while phases run | Existing phases drain; new dispatch uses reduced ceilings |
| Continuous short support work | Aged compatible main/transfer work obtains released capacity eventually |
| Fence within otherwise independent work | No operation beyond the fence starts until its prerequisite region succeeds |
| Items added during run | Next-run plan sees active reservations and reconciled final outcomes |
| App/webview restart during commit | Journal/process reconciliation precedes any conflicting execution or sweeping |
| Hash requested on involved file | Immediate action stays blocked; never enqueued as a supporting task |
| Unknown hardware/provider capability | Conservative admission with explanation, not an invented guarantee |

Use a fake clock and fake adapters for graph/event/resource property tests. Generate
random operation sequences with namespace generations and fault injection at every
prepare/publish/journal boundary. Compare their final logical filesystem with a serial
reference interpreter of user order. This is a semantic equivalence oracle, not a
requirement that wall-clock ordering match the serial run.

Add real disposable-file tests for same/cross-volume moves, group references, aliases,
external collisions, Windows path constraints, file changes, process termination and
recovery. Remote tests need controlled SFTP/FTP/SMB fixtures and injected disconnects;
unit tests alone cannot establish provider publication/cancellation guarantees.

Performance scenarios measure actual makespan, completed chains, temporary-space peak,
per-device I/O, tool threads, queue wait reasons and UI responsiveness. Compare current
scheduler, new assistance-off and new assistance-on under the same inputs/host. Use
at least CPU-heavy, disk-heavy, network-heavy and mixed workloads. Simulated speedups
are labelled simulated; don't promise a fixed percentage improvement. Resource defaults
are changed only with reproducible measurements and no safety-invariant regressions.

## 18. Implementation sequence and rollout gates

Each stage is independently reviewable. No runtime changes are requested by this design.

1. **Contracts and regression fixtures.** Inventory every adapter's complete read/write
   sets. Fix descriptor-track declarations and shared-directory dependency semantics.
   Add serial-reference tests, generation fixtures and existing-behaviour snapshots.
2. **Serializable backend planning.** Introduce artifact/binding IDs, operation specs,
   exact name reservations and backend plan versioning. UI consumes plan projections.
   Keep existing concurrency and assistance disabled. Require old/new plan parity for
   supported existing cases; explicitly mark previously ambiguous cases.
3. **Global admission and resource broker.** Add complete leases, P/S counters, CPU/tool
   budgets, storage/endpoint mapping and wait diagnostics. Bring direct operations under
   admission. Initially use legacy monolithic adapters with conservative profiles.
4. **Owned staging and remote concurrency.** Remove long-held session-map locks, define
   per-session admission, split materialise/transform/publish phases and instrument
   resources. Enable assistance only for adapters with isolated, validated boundaries.
5. **Durable recovery and supervisor.** Add journal and process ownership, make startup
   cleanup journal-aware, fault-test publication/retry/cancellation. If this is not ready,
   do not expose automatic resume after app restart.
6. **Discovery and groups.** Implement exact CHD geometry/archive manifests, late selectors,
   directory-version semantics and validated descriptor-group rename/relocation.
7. **Smart priority and UI.** Add deterministic continuation/aging policy, lanes, fence,
   wait reasons, settings migration and explicit assistance opt-in. Benchmark before
   recommending automatic assistance broadly.

Stages 4-6 can be developed incrementally but public assistance is gated on: accurate
access sets, no registry mutex held across unrelated remote I/O, bounded capacities,
truthful ghosts, safe phase retry and recovery behaviour clearly exposed to the user.
Acceptance includes archive-only, transfer/organisation-only and mixed-format workloads,
plus a synthetic new capability. Passing CHD workflows alone does not satisfy the gate.

Rollback strategy: disable assistance and run validated operation descriptors under
single-operation admission. Preserve journal/reservations/recovery information. Do not
fall back to an old executor which cannot understand an active plan's generations or
partially committed group. Finish/reconcile or safely stop that plan first.

## 19. Decisions made and remaining validation

No mandatory product question remains for producing this specification. Proposed choices
are explicit: optional assistance, bounded automatic support, preserved logical list,
fences for strict local order, no live append initially, immediate non-queueable hashes,
and safe partial success/retry semantics.

The following are engineering validation tasks, not questions the user must guess:

- Measure physical-device/profile costs and select tested storage/memory defaults.
- Verify each external tool's thread controls and each provider's cancellation/rename
  semantics on supported platforms.
- Choose and pin the SQLite Rust integration according to the project's runtime and
  licensing/build requirements; document transaction durability settings.
- Establish source fingerprints and alias detection capabilities per platform/provider.
- Validate true remote concurrency after splitting the session registry and handles.
- Establish backup/stage ownership and space accounting across every publication path.

If validation shows a product-visible compromise (e.g. a backend cannot support safe
reference-group renaming or the chosen assistance defaults overload typical hosts),
bring one consolidated briefing with measured evidence, alternatives and a recommended
choice before changing the promised behaviour.

## 20. Resumen de producto en español

Dogu se diseña como un explorador con herramientas para preparar ROMs y organizar
archivos. La cola orquesta todas sus operaciones: CHD, CSO, XISO, RVZ, extracción,
compresión, transferencias, renombres y futuras herramientas. Ningún formato obtiene
prioridad especial ni es necesario que haya conversiones para aprovechar la asistencia.

La propuesta conserva el orden lógico de la lista y permite adelantar trabajo compatible.
Los workers principales aportan capacidad general para procesamiento, transformaciones
e inspecciones o verificaciones costosas. Los asistentes aportan capacidad adicional para
transferencias, publicación, renombres y limpieza, respetando su consumo real. El papel
se decide por el contrato de cada fase: una compresión siempre necesita capacidad
principal, aunque continúe una cadena ya iniciada. Una transferencia mantiene el mismo
perfil tanto si es independiente como si sigue a otra operación.

Las herramientas se integran mediante adaptadores con contratos versionados de entradas,
salidas, recursos, validación y recuperación. Incorporar una nueva conversión u otra
función no exige enseñar al planificador reglas especiales para esa herramienta. Si un
adaptador no puede garantizar fases separadas, empieza con una ejecución conservadora.

El usuario controla los principales. La asistencia es opcional: desactivada, automática
con hasta dos asistentes, o manual con límite de uno a cuatro. La migración conserva el
límite actual y deja la asistencia apagada hasta que el usuario la active. Además de esos
máximos, Dogu limita CPU, memoria, discos, conexiones y espacio temporal. No se lanzan
asistentes ilimitados ni se promete que todos los huecos puedan usarse siempre.

Un resultado terminado y validado puede publicarse, moverse y renombrarse mientras siguen
otras extracciones, compresiones, conversiones o transferencias compatibles. Cada
consumidor espera la publicación de sus datos concretos. Una
operación sobre una carpeta completa espera todos los resultados que lógicamente deben
estar dentro. Compartir carpeta no obliga a serializar archivos independientes.

Los fantasmas representan identidades y generaciones, además de rutas: se fija qué
archivo crea cada operación, qué nombre promete y qué archivo consume la siguiente.
Adelantar tareas no cambia esos contratos. Las salidas desconocidas se descubren antes
de ejecutar consumidores exactos; si hace falta decidir un formato o cambiar un destino,
se pide revisión. Un GDI/CUE mantiene la asociación con todas sus pistas y un cambio de
nombre no puede romper esas referencias sin advertencia o soporte explícito.

Las tareas que esperan no ocupan workers. Los permisos de ejecución se conceden juntos
para impedir esperas circulares por recursos. Las pausas liberan capacidad y conservan
reservas lógicas. Los reintentos continúan desde una fase segura y no repiten una
operación cuyo resultado ya se publicó si solo falló borrar el original.

Un fallo parcial bloquea las rutas afectadas y preserva los datos de recuperación. El
resto del trabajo independiente puede continuar. La recuperación tras cerrar o caer la
aplicación necesita un registro persistente y control de los procesos externos; nunca
se promete completar una operación con datos inciertos ni se borran temporales de una
recuperación pendiente al iniciar Dogu.

La interfaz mantiene la lista del usuario y añade una vista de ejecución con principales,
asistentes y motivos de espera. El modo secuencial sigue siendo estricto. En el modo
inteligente se puede añadir una barrera explícita para exigir que una región termine
antes de continuar. Las tareas añadidas durante una ejecución quedan para la siguiente,
respetando las reservas y resultados de la ejecución activa.

La implementación debe comenzar por las dependencias completas y las identidades de
archivos. Después se incorporan el coordinador y los recursos, las fases seguras,
la recuperación, el descubrimiento de salidas y finalmente la asistencia y prioridades.
La validación compara el resultado con una ejecución secuencial de referencia e inyecta
fallos; las mejoras de rendimiento se miden con cargas reales.
