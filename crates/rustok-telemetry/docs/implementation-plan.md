# rustok-telemetry implementation plan

## Current state

`rustok-telemetry` owns shared tracing, OpenTelemetry, the process Prometheus
registry, and neutral instrumentation helpers. `apps/server` composes process-wide
bootstrap; modules and runtime workers own metric meaning, bounded label values,
alert thresholds, and runbooks.

The crate now supports bounded runtime collector registration after telemetry
initialization. The first concrete consumer is the Social Graph → Index durable
worker, which registers one shared collector for throughput, terminal outcomes,
retries, bounded failures, DLQ publication, processing duration, lifecycle,
in-flight state/timestamps, and last success.

No runtime-consumer metric uses tenant, event, relation, partition, offset, payload,
ack-token, or raw error-message values as labels. Source position and lag series are
intentionally absent until a connector can supply partition-qualified acknowledged
positions and broker high-watermarks.

## Boundary

- Owner: platform observability.
- Process-wide tracing initialization happens once. The native server initializes
  full telemetry only when it owns the subscriber and initializes metrics separately
  otherwise.
- The crate owns one Prometheus registry, OTel/tracing wiring, and neutral bounded
  collector helpers.
- Domain/runtime owners choose stable metric names, bounded semantic labels, and
  operational response.
- Runtime collectors must register into the existing registry through
  `register_runtime_collector`; creating a second global registry is forbidden.
- Dynamic identities, transport positions, payloads, claims, credentials, raw storage
  causes, and other high-cardinality or sensitive values are forbidden as labels.

## Delivered result: bounded runtime consumer metrics

- `runtime_consumer_metrics::ensure_registered` installs one cloneable collector in
  the initialized process registry and fails visibly when no registry exists.
- Metrics expose received and terminally acknowledged deliveries, outcome classes,
  retries by stage, failures by bounded stage/stable code, DLQ publish results,
  receive-to-terminal-ack duration, starts/terminations, current in-flight state and
  start time, and last success time.
- Labels are limited to `consumer`, `outcome`, `stage`, stable `error_code`, `result`,
  and termination `reason`.
- Worker termination clears in-flight gauges while preserving the last successful
  acknowledgement timestamp.
- The collector is rendered automatically by the existing `/metrics` registry.
- Static verification guards metric names, bounded label sets, absence of incomplete
  source-position/lag metrics, staged DLQ order, clean in-flight lifecycle, and
  acknowledgement-only recovery.

## Next results

1. **Prove bootstrap and shutdown behavior in each host mode.** Test native server,
   CLI compatibility, OTel enabled/disabled, metrics disabled, repeated
   initialization, dynamic collector registration, and graceful exporter shutdown.
2. **Harden the shared metrics contract.** Add focused regression coverage for
   duplicate/concurrent runtime registration, disabled telemetry, collector render,
   bounded labels, and worker termination while a delivery is unacknowledged.
3. **Add a reviewed high-watermark convention.** Once transport connectors expose a
   partition-qualified acknowledged-position snapshot and partition high-watermarks,
   define neutral source-position and lag metrics. Do not infer them from event age,
   processing duration, or one global offset.
4. **Align module instrumentation with operations.** Validate representative modules
   and `/metrics` against a small correlation/service-health convention without
   moving domain semantics or runbooks into this crate.

## Verification

- `cargo test -p rustok-telemetry`
- `RUSTFLAGS="-Dwarnings" cargo check -p rustok-telemetry --all-targets`
- `node scripts/verify/verify-runtime-consumer-metrics.mjs`
- `node scripts/verify/verify-social-graph-index-worker-lifecycle.mjs`
- `scripts/verify/verify-architecture.sh`
- Targeted `/metrics`, OTel configuration, bootstrap, registration, and shutdown tests.

These commands remain maintainer-run and were not executed manually in this slice.

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [Telemetry reference package](../../../docs/references/telemetry/README.md)
- [Observability quickstart](../../../docs/guides/observability-quickstart.md)
