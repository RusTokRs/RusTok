---
id: doc://docs/guides/tracing.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Distributed Tracing Guide

Полное руководство по распределённой трассировке находится в [`docs/standards/distributed-tracing.md`](../standards/distributed-tracing.md).

## Краткое резюме

RusToK использует OpenTelemetry + `tracing` crate для сквозной трассировки запросов.

- **Crate:** `crates/rustok-telemetry`
- **Протокол экспорта:** OTLP (совместим с Jaeger, Tempo, Honeycomb и др.)
- **Correlation:** каждый span содержит `tenant_id`, `request_id`, `trace_id`

## Быстрый старт

```rust
use tracing::instrument;

#[instrument(skip(db), fields(tenant_id = %tenant_id))]
pub async fn create_order(db: &DatabaseConnection, tenant_id: Uuid) -> Result<Order> {
    // автоматически создаётся span с именем функции
}
```

## Конфигурация

Настраивается через `settings.rustok` в `apps/server/config/*.yaml`:

```yaml
rustok:
  telemetry:
    otlp_endpoint: "http://localhost:4317"
    service_name: "rustok-server"
```

## Полная документация

→ [`docs/standards/distributed-tracing.md`](../standards/distributed-tracing.md)  
→ [`docs/guides/observability-quickstart.md`](./observability-quickstart.md)
