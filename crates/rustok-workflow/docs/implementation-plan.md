# Implementation plan for `rustok-workflow`

Status: workflow module already has a working execution/runtime baseline; the key
task now is to maintain the boundary between orchestration layer, event contracts and
capability integrations without drift and broken documentation.

## Execution checkpoint

- Current phase: phase_b_ready + fba_provider_static_evidence + compile_free_runtime_smoke
- Last checkpoint: workflow admin native server-function transport and all workflow HTTP/webhook ingress now consume host-provided `rustok_api::HostRuntimeContext`; REST workflow/step/execution and webhook handlers use narrow `WorkflowHttpRuntime`, published through manifest-owned Axum routers without a server shim or `loco-rs` dependency. Workflow admin FFA Phase B is considered closed; FBA slice #1 added `WorkflowReadPort` / `workflow.read_projection.v1`, provider registry `crates/rustok-workflow/contracts/workflow-fba-registry.json`, static matrix `crates/rustok-workflow/contracts/evidence/workflow-contract-test-static-matrix.json`, compile-free runtime/fallback smoke packet `crates/rustok-workflow/contracts/evidence/workflow-read-projection-runtime-smoke.json` and fast gate `npm run verify:workflow:fba` without long compilation.
- Next step: Replace compile-free runtime smoke with live backend evidence: native server-function `list_workflows` over `HostRuntimeContext`, GraphQL selected-path execution and typed PortError mapping; do not promote FBA above `in_progress` until live evidence.
- Open blockers: None.
- Hand-off notes for next agent: After each increment, update this block; avoid long full-workspace compilations, use targeted checks/timeouts.
- Last updated at (UTC): 2026-06-20T00:00:00Z


## FFA/FBA status

- FFA status: `phase_b_ready`
- FBA status: `in_progress`
- Structural shape: `core_transport_ui`
- Evidence:
  - batch no-compile gate `scripts/verify/verify-owner-fba-runtime-order.mjs` checks `crates/rustok-workflow/contracts/evidence/workflow-provider-runtime-order-smoke.json`: shared read policy → tenant scope → owner `WorkflowService` invocation → typed error mapping and fallback/degraded registry parity; status remains `in_progress` until live native/GraphQL execution;
  - module plan synchronized with central FFA/FBA readiness board; UI surface already published and managed in migration/backlog rhythm;
  - FFA admin slice: status badge presentation, workflow table row mapping, template category styling, template-name normalization, module route toggle/legacy href policy, transport request context, transport error presentation and template create command/name policy now live in framework-agnostic `admin/src/core/` with unit tests;
  - transport slice: current GraphQL adapter lives in `admin/src/transport/graphql_adapter.rs`, native server-function adapter added in `admin/src/transport/native_server_adapter.rs`, and `admin/src/transport/mod.rs` became a build-profile-selected native/GraphQL facade; Leptos UI no longer depends on raw adapter modules directly;
  - Loco-free native admin transport evidence: `admin/src/transport/native_server_adapter.rs` consumes `HostRuntimeContext`, `admin/Cargo.toml` no longer declares `loco-rs`, and `scripts/verify/verify-workflow-admin-boundary.mjs` plus `scripts/verify/verify-api-surface-contract.mjs` guard the boundary;
  - UI adapter slice: Leptos-only render code moved to `admin/src/ui/leptos.rs`, and crate root left as composition/re-export layer for future addition of other host adapters;
  - fast boundary guardrail: `scripts/verify/verify-workflow-admin-boundary.mjs` and fixture tests lock absence of legacy `api.rs`/flat `transport.rs`, Leptos-free `core/`, raw-adapter-free UI and split native/GraphQL transport adapters;
  - Phase B closure decision: workflow admin FFA will not be expanded further without a new workflow-owned UI/transport surface; further promotion to `parity_verified` requires runtime parity evidence for native/server-function + GraphQL selected path and local+central docs update in the same change.
  - FBA provider slice: `crates/rustok-workflow/src/ports.rs` declares `WorkflowReadPort` / `workflow.read_projection.v1` for workflow admin read projection consumers with typed `PortContext`/`PortError`, tenant-scope preservation and read deadline semantics; `crates/rustok-workflow/contracts/workflow-fba-registry.json` plus `crates/rustok-workflow/contracts/evidence/workflow-contract-test-static-matrix.json` lock planned contract cases and fallback profiles under `npm run verify:workflow:fba` while runtime execution/fallback smoke remains pending before `boundary_ready`.
  - Compile-free runtime smoke slice: `crates/rustok-workflow/contracts/evidence/workflow-read-projection-runtime-smoke.json` pins native server-function entrypoints, GraphQL selected-path entrypoints, selected-path facade assertions and live-evidence closeout criteria without promoting FBA beyond `in_progress`.
- Last verified at (UTC): 2026-06-19T00:00:00Z
- Owner: `rustok-workflow` module team

## Scope of work

- maintain `rustok-workflow` as owner of the workflow execution domain;
- synchronize triggers, steps, transport/UI surfaces and local docs;
- do not let workflow turn into a separate event transport or generic scripting bucket.

## Current state

- workflow storage and execution journal are already defined inside the module;
- engine, trigger handlers, cron/manual/webhook/event triggers and base step types already form a working baseline;
- GraphQL, REST/webhook ingress and module-owned admin UI already live inside the module;
- webhook ingress is locked as a module-owned Axum transport surface, and the cron path is kept separate from `event_listener`;
- integration with `alloy` is already a capability-level step integration, not a registry-level hard dependency.

## Stages

### 1. Contract stability

- [x] lock workflow engine and execution journal as module-owned runtime;
- [x] lock transport adapters and admin UI inside the module;
- [x] normalize local docs and remove broken encoding from module docs;
- [~] maintain sync between workflow runtime contract, UI surfaces and module metadata; current FFA slice extracted presentation/view-model helpers, module route policy, transport request context, error presentation and template create command policy from Leptos render path into `admin/src/core/`, added build-profile-selected native/GraphQL transport facade and extracted `ui/leptos` adapter without changing the external GraphQL contract.

### 2. Execution hardening

- [~] deliver integration tests for real DB and execution history flows; added
  SQLite integration scenario for duplicate delivery after event handler recreation,
  and `(workflow_id, trigger_event_id)` locked as unique index; live PostgreSQL history
  matrix remains a separate verification gate;
- [ ] complete production-grade implementation of `alloy_script` and `notify` steps;
- [ ] evaluate DAG/branching expansion only under real product pressure, without breaking the current linear-step contract.

### 3. Operability

- [ ] develop system events `workflow.execution.*` and execution observability;
- [ ] document new runtime guarantees concurrently with trigger/step semantics changes;
- [ ] keep local docs and `README.md` synchronized with live code.

## Verification

- `cargo xtask module validate workflow`
- `cargo xtask module test workflow`
- `node scripts/verify/verify-workflow-admin-boundary.mjs`
- targeted tests for triggers, steps, execution journal, tenant isolation and admin/runtime contracts

## Update rules

1. When changing workflow runtime contract, first update this file.
2. When changing public/runtime surface, synchronize `README.md` and `docs/README.md`.
3. When changing module metadata, synchronize `rustok-module.toml`.
4. When changing event/alloy integration expectations, update related docs of foundation and capability modules.


## Quality backlog

- [ ] Update test coverage for key module scenarios.
- [ ] Verify completeness and accuracy of `README.md` and local docs.
- [ ] Lock/update verification gates for current module state.
