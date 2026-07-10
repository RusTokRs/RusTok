---
id: doc://docs/references/telemetry/README.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---

# Telemetry Reference Package (RusToK)

Last updated: **2026-02-19**.

> This package captures the basic working patterns of `rustok-telemetry` (tracing/metrics initialization) and prevents incorrect migrations from ad-hoc logging.

## 1) Minimal working example: telemetry initialization

```rust
use rustok_telemetry::{init, LogFormat, TelemetryConfig};

let handles = init(TelemetryConfig {
    service_name: "rustok-server".to_string(),
    log_format: LogFormat::Json,
    metrics: true,
    otel: None,
})?;
let metrics = handles.metrics;
```

## 2) Minimal working example: metrics rendering

```rust
if let Some(handle) = rustok_telemetry::metrics_handle() {
    let body = handle.render();
    // return body in /metrics
}
```

## 3) Current API signatures (in repository)

- `pub fn init(config: TelemetryConfig) -> Result<TelemetryHandles, TelemetryError>`
- `pub fn metrics_handle() -> Option<Arc<MetricsHandle>>`
- `pub fn render_metrics() -> Result<String, prometheus::Error>`
- `pub fn current_trace_id() -> Option<String>`
- `pub fn register_all(registry: &Registry) -> Result<(), prometheus::Error>`
- `pub fn record_event_published(event_type: &str, tenant_id: &str)`
- `pub fn record_event_dispatched(event_type: &str, handler: &str)`
- `pub fn update_queue_depth(transport: &str, depth: i64)`

## 4) What not to do (typical incorrect patterns)

1. **Do not initialize telemetry multiple times at runtime.** Initialization must be centralized.
2. **Do not mix ad-hoc metrics and platform metrics without a single registry.**
3. **Do not replace trace context with manual strings where `current_trace_id()` is available.**
4. **Do not silently ignore telemetry initialization errors in environments where observability is mandatory.**

## 5) Synchronization with code (procedure)

- When changes are made to `crates/rustok-telemetry/**` and `apps/server/src/controllers/metrics.rs`:
  1) update examples and signatures;
  2) update the date in the header;
  3) verify that the anti-patterns remain relevant.
