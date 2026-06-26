---
id: doc://docs/standards/opentelemetry-integration.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# 📊 OpenTelemetry Integration Guide

> **Status:** ✅ Complete  
> **Date:** 2026-02-13  
> **Sprint:** 3 - Task 3.1

## 📋 Overview

This guide explains how to use OpenTelemetry distributed tracing in RusToK.

### What is OpenTelemetry?

OpenTelemetry (OTel) is an observability framework for creating and managing telemetry data (traces, metrics, and logs). It helps you:

- **Trace requests** across microservices and modules
- **Identify bottlenecks** in your application
- **Debug distributed systems** with context propagation
- **Visualize dependencies** between services
- **Monitor performance** in production

### Architecture

```
┌─────────────┐
│   RusToK    │
│   Server    │
│             │
│ ┌─────────┐ │
│ │ Service │ │ --[traces]--> ┌──────────────┐
│ │  Layer  │ │               │ OTLP         │
│ └─────────┘ │               │ Collector    │
│             │               └──────────────┘
│ ┌─────────┐ │                      │
│ │  Event  │ │                      ▼
│ └─────────┘ │               ┌──────────────┐
└─────────────┘               │   Jaeger     │
                              │   Tempo      │
                              │   Zipkin     │
                              └──────────────┘
```

## 🚀 Quick Start

### 1. Enable OpenTelemetry

Set environment variables:

```bash
# Enable OpenTelemetry
export OTEL_ENABLED=true

# Configure OTLP endpoint (default: http://localhost:4317)
export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317

# Optional: Set service name
export OTEL_SERVICE_NAME=rustok-server

# Optional: Set sampling rate (0.0-1.0, default: 1.0 = 100%)
export OTEL_SAMPLING_RATE=1.0

# Optional: Set environment
export RUST_ENV=production
```

### 2. Start OTLP Collector

Using Jaeger (with OTLP support):

```bash
docker run -d \
  --name jaeger \
  -p 16686:16686 \
  -p 4317:4317 \
  jaegertracing/all-in-one:latest
```

Or using Grafana Tempo:

```bash
# See docker-compose.observability.yml
docker-compose -f docker-compose.observability.yml up -d tempo
```

### 3. Run RusToK Server

```bash
cargo run --bin rustok-server
```

### 4. View Traces

Open Jaeger UI at http://localhost:16686

## 📝 Instrumenting Your Code

### Basic Instrumentation

Use the `#[tracing::instrument]` attribute:

```rust
use tracing::instrument;
use uuid::Uuid;

#[instrument(
    name = "create_product",
    skip(self),
    fields(
        tenant_id = %tenant_id,
        otel.kind = "internal"
    )
)]
pub async fn create_product(
    &self,
    tenant_id: Uuid,
    actor_id: Uuid,
    input: CreateProductInput,
) -> Result<ProductResponse> {
    // Your business logic
    let product = self.repository.create(input).await?;
    
    // Record additional span attributes
    tracing::Span::current().record("product_id", &product.id.to_string());
    tracing::Span::current().record("product_sku", &product.sku);
    
    Ok(product)
}
```

### Adding Custom Spans

For more granular control:

```rust
use tracing::{info_span, Instrument};

pub async fn complex_operation(&self) -> Result<()> {
    // Create a span manually
    let span = info_span!(
        "database_query",
        db.statement = "SELECT * FROM products WHERE ...",
        otel.kind = "client"
    );
    
    // Execute code within the span
    let products = async {
        self.db.query_all().await
    }
    .instrument(span)
    .await?;
    
    // Another span for processing
    let process_span = info_span!("process_products", count = products.len());
    async {
        for product in products {
            self.process(product).await?;
        }
        Ok::<_, Error>(())
    }
    .instrument(process_span)
    .await?;
    
    Ok(())
}
```

### Span Attributes (Semantic Conventions)

Follow OpenTelemetry semantic conventions:

```rust
#[instrument(
    skip(self),
    fields(
        // Service attributes
        service.name = "rustok-server",
        service.version = env!("CARGO_PKG_VERSION"),
        
        // HTTP attributes (for HTTP handlers)
        http.method = %req.method(),
        http.route = %req.uri().path(),
        http.target = %req.uri(),
        
        // Database attributes
        db.system = "postgresql",
        db.statement = tracing::field::Empty,
        
        // Custom business attributes
        tenant_id = tracing::field::Empty,
        user_id = tracing::field::Empty,
        
        // Span kind
        otel.kind = "server" // or "client", "producer", "consumer", "internal"
    )
)]
pub async fn handler(&self, req: Request) -> Result<Response> {
    let span = tracing::Span::current();
    
    // Fill in empty fields
    span.record("tenant_id", &tenant_id.to_string());
    span.record("user_id", &user_id.to_string());
    span.record("db.statement", "SELECT * FROM nodes WHERE tenant_id = $1");
    
    // Your logic
    Ok(response)
}
```

### Event Bus Tracing

Propagate trace context through events:

```rust
use rustok_core::events::{DomainEvent, EventEnvelope};

// When publishing an event
pub async fn publish_event(&self, event: DomainEvent) -> Result<()> {
    let span = tracing::Span::current();
    let trace_id = span.context().span().span_context().trace_id().to_string();
    
    // Attach trace context to event metadata
    let envelope = EventEnvelope {
        event,
        metadata: EventMetadata {
            trace_id: Some(trace_id),
            ..Default::default()
        },
    };
    
    self.event_bus.publish(envelope).await
}

// When handling an event
#[instrument(skip(self, envelope), fields(trace_id = %envelope.metadata.trace_id))]
pub async fn handle_event(&self, envelope: EventEnvelope) -> Result<()> {
    // The trace_id field will link this span to the original request
    
    // Your event handling logic
    Ok(())
}
```

## 🔧 Configuration

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `OTEL_ENABLED` | Enable/disable OpenTelemetry | `false` |
| `OTEL_EXPORTER_OTLP_ENDPOINT` | OTLP gRPC endpoint | `http://localhost:4317` |
| `OTEL_SERVICE_NAME` | Service name in traces | `rustok-server` |
| `OTEL_SERVICE_VERSION` | Service version | `0.1.0` |
| `OTEL_SAMPLING_RATE` | Sampling rate (0.0-1.0) | `1.0` (100%) |
| `RUST_ENV` / `ENVIRONMENT` | Environment name | `development` |

### Sampling Strategies

**Development (100% sampling):**
```bash
export OTEL_SAMPLING_RATE=1.0
```

**Production (10% sampling):**
```bash
export OTEL_SAMPLING_RATE=0.1
```

**High-traffic production (1% sampling):**
```bash
export OTEL_SAMPLING_RATE=0.01
```

### Batch Configuration

The default batch configuration (in `rustok-telemetry/src/otel.rs`):

```rust
let batch_config = BatchConfig::default()
    .with_max_queue_size(2048)          // Max spans in queue
    .with_max_export_batch_size(512)    // Max spans per export
    .with_scheduled_delay(Duration::from_secs(5)); // Export interval
```

Adjust these values based on your traffic:
- **Low traffic:** Smaller queue (512), faster export (2s)
- **High traffic:** Larger queue (4096), batch size (1024)

## 📊 Visualization Backends

### Jaeger

**Pros:**
- Easy to set up with Docker
- Native OTLP support
- Good UI for trace exploration
- Free and open-source

**Setup:**
```bash
docker run -d \
  --name jaeger \
  -p 16686:16686 \
  -p 4317:4317 \
  jaegertracing/all-in-one:latest
```

**UI:** http://localhost:16686

### Grafana Tempo

**Pros:**
- Scalable for large volumes
- Integrates with Grafana
- Cost-effective storage

**Setup:** See `docker-compose.observability.yml`

**UI:** Grafana → Explore → Tempo

### Zipkin

**Setup:**
```bash
docker run -d -p 9411:9411 openzipkin/zipkin
```

**Note:** Zipkin uses HTTP endpoint 9411, not OTLP 4317

## 🧪 Testing with OpenTelemetry

### Unit Tests

OpenTelemetry is automatically disabled in tests unless explicitly enabled:

```rust
#[tokio::test]
async fn test_with_tracing() {
    // Traces are created but not exported in tests
    let service = ProductService::new(db);
    let product = service.create_product(input).await.unwrap();
    assert_eq!(product.sku, "TEST-001");
}
```

### Integration Tests with Trace Verification

```rust
use opentelemetry::global;
use opentelemetry_sdk::trace::TracerProvider;

#[tokio::test]
async fn test_distributed_trace() {
    // Set up in-memory trace provider
    let provider = TracerProvider::builder().build();
    global::set_tracer_provider(provider.clone());
    
    // Run your test
    let response = app.create_and_publish_event().await;
    
    // Verify spans were created
    let spans = provider.force_flush();
    assert_eq!(spans.len(), 2); // create + publish
    assert_eq!(spans[0].name, "create_product");
    assert_eq!(spans[1].name, "publish_event");
}
```

## 🎯 Best Practices

### 1. Instrument Key Operations

Focus on instrumenting:
- ✅ Service layer methods (create, update, delete)
- ✅ External API calls (HTTP, gRPC)
- ✅ Database queries (SELECT, INSERT, UPDATE)
- ✅ Event publishing and handling
- ✅ Cache operations
- ✅ Authentication and authorization

### 2. Use Meaningful Span Names

**Good:**
```rust
#[instrument(name = "create_product")]
#[instrument(name = "fetch_user_by_email")]
#[instrument(name = "process_payment")]
```

**Bad:**
```rust
#[instrument(name = "do_stuff")]
#[instrument(name = "handler")]
#[instrument(name = "main_function")]
```

### 3. Add Context with Attributes

```rust
#[instrument(
    skip(self),
    fields(
        tenant_id = %tenant_id,
        product_sku = %input.sku,
        operation_type = "create"
    )
)]
```

### 4. Record Dynamic Attributes

```rust
let span = tracing::Span::current();
span.record("product_id", &product.id.to_string());
span.record("created_at", &product.created_at.to_rfc3339());
```

### 5. Don't Over-Instrument

Avoid creating spans for:
- ❌ Simple getters/setters
- ❌ Pure computations (no I/O)
- ❌ Very high-frequency operations (>1000 RPS per instance)

### 6. Handle Errors in Spans

```rust
use tracing::error;

#[instrument]
pub async fn risky_operation(&self) -> Result<()> {
    match self.do_something().await {
        Ok(result) => {
            tracing::info!("Operation succeeded");
            Ok(result)
        }
        Err(e) => {
            // Record error in span
            tracing::error!(error = %e, "Operation failed");
            Err(e)
        }
    }
}
```

### 7. Propagate Context

Always propagate trace context when:
- Making HTTP requests (use `reqwest` with tracing)
- Publishing events (attach trace_id to metadata)
- Spawning background tasks (use `Instrument` trait)

```rust
use tracing::Instrument;

// Spawn task with current span context
tokio::spawn(
    async move {
        // This work is traced as a child of the current span
        background_work().await
    }
    .instrument(tracing::Span::current())
);
```

## 🔍 Common Issues

### Issue: No traces in Jaeger

**Checklist:**
1. ✅ Is `OTEL_ENABLED=true`?
2. ✅ Is Jaeger running on port 4317?
3. ✅ Is `OTEL_EXPORTER_OTLP_ENDPOINT` correct?
4. ✅ Check server logs for OTel initialization
5. ✅ Is sampling rate > 0?

### Issue: Traces are incomplete

**Possible causes:**
- Not all operations are instrumented
- Spans are dropped due to queue overflow (increase `max_queue_size`)
- Sampling rate is too low for testing

### Issue: High overhead

**Solutions:**
- Reduce sampling rate in production
- Increase batch export delay
- Remove instrumentation from hot paths
- Use `tracing::instrument(skip_all)` for methods with many arguments

## 📈 Performance Impact

### Overhead

| Scenario | CPU Overhead | Latency Overhead |
|----------|--------------|------------------|
| 100% sampling | 2-5% | <1ms per span |
| 10% sampling | <1% | <0.1ms avg |
| Disabled | 0% | 0ms |

### Recommendations

- **Development:** 100% sampling
- **Staging:** 50% sampling
- **Production (low traffic):** 10-25% sampling
- **Production (high traffic):** 1-5% sampling

## 🚀 Next Steps

1. **Task 3.2:** Add distributed tracing for event flows
   - Propagate trace context through EventEnvelope
   - Visualize event chains in Jaeger
   
2. **Task 3.3:** Create Grafana dashboards
   - Trace metrics (latency, error rate)
   - Service dependencies
   - RED metrics (Rate, Errors, Duration)

## 📚 References

- [OpenTelemetry Rust SDK](https://docs.rs/opentelemetry)
- [Tracing Subscriber](https://docs.rs/tracing-subscriber)
- [Semantic Conventions](https://opentelemetry.io/docs/specs/semconv/)
- [Jaeger Documentation](https://www.jaegertracing.io/docs/)
- [Grafana Tempo](https://grafana.com/docs/tempo/latest/)

---

**Last Updated:** 2026-02-13  
**Next Review:** After Sprint 3 completion
