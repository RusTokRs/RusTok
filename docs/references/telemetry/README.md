---
id: doc://docs/references/telemetry/README.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Telemetry Reference-пакет (RusToK)

Дата последней актуализации: **2026-02-19**.

> Пакет фиксирует базовые рабочие паттерны `rustok-telemetry` (инициализация tracing/metrics) и предотвращает ложные переносы из ad-hoc логгирования.

## 1) Минимальный рабочий пример: инициализация telemetry

```rust
use rustok_telemetry::{init, TelemetryConfig};

let handles = init(TelemetryConfig::default())?;
let _guard = handles.guard;
```

## 2) Минимальный рабочий пример: рендер metrics

```rust
if let Some(handle) = rustok_telemetry::metrics_handle() {
    let body = handle.render();
    // вернуть body в /metrics
}
```

## 3) Актуальные сигнатуры API (в репозитории)

- `pub fn init(config: TelemetryConfig) -> Result<TelemetryHandles, TelemetryError>`
- `pub fn metrics_handle() -> Option<Arc<MetricsHandle>>`
- `pub fn render_metrics() -> Result<String, prometheus::Error>`
- `pub fn current_trace_id() -> Option<String>`
- `pub fn register_all(registry: &Registry) -> Result<(), prometheus::Error>`
- `pub fn record_event_published(event_type: &str, tenant_id: &str)`
- `pub fn record_event_dispatched(event_type: &str, handler: &str)`
- `pub fn update_queue_depth(transport: &str, depth: i64)`

## 4) Чего делать нельзя (типичные ложные паттерны)

1. **Нельзя инициализировать telemetry многократно в runtime.** Инициализация должна быть централизована.
2. **Нельзя смешивать ad-hoc метрики и platform metrics без единого registry.**
3. **Нельзя подменять trace-контекст ручными строками там, где доступен `current_trace_id()`.**
4. **Нельзя silently игнорировать ошибку инициализации telemetry в окружениях, где observability обязательна.**

## 5) Синхронизация с кодом (регламент)

- При изменениях в `crates/rustok-telemetry/**` и `apps/server/src/controllers/metrics.rs`:
  1) обновить примеры и сигнатуры;
  2) обновить дату в шапке;
  3) проверить, что anti-patterns остаются релевантными.
