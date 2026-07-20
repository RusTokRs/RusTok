# alloy

## Purpose

`alloy` owns the Alloy authoring and automation capability for RusToK.

## Responsibilities

- Own script/source storage, authoring contracts, scheduler, migrations and source lineage.
- Own Alloy-specific hook orchestration, execution log projection and transport surfaces.
- Consume the neutral Rhai execution kernel from `rustok-sandbox`; do not own a parallel production sandbox.
- Expose the canonical Alloy runtime API used by MCP, workflow integrations, and server wiring.
- Expose host-neutral runtime construction so server bootstrap can register Alloy without depending on host-wide context.
- Keep GraphQL runtime access on `SharedAlloyRuntime` schema data instead of host framework context.
- Keep REST script/execution/release handlers on narrow `AlloyHttpRuntime` state; the manifest-declared Axum router builds it from host-provided `SharedAlloyRuntime` and `AlloyReleaseGovernanceHandle`.

## Interactions

- Used by `apps/server` through generated module wiring from `rustok-module.toml`.
- Used by `rustok-mcp` as the canonical Alloy capability backend.
- Used by `rustok-core` for scripting-aware auth/domain integrations.
- Used by `rustok-workflow` through the `ScriptRunner` abstraction without making Alloy a tenant module.

## Entry points

- `create_default_engine`
- `build_alloy_runtime`
- `SharedAlloyRuntime`
- `AlloyHttpRuntime`
- `ScriptEngine`
- `ScriptOrchestrator`
- `Scheduler`
- `ScriptRegistry`
- `SeaOrmStorage`
- `graphql::AlloyQuery`
- `graphql::AlloyMutation`
- `controllers::axum_router`
- `PhaseCapabilities`
- `stage_rhai_module_release`
- `fork_rhai_module_release`
- `HttpCapabilityBridge`
- `create_sandbox_rhai_executor`

## Runtime guarantees

Production `ScriptExecutor` uses `AlloyDraftRuntime` over the neutral
`SandboxRuntime`; `ScriptEngine` is retained only for compile-time validation.
The sandbox Rhai executor enforces configured Rhai operation, call-depth, string, array,
and map-size limits. Runs that exceed the wall-clock budget return
`ScriptError::Timeout`; Rhai operation pressure returns `ScriptError::OperationLimit`;
data-size pressure returns `ScriptError::ResourceLimit`. Use
`RhaiConfig::limits()` to expose the effective sandbox profile to operators. The machine-readable runtime contract now also source-locks the default/strict/relaxed sandbox profiles, timeout mapping, native Rhai limit-error mapping, scheduler tenant/phase semantics, running-flag recovery, and typed hook outcomes so these guarantees can be checked without compiling. Runtime-created orchestrators and the scheduler attach `SeaOrmExecutionLog` directly to `ScriptExecutor`, so manual GraphQL/HTTP runs, hooks, on-commit scripts, and cron jobs persist one canonical execution-history row with user and tenant context when available. Operators can inspect the same tenant-scoped history through GraphQL `scriptExecutionHistory(scriptId, pagination)` / `recentScriptExecutions(pagination)` or REST `GET /api/alloy/executions`. History reads use DB-level `page`/`per_page` inputs normalized to `page >= 1` and `per_page` 1..100 before DB-level offset/limit pagination, keep tenant filtering ahead of offset application, and expose exact scoped total metadata from the database. `PhaseCapabilities` exposes the helper families enabled for each execution phase so integrations do not infer bridge availability from registration side effects.

External `http_*` functions are not registered on reusable Alloy engines:
`create_sandbox_rhai_executor()` adds them per sandbox request through
`HttpCapabilityBridge`, which delegates every call to the shared `SandboxHost`
broker under the `platform.http` grant.

Script-list REST reads use the same `page >= 1` and `per_page` 1..100
normalization before storage pagination. If callers provide a `status` query
filter, it must match a known script status; unknown values return validation
errors instead of silently widening the operator query to all scripts. In-memory
storage uses the same filter-first, name-ordered pagination contract as SeaORM
so local runtime paths and tests do not depend on `HashMap` iteration order.
REST and GraphQL create/update flows now share the hardened validation contract: cron triggers are validated before persistence, changed script code is compiled before save, cache invalidation happens on rename/code update, duplicate REST names map to conflict responses, and compilation/cron failures map to validation errors.
Lifecycle status and deletion mutations also require the caller's
`expected_version`; owner storage applies the same revision CAS before
mutating or removing a script.

The machine-readable static contract lives in
`crates/alloy/contracts/alloy-runtime-contract.json`; its evidence matrix lives in
`crates/alloy/contracts/evidence/alloy-runtime-static-matrix.json` and is checked
without compilation by `npm run verify:alloy:runtime-contract`.

## Marketplace lineage

Alloy stages reviewed Rhai source as a `rustok-modules` artifact descriptor. A
published module release is immutable: further Alloy work forks source lineage,
then publishes a new semantic version and digest. The installed release is never
changed in place.

## Execution history surfaces

Operators can inspect the canonical execution log without bypassing Alloy
transport wiring:

- GraphQL: `scriptExecutionHistory(scriptId, pagination)` and
  `recentScriptExecutions(pagination)`, with legacy
  `scriptExecutions(scriptId, limit)` retained as a compact history list.
- HTTP routes: `GET /api/alloy/executions` and
  `GET /api/alloy/scripts/{id}/executions`.
- Generic Axum router: `GET /executions` and
  `GET /scripts/{id}/executions`.

All surfaces return execution id, script id/name, phase, outcome, duration,
error text, optional user/tenant context, and creation time ordered by newest
execution first.

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
