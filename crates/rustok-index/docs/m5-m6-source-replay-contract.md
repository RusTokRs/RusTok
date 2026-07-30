# M5/M6 bounded source replay contract

This slice establishes the source-owned read boundary used by future incremental ingestion,
rebuild, reconciliation, and repair workers. It does not create or claim a durable worker,
checkpoint writer, scheduler, replay command, or production consumer cutover.

## Ownership

- Source modules own normalized state and implement `IndexSource`.
- Index owns source registration, schema/source ownership validation, request bounds,
  continuation safety, returned-mutation scope validation, persistence, checkpoints, jobs,
  leases, retry policy, and operator controls.
- Index core never imports source-domain crates and never reads source tables directly.

## Registration

`IndexSourceCatalog` is seeded beside `IndexSchemaSourceCatalog` during module runtime
registration. A source module registers:

- one bounded lowercase owner module slug;
- one unique bounded source name;
- one or more exact `SchemaRef` values;
- one `IndexSource` implementation.

One exact schema and one complete `(module, entity)` identity cannot move between replay
sources across versions. Materialization checks every replay schema against the source-owned
schema catalog and rejects missing schemas or owner drift. An absent or empty source catalog
materializes to `None`; merely publishing a schema does not falsely claim rebuild readiness.

## Cursor-based scan

`IndexSource::scan` receives `IndexSourceScanRequest` with one non-nil tenant, one exact schema,
an optional opaque JSON cursor, and a limit from 1 through 1000. Cursor JSON is non-null and at
most 8 KiB when encoded.

`IndexSourcePage` is accepted only when:

- the mutation count does not exceed the requested limit;
- every mutation stays in the requested tenant and exact schema;
- entity keys are unique within the page;
- an empty page does not advertise continuation;
- a continuation cursor differs from the request cursor.

`None` means the scan is complete. The contract therefore supports bounded streaming without
collecting every source ID before work begins.

## Targeted load

`IndexSource::load` receives one to 256 unique `EntityKey` values from exactly one non-nil
tenant and exact schema. A returned mutation must correspond to one requested key, and each key
may appear at most once. Missing keys are permitted so deletion or authorization-safe absence
can be represented by the owner adapter's chosen mutation semantics without widening the
requested scope.

## Failures

Source adapters expose only a bounded machine-readable code and classify it as `Retryable` or
`Permanent`. Raw database, transport, or source-domain errors must remain in owner logs and must
not cross this neutral contract.

The future durable worker will map retryable failures to bounded backoff and permanent failures
to durable terminal state. This slice deliberately does not define that policy yet.

## Still open

- mutation-source event registry and broker acknowledgement orchestration;
- batch persistence across source pages;
- durable `index_jobs` / `index_checkpoints` ownership, fencing, heartbeat, cancellation, and
  resume;
- dry-run, targeted/full/shadow rebuild commands;
- reconciliation, drift repair, retained freshness/outage/recovery evidence, and incremental /
  rebuild equivalence;
- source adapters for Product and later vertical slices.

## Owner validation

```bash
node scripts/verify/verify-index-source-replay-contract.mjs
cargo check -p rustok-index --all-targets
cargo test -p rustok-index source_registry --lib -- --nocapture
```

These commands remain maintainer-run for this slice.