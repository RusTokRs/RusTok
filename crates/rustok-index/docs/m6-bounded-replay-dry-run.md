# M6 bounded replay dry-run

Status: `source_complete_transport_pending`

This slice adds an Index-owned, side-effect-free validation runtime over the exact immutable
`SharedIndexSourceRegistry` and `SharedIndexSchemaRegistry` used by production replay, and now
binds that capability behind the server-owned request-bound replay operator guard.

## Contract

`IndexReplayDryRunRequest` carries:

- one non-nil tenant;
- one exact registered schema;
- one optional source-owned continuation cursor;
- one source page limit already bounded by `IndexSourceScanRequest`;
- one invocation budget from 1 through 1024 pages.

`SharedIndexReplayDryRunRuntime::run` resolves source ownership only from the immutable registry,
then scans at most the requested page budget. For every returned page it verifies:

- the existing source-page tenant/schema, size, key uniqueness, and cursor-progress contract;
- non-nil event UUIDs;
- page-local event UUID uniqueness before accepting the page;
- complete `SchemaRegistry::validate_mutation` validity for every upsert or delete.

The outcome reports the stable source name, completion or bounded yield, a resume cursor only when
unfinished, page/mutation/upsert/delete counts, and the maximum observed source version.

## Source-call boundary

Dry-run invokes `SharedIndexSourceRegistry::scan`; it does not keep or call a source adapter outside
the immutable registry. Product, ProductVariant, SalesChannel, and future production bridges that
register through the canonical `register_index_source` helper therefore inherit the existing
30-second source-call timeout and its bounded retryable `index_source_scan_timeout` failure.

Direct low-level `IndexSourceCatalog::register` usage remains available to isolated fixtures and
contract tests. It intentionally does not imply the production timeout policy.

## No-write boundary

The dry-run runtime owns no database connection and receives no mutation sink, job store,
checkpoint store, reconciliation progress store, scheduler, or task handle. It cannot write:

- `index_entities` or `index_links`;
- `index_inbox`;
- `index_jobs`;
- `index_checkpoints`;
- reconciliation progress or terminal state.

Materialization performs no source call and no database I/O. The capability is published only
when both immutable registries exist; an absent source registry publishes nothing, and a source
registry without its shared schema registry fails closed.

A yielded run is resumed only by explicitly passing its returned opaque cursor into another
bounded request. No durable cursor is implied.

## Server-owned host guard

`IndexReplayOperatorRuntime` now retains the already-materialized
`SharedIndexReplayDryRunRuntime` beside the durable `SharedIndexReplayRuntime` and exposes
`IndexReplayOperatorRuntime::run_shadow`.

`run_shadow` reuses the same request-bound `modules:manage` authorization boundary as durable full
replay. The exact request tenant must match the operator context before the side-effect-free source
scan can start. The operator publishes no database connection, source registry, scheduler, worker
handle, job identifier, lease control, cancellation state, or retry/requeue control to the caller.

Server composition fails closed if durable replay materializes but the expected dry-run capability
is missing. Retained server source evidence covers `modules:read` rejection and an authorized
`modules:manage` empty-page completion through the guarded Shadow method. That completion creates
no durable replay job or checkpoint and does not alter the Full runner.

## Explicitly open

- GraphQL, HTTP, CLI, or admin transport surfaces for Shadow invocation;
- authorization-first parsing/serialization of a bounded Shadow transport request and resume cursor;
- optional canonical locale scope for the Shadow/dry-run transport path;
- persisted dry-run reports or comparison snapshots;
- cross-page duplicate event detection beyond one bounded page;
- mutation/checkpoint timing simulation and interruption;
- targeted mutation execution over `IndexSource::load`;
- cooperative cancellation while a source or validation future is already pending;
- configurable per-source or per-request timeout policy;
- retained production-source and PostgreSQL execution evidence.

Transport code must call the guarded operator boundary rather than treat
`SharedIndexReplayDryRunRuntime` as an authorized public endpoint. The existing GraphQL
`runIndexReplay` command remains durable Full replay and is unchanged by the host-guard slice.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, source-adapter execution, and PostgreSQL
validation are maintainer-run. The implementation agent did not execute them.

Suggested commands:

```bash
cargo test -p rustok-index --test replay_dry_run_contract -- --nocapture
cargo test -p rustok-index replay_dry_run -- --nocapture
cargo check -p rustok-index --all-targets
node scripts/verify/verify-index-replay-runtime-composition.mjs
node scripts/verify/verify-index-replay-shadow-host-dispatch.mjs
```
