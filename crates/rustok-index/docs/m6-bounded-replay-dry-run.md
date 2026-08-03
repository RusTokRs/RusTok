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

## Explicitly open

- server-owned request-bound `modules:manage` delegation for dry-run invocation;
- GraphQL, HTTP, CLI, or admin transport surfaces;
- persisted dry-run reports or comparison snapshots;
- cross-page duplicate event detection beyond one bounded page;
- mutation/checkpoint timing simulation and interruption;
- targeted-key, full, and shadow rebuild mode contracts;
- cooperative cancellation while a source or validation future is already pending;
- configurable per-source or per-request timeout policy;
- retained production-source and PostgreSQL execution evidence.

Transport code must not treat `SharedIndexReplayDryRunRuntime` as an authorized public endpoint.
The raw capability is an engine seam awaiting the same server-owned guard used by executable
replay.

The canonical combined roadmap item for complete in-page interruption, dry-run, and rebuild modes
remains open because this slice provides only bounded no-write source validation. It does not add
a guarded invocation surface or any targeted/full/shadow rebuild execution.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript verifiers, source-adapter execution, and PostgreSQL
validation are maintainer-run. The implementation agent did not execute them.

Suggested commands:

```bash
cargo test -p rustok-index --test replay_dry_run_contract -- --nocapture
cargo test -p rustok-index replay_dry_run -- --nocapture
cargo check -p rustok-index --all-targets
node scripts/verify/verify-index-replay-runtime-composition.mjs
```
