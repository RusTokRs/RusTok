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
- typed source-redacted script authoring through the authenticated remote MCP
  transport, composed by `apps/server` from the same owner-scoped runtime as
  HTTP and GraphQL; remote audit metadata is replaced with a fixed redaction
  marker, while generic stdio MCP exposes no script-authoring tools;
- integration contracts `ScriptableEntity` and `HookExecutor` for host modules;
- staging and forking Rhai module artifacts through `rustok-modules` with immutable release lineage;
- authenticated HTTP/GraphQL import of one exact published Rhai workspace through
  the module owner projection and verified CAS bytes; authenticated remote MCP
  import uses the same tenant-bound provider, while generic stdio MCP does not
  advertise an import operation;
- durable imported-release drafts with exact-replay receipts and immutable
  parent-release identity on every source revision;
- immutable source provenance on every revision: the authenticated author plus
  owner-generated origin/tool identity and, when a separately governed AI
  owner provides it, only a canonical prompt digest. Raw prompts, tool
  arguments, completions, and results are never accepted into Alloy storage;
  deleted scripts also hide their retained source, review, and test evidence
  from every owner read path and review/test idempotency replay. A test lease
  that races with deletion is settled for retention, then returns `NotFound`;
  the durable tombstone keeps that script ID non-reusable until retention policy
  purges it;
- owner-resolved installed-parent sandbox policy for imported-draft previews
  and workspace tests, with no default-policy fallback;
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

Alloy derives capability declarations directly from immutable executable
`src/*.rhai` source before descriptor staging and package construction. The
neutral `http_*` helpers use `platform.http`; generic `capability_call` must
use a literal valid name. The descriptor's capability set must be exact, so
missing or unused declarations, dynamic names, and attempts to redefine a
reserved capability helper are rejected before module-owner admission.

`RhaiCapabilityBridge` is installed in the standalone neutral Rhai worker. It
has no network client: its `http_*` helpers create
`platform.http` calls for `SandboxHost`. The host validates admitted HTTP
host/method/path-prefix constraints before the broker applies its credential and
audit policy, so Alloy drafts and marketplace artifacts share the same boundary.

Canonical `RhaiWorkspace`, in-memory imports, standard functions, and
serializable `RhaiScopeInput`/`RhaiScopeOutput` records are owned by
`rustok-sandbox`. Alloy maps its `params`, `entity`, and `entity_before` data to
that neutral contract before the request crosses mTLS. The worker therefore has
no Alloy, AI, database, storage, or product-infrastructure dependency.

Published Rhai import is split at an explicit owner boundary.
`AlloyReleaseImporter` accepts only an exact `ArtifactReleaseRef`; its source
provider must return the matching immutable release and canonical workspace
whose digest equals both release lineage and descriptor payload identity.
Alloy then creates the draft, its first source revision, and a durable
tenant/idempotency receipt atomically. The production provider is intentionally
not composed from manifest-only marketplace metadata: it remains unavailable
until registry publication exposes the canonical artifact/evidence projection
and digest-pinned CAS/OCI workspace materialization.

Imported-draft execution uses a second host-owned port. Alloy provides the
immutable parent release reference and tenant identity; the server resolves the
exact active installation and sandbox policy through `rustok-modules`. The
owner rechecks admission, lifecycle, policy revision, descriptor/runtime ABI,
and tenant scope on every preview or revision-pinned test. Failure to resolve
that policy blocks execution; Alloy never substitutes its default policy. The
fixed publication smoke remains zero-grant and inherits only the resolved
limits.

Fork publication preserves the same immutable parent reference: Alloy passes
it only to the module owner with the reviewed source stage, and the owner
verifies the active predecessor plus monotonic version before persisting direct
lineage beside the final artifact contract. Existing installations are never
modified by publication.

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
   `GET /api/alloy/executions` and
   `GET /api/alloy/scripts/{id}/executions`.
   All responses are based on `SeaOrmExecutionLog`, normalize `page >= 1` and `per_page` into the range 1..100 before DB-level offset/limit
   pagination, apply tenant filter before offset, return exact scoped total
   metadata from the database and are sorted newest-first.
   Responses return canonical fields: execution id, script id/name, phase,
   outcome, duration, error, user/tenant context, exact source
   revision/digest, sandbox policy digest, executor kind, runtime ABI, and
   creation time. Evidence fields are nullable only for rows created before
   the execution-evidence migration.
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

## Operator Transport Authorization

The only Alloy HTTP router is `controllers::axum_router`, composed by the host.
Every script, source, review, test, execution-history, lifecycle, validation,
and manual-run operation requires a `scripts.manage` principal whose tenant
equals the host-resolved request tenant. HTTP and GraphQL derive the author of
each new source revision from that authenticated principal; client payloads
cannot select a tenant, author, source-provenance, prompt, or tool-argument
identity. The owner records the concrete HTTP, GraphQL, remote-MCP, release-
import, or internal origin and an optional canonical prompt digest. The generic
in-memory Axum router was removed because it could not enforce these production
boundaries.
