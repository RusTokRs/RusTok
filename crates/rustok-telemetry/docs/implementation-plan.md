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

The dedicated `rbac_invalidation_metrics` module exposes process-level durable and
applied authorization-generation state, signed lag, watchdog health and bounded
recovery counters. It is registered in the same canonical process registry. RBAC and
`apps/server` retain ownership of generation semantics, recovery decisions, alert
thresholds and the incident runbook.

No runtime-consumer, poison-receipt or RBAC invalidation metric uses tenant, user,
role, permission, session, OAuth client, event, relation, partition, offset, payload,
delivery UUID, publisher identity, acknowledgement token, credential, cache key or
raw error-message values as labels. Lag is never inferred from event age, processing
duration, one delivered offset, a local cursor counter or Redis state.

## Boundary

- Owner: platform observability.
- Process-wide tracing initialization happens once. The native server initializes full
  telemetry only when it owns the subscriber and metrics separately otherwise.
- The crate owns one Prometheus registry, OTel/tracing wiring, and neutral bounded
  collector helpers.
- Domain/runtime owners choose stable metric meanings, bounded semantic values, alert
  thresholds, and operational response.
- Runtime collectors register through `register_runtime_collector`; statically known
  bounded collectors register through the same initialization path. A second global
  registry is forbidden.
- Dynamic identities, raw positions, payloads, claims, credentials, storage causes,
  and unbounded error values are forbidden as metric labels.
- Metrics provide observations only. They never acknowledge, reclaim, repair, delete,
  retain, authorize, invalidate snapshots, advance generations or select delivery
  policy.

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

## Delivered result: bounded RBAC invalidation metrics

- `rbac_invalidation_metrics::register` installs eight statically known metric families
  in the existing process registry.
- Gauges expose durable database generation, locally applied generation, signed
  durable-minus-applied lag and watchdog running state.
- A positive lag represents pending catch-up, zero represents an applied checkpoint and
  a negative lag represents database regression below the monotonic process checkpoint.
- Counters expose durable-generation database read failures, watchdog restarts,
  recovery actions and process-wide permission snapshot clears.
- The only vector label is `reason`; values are selected by the canonical worker from
  bounded sets such as `panic`, `unexpected_exit`, `runtime_replaced`, `initial_sync`,
  `generation_advanced` and `generation_regressed`.
- The collector performs no database reads, cache operations or worker supervision. The
  existing `apps/server` watchdog supplies observations and remains the only recovery
  path.
- Unit coverage locks signed lag behavior and registration of all metric families.
- `scripts/verify/verify-rbac-invalidation-observability.mjs` guards registry
  composition, forbidden labels, canonical worker wiring, RBAC operator documentation
  and cycle cursor synchronization.

## Next results

1. **Prove bootstrap and shutdown behavior in every host mode.** Cover native server,
   CLI compatibility, OTel enabled/disabled, metrics disabled, repeated initialization,
   dynamic collector registration, poison observer shutdown, RBAC watchdog shutdown,
   and graceful exporter shutdown.
2. **Harden the shared metrics contract.** Add focused coverage for duplicate/concurrent
   registration, disabled telemetry, collector rendering, bounded labels, incomplete
   snapshot clearing, poison snapshot unavailability, RBAC generation saturation and
   worker termination with an unacknowledged delivery.
3. **Retain live lag and receipt evidence.** Verify every-partition Iggy snapshots,
   concurrent broker movement, missing checkpoints, observer reconnect, PostgreSQL
   aggregate consistency, expired leases, TLS/auth failure, RBAC Redis outage/restart,
   missed publication and multi-replica semantics before promoting operational gates.
4. **Align module instrumentation with operations.** Validate representative modules and
   `/metrics` against a small correlation/service-health convention without moving
   domain semantics or runbooks into this crate. RBAC still needs one retained incident
   chain connecting evaluator decision, relation state, cache snapshot, durable
   generation and recovery action.

## Verification

- `cargo test -p rustok-telemetry`
- `cargo test -p rustok-telemetry rbac_invalidation_metrics`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-telemetry --all-targets`
- `cargo test -p rustok-server --lib rbac_invalidation_generation`
- `node scripts/verify/verify-rbac-invalidation-observability.mjs`
- `node scripts/verify/verify-runtime-consumer-metrics.mjs`
- `node scripts/verify/verify-iggy-consumer-position.mjs`
- `node scripts/verify/verify-social-graph-index-worker-lifecycle.mjs`
- `node scripts/verify/verify-social-graph-index-poison-observer.mjs`
- `scripts/verify/verify-architecture.sh`
- Targeted `/metrics`, bootstrap, registration, snapshot, shutdown and recovery tests.

These commands remain maintainer-run and were not executed manually in this slice.

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [Telemetry reference package](../../../docs/references/telemetry/README.md)
- [Observability quickstart](../../../docs/guides/observability-quickstart.md)
