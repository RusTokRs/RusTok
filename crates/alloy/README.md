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

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
