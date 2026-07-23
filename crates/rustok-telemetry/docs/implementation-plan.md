# rustok-telemetry implementation plan

## Current state

`rustok-telemetry` owns shared tracing, OpenTelemetry, Prometheus registry, and
instrumentation helpers. `apps/server` composes the process-wide bootstrap;
modules emit owner-specific measurements through this shared surface. The crate
must not absorb domain metric policy, runbooks, or transport behavior.

## Boundary

- Owner: platform observability.
- Process-wide tracing initialization happens once. During the native runtime
  runtime transition, the server initializes full telemetry only when it owns
  the subscriber and initializes metrics separately otherwise.
- The crate owns one Prometheus registry and OTel/tracing wiring; modules own
  metric meaning, labels, alert thresholds, and operational response.

## Next results

1. **Prove bootstrap and shutdown behavior in each host mode.** Test native
   server, current CLI compatibility, OTel enabled/disabled, metrics
   disabled, repeated initialization, and graceful exporter shutdown. Done when
   the supported modes have executable evidence and no mode silently loses
   tracing or registers a second subscriber.
2. **Harden the shared metrics contract.** Audit shared labels and registration
   for bounded cardinality, tenant/privacy safety, error handling, and render
   behavior; add regression coverage for the public registry and metric helpers.
   Done when a high-cardinality or duplicate-registration regression fails a
   focused test instead of production scraping.
3. **Align module instrumentation with operations.** Define a small common
   correlation and service-health convention, then validate representative
   modules and the `/metrics` endpoint against it. Done when metrics/traces can
   be linked to an owner runbook without moving domain semantics here.

## Verification

- Contract tests cover every public use case.
- `cargo test -p rustok-telemetry`
- `scripts/verify/verify-architecture.sh` (single telemetry initialization)
- Targeted `/metrics`, OTel configuration, bootstrap, and shutdown tests.

## References

- [Crate README](../README.md)
- [Module documentation](./README.md)
- [Telemetry reference package](../../../docs/references/telemetry/README.md)
- [Observability quickstart](../../../docs/guides/observability-quickstart.md)
