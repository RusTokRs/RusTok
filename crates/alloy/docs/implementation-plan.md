# `alloy` — Implementation Plan

Status: capability runtime locked; local documentation and module contract unified.

## Execution checkpoint

- Current phase: runtime_hardening_verified
- Last checkpoint: Alloy runtime exposes `build_alloy_runtime(DatabaseConnection)` and `SharedAlloyRuntime` for host-neutral bootstrap; `apps/server` registers it through `ServerRuntimeContext`, GraphQL reads schema-owned `SharedAlloyRuntime`, and manifest-declared `controllers::axum_router` builds `AlloyHttpRuntime` from `HostRuntimeContext`. REST handlers use `rustok_web::HttpError`; the crate no longer depends on Loco. Earlier executable Alloy compile/test evidence remains the baseline for runtime hardening.
- Next step: Promote remaining static route/schema/pagination/scheduler/hook source locks into executable router/schema/runtime integration checks where host test fixtures permit, then continue MCP/Admin Alloy draft-review surface work.
- Open blockers: None for the Alloy crate validation path.
- Hand-off notes for next agent: Alloy compile/test gates are no longer blocked. Keep `rustok-api/server` enabled for `alloy` while HTTP/GraphQL controllers use server-gated API context types. Rhai sandbox limits are applied natively in `ScriptEngine::new`; do not remove them or the runtime hardening contract will drift from executable behavior. Static contract paths remain `crates/alloy/contracts/alloy-runtime-contract.json`, `crates/alloy/contracts/evidence/alloy-runtime-static-matrix.json`, and `scripts/verify/verify-alloy-runtime-contract.mjs`.
- Last updated at (UTC): 2026-06-30T00:00:00Z

## Scope of work

- maintain `alloy` as a capability-oriented module of the platform script/runtime layer for scripts, scheduler and hook execution;
- synchronize runtime contract, `ModuleRegistry` wiring and local docs;
- evolve the script platform without turning `alloy` into a tenant-scoped business module.

## Current state

- storage, migrations and execution log already built into the capability crate;
- `ScriptEngine`, `ScriptOrchestrator`, `Scheduler` and bridge/helper layer already form the base runtime;
- GraphQL/HTTP transport surfaces live inside `alloy`, and the host connects them through generated module wiring;
- `AlloyModule` is registered as a regular optional module and publishes the script permission surface;
- local docs and root `README.md` are now part of the scoped module audit path.

## Stages

### 1. Contract stability

- [x] normalize local docs and remove broken encoding from module docs;
- [x] maintain `alloy` in the module-standard verification path;
- [x] maintain sync between host wiring, transport surfaces and capability metadata.

### 2. Runtime hardening

- [x] bring resource limits, timeout semantics and sandbox guarantees to a stable production contract;
- [x] maintain audit log and execution history as the canonical operator surface with DB-level pagination and exact scoped total metadata;
- [x] align in-memory registry pagination with DB ordering contract for deterministic non-DB runtime/test paths;
- [x] lock the runtime route/schema/pagination/sandbox/scheduler/hook/script CRUD validation contract in a machine-readable static gate without compilation;
- [x] extend integration helpers only through explicit phase-aware contracts.

### 3. Operability

- [x] develop a runbook for scheduler/runtime failures and hook debugging;
- [x] cover execution, scheduler, bridge invariants and canonical transport field mapping with targeted tests;
- [x] document new runtime guarantees simultaneously with capability surface changes.

## Verification

- `cargo xtask module validate alloy`
- `cargo xtask module test alloy`
- `npm run verify:alloy:runtime-contract`
- targeted runtime tests for script execution, scheduling, tenant isolation and bridge semantics

## Update rules

1. When changing the runtime contract, first update this file.
2. When changing the public/capability surface, synchronize `README.md` and `docs/README.md`.
3. When changing module metadata or host wiring, synchronize `rustok-module.toml`.


## Quality backlog

- [x] Update no-compile static coverage for key route/schema/pagination scenarios of the module.
- [ ] Raise static coverage to executable Rust integration tests after the compilation ban is lifted.
- [x] Verify completeness and relevance of `README.md` and local docs.
- [x] Lock/update verification gates for the current module state.
