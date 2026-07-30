# rustok-telemetry / CRATE_API

## Public modules

`consumer_poison_metrics`, `dlq_duplicate_alert_metrics`, `metrics`, `otel`,
`runtime_consumer_metrics`, `social_graph_index_privacy_shadow_metrics`.

## Primary public types and functions

- `TelemetryConfig`, `TelemetryHandles`, `LogFormat`, `TelemetryError`
- `init`, `init_metrics`, `metrics_handle`, `render_metrics`, `current_trace_id`
- `register_runtime_collector`
- `dlq_duplicate_alert_metrics::{register, record_state, record_snapshot}`
- `social_graph_index_privacy_shadow_metrics::{ensure_registered, record_observation, record_failure}`
- `SocialGraphIndexPrivacyShadowOperation`, `SocialGraphIndexPrivacyShadowOutcome`
- `otel::OtelConfig`, `otel::init_tracing`, `otel::shutdown`

## Social Graph Index privacy shadow metrics

- `rustok_social_graph_index_privacy_shadow_observations_total{operation,outcome}` records
  fixed parity outcomes.
- `rustok_social_graph_index_privacy_shadow_failures_total{operation,error_code,retryable}`
  records only two known stable privacy error codes or `other`.
- `rustok_social_graph_index_privacy_shadow_comparison_duration_seconds{operation,outcome}`
  measures the Index comparison after the authoritative owner read.
- `rustok_social_graph_index_privacy_shadow_last_observation_timestamp_seconds{operation,outcome}`
  records the latest observation time for each bounded series.
- Boolean outcomes include `match_positive`, `match_negative`, `false_negative`, and
  `false_positive`; batch outcomes include empty/non-empty matches, missing, extra, and
  `batch_mixed`.
- Tenant and user identifiers, relation/entity IDs, payloads, SQL, and raw storage errors are
  forbidden labels.
- The collector uses the single process registry. An explicitly enabled evidence shadow may
  fail activation when registration is unavailable rather than silently running unmeasured.

## Contract invariants

- A process installs at most one global tracing subscriber.
- Metrics use the single registry initialized through this crate.
- Modules may emit measurements but retain domain label policy and alert/runbook ownership.
- Physical DLQ duplicate metrics use only bounded deployment, scan-mode, health state,
  alert-level, availability, and evaluation-flag labels. They exclude message, tenant,
  broker-coordinate, payload, credential, threshold, source-count, timestamp, and raw-error
  labels.
- Privacy-shadow metrics use fixed operation/outcome domains and bounded error-code mapping;
  they never carry tenant and user identifiers.
- OpenTelemetry configuration and exporter failures use the documented `TelemetryError` or
  explicit fallback behavior; they must not be silently represented as a second telemetry
  pipeline.

## Errors

`TelemetryError::SubscriberAlreadySet` identifies a duplicate global subscriber attempt.
Prometheus registration failures are returned as `TelemetryError::Prometheus` or directly
from runtime collector registration.
