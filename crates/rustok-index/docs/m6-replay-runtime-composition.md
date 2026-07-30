# M6 replay runtime host composition

Status: `source_complete_owner_execution_pending`

This slice publishes the bounded replay runner through the existing typed runtime-extension seam.
It does not add a scheduler, background task, command transport, automatic retry, or source adapter.

## Composition order

The executable server composes replay only after all selected modules have registered their generic
Index contracts:

1. `rustok-distribution` builds module-owned `IndexSchemaSourceCatalog` and
   `IndexSourceCatalog` contributions and materializes `SharedIndexSchemaRegistry`;
2. the server materializes `SharedIndexSourceRegistry` from the complete source catalog and exact
   schema-owner catalog;
3. `materialize_postgres_index_replay_runtime` requires both immutable registries, binds them to the
   host `DatabaseConnection`, and publishes `SharedIndexReplayRuntime`;
4. the server wraps that infrastructure capability in `IndexReplayOperatorRuntime` before any
   transport may run or cancel rebuild work;
5. all typed values transfer through `ModuleRuntimeExtensions` and `HostRuntimeContext`.

An absent or empty source catalog is valid and publishes neither source registry nor replay runtime.
A source registry without a shared schema registry fails closed. Duplicate source, replay, or
operator runtime materialization also fails startup.

Composition performs no SQL and starts no task. Runtime presence therefore does not claim a
supported production backend, persisted tenant schema readiness, source health, successful replay,
or retained PostgreSQL evidence.

## Operator authorization boundary

`IndexReplayOperatorRuntime` is the server-owned invocation boundary. Every call requires an
`IndexReplayOperatorContext` containing one non-nil tenant and actor. Before delegating to Index it:

- requires a request-bound effective RBAC permission snapshot for that exact tenant/actor;
- requires `modules:manage`;
- rejects a run request whose tenant differs from the authorized operator tenant;
- derives cancellation tenant scope only from the authorized context;
- exposes only bounded `run` and `request_cancel` operations.

Transport adapters must not retrieve or call `SharedIndexReplayRuntime` directly. Authentication,
command input decoding, audit/event publication, and stable HTTP/GraphQL/CLI error mapping remain
transport-owned later slices.

## Task and shutdown boundary

Neither materializer calls `tokio::spawn`, sleeps, loops, polls, or owns a stop handle. One explicit
operator invocation runs at most the request's bounded page budget and returns. Graceful shutdown
and task ownership remain open together with the future global scheduler; this source-complete host
composition does not falsely claim them.

## Still open

- authorized GraphQL, HTTP, CLI, or admin command surfaces;
- host scheduler, bounded retry/backoff, dead-letter state, and stop-handle ownership;
- in-page source timeout/interruption;
- dry-run and targeted/full/shadow modes;
- locale/partition checkpoint dimensions;
- Product and later production source adapters;
- retained PostgreSQL authorization, cancellation, restart, and multi-instance evidence.

## Owner validation

```bash
node scripts/verify/verify-index-replay-runtime-composition.mjs
node scripts/verify/verify-index-replay-multipage-runner.mjs
node scripts/verify/verify-index-replay-job-leases.mjs
node scripts/verify/verify-index-query-contract.mjs
cargo check -p rustok-index --all-targets
cargo check -p rustok-server --all-targets
cargo test -p rustok-index replay_runtime --lib -- --nocapture
cargo test -p rustok-server index_replay_runtime_composition -- --nocapture
```

These commands are maintainer-run for this slice and were not executed by the implementation agent.
