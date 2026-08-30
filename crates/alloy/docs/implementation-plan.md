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
- production server composition through the readiness-checked mTLS
  `isolated_worker` adapter shared with admitted artifacts, with no in-process
  fallback;
- neutral broker-backed HTTP capability helpers with no direct HTTP client;
- versioned `AlloyDraftRequestBuilder` that pins draft ID, source revision,
  source digest, sandbox phase, tenant, actor, input, grants, and limits;
- data-only `AlloyDraftInput` mapped to neutral serialized
  `RhaiScopeInput` constants and records; `RhaiScopeOutput` carries bounded
  entity changes while return values remain in the strict shared
  `RhaiBindingInput`/`RhaiBindingOutput` external envelope;
- canonical `RhaiWorkspace`, in-memory import resolver, record proxy, standard
  library, and brokered HTTP helpers owned by `rustok-sandbox`, so the isolated
  worker remains independent from Alloy and product infrastructure;
- immutable Rhai descriptor/source lineage staging, packaging, and forking
  helpers.

Remaining:

- production draft/manual/hook/scheduled execution uses
  `AlloyDraftRuntime` over `SandboxRuntime`; `ScriptEngine` remains only for
  compile-time CRUD validation and internal unit tests, never production code
  execution;
- tenant-scoped `SeaOrmStorage` now applies the tenant predicate to every
  single-script read, save, delete, status, and error path, applies an atomic
  version predicate to deletion, and rejects a cross-tenant save as
  `NotFound`;
- `ScriptRegistry::save` now treats the stored script version as the expected
  revision and uses a durable revision predicate for SeaORM updates. Every
  storage mutation advances that revision, and stale saves fail as
  `RevisionConflict` instead of overwriting current state;
- `alloy_script_revisions` now records immutable workspace, digest, author, and
  parent-revision lineage in the same transaction as every admitted SeaORM
  mutation. A pre-ledger script receives a baseline snapshot before its first
  new revision commits. Owner storage exposes tenant-scoped lookup by
  `(script_id, revision)` and revision-ascending history without SQL bypass;
- REST and GraphQL update and lifecycle commands require the caller's expected
  revision; manual-run commands use the same requirement and execute the loaded
  snapshot without a second registry lookup. Deletion additionally requires a
  bounded reason and a non-nil idempotency key. HTTP, GraphQL, and remote MCP
  derive the actor from their authenticated owner boundary, then persist the
  actor, reason, request digest, idempotency key, and deletion time in one
  tenant-scoped tombstone transaction. Only an exact request digest replays
  after physical removal; reusing its idempotency key with a different command
  fails closed. Workspace-level command revisions, review, and publication
  orchestration still need owner contracts;
- all host-composed HTTP operator routes now require a matching authenticated
  tenant and `scripts.manage`, including source/history reads, validation,
  manual runs, lifecycle, review, and tests. HTTP and GraphQL derive every
  source-revision author from that principal. Every immutable revision also
  persists an owner-generated source provenance record: HTTP, GraphQL, remote
  MCP, release import, or internal owner origin; normalized tool name; and an
  optional canonical prompt digest. No client request can provide a provenance
  record, raw prompt, tool arguments, model completion, or tool result. Deleted
  scripts no longer expose their retained source/review/test evidence through
  owner reads or review/test idempotency replay. A test lease that races with
  deletion is still settled for retention, but its completion returns
  `NotFound`; a durable tombstone keeps its ID non-reusable until the retention
  policy purges it. `rustok-core::RetentionPolicy` now provides the canonical
  `owner_lifecycle` / `retain_until` / `legal_hold` vocabulary and deadline
  invariant shared with Translation Memory. Alloy deletion atomically assigns a
  fixed 30-day `retain_until` window. Its global owner scheduler reaps only
  expired `retain_until` tombstones, their source revisions, reviews, and test
  runs in one transaction, then retains a content-free receipt with counts and
  the deletion request digest. `legal_hold` is excluded from automatic
  collection. Owner HTTP, GraphQL, and remote MCP commands now read a
  source-free retention state and use its deletion digest plus a separate
  retention revision for exact, tenant-scoped idempotent hold transitions.
  Applying a hold clears the deadline; releasing it begins a new fixed 30-day
  `retain_until` window. The durable retention receipt retains actor, action,
  policy, revision, and request digests but never the reason. At expiry the
  collector irreversibly erases review reasons and test diagnostics rather than
  retaining a redacted copy. The generic in-memory Axum router has
  been removed so it cannot bypass tenant, permission, or provenance policy;
- published-release import now has durable exact-replay receipts and immutable
  parent lineage. Registry publication projects the canonical artifact/evidence
  and source-lineage contract through the module owner. The production source
  provider resolves only an exact active owner projection and verified,
  digest-pinned CAS workspace; host-composed HTTP and GraphQL imports require
  the authenticated tenant plus `scripts.manage` and `modules.manage`. The
  authenticated remote MCP import uses that same tenant-bound provider; generic
  stdio MCP does not advertise it because it cannot compose the durable host
  boundary;
- AI-assisted Rust/WIT authoring must use the isolated build worker;
- operator draft-review surfaces need canonical transport and audit evidence.
- persisted workspaces now use bounded canonical JSON with sources, tests,
  fixtures, schemas, policy, and generated-file kinds; their path, per-file,
  total-size, and file-count limits are enforced before storage and execution.
  The sandbox receives canonical workspace bytes and resolves only the declared
  entry source itself, never a guest filesystem. Bounded
  Rhai imports resolve only through a request-private static in-memory resolver
  assembled in dependency order: exact `src/*.rhai` paths, no host filesystem,
  bounded depth, and cycle rejection;
- release staging is host-composed on REST and GraphQL: both transports require
  `scripts:manage` and `modules:manage`, verify authenticated-tenant/request-
  tenant equality, pin both the expected script revision and the separate
  owner-issued publish-request aggregate revision, then delegate marketplace
  writes to `rustok-modules`. The owner returns the resulting request revision;
  typed not-found, stale-revision, and idempotency-conflict outcomes remain
  distinct transport errors;
- untrusted marketplace/source/log/MCP content needs explicit prompt-injection
  and tool-policy isolation.
- execution history persists and exposes the exact source revision/digest,
  sandbox policy digest, executor kind, and runtime ABI without persisting
  source, input, output, or capability results as evidence metadata.

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
- [x] Persist immutable single-source revision lineage with digest, author,
  owner-generated content-free provenance, and parent revision in the same
  transaction as the current draft mutation.
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
- [x] Require the expected revision for REST activate/pause commands and all
  GraphQL status mutations (activate, pause, disable, archive, and reset
  errors); stale lifecycle writes fail with a revision conflict.
- [x] Require an attributable idempotent command for REST, GraphQL, and remote
  MCP deletion: expected revision, bounded reason, and idempotency key arrive
  from the command, while the actor is derived from authenticated owner state.
  Owner storage applies the atomic version predicate and writes a durable
  tenant-scoped audit tombstone in the same transaction; only the exact
  request-digest retry replays after removal.
- [x] Remove generic MCP script CRUD, validation, and execution. It cannot
  compose an owner-scoped Alloy runtime, so it must not simulate tenant or
  actor binding. Canonical authoring stays on host-composed HTTP and GraphQL.
- [x] Add remote MCP script authoring only through the same owner-scoped Alloy
  runtime used by host HTTP and GraphQL. The server resolves the durable MCP
  binding, requires `scripts.manage`, matches the authenticated identity's
  tenant to the bound tenant, and derives the actor instead of accepting either
  value from tool JSON. `AlloyAuthoringService` owns typed authoring commands
  over that scoped runtime and returns source-redacted evidence. The remote
  audit path replaces caller metadata with a fixed redaction marker, and its
  SeaORM integration test proves cross-tenant mutation fails closed. Generic
  stdio and in-process script tools remain absent.
- Durable review decisions now bind an exact source digest, expected current
  revision, policy revision, reviewer identity, reason, and request fingerprint.
  The owner storage replays only an identical idempotency key/fingerprint pair
  while the owning draft exists, and rejects invalid per-revision transitions.
  GraphQL and host HTTP transports require a verified `scripts.manage` actor
  and never accept an actor identity from client JSON.
- [x] Use only host-composed tenant-bound HTTP routes and matching GraphQL
  authorization for Alloy authoring. Source/history reads, validation, manual
  execution, lifecycle, reviews, and tests require `scripts.manage`; create,
  update, and every lifecycle revision derive `author_id` from the authenticated
  principal, and manual execution evidence records that actor. The former
  generic in-memory Axum router was deleted instead of retained as a parallel
  unauthenticated surface.
- Require workspace revision/CAS and idempotency for test, build, and
  publish. Test commands now durably reserve a revision-pinned source digest,
  declared test path, actor, and request fingerprint before sandbox execution.
  The owner replays terminal evidence only for an identical command while the
  owning draft exists, returns an in-progress pending lease without duplicate
  work, and may reclaim only an expired lease against the same immutable source
  snapshot. After deletion, a completion settles its held lease for retention
  and returns `NotFound`. Host HTTP and
  GraphQL derive a `scripts.manage` actor from authentication; build-command
  idempotency remains pending. Release staging now requires the current Alloy
  revision, the current owner-issued publish-request revision, and its latest
  approved review, then uses an owner-owned
  `rustok-modules` Alloy-authored stage with an idempotency key bound to the
  immutable source and review evidence. The uploaded workspace checksum must
  equal the reviewed source digest. Owner artifact upload now accepts only the
  bounded workspace representation for `alloy_authored` requests. Authenticated
  HTTP and GraphQL release-stage adapters derive the actor from host auth,
  require both revisions and module authority, and delegate idempotent staging
  to the owner service; final marketplace promotion remains an owner governance
  operation.
- Published Rhai packages retain canonical workspace bytes and use the
  workspace OCI media type. Admission persists that exact media type and the
  artifact runtime reuses it from durable admission state, so multi-file
  imports cannot be reinterpreted as single-source Rhai at execution time.
- Workspace test execution now selects only a declared immutable `tests/*.rhai`
  entrypoint from the revision-pinned canonical workspace. It uses the same
  digest and in-memory `src/*.rhai` resolver as production source, rejects
  entity mutations, and requires a boolean result. An imported draft resolves
  the exact installed parent artifact policy through the host on every run;
  missing, disabled, stale, or mismatched parent state fails closed instead of
  using the draft default. The sandbox test phase remains explicit for
  broker-side phase constraints. Durable test-command CAS/idempotency evidence
  is recorded separately from sandbox work and terminal test evidence is linked
  to that exact revision.
- [x] Link execution/test evidence to the exact revision. Production execution
  rows carry source revision/digest plus sandbox policy digest, executor kind,
  and runtime ABI; durable test rows already bind their immutable revision and
  source digest.
- Define review, changes-requested, approved, rejected, archived, and superseded
  transitions with typed owner errors.
- Materialize a bounded revisioned workspace from DB/object storage and resolve
  Rhai imports without guest filesystem access.

**Done when:** stale revisions cannot execute/publish as current and every
review decision references immutable evidence.

### A3 - Rhai Release Publication and Forking

- [x] Stage a canonical Rhai descriptor whose capabilities exactly match the
  immutable executable source. Alloy recognizes only the neutral
  `capability_call` and `http_*` helper surface, requires literal capability
  names for generic calls, and rejects missing/unused declarations, dynamic
  names, and helper shadowing before staging or packaging.
- Stage approved source through `rustok-modules`; do not write marketplace
  state. The owner records a distinct `alloy_authored` origin with the source
  digest/revision, Alloy tenant/script identity, and review evidence under
  durable idempotency. Origin-aware artifact upload and validation now accept
  only the bounded canonical workspace with a checksum equal to the reviewed
  source digest. Authenticated HTTP and GraphQL staging adapters delegate to
  the owner service; matching platform admission and final release promotion
  remain owner workflows. The package's workspace media type is an immutable admission
  fact and survives runtime resolution.
- [x] Persist an eligible published Rhai workspace as a new tenant-scoped draft
  with its exact parent release on both the current row and immutable source
  revision. A durable `(tenant_id, idempotency_key)` receipt is created in the
  same transaction; exact replay returns the original draft, while conflicting
  replay and duplicate tenant-scoped names fail closed.
- [x] Compose the production owner source provider and authenticated GraphQL /
  HTTP import adapters. The provider consumes only the exact active published
  release projection, requires the admitted Rhai workspace media type and
  source digest, and materializes canonical bytes from verified CAS. The
  host-composed `POST /api/alloy/releases/import` route and
  `importPublishedRelease` mutation derive the tenant and actor from
  authentication, require `scripts.manage` and `modules.manage`, and pass a
  tenant-scoped registry plus idempotency key to the importer. Mutable tags and
  manifest-only catalog metadata are not source authorities.
- [x] Compose the tenant-bound remote MCP import adapter. The authenticated
  `alloy_import_published_release` tool derives tenant and actor identity from
  the durable MCP runtime binding, requires `scripts.manage` and
  `modules.manage`, then imports through the same owner provider and
  tenant-scoped Alloy registry as HTTP and GraphQL. Its result redacts source
  bytes while preserving exact parent-release lineage. Generic stdio MCP does
  not advertise an import operation without this host composition.
- [x] Preserve parent release/source digest and require a newer semantic
  version for a fork. Storage prevents imported parent replacement/removal and
  `ArtifactRelease::fork` rejects a non-incremented version or changed slug.
- [x] Resolve tests and preview executions of an imported draft against the
  exact active installed parent artifact policy. Alloy holds only the immutable
  parent release reference; the server rechecks tenant scope, admission,
  lifecycle, descriptor runtime ABI, and policy revision through
  `rustok-modules`. No eligible parent policy means no execution. Publication
  smoke remains zero-grant and inherits only the resolved policy limits.
- [x] Persist the domain-separated digest of the fixed zero-input, zero-grant
  Rhai publication-smoke scenario in the immutable owner staging receipt and
  require it during security reconciliation. This is the first durable parity
  case for a future Rust/WASM rewrite, not evidence that a candidate
  implementation is equivalent.
- [x] Publish a fork as a new immutable release without changing installed
  parents. The exact imported `ArtifactReleaseRef` reaches owner staging and
  the final artifact contract; the owner validates its active predecessor and
  monotonic version, while existing installations remain unchanged.

**Done when:** publish, install, import, edit, test, and republish scenarios
preserve reproducible lineage.

### A4 - AI-Assisted Rust/WASM Evolution

- Generate typed Rust against the approved WIT guest contract.
- Treat conversion as a reviewed rewrite, not an automatic Rhai AST compiler.
- Submit source only through the owner build control as a host-prepared,
  non-serializable `PreparedModuleSourceArchive`, created exclusively by the
  shared `ModuleAuthoringSourceArchiveBuilder`; Alloy and its transports must
  never carry a filesystem path in an evolution command or duplicate archive
  writing and limit selection. Reviewed candidate files must first pass through
  the shared `SourceTreeMaterializer`; Alloy cannot recursively write caller
  paths or retain filesystem metadata. The owner rehashes and strictly scans that
  archive before its source-CAS publish and remote-worker enqueue.
- Persist every submitted Rust Component candidate as an immutable Alloy
  record before it can reach source preparation. The candidate is bound to its
  tenant, current approved Rhai draft revision and source digest, exact
  published Rhai parent release, canonical Rust source digest, canonical
  scenario digest, authenticated actor, and idempotency receipt. Candidate
  workspace content is data-only and remains source-redacted from operator
  responses; a candidate cannot be created from a filesystem path or from an
  unreviewed, stale, or release-unpinned Rhai revision. Candidate source and
  its receipt share the owning draft's retention lifecycle and are physically
  erased by the same expiry collector, never left as hidden orphaned source.
  Admission also derives the candidate manifest identity and rejects a slug
  mismatch or a version that is not strictly newer than its Rhai parent before
  either candidate or review state is written.
- Record candidate review decisions in a separate immutable state machine.
  Each decision binds the candidate ID plus its source and scenario digests,
  policy revision, authenticated reviewer, idempotency receipt, and transition
  history. Candidate approval is necessary, but not yet sufficient, to enqueue
  a source-preparation or isolated-worker build: that next owner operation must
  re-read the exact approved candidate and current parent-release eligibility.
- [x] Dispatch an approved candidate only through `AlloyEvolutionBuildService`.
  The host injects an existing non-symlink work root; the service creates a
  correlation-bound private operation directory, materializes the reviewed
  data-only source with `ModuleAuthoringSourceArchiveBuilder`, and submits its
  non-serializable archive to `ModuleAuthoringBuildControl`. It records a
  durable receipt binding candidate/source/scenario digests to the exact
  source-CAS digest, `cas://` reference, build request, authenticated command
  context, and idempotency digest. Exact replay returns that receipt before
  source preparation; a mismatched replay, unapproved candidate, deleted
  parent, or invalid owner submission fails closed. The ephemeral operation
  directory is removed after submission. This is an enqueue boundary only; it
  is not evidence that an isolated worker executed the candidate or that Rhai
  and WASM behavior is equivalent. The immutable build request carries the
  same source-local scenario path and reviewed canonical digest, so an isolated
  worker can reject a substituted scenario before it runs the candidate.
- Compare deterministic scenario/contract evidence between Rhai and WASM. The
  fixed publication smoke has a canonical persisted scenario digest. The
  neutral sandbox now defines the candidate comparison projection as a
  domain-separated scenario digest plus a redacted `success` or
  `expected_error` result; Alloy's current smoke remains Rhai-only until its
  candidate adapter executes that shared contract. Candidate generation and
  executable Rhai/WASM comparison remain required.
- Publish the WASM implementation as a new release with Rhai parent lineage.
  The owner-owned platform-build request, durable staging receipt, and final
  marketplace artifact contract now preserve one exact active Rhai predecessor;
  a replay cannot substitute it. The Alloy evolution workflow and scenario
  review remain required before a generated WASM release can be staged.
- [x] Never generate or load native dynamic libraries. The worker accepts only a
  validated WASM Component payload, launches the fixed OCI job with the
  immutable `wasm32-wasip2` target, and rejects a receipt that does not bind
  that exact target; no Alloy path receives a native loader handle. The shared
  isolation verifier scans every Rust Component production boundary for native
  dynamic-loader APIs or dependencies.

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
