# M5/M6 bounded source replay contract

This slice establishes the source-owned read boundary used by incremental ingestion,
rebuild, reconciliation, and repair workers. It also adds one bounded replay-page
executor, schema-scoped durable replay job claims, and a lease-bound PostgreSQL
checkpoint adapter. It does not claim a scheduler, multi-page replay loop,
cancellation command, or production consumer cutover.

## Ownership

- Source modules own normalized state and implement `IndexSource`.
- Index owns source registration, schema/source ownership validation, request bounds,
  continuation safety, returned-mutation scope validation, persistence, checkpoints,
  jobs, leases, retry policy, and operator controls.
- Index core never imports source-domain crates and never reads source tables directly.

## Registration

`IndexSourceCatalog` is seeded beside `IndexSchemaSourceCatalog` during module runtime
registration. A source module registers:

- one bounded lowercase owner module slug;
- one unique bounded source name;
- one or more exact `SchemaRef` values;
- one `IndexSource` implementation.

One exact schema and one complete `(module, entity)` identity cannot move between replay
sources across versions. Materialization checks every replay schema against the
source-owned schema catalog and rejects missing schemas or owner drift. An absent or
empty source catalog materializes to `None`; merely publishing a schema does not falsely
claim rebuild readiness.

## Cursor-based scan

`IndexSource::scan` receives `IndexSourceScanRequest` with one non-nil tenant, one exact
schema, an optional opaque JSON cursor, and a limit from 1 through 1000. Cursor JSON is
non-null and at most 8 KiB when encoded.

`IndexSourcePage` is accepted only when:

- the mutation count does not exceed the requested limit;
- every mutation stays in the requested tenant and exact schema;
- entity keys are unique within the page;
- an empty page does not advertise continuation;
- a continuation cursor differs from the request cursor.

`None` means the scan is complete. The contract therefore supports bounded streaming
without collecting every source ID before work begins.

## Targeted load

`IndexSource::load` receives one to 256 unique `EntityKey` values from exactly one
non-nil tenant and exact schema. A returned mutation must correspond to one requested
key, and each key may appear at most once. Missing keys are permitted so deletion or
authorization-safe absence can be represented by the owner adapter's chosen mutation
semantics without widening the requested scope.

## One-page replay progression

`IndexReplayWorker::run_next_page` executes exactly one bounded source page:

1. Resolve the registered source for the exact schema.
2. Read the durable rebuild checkpoint identified by tenant, source, and exact schema.
3. Fail closed if the checkpoint store returns another replay identity.
4. Return `AlreadyComplete` without calling the source when the stored cursor is complete.
5. Scan from the stored opaque cursor.
6. Validate every page event UUID before the first mutation write; nil or duplicate IDs
   reject the whole page.
7. Apply mutations sequentially through `PostgresMutationStore`, using each mutation
   event UUID as the stable delivery ID.
8. Commit the next cursor only after every mutation result is durable.

A source adapter must return the same non-nil event UUID for the same logical entity
mutation and source version whenever a page is retried. This makes the replay delivery
identity stable without importing source-domain identifiers into Index core.

Applied, duplicate, and stale mutation outcomes are counted separately. A failure after
one or more mutation commits but before checkpoint commit does not lose progress safety:
the page is retried from the previous cursor and the existing inbox identity makes the
same stable event deliveries idempotent.

The checkpoint source-version watermark is inherited from the stored row and advanced
with the maximum observed page version; a lower-version or empty final page cannot
regress it. The last delivery ID is likewise retained for an empty final page.

## Fenced job and checkpoint ownership

`PostgresIndexReplayJobStore` owns one exact tenant/source/schema rebuild job. It
validates the `index_replay_job_v1` request, requires an active persisted schema,
serializes acquisition with a PostgreSQL advisory lock, heartbeats an unexpired lease,
and reclaims expired attempts with an incremented attempt fence.

`PostgresIndexReplayCheckpointStore` is constructed from the acquired
`IndexReplayJobLease`. Before each checkpoint read or write it validates and locks the
exact `(job_id, worker_id, attempt_count)`. Another tenant, source, or schema is rejected
before opening the transaction. A stale worker may complete an already-started
idempotent mutation write, but it cannot advance the durable cursor.

The checkpoint row uses JSON `null` for completion. Locale and partition dimensions
remain reserved empty values until separately admitted contracts exist. A replay job
can enter `succeeded` only while its lease remains active and its exact durable rebuild
checkpoint exists with a null cursor. See
[`m6-replay-job-leases.md`](./m6-replay-job-leases.md) for the full fencing boundary.

## Failures

Source adapters and replay dependencies expose only a bounded machine-readable code and
classify it as `Retryable` or `Permanent`. Raw database, transport, or source-domain
errors remain in owner logs and do not cross the neutral contract.

Mutation validation/delivery rejection is permanent. Transient storage, concurrent
mutation, inbox-completion, and checkpoint I/O failures are retryable. Lease loss is
terminal for the current attempt. A future bounded multi-page runner will map retryable
failures to bounded backoff and permanent failures to durable terminal state; this
slice does not define that scheduling policy.

## Still open

- mutation-source event registry and broker acknowledgement orchestration;
- binding replay job requests directly to the materialized source registry in server
  composition;
- a bounded multi-page loop, heartbeat cadence, graceful lease loss, cancellation,
  resume command, dry-run, targeted/full/shadow modes, and retry/dead-letter scheduling;
- locale/partition replay checkpoint dimensions;
- reconciliation, drift repair, retained freshness/outage/recovery evidence, and
  incremental/rebuild equivalence;
- source adapters for Product and later vertical slices.

## Owner validation

```bash
node scripts/verify/verify-index-source-replay-contract.mjs
node scripts/verify/verify-index-replay-job-leases.mjs
cargo check -p rustok-index --all-targets
cargo test -p rustok-index source_registry --lib -- --nocapture
cargo test -p rustok-index source_replay --lib -- --nocapture
cargo test -p rustok-index source_replay_job --lib -- --nocapture
```

These commands remain maintainer-run for this slice.
