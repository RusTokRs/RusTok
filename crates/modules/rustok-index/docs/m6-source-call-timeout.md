# M6 bounded source-call timeout

Status: `source_complete_owner_execution_pending`

This slice adds a bounded cancellation boundary around every production
`IndexSource::scan` and targeted `IndexSource::load` registered through the canonical
`register_index_source` helper.

## Contract

- The default source-call deadline is `30 seconds`.
- The wrapper applies to both cursor scans and targeted loads.
- Expiry drops the owner source future; Index does not wait indefinitely for the source.
- Scan expiry becomes retryable code `index_source_scan_timeout`.
- Targeted-load expiry becomes retryable code `index_source_load_timeout`.
- Raw database, transport, source, or timeout causes are not copied into the public failure.
- Successful and owner-classified failed calls pass through unchanged.

Selected Product, ProductVariant, and SalesChannel PostgreSQL sources already register through
`register_index_source`, so they receive this boundary without source-domain changes. Future
production bridges must use the same helper. Direct `IndexSourceCatalog::register` remains
available for isolated fixtures and low-level contract tests and intentionally does not imply a
production timeout policy.

## Lease boundary

The source wrapper does not acquire, heartbeat, extend, or terminalize replay/reconciliation
leases; this source wrapper never extends or heartbeats a job lease. Operators must configure a
lease longer than the 30-second source deadline plus enough margin for mutation persistence,
checkpoint/progress persistence, cancellation observation, and terminal state publication.

A source timeout can occur after earlier pages are durable. Replay retries remain safe because
stable event UUIDs and `PostgresMutationStore` inbox deduplication already protect repeated page
delivery. Reconciliation retains the same deterministic event identities and monotonic source
versions.

## Explicitly open

- configurable per-source or per-run timeout policy;
- timeout around mutation persistence and checkpoint/progress writes;
- cooperative source cancellation tokens;
- in-page operator cancellation rather than between-page observation;
- automatic retry/backoff and dead-letter scheduling;
- host scheduler and graceful shutdown ownership;
- retained PostgreSQL timeout, retry, lease-expiry, and restart evidence.

The canonical implementation-plan item for complete in-page interruption/timeouts remains open
because this slice bounds owner source calls only; it does not claim full page or persistence
interruption.

## Validation ownership

Formatting, Cargo checks/tests, JavaScript guards, workflow execution, and live PostgreSQL
validation are maintainer-run. The implementation agent did not execute them.
