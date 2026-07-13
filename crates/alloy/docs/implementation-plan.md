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

## Current State

Implemented:

- script model/storage, triggers, hooks, scheduling, execution history, GraphQL,
  HTTP, and runtime composition;
- stable runtime hardening contract and static verifier;
- generic Rhai kernel extraction into `rustok-sandbox`;
- Alloy adapter over the neutral Rhai engine;
- broker-backed HTTP capability bridge with no direct HTTP client;
- versioned `AlloyDraftRequestBuilder` that pins draft ID, source revision,
  source digest, sandbox phase, tenant, actor, input, grants, and limits;
- v1 data-only `AlloyDraftInput`/`AlloyDraftOutput` bindings for parameters,
  entity snapshots, returned values, and entity changes;
- immutable Rhai descriptor/source lineage staging, packaging, and forking
  helpers.

Remaining:

- production draft/manual/hook/scheduled execution still needs atomic cutover
  from the direct `ScriptEngine` path to `SandboxRuntime`; the request builder
  exists but entity/parameter scope extensions are required before callers move;
- entity/parameter semantics must become request-scoped Alloy extensions;
- draft revision/CAS, review, and publication orchestration need owner contracts;
- marketplace release import/fork needs a complete persisted workflow;
- AI-assisted Rust/WIT authoring must use the isolated build worker;
- operator draft-review surfaces need canonical transport and audit evidence.
- the current single-source `code: String` model needs a revisioned workspace
  for modules/imports, tests, fixtures, schemas, policy, and generated artifacts;
- untrusted marketplace/source/log/MCP content needs explicit prompt-injection
  and tool-policy isolation.

## FFA/FBA Boundary

- FFA status: `not_started`.
- FBA status: `in_progress`.
- Structural shape: `no_ui_boundary`.
- Capability runtime contract:
  `crates/alloy/contracts/alloy-runtime-contract.json` and
  `crates/alloy/contracts/evidence/alloy-runtime-static-matrix.json`.
- Static gate:
  `scripts/verify/verify-alloy-runtime-contract.mjs` /
  `npm run verify:alloy:runtime-contract`.

## Local Work Phases

### A1 - Shared Sandbox Cutover

- Define the versioned Alloy draft input/output binding.
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
- Require revision/CAS and idempotency for save, test, review, build, and publish.
- Link execution/test evidence to the exact revision.
- Define review, changes-requested, approved, rejected, archived, and superseded
  transitions with typed owner errors.
- Materialize a bounded revisioned workspace from DB/object storage and resolve
  Rhai imports without guest filesystem access.

**Done when:** stale revisions cannot execute/publish as current and every
review decision references immutable evidence.

### A3 - Rhai Release Publication and Forking

- Stage canonical Rhai descriptor and declared capabilities.
- Submit publication through `rustok-modules`; do not write marketplace state.
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
