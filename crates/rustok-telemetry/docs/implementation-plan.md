# rustok-telemetry implementation plan

## Current state

`rustok-telemetry` owns shared tracing, OpenTelemetry, the process Prometheus
registry, and neutral instrumentation helpers. `apps/server` composes process-wide
bootstrap; modules and runtime workers own metric meaning, bounded label values,
alert thresholds, and runbooks.

The shared runtime-consumer collector now covers throughput, terminal outcomes,
retries, bounded failures, DLQ publication, processing duration, lifecycle,
in-flight state/timestamps, last success, and broker-backed consumer lag.

Consumer lag is accepted only from a partition-qualified snapshot that reads every
topic partition and its persistent group checkpoint. The collector publishes snapshot
time, partition count, completeness, and bounded `total`/`max` lag aggregations.
Incomplete snapshots clear lag values and set completeness to zero so stale lag cannot
be mistaken for a current observation.

No runtime-consumer metric uses tenant, event, relation, partition, offset, payload,
ack-token, credential, or raw error-message values as labels. Lag is never inferred
from event age, processing duration, one delivered offset, or a local cursor counter.

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
- Dynamic identities, raw positions, payloads, claims, credentials, and storage causes
  are forbidden as metric labels.

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

## Next results

1. **Prove bootstrap and shutdown behavior in every host mode.** Cover native server,
   CLI compatibility, OTel enabled/disabled, metrics disabled, repeated initialization,
   dynamic collector registration, and graceful exporter shutdown.
2. **Harden the shared metrics contract.** Add focused coverage for duplicate/concurrent
   registration, disabled telemetry, collector rendering, bounded labels, incomplete
   snapshot clearing, and worker termination with an unacknowledged delivery.
3. **Retain live lag evidence.** Verify every-partition Iggy snapshots, concurrent broker
   movement during capture, missing checkpoints, observer reconnect, TLS/auth failure,
   and multi-replica group semantics before defining alerts.
4. **Align module instrumentation with operations.** Validate representative modules and
   `/metrics` against a small correlation/service-health convention without moving
   domain semantics or runbooks into this crate.

## Verification

- `cargo test -p rustok-telemetry`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-telemetry --all-targets`
- `node scripts/verify/verify-runtime-consumer-metrics.mjs`
- `node scripts/verify/verify-iggy-consumer-position.mjs`
- `node scripts/verify/verify-social-graph-index-worker-lifecycle.mjs`
- `scripts/verify/verify-architecture.sh`
- Targeted `/metrics`, bootstrap, registration, snapshot, and shutdown tests.

These commands remain maintainer-run and were not executed manually in this slice.

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [Telemetry reference package](../../../docs/references/telemetry/README.md)
- [Observability quickstart](../../../docs/guides/observability-quickstart.md)
