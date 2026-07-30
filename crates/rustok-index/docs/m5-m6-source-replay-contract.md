# M5/M6 bounded source replay contract

This slice establishes the source-owned read boundary used by incremental ingestion,
rebuild, reconciliation, and repair workers. It also adds one bounded replay-page
executor and PostgreSQL checkpoint adapter. It does not claim fenced job ownership,
a scheduler, a long-running replay command, or production consumer cutover.

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
2. Read the durable rebuild checkpoint identified by tenant, source, schema, and bounded
   partition key.
3. Return `AlreadyComplete` without calling the source when the stored cursor is complete.
4. Scan from the stored opaque cursor.
5. Apply mutations sequentially through `PostgresMutationStore`, using each mutation
   event UUID as the stable delivery ID.
6. Commit the next cursor only after every mutation result is durable.

Applied, duplicate, and stale mutation outcomes are counted separately. A failure after
one or more mutation commits but before checkpoint commit does not lose progress safety:
the page is retried from the previous cursor and the existing inbox identity makes the
same event deliveries idempotent.

`PostgresIndexReplayCheckpointStore` writes the existing `index_checkpoints` rebuild
row. JSON `null` represents a completed cursor. Empty final pages preserve the last
stored source version and delivery ID through `COALESCE`, while still marking the cursor
complete. This adapter does not claim a job lease or global worker owner.

## Failures

Source adapters and replay dependencies expose only a bounded machine-readable code and
classify it as `Retryable` or `Permanent`. Raw database, transport, or source-domain
errors remain in owner logs and do not cross the neutral contract.

Mutation validation/delivery rejection is permanent. Transient storage, concurrent
mutation, inbox-completion, and checkpoint I/O failures are retryable. The future fenced
job worker will map retryable failures to bounded backoff and permanent failures to
durable terminal state; this slice does not define that scheduling policy.

## Still open

- mutation-source event registry and broker acknowledgement orchestration;
- fenced `index_jobs` claims, leases, heartbeat, cancellation, and global ownership;
- a bounded multi-page loop, resume command, dry-run, targeted/full/shadow modes, and
  retry/dead-letter scheduling;
- reconciliation, drift repair, retained freshness/outage/recovery evidence, and
  incremental/rebuild equivalence;
- source adapters for Product and later vertical slices.

## Owner validation

```bash
node scripts/verify/verify-index-source-replay-contract.mjs
cargo check -p rustok-index --all-targets
cargo test -p rustok-index source_registry --lib -- --nocapture
cargo test -p rustok-index source_replay --lib -- --nocapture
```

These commands remain maintainer-run for this slice.
