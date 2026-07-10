# rustok-telemetry / CRATE_API

## Public modules

`metrics`, `otel`.

## Primary public types and functions

- `TelemetryConfig`, `TelemetryHandles`, `LogFormat`, `TelemetryError`
- `init`, `init_metrics`, `metrics_handle`, `render_metrics`, `current_trace_id`
- `otel::OtelConfig`, `otel::init_tracing`, `otel::shutdown`

## Contract invariants

- A process installs at most one global tracing subscriber.
- Metrics use the single registry initialized through this crate.
- Modules may emit measurements but retain domain label policy and alert/runbook
  ownership.
- OpenTelemetry configuration and exporter failures use the documented
  `TelemetryError` or explicit fallback behavior; they must not be silently
  represented as a second telemetry pipeline.

## Errors

`TelemetryError::SubscriberAlreadySet` identifies a duplicate global subscriber
attempt. Prometheus registration failures are returned as `TelemetryError::Prometheus`.
