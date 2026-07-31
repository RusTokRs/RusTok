# M6 shared reconciliation runtime composition

Status: `source_complete_server_guard_and_execution_pending`

## Scope

This slice publishes one cloneable Index-owned capability around the existing
`PostgresIndexReconciliationRunner`:

- `SharedIndexReconciliationRuntime::run`
- `SharedIndexReconciliationRuntime::request_cancel`
- `materialize_postgres_index_reconciliation_runtime`

The runtime is assembled only from the immutable shared schema/source registries and
the host database selected during composition. Callers do not receive the database
connection, source catalog, schema registry, scheduler, or task handle.

## Composition rules

Materialization is fail-closed:

1. a previously published reconciliation runtime returns `AlreadyMaterialized`;
2. no shared source registry returns `Ok(None)` and publishes no false capability;
3. a source registry without the shared schema registry returns
   `MissingSchemaRegistry`;
4. complete registries publish exactly one `SharedIndexReconciliationRuntime` in
   `ModuleRuntimeExtensions`.

Materialization performs no database I/O and starts no worker. Database access begins
only when an authorized host later invokes the bounded runner.

## Preserved runner contract

This slice does not change the existing reconciliation state machine. It preserves:

- bounded page and pass counts;
- durable `index_jobs` reconciliation ownership;
- source cursor and completed-pass progression;
- lease, attempt, heartbeat, yield, cancellation, and terminal fencing;
- mutation inbox deduplication and monotonic source-version guards;
- bounded machine-readable failure diagnostics.

## Explicit non-claims

The capability is not yet published by `apps/server` and has no GraphQL, HTTP, CLI,
admin, MCP, or scheduler transport. Request-bound tenant/actor authorization and
`modules:manage` admission remain required before any executable host exposes it.

The existing reconciliation runner replays authoritative current-state source
mutations. This is not yet complete drift repair for rows that are absent from a
source scan and have no retained owner tombstone. The following remain open:

- authoritative orphan detection and removal;
- source/index digest comparison;
- targeted repair planning;
- locale and partition checkpoint dimensions;
- automatic scheduling and graceful shutdown;
- retained PostgreSQL concurrency, restart, cancellation, and recovery evidence.

The canonical M6 reconciliation and drift-repair item therefore remains open.

## Compatibility

- no migration or table-shape change;
- no source, cursor, mutation, checkpoint, event identity, or schema change;
- no Product, Channel, or other source-domain dependency added to Index core;
- no root `lib.rs` re-export while the open dry-run PR owns that file;
- consumers can use the capability through
  `rustok_index::infrastructure::reconciliation` until a conflict-free root export is
  added;
- no changed-file overlap with open Index PRs #2636, #2639, #2642, #2644, #2648,
  #2649, or #2652.

## Maintainer validation

Execution is maintainer-owned. Suggested commands were not run by the implementation
agent:

```bash
cargo test -p rustok-index infrastructure::reconciliation::tests -- --nocapture
cargo check -p rustok-index --all-targets
node scripts/verify/verify-index-reconciliation-runtime.mjs
```
