# M6 bounded replay dry-run

Status: `source_complete_host_guard_pending`

This slice adds an Index-owned, side-effect-free validation runtime over the exact immutable
`SharedIndexSourceRegistry` and `SharedIndexSchemaRegistry` used by production replay.

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

## Explicitly open

- server-owned request-bound `modules:manage` delegation for dry-run invocation;
- GraphQL, HTTP, CLI, or admin transport surfaces;
- persisted dry-run reports or comparison snapshots;
- cross-page duplicate event detection beyond one bounded page;
- mutation/checkpoint timing simulation;
- targeted-key, full, and shadow rebuild mode contracts;
- cooperative in-page operator cancellation and source timeout integration;
- retained PostgreSQL/source-adapter execution evidence.

Transport code must not treat `SharedIndexReplayDryRunRuntime` as an authorized public endpoint.
The raw capability is an engine seam awaiting the same server-owned guard used by executable
replay.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, source-adapter execution, and PostgreSQL
validation are maintainer-run. The implementation agent did not execute them.
