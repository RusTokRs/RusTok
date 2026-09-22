# rustok-telemetry documentation

`rustok-telemetry` owns the shared tracing, OpenTelemetry, Prometheus registry,
and instrumentation helpers for RusToK. It does not own module metric meaning,
alerts, or domain runbooks.

The server composes process-wide telemetry once; modules use the shared API for
instrumentation. Current bootstrap, registry, and operations work is tracked in
the [implementation plan](./implementation-plan.md).
