# `alloy` Documentation

`alloy` is a capability module of the platform authoring and automation layer.
It is part of `ModuleRegistry` and is installed/removed like other optional
modules, but remains a capability-only layer, not a tenant business domain.

## Purpose

- publish the canonical runtime entry point for script execution;
- keep storage, execution log, scheduler and bridge/helper layer inside the capability crate;
- provide a unified contract for host integration without spreading script runtime across `apps/server`.

## Scope

- source revisions, `ScriptOrchestrator`, `Scheduler` and Alloy execution lifecycle;
- Alloy context adaptation over the neutral `rustok-sandbox` Rhai kernel;
- storage/migrations for scripts and execution log;
- GraphQL/HTTP transport surfaces (`graphql::*`, `controllers::axum_router`), including tenant-scoped execution history;
- integration contracts `ScriptableEntity` and `HookExecutor` for host modules;
- staging and forking Rhai module artifacts through `rustok-modules` with immutable release lineage;
- no transformation of the script runtime into a separate tenant business domain.

## Responsibility Zone

Alloy owns capability-level script authoring, scheduling, execution, and their
transport adapters. Domain modules own the business policies and call Alloy only
through its public hook and integration contracts.

## Integration

- connected by `apps/server` via generated module wiring from `modules.toml` and `rustok-module.toml`;
- registered in `ModuleRegistry` as a regular optional module and publishes script permission surface;
- uses the neutral sandbox Rhai executor and must request only explicitly granted capabilities;
- can be called by domain modules through hook/integration contracts without blurring their own runtime boundaries.

## Verification

- `cargo xtask module validate alloy`
- `cargo xtask module test alloy`
- targeted runtime tests for script execution, scheduler and bridge semantics when changing capability surface

## Related Documentation

- [README crate](../README.md)
- [Implementation Plan](./implementation-plan.md)
- [Alloy Concept](../../../docs/alloy-concept.md)
- [Manifest Layer Contract](../../../docs/modules/manifest.md)

## Runtime Hardening Contract

Alloy applies resource controls in the embedded Rhai engine before compiling and
executing each script. The default profile is intentionally conservative for
host-triggered hooks:

- `max_operations = 50_000` enforced by Rhai and returns `ScriptError::OperationLimit`;
- `timeout = 100ms` measured around evaluated AST and returns `ScriptError::Timeout` if execution exceeds the configured budget;
- `max_call_depth = 16` enforced by Rhai function-call limits;
- `max_string_size = 64 KiB`, `max_array_size = 10_000` and `max_map_depth = 16` enforced as data-size limits and mapped to `ScriptError::ResourceLimit`.

Use `RhaiConfig::strict()` for latency-sensitive pre-commit hooks and
`RhaiConfig::relaxed()` only for operator-controlled maintenance scripts.
Public callers can obtain a snapshot of effective limits via `RhaiConfig::limits()`
without depending on Rhai internals. `PhaseCapabilities` fixes the helper families
allowed for each execution phase, so integrations do not infer bridge
availability from side effects of registration.

Alloy release staging requires the fixed
`tests/publication_smoke.rhai` entrypoint. `RevisionedReleaseStager` executes
that entrypoint against the exact reviewed source revision through the same
production `rustok-sandbox` runtime, with all capability grants removed. The
same request also compiles the declared production entrypoint and its reachable
imports before executing the smoke test. The test must return `true` and may
not produce entity mutations. Only redacted
execution identity, executor, runtime ABI, and effective policy digest cross
into the module-governance staging record together with an explicit zero-grant
count; source, input, and output remain
outside marketplace persistence. The release idempotency key is the stable
logical sandbox execution identity for retry-safe staging.

`HttpCapabilityBridge` is installed only on the request-scoped Rhai sandbox
executor. It has no network client: its `http_*` helpers create
`platform.http` calls for `SandboxHost`. The host validates admitted HTTP
host/method/path-prefix constraints before the broker applies its credential and
audit policy, so Alloy drafts and marketplace artifacts share the same boundary.

## Runbook for Scheduler and Hook Debugging

1. Check `execution_id`, `script.id`, `script.name` and `execution.phase` in
   tracing span `alloy.script.execute`.
2. For scheduler failures, call the scheduler status surface and verify the job
   is not stuck with `running = true`; the scheduler resets the flag after successful,
   aborted or failed execution and updates `next_run` from cron expression.
3. For hook failures, separate `Before` rejection and runtime failure:
   `ScriptError::Aborted` means intentional business rejection, while
   `OperationLimit`, `Timeout` and `ResourceLimit` indicate sandbox pressure.
4. Use the execution log as canonical operator history before replaying a script.
   `ScriptExecutor` writes an execution-history record for every runtime path
   connected through `AlloyRuntime`: GraphQL/HTTP manual runs, hooks,
   on-commit scripts and scheduler jobs. Replay must preserve the same phase and
   tenant context so that bridge/helper availability remains phase-aware.
   To read history, use the supported transport surfaces:
   GraphQL `scriptExecutionHistory(scriptId, pagination)` /
   `recentScriptExecutions(pagination)` and legacy compact list
   `scriptExecutions(scriptId, limit)`, HTTP
   `GET /api/alloy/executions`, `GET /api/alloy/scripts/{id}/executions` or
   generic Axum router `GET /executions`, `GET /scripts/{id}/executions`.
   All responses are based on `SeaOrmExecutionLog`, normalize `page >= 1` and `per_page` into the range 1..100 before DB-level offset/limit
   pagination, apply tenant filter before offset, return exact scoped total
   metadata from the database and are sorted newest-first.
   Responses return canonical fields: execution id, script id/name, phase,
   outcome, duration, error, user/tenant context and creation time.
5. For listing scripts, use only known `status` values; an unknown
   status must return a validation error and must not expand the fetch to
   all scripts. In-memory registry paths must preserve the same ordering as
   SeaORM (`name`, then `id`), and apply offset/limit after filtering.
   The machine-readable static contract is stored in
   `crates/alloy/contracts/alloy-runtime-contract.json`, the evidence matrix in
   `crates/alloy/contracts/evidence/alloy-runtime-static-matrix.json`; the fast
   no-compile gate is run via `npm run verify:alloy:runtime-contract`.
   The same contract now captures default/strict/relaxed sandbox profiles,
   timeout/native Rhai error mapping, scheduler `Scheduled` phase + tenant
   propagation, reset `running` flag after load/completion paths and typed hook
   outcomes (`Continue`, `Rejected`, `Error`) without running compilation.
6. Do not bypass GraphQL/HTTP/module wiring when debugging production scripts; these
   surfaces are part of the supported capability contract and keep audit and
   permission checks in a single path.
