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
`EngineConfig::limits()` to expose the effective sandbox profile to operators. Runtime-created orchestrators and the scheduler attach `SeaOrmExecutionLog` directly to `ScriptExecutor`, so manual GraphQL/HTTP runs, hooks, on-commit scripts, and cron jobs persist one canonical execution-history row with user and tenant context when available. Operators can inspect the same tenant-scoped history through GraphQL `scriptExecutions(scriptId, limit)` or REST `GET /api/alloy/executions`. `PhaseCapabilities` exposes the helper families enabled for each execution phase so integrations do not infer bridge availability from registration side effects.

## Execution history surfaces

Operators can inspect the canonical execution log without bypassing Alloy
transport wiring:

- GraphQL: `scriptExecutionHistory(scriptId, pagination)` and
  `recentScriptExecutions(pagination)`.
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
