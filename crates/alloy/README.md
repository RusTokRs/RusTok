# alloy

## Purpose

`alloy` owns the module-agnostic Alloy runtime for RusToK automation.

## Responsibilities

- Own script storage, execution contracts, scheduler, and migrations.
- Own the Rhai runtime, hook orchestration, execution log, and transport surfaces.
- Expose the canonical Alloy runtime API used by MCP, workflow integrations, and server wiring.

## Interactions

- Used by `apps/server` through generated module wiring from `rustok-module.toml`.
- Used by `rustok-mcp` as the canonical Alloy capability backend.
- Used by `rustok-core` for scripting-aware auth/domain integrations.
- Used by `rustok-workflow` through the `ScriptRunner` abstraction without making Alloy a tenant module.

## Entry points

- `create_default_engine`
- `ScriptEngine`
- `ScriptOrchestrator`
- `Scheduler`
- `ScriptRegistry`
- `SeaOrmStorage`
- `graphql::AlloyQuery`
- `graphql::AlloyMutation`
- `controllers::routes`
- `PhaseCapabilities`

## Runtime guarantees

`ScriptEngine` enforces the configured Rhai operation, call-depth, string, array,
and map-size limits. Runs that exceed the wall-clock budget return
`ScriptError::Timeout`; Rhai operation pressure returns `ScriptError::OperationLimit`;
data-size pressure returns `ScriptError::ResourceLimit`. Use
`EngineConfig::limits()` to expose the effective sandbox profile to operators. The machine-readable runtime contract now also source-locks the default/strict/relaxed sandbox profiles, timeout mapping, native Rhai limit-error mapping, scheduler tenant/phase semantics, running-flag recovery, and typed hook outcomes so these guarantees can be checked without compiling. Runtime-created orchestrators and the scheduler attach `SeaOrmExecutionLog` directly to `ScriptExecutor`, so manual GraphQL/HTTP runs, hooks, on-commit scripts, and cron jobs persist one canonical execution-history row with user and tenant context when available. Operators can inspect the same tenant-scoped history through GraphQL `scriptExecutionHistory(scriptId, pagination)` / `recentScriptExecutions(pagination)` or REST `GET /api/alloy/executions`. History reads use DB-level `page`/`per_page` inputs normalized to `page >= 1` and `per_page` 1..100 before DB-level offset/limit pagination, keep tenant filtering ahead of offset application, and expose exact scoped total metadata from the database. `PhaseCapabilities` exposes the helper families enabled for each execution phase so integrations do not infer bridge availability from registration side effects.

Script-list REST reads use the same `page >= 1` and `per_page` 1..100
normalization before storage pagination. If callers provide a `status` query
filter, it must match a known script status; unknown values return validation
errors instead of silently widening the operator query to all scripts. In-memory
storage uses the same filter-first, name-ordered pagination contract as SeaORM
so local runtime paths and tests do not depend on `HashMap` iteration order.
REST and GraphQL create/update flows now share the hardened validation contract: cron triggers are validated before persistence, changed script code is compiled before save, cache invalidation happens on rename/code update, duplicate REST names map to conflict responses, and compilation/cron failures map to validation errors.

The machine-readable static contract lives in
`crates/alloy/contracts/alloy-runtime-contract.json`; its evidence matrix lives in
`crates/alloy/contracts/evidence/alloy-runtime-static-matrix.json` and is checked
without compilation by `npm run verify:alloy:runtime-contract`.

## Execution history surfaces

Operators can inspect the canonical execution log without bypassing Alloy
transport wiring:

- GraphQL: `scriptExecutionHistory(scriptId, pagination)` and
  `recentScriptExecutions(pagination)`, with legacy
  `scriptExecutions(scriptId, limit)` retained as a compact history list.
- HTTP/Loco routes: `GET /api/alloy/executions` and
  `GET /api/alloy/scripts/{id}/executions`.
- Generic Axum router: `GET /executions` and
  `GET /scripts/{id}/executions`.

All surfaces return execution id, script id/name, phase, outcome, duration,
error text, optional user/tenant context, and creation time ordered by newest
execution first.

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
