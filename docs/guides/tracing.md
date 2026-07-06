---
id: doc://docs/guides/tracing.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# Distributed Tracing Guide

The complete distributed tracing guide is in [`docs/standards/distributed-tracing.md`](../standards/distributed-tracing.md).

## Quick Summary

RusToK uses OpenTelemetry + `tracing` crate for end-to-end request tracing.

- **Crate:** `crates/rustok-telemetry`
- **Export protocol:** OTLP (compatible with Jaeger, Tempo, Honeycomb, etc.)
- **Correlation:** every span contains `tenant_id`, `request_id`, `trace_id`

## Quick Start

```rust
use tracing::instrument;

#[instrument(skip(db), fields(tenant_id = %tenant_id))]
pub async fn create_order(db: &DatabaseConnection, tenant_id: Uuid) -> Result<Order> {
    // automatically creates a span with the function name
}
```

## Configuration

Configured via `settings.rustok` in `apps/server/config/*.yaml`:

```yaml
rustok:
  telemetry:
    otlp_endpoint: "http://localhost:4317"
    service_name: "rustok-server"
```

## Full Documentation

→ [`docs/standards/distributed-tracing.md`](../standards/distributed-tracing.md)  
→ [`docs/guides/observability-quickstart.md`](./observability-quickstart.md)
