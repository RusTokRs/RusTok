# 🔍 Observability Quick Start

> **Status:** Sprint 3 Task 3.1 Complete ✅  
> **Updated:** 2026-02-12

Quick guide to get started with RusToK observability stack.

---

## 🚀 Quick Start (5 minutes)

### 1. Start Observability Stack

```bash
# Start Jaeger, Prometheus, and Grafana
docker-compose -f docker-compose.observability.yml up -d

# Check services are running
docker-compose -f docker-compose.observability.yml ps
```

### 2. Run RusToK Server with Tracing

```bash
# Set OpenTelemetry endpoint
export OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317
export OTEL_SERVICE_NAME=rustok-server
export OTEL_ENABLED=true

# Run server (when Rust is set up)
cargo run -p rustok-server
```

### 3. Access Dashboards

Open in your browser:

| Service | URL | Credentials |
|---------|-----|-------------|
| **Jaeger UI** | http://localhost:16686 | None |
| **Prometheus** | http://localhost:9090 | None |
| **Grafana** | http://localhost:3000 | admin / admin |

---

## 📊 What You Get

### Jaeger (Distributed Tracing)
- **Service Map:** Visualize service dependencies
- **Trace Timeline:** See request flow across services
- **Performance:** Identify slow operations
- **Errors:** Track errors with full context

### Prometheus (Metrics)
- **HTTP Metrics:** Request rate, duration, errors
- **Content Metrics:** Operations, node count
- **Commerce Metrics:** Orders, products
- **System Metrics:** Container and host metrics

### Grafana (Unified Dashboard)
- **RusToK Overview Dashboard:**
  - HTTP request rate and P95 latency
  - Request status distribution (2xx, 4xx, 5xx)
  - Content operations rate
  - Commerce operations rate
  - Total nodes and products

---

## 🔧 Configuration

### Environment Variables

```bash
# Required
OTEL_EXPORTER_OTLP_ENDPOINT=http://localhost:4317  # OTLP collector endpoint
OTEL_SERVICE_NAME=rustok-server                    # Service identifier

# Optional
OTEL_SERVICE_VERSION=0.1.0                          # Service version
OTEL_SAMPLING_RATE=1.0                              # 1.0 = 100% sampling
OTEL_ENABLED=true                                   # Enable/disable tracing
RUST_ENV=development                                # Environment name
```

### Docker Compose Ports

```yaml
Jaeger:
  - 16686: UI
  - 4317:  OTLP gRPC (OpenTelemetry)
  - 4318:  OTLP HTTP

Prometheus:
  - 9090:  UI and API

Grafana:
  - 3000:  UI

cAdvisor:
  - 8080:  Container metrics

Node Exporter:
  - 9100:  Host metrics
```

---

## 📝 Usage Examples

### Rust Code

```rust
use rustok_telemetry::otel::{OtelConfig, init_tracing};

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load config from environment
    let config = OtelConfig::from_env();
    
    // Initialize OpenTelemetry tracing
    init_tracing(config).await?;
    
    // Your application code
    tracing::info!("Server starting...");
    
    // Run your server...
    
    // Shutdown gracefully (flushes pending spans)
    rustok_telemetry::otel::shutdown().await;
    
    Ok(())
}
```

### Creating Spans

```rust
use tracing::{info_span, info};

// Create a span
let span = info_span!("fetch_user", user_id = %user_id, tenant_id = %tenant_id);
let _guard = span.enter();

// Log within span
info!("Fetching user from database");

// Span automatically closes when guard drops
```

### Querying Traces in Jaeger

1. Open http://localhost:16686
2. Select "rustok-server" from service dropdown
3. Click "Find Traces"
4. Click on a trace to see detailed timeline

### Querying Metrics in Prometheus

1. Open http://localhost:9090
2. Enter a PromQL query:
   ```promql
   # HTTP request rate
   rate(rustok_http_requests_total[5m])
   
   # P95 latency
   histogram_quantile(0.95, sum(rate(rustok_http_request_duration_seconds_bucket[5m])) by (le))
   
   # Error rate
   sum(rate(rustok_http_requests_total{status=~"5.."}[5m]))
   ```

---

## 🎨 Grafana Dashboard

### Access Pre-configured Dashboard

1. Open http://localhost:3000
2. Login with `admin` / `admin`
3. Go to **Dashboards** → **RusToK** folder
4. Open **RusToK Overview**

### Dashboard Panels

1. **HTTP Request Rate** - Requests per second over time
2. **HTTP Request Duration (P95)** - 95th percentile latency gauge
3. **HTTP Requests by Status** - Pie chart of status codes
4. **Content Operations Rate** - Content ops per second
5. **Commerce Operations Rate** - Commerce ops per second
6. **Total Content Nodes** - Current node count
7. **Total Products** - Current product count

---

## 🛠️ Troubleshooting

### Services Not Starting

```bash
# Check Docker logs
docker-compose -f docker-compose.observability.yml logs

# Restart specific service
docker-compose -f docker-compose.observability.yml restart jaeger
```

### No Traces in Jaeger

**Check:**
1. Is OTLP endpoint correct? (`http://localhost:4317`)
2. Is `OTEL_ENABLED=true`?
3. Are spans being created in your code?
4. Check server logs for OTel errors

```bash
# Test OTLP connection
curl http://localhost:4317
# Should return gRPC error (means service is up)
```

### No Metrics in Prometheus

**Check:**
1. Is metrics endpoint accessible?
   ```bash
   curl http://localhost:3000/api/_health/metrics
   ```
2. Check Prometheus targets:
   - Open http://localhost:9090/targets
   - Status should be "UP"

### Grafana Dashboard Empty

**Check:**
1. Are datasources configured?
   - **Settings** → **Data Sources**
   - Should see Prometheus and Jaeger
2. Is Prometheus scraping metrics?
   - Check Prometheus UI
3. Is time range correct?
   - Try "Last 1 hour" or "Last 6 hours"

---

## 🧹 Cleanup

### Stop Services

```bash
# Stop but keep data
docker-compose -f docker-compose.observability.yml stop

# Stop and remove containers
docker-compose -f docker-compose.observability.yml down

# Stop, remove, and delete data
docker-compose -f docker-compose.observability.yml down -v
```

---

## 📚 Next Steps

### Task 3.2: Distributed Tracing (Coming Soon)
- Add spans to HTTP handlers
- Add spans to GraphQL resolvers
- Add spans to EventBus
- Add spans to database queries

### Task 3.3: Metrics Dashboard (Coming Soon)
- Custom Prometheus metrics
- Alert rules for SLOs
- Additional Grafana dashboards

---

## 📖 Documentation

### Internal Docs
- [SPRINT_3_START.md](./SPRINT_3_START.md) - Sprint 3 overview
- [crates/rustok-telemetry/src/otel.rs](./crates/rustok-telemetry/src/otel.rs) - Implementation

### External Resources
- [OpenTelemetry Rust](https://docs.rs/opentelemetry/)
- [Jaeger Documentation](https://www.jaegertracing.io/docs/)
- [Prometheus Documentation](https://prometheus.io/docs/)
- [Grafana Documentation](https://grafana.com/docs/)

---

## 🎯 Features Status

| Feature | Status | Description |
|---------|--------|-------------|
| OpenTelemetry Pipeline | ✅ Complete | OTLP export to Jaeger |
| Jaeger UI | ✅ Complete | Distributed tracing visualization |
| Prometheus Scraping | ✅ Complete | Metrics collection |
| Grafana Dashboards | ✅ Complete | Unified observability dashboard |
| HTTP Tracing | 📋 Planned | Task 3.2 |
| GraphQL Tracing | 📋 Planned | Task 3.2 |
| EventBus Tracing | 📋 Planned | Task 3.2 |
| Custom Metrics | 📋 Planned | Task 3.3 |
| Alert Rules | 📋 Planned | Task 3.3 |

---

**Status:** Task 3.1 Complete ✅  
**Sprint 3 Progress:** 33% (1/3 tasks)  
**Overall Progress:** 56% (9/16 tasks)

---

## 📈 Outbox Reliability Metrics & Alerts

Для event reliability трека добавьте наблюдение за outbox/DLQ метриками:

- `rustok_outbox_backlog_size` (gauge) — размер очереди необработанных событий.
- `rustok_outbox_retries_total` (counter) — количество retry попыток relay.
- `rustok_outbox_dlq_total` (counter) — количество событий, попавших в DLQ.

Старые имена `outbox_backlog_size`, `outbox_retries_total` и `outbox_dlq_total`
пока отдаются как compatibility aliases. Новые dashboard/runbook правила должны
использовать только `rustok_*` имена.

### Recommended PromQL

```promql
# Backlog current size
rustok_outbox_backlog_size

# Retry rate (5m)
rate(rustok_outbox_retries_total[5m])

# DLQ growth (15m)
increase(rustok_outbox_dlq_total[15m])
```

### Alerting policy (baseline)

- **Warning:** `rustok_outbox_backlog_size > 500` в течение 10 минут.
- **Critical:** `rustok_outbox_backlog_size > 2000` в течение 15 минут.
- **Warning:** `rate(rustok_outbox_retries_total[5m])` стабильно выше baseline 30 минут.
- **Critical:** `increase(rustok_outbox_dlq_total[15m]) > 0` для production tenant'ов.

### Triage checklist

1. Проверить downstream transport (`iggy`/subscriber availability).
2. Проверить ошибки сериализации/схемы в relay logs.
3. Проверить рост DLQ по конкретным `event_type`.
4. При необходимости запустить контролируемый DLQ replay после устранения root cause.
