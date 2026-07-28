# rustok-telemetry implementation plan

## Current state

`rustok-telemetry` owns shared tracing, OpenTelemetry, the process Prometheus
registry, and neutral instrumentation helpers. `apps/server` composes process-wide
bootstrap; modules and runtime workers own metric meaning, bounded label values,
alert thresholds, and runbooks.

The shared runtime-consumer collector covers throughput, terminal outcomes, retries,
bounded failures, DLQ publication, processing duration, lifecycle, in-flight
state/timestamps, last success, and broker-backed consumer lag.

Consumer lag is accepted only from a partition-qualified snapshot that reads every
topic partition and its persistent group checkpoint. The collector publishes snapshot
time, partition count, completeness, and bounded `total`/`max` lag aggregations.
Incomplete snapshots clear lag values and set completeness to zero so stale lag cannot
be mistaken for a current observation.

The separate `consumer_poison_metrics` collector now exposes one count-only neutral
receipt snapshot for a bounded durable consumer. It publishes fixed states `total`,
`reserved`, `publishing`, `expired_publishing`, `published`, and `acknowledged`, plus
snapshot availability and timestamp. Inspection failure or observer shutdown clears
all counts and timestamp so stale values cannot appear current.

No runtime-consumer or poison-receipt metric uses tenant, event, relation, partition,
offset, payload, delivery UUID, publisher identity, acknowledgement token, credential,
or raw error-message values as labels. Lag is never inferred from event age,
processing duration, one delivered offset, or a local cursor counter.

## Boundary

- Owner: platform observability.
- Process-wide tracing initialization happens once. The native server initializes full
  telemetry only when it owns the subscriber and metrics separately otherwise.
- The crate owns one Prometheus registry, OTel/tracing wiring, and neutral bounded
  collector helpers.
- Domain/runtime owners choose stable metric meanings, bounded semantic values, alert
  thresholds, and operational response.
- Runtime collectors register through `register_runtime_collector`; a second global
  registry is forbidden.
- Dynamic identities, raw positions, payloads, claims, credentials, storage causes,
  and unbounded error values are forbidden as metric labels.
- Metrics provide observations only. They never acknowledge, reclaim, repair, delete,
  retain, authorize, or select delivery policy.

## Delivered result: bounded runtime consumer metrics

- `runtime_consumer_metrics::ensure_registered` installs one cloneable collector in the
  initialized process registry.
- Delivery metrics expose received and terminally acknowledged outcomes, stage retries,
  stable-code failures, DLQ results, receive-to-ack duration, starts/terminations,
  in-flight state/start time, and last success.
- Position metrics expose:
  - `rustok_runtime_consumer_position_snapshot_timestamp_seconds`;
  - `rustok_runtime_consumer_position_partition_count`;
  - `rustok_runtime_consumer_position_complete`;
  - `rustok_runtime_consumer_lag{aggregation="total|max"}`.
- Lag labels remain bounded to consumer identity and the fixed aggregation set; partition
  and offset remain snapshot values rather than labels.
- A complete snapshot requires every partition to have an exact lag. Empty partitions
  contribute zero; missing/incoherent checkpoints make the snapshot incomplete.
- Worker termination clears in-flight gauges while preserving the last success and last
  position snapshot for incident diagnosis.
- The existing `/metrics` registry renders all series automatically.
- Static verification guards names, label sets, completeness clearing, broker-backed
  snapshot origin, DLQ ordering, and acknowledgement-only recovery.

## Delivered result: count-only poison receipt metrics

- `consumer_poison_metrics::ensure_registered` installs a separate bounded collector in
  the same process registry.
- `rustok_runtime_consumer_poison_receipts{consumer,state}` reports only six fixed
  aggregate states.
- `rustok_runtime_consumer_poison_snapshot_available` distinguishes a real all-zero
  snapshot from unavailable storage inspection.
- `rustok_runtime_consumer_poison_snapshot_timestamp_seconds` identifies the latest
  successful snapshot and resets to zero when unavailable.
- Counts saturate at the Prometheus signed gauge range instead of wrapping.
- The Social Graph Index owner observer supplies only connector-validated aggregates,
  uses one fixed consumer label, logs bounded stable failure codes, and leaves
  projection active on inspection failure.
- Alert thresholds and any reclaim, repair, or retention response remain external
  reviewed operator policy.

## Next results

1. **Prove bootstrap and shutdown behavior in every host mode.** Cover native server,
   CLI compatibility, OTel enabled/disabled, metrics disabled, repeated initialization,
   dynamic collector registration, poison observer shutdown, and graceful exporter
   shutdown.
2. **Harden the shared metrics contract.** Add focused coverage for duplicate/concurrent
   registration, disabled telemetry, collector rendering, bounded labels, incomplete
   snapshot clearing, poison snapshot unavailability, and worker termination with an
   unacknowledged delivery.
3. **Retain live lag and receipt evidence.** Verify every-partition Iggy snapshots,
   concurrent broker movement, missing checkpoints, observer reconnect, PostgreSQL
   aggregate consistency, expired leases, TLS/auth failure, and multi-replica semantics
   before selecting alerts.
4. **Align module instrumentation with operations.** Validate representative modules and
   `/metrics` against a small correlation/service-health convention without moving
   domain semantics or runbooks into this crate.

## Verification

- `cargo test -p rustok-telemetry`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-telemetry --all-targets`
- `node scripts/verify/verify-runtime-consumer-metrics.mjs`
- `node scripts/verify/verify-iggy-consumer-position.mjs`
- `node scripts/verify/verify-social-graph-index-worker-lifecycle.mjs`
- `node scripts/verify/verify-social-graph-index-poison-observer.mjs`
- `scripts/verify/verify-architecture.sh`
- Targeted `/metrics`, bootstrap, registration, snapshot, and shutdown tests.

These commands remain maintainer-run and were not executed manually in this slice.

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [Telemetry reference package](../../../docs/references/telemetry/README.md)
- [Observability quickstart](../../../docs/guides/observability-quickstart.md)
