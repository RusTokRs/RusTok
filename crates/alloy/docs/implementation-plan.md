# Implementation Plan for Alloy

## Scope

Alloy owns script and module authoring: source workspaces, drafts, revisions,
tests, repair, scheduling/hooks, review, AI-assisted evolution, release staging,
and marketplace forks. It is a capability runtime, not a tenant business
module.

Generic isolation, executor policy, capability enforcement, and sandbox outcomes
belong to `rustok-sandbox`. Marketplace publication, installation, activation,
and release governance belong to `rustok-modules`.

The cross-component sequence and completion rules are defined by the
[canonical module-platform plan](../../../docs/modules/module-control-plane-consolidation-plan.md).

## Current state

Implemented:

- script model/storage, triggers, hooks, scheduling, execution history, GraphQL,
  HTTP, and runtime composition;
- stable runtime hardening contract and static verifier;
- generic Rhai kernel extraction into `rustok-sandbox`;
- Alloy adapter over the neutral Rhai engine;
- broker-backed HTTP capability bridge with no direct HTTP client;
- versioned `AlloyDraftRequestBuilder` that pins draft ID, source revision,
  source digest, sandbox phase, tenant, actor, input, grants, and limits;
- data-only `AlloyDraftInput`/`AlloyDraftOutput` payloads for parameters,
  entity snapshots, returned values, and entity changes, carried inside the
  strict shared `RhaiBindingInput`/`RhaiBindingOutput` v1 envelope;
- request-scoped `AlloyDraftScopeExtension` that reconstructs `params`,
  `entity`, and `entity_before` for the neutral Rhai executor and emits typed
  entity changes;
- immutable Rhai descriptor/source lineage staging, packaging, and forking
  helpers.

Remaining:

- production draft/manual/hook/scheduled execution now uses
  `AlloyDraftRuntime` over `SandboxRuntime`; `ScriptEngine` remains only for
  compile-time CRUD validation and internal unit tests, never production code
  execution;
- tenant-scoped `SeaOrmStorage` now applies the tenant predicate to every
  single-script read, save, delete, status, and error path, and rejects a
  cross-tenant save as `NotFound`;
- `ScriptRegistry::save` now treats the stored script version as the expected
  revision and uses a durable revision predicate for SeaORM updates. Every
  storage mutation advances that revision, and stale saves fail as
  `RevisionConflict` instead of overwriting current state;
- `alloy_script_revisions` now records immutable workspace, digest, author, and
  parent-revision lineage in the same transaction as every admitted SeaORM
  mutation. A pre-ledger script receives a baseline snapshot before its first
  new revision commits. Owner storage exposes tenant-scoped lookup by
  `(script_id, revision)` and revision-ascending history without SQL bypass;
- entity/parameter semantics must become request-scoped Alloy extensions;
- REST and GraphQL update commands now require the caller's expected revision;
  manual-run commands use the same requirement and execute the loaded snapshot
  without a second registry lookup. Idempotency, workspace-level command
  revisions, review, and publication orchestration still need owner contracts;
- marketplace release import/fork needs a complete persisted workflow;
- AI-assisted Rust/WIT authoring must use the isolated build worker;
- operator draft-review surfaces need canonical transport and audit evidence.
- persisted workspaces now use bounded canonical JSON with sources, tests,
  fixtures, schemas, policy, and generated-file kinds; their path, per-file,
  total-size, and file-count limits are enforced before storage and execution.
  The sandbox receives canonical workspace bytes and Alloy resolves only its
  entry source through a request extension, never a guest filesystem. Bounded
  Rhai imports resolve only through a request-private static in-memory resolver
  assembled in dependency order: exact `src/*.rhai` paths, no host filesystem,
  bounded depth, and cycle rejection;
- release staging is host-composed on REST and GraphQL: both transports require
  `scripts:manage` and `modules:manage`, verify authenticated-tenant/request-
  tenant equality, pin the expected script revision, and delegate marketplace
  writes to `rustok-modules`. Typed owner not-found and idempotency-conflict
  outcomes remain distinct transport errors;
- untrusted marketplace/source/log/MCP content needs explicit prompt-injection
  and tool-policy isolation.

## FFA/FBA Boundary

- FFA status: `not_started`.
- FBA status: `boundary_ready`.
- Structural shape: `no_ui_boundary`.
- Capability runtime contract:
  `crates/alloy/contracts/alloy-runtime-contract.json` and
  `crates/alloy/contracts/evidence/alloy-runtime-static-matrix.json`.
- Static gate:
  `scripts/verify/verify-alloy-runtime-contract.mjs` /
  `npm run verify:alloy:runtime-contract`.

## Local Work Phases

### A1 - Shared Sandbox Cutover

- [x] Use the shared versioned Rhai input/output envelope for Alloy drafts;
  Alloy owns only its nested data payload and does not retain a raw or
  Alloy-specific versioned runtime binding.
- Build requests with draft ID, monotonic revision, tenant, actor, phase,
  trace/correlation, source digest, input, grants, and limits.
- Preserve entity proxies, parameters, validation helpers, and broker-backed
  services as Alloy-owned request-scoped extensions.
- Migrate manual, hook, scheduled, validation, and test execution atomically.
- Delete the parallel production execution path after callers move.

**Done when:** all production Alloy code execution is observable as
`SandboxSubject::AlloyDraft` and draft/published Rhai parity tests pass.

### A2 - Revisioned Authoring and Review

- Persist draft workspace, monotonic revision, source digest, parent lineage,
  author, review status, and policy revision.
- [x] Guard single-script persistence with a durable version predicate and
  `RevisionConflict`; every storage mutation advances the version.
- [x] Persist immutable single-source revision lineage with digest, author, and
  parent revision in the same transaction as the current draft mutation.
- [x] Expose immutable source-revision lookup and ordered history through the
  tenant-scoped owner storage contract.
- [x] Replace single-source draft persistence with a bounded canonical workspace
  stored and hashed as one immutable revision snapshot; resolve its entry source
  through the Alloy sandbox extension without guest filesystem access.
- [x] Resolve workspace Rhai imports only from exact in-memory `src/*.rhai`
  files, rejecting non-workspace paths, cycles, and depth overflow.
- [x] Require an explicit expected revision for REST and GraphQL draft updates.
- [x] Require the same revision for REST and GraphQL manual execution and run
  the loaded source snapshot rather than a second name lookup.
- Durable review decisions now bind an exact source digest, expected current
  revision, policy revision, reviewer identity, reason, and request fingerprint.
  The owner storage replays only an identical idempotency key/fingerprint pair
  and rejects invalid per-revision transitions. GraphQL and host HTTP transports
  require a verified `scripts.manage` actor and never accept an actor identity
  from client JSON.
- Require workspace revision/CAS and idempotency for test, build, and
  publish. Test commands now durably reserve a revision-pinned source digest,
  declared test path, actor, and request fingerprint before sandbox execution.
  The owner replays terminal evidence only for an identical command, returns an
  in-progress pending lease without duplicate work, and may reclaim only an
  expired lease against the same immutable source snapshot. Host HTTP and
  GraphQL derive a `scripts.manage` actor from authentication; build-command
  idempotency remains pending. Release staging now requires the current Alloy
  revision and its latest approved review, then uses an owner-owned
  `rustok-modules` Alloy-authored stage with an idempotency key bound to the
  immutable source and review evidence. The uploaded workspace checksum must
  equal the reviewed source digest. Owner artifact upload now accepts only the
  bounded workspace representation for `alloy_authored` requests. Authenticated
  HTTP and GraphQL release-stage adapters derive the actor from host auth,
  require the current revision and module authority, and delegate idempotent
  staging to the owner service; final marketplace promotion remains an owner
  governance operation.
- Published Rhai packages retain canonical workspace bytes and use the
  workspace OCI media type. Admission persists that exact media type and the
  artifact runtime reuses it from durable admission state, so multi-file
  imports cannot be reinterpreted as single-source Rhai at execution time.
- Workspace test execution now selects only a declared immutable `tests/*.rhai`
  entrypoint from the revision-pinned canonical workspace. It uses the same
  digest and in-memory `src/*.rhai` resolver as production source, receives no
  capability grants, rejects entity mutations, and requires a boolean result.
  Durable test-command CAS/idempotency evidence is recorded separately from
  sandbox work and terminal test evidence is linked to that exact revision.
- Link execution/test evidence to the exact revision.
- Define review, changes-requested, approved, rejected, archived, and superseded
  transitions with typed owner errors.
- Materialize a bounded revisioned workspace from DB/object storage and resolve
  Rhai imports without guest filesystem access.

**Done when:** stale revisions cannot execute/publish as current and every
review decision references immutable evidence.

### A3 - Rhai Release Publication and Forking

- Stage canonical Rhai descriptor and declared capabilities.
- Stage approved source through `rustok-modules`; do not write marketplace
  state. The owner records a distinct `alloy_authored` origin with the source
  digest/revision, Alloy tenant/script identity, and review evidence under
  durable idempotency. Origin-aware artifact upload and validation now accept
  only the bounded canonical workspace with a checksum equal to the reviewed
  source digest. Authenticated HTTP and GraphQL staging adapters delegate to
  the owner service; matching platform admission and final release promotion
  remain owner workflows. The package's workspace media type is an immutable admission
  fact and survives runtime resolution.
- Import an eligible marketplace Rhai release into a new workspace.
- Preserve parent release/source digest and require a newer semantic version.
- Publish a fork as a new immutable release without changing installed parents.

**Done when:** publish, install, import, edit, test, and republish scenarios
preserve reproducible lineage.

### A4 - AI-Assisted Rust/WASM Evolution

- Generate typed Rust against the approved WIT guest contract.
- Treat conversion as a reviewed rewrite, not an automatic Rhai AST compiler.
- Submit source only to the isolated build worker.
- Compare deterministic scenario/contract evidence between Rhai and WASM.
- Publish the WASM implementation as a new release with Rhai parent lineage.
- Never generate or load native dynamic libraries.

**Done when:** the WASM release passes build/trust/admission and scenario parity
while the Rhai parent remains installable and reproducible.

### A5 - Agent and Operator Tools

- Expose typed execute, validate, test, save, build, inspect, stage, review,
  publish, import, and fork tools.
- Route MCP calls through approved broker capabilities.
- Do not expose unrestricted shell, filesystem, database, network, signing, or
  registry credentials.
- Add operator review transports and audit history through capability-owned
  contracts.
- Treat source, marketplace metadata, README, build/test logs, MCP results, and
  module output as untrusted context; enforce tool policy, iteration/cost
  budgets, revisions, approvals, and audit outside the model.

**Done when:** tools call owner services, enforce actor/tenant/policy/revision,
and leave complete audit evidence.

## Verification

- `cargo xtask module validate alloy`.
- `cargo xtask module test alloy`.
- `npm run verify:alloy:runtime-contract`.
- Draft/artifact parity, revision conflict, review transition, lineage, fork,
  publication, capability denial, scheduler/hook, and tenant-isolation tests.
- Host-composed GraphQL/HTTP/MCP schema and execution integration tests.

## Completion Condition

This local plan is complete when Alloy is a revisioned authoring/evolution
capability over the shared sandbox, publishes and forks releases only through
`rustok-modules`, builds Rust only through the isolated worker, and retains no
parallel production sandbox or marketplace write path.

## Change Rules

1. Keep source/revision/review/tool semantics in Alloy and generic execution in
   `rustok-sandbox`.
2. Keep marketplace/release/install semantics in `rustok-modules`.
3. Update the runtime contract, evidence, local docs, central plan, and module
   registry with every boundary or behavior change.
