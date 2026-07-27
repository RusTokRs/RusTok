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
in-flight state/timestamps, last success, and observed source offsets.

No runtime-consumer metric uses tenant, event, relation, payload, partition,
ack-token, or raw error-message values as labels. Observed offsets are not presented
as true broker lag; that requires a connector high-watermark contract.

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
- Dynamic identities, payloads, claims, credentials, raw storage causes, and other
  high-cardinality or sensitive values are forbidden as metric labels.

## Delivered result: bounded runtime consumer metrics

- `runtime_consumer_metrics::ensure_registered` installs one cloneable collector in
  the initialized process registry and fails visibly when no registry exists.
- Metrics expose received and terminally acknowledged deliveries, outcome classes,
  retries by stage, failures by bounded stage/stable code, DLQ publish results,
  receive-to-terminal-ack duration, starts/terminations, current in-flight state and
  start time, last success time, and received/acknowledged offsets.
- Labels are limited to `consumer`, `outcome`, `stage`, stable `error_code`, `result`,
  termination `reason`, and offset `state`.
- Offset values saturate at Prometheus signed gauge range instead of wrapping.
- The collector is rendered automatically by the existing `/metrics` registry.
- Static verification guards metric names, label sets, staged DLQ order, and
  acknowledgement-only recovery.

## Next results

1. **Prove bootstrap and shutdown behavior in each host mode.** Test native server,
   CLI compatibility, OTel enabled/disabled, metrics disabled, repeated
   initialization, dynamic collector registration, and graceful exporter shutdown.
2. **Harden the shared metrics contract.** Add focused regression coverage for
   duplicate/concurrent runtime registration, disabled telemetry, collector render,
   bounded labels, and worker termination while a delivery is unacknowledged.
3. **Add a reviewed high-watermark convention.** Once transport connectors expose
   partition high-watermarks, define a neutral offset-lag metric that does not infer
   lag from event timestamps or processing duration.
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
