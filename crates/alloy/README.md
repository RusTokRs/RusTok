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

## Runtime guarantees

Production `ScriptExecutor` uses `AlloyDraftRuntime` over the neutral
`SandboxRuntime`; `ScriptEngine` is retained only for compile-time validation.
The sandbox Rhai executor enforces configured Rhai operation, call-depth, string, array,
and map-size limits. Runs that exceed the wall-clock budget return
`ScriptError::Timeout`; Rhai operation pressure returns `ScriptError::OperationLimit`;
data-size pressure returns `ScriptError::ResourceLimit`. Use
`RhaiConfig::limits()` to expose the effective sandbox profile to operators. The machine-readable runtime contract now also source-locks the default/strict/relaxed sandbox profiles, timeout mapping, native Rhai limit-error mapping, scheduler tenant/phase semantics, running-flag recovery, and typed hook outcomes so these guarantees can be checked without compiling. Runtime-created orchestrators and the scheduler attach `SeaOrmExecutionLog` directly to `ScriptExecutor`, so manual GraphQL/HTTP runs, hooks, on-commit scripts, and cron jobs persist one canonical execution-history row with user and tenant context when available. Operators can inspect the same tenant-scoped history through GraphQL `scriptExecutionHistory(scriptId, pagination)` / `recentScriptExecutions(pagination)` or REST `GET /api/alloy/executions`. History reads use DB-level `page`/`per_page` inputs normalized to `page >= 1` and `per_page` 1..100 before DB-level offset/limit pagination, keep tenant filtering ahead of offset application, and expose exact scoped total metadata from the database. `PhaseCapabilities` exposes the helper families enabled for each execution phase so integrations do not infer bridge availability from registration side effects.

Production Alloy execution is composed by `apps/server` with the same
readiness-checked mTLS `GrpcRhaiExecutor` used for admitted artifacts. The
neutral `RhaiCapabilityBridge` exposes `http_*` helpers through the original
scoped `SandboxHost` and the `platform.http` grant; Alloy has no direct HTTP
client, production in-process executor, or fallback placement.

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

All operator HTTP routes are composed only through `controllers::axum_router`.
Every route requires an authenticated principal whose tenant matches the request
tenant and who holds `scripts.manage`; source, execution history, validation,
manual runs, lifecycle, review, and test operations all use that same gate.
The transport derives tenant and source-revision author identity from the
principal, never from client JSON. GraphQL enforces the same tenant equality
before any Alloy admin operation. The former generic in-memory Axum router is
not a production surface and has been removed.

The machine-readable static contract lives in
`crates/alloy/contracts/alloy-runtime-contract.json`; its evidence matrix lives in
`crates/alloy/contracts/evidence/alloy-runtime-static-matrix.json` and is checked
without compilation by `npm run verify:alloy:runtime-contract`.

## Marketplace lineage

Alloy stages reviewed Rhai source as a `rustok-modules` artifact descriptor. A
published module release is immutable: further Alloy work forks source lineage,
then publishes a new semantic version and digest. The installed release is never
changed in place.

Before descriptor staging or package construction, Alloy derives capability use
from every executable `src/*.rhai` file. The neutral `http_*` helpers require
`platform.http`; generic `capability_call` accepts only a literal valid
capability name. The declared descriptor set must match exactly, so missing or
unused grants, dynamic capability selection, and attempts to shadow a reserved
helper fail before owner admission.

The server imports a published Rhai release only through the module owner's
active publication projection and verified, digest-pinned CAS workspace. The
authenticated `POST /api/alloy/releases/import` route and GraphQL
`importPublishedRelease` mutation require both `scripts.manage` and
`modules.manage`, derive actor and tenant from host context, and create a new
tenant-scoped draft with immutable parent lineage. They never consume catalog
DTOs, mutable OCI tags, or caller-supplied tenant/actor fields. MCP does not
expose this import through generic stdio. The server's authenticated remote MCP
`alloy_import_published_release` tool uses the same tenant-bound owner
composition and permissions on its JSON and SSE tool transports.

An imported draft retains only its immutable parent-release reference. Preview
and revision-pinned workspace tests resolve the exact active parent
installation and its sandbox policy through `rustok-modules` for the draft
tenant. This rechecks admission, lifecycle, descriptor runtime ABI, and policy
revision on every run; missing or ineligible parent state fails closed rather
than using Alloy's default policy. Publication smoke remains zero-grant while
preserving the resolved limits.

When an imported draft is staged for publication, its immutable parent release
is carried through the smoke evidence into the owner-only governance command.
`rustok-modules` verifies the active exact predecessor and the new semantic
version, then stores direct lineage with the final artifact contract.
Publication never rewrites an installed parent release.

## Execution history surfaces

Operators can inspect the canonical execution log without bypassing Alloy
transport wiring:

- GraphQL: `scriptExecutionHistory(scriptId, pagination)` and
  `recentScriptExecutions(pagination)`, with legacy
  `scriptExecutions(scriptId, limit)` retained as a compact history list.
- HTTP routes: `GET /api/alloy/executions` and
  `GET /api/alloy/scripts/{id}/executions`.

All surfaces return execution id, script id/name, phase, outcome, duration,
error text, optional user/tenant context, and creation time ordered by newest
execution first.

## Docs

- [Module docs](./docs/README.md)
- [Platform docs index](../../docs/index.md)
