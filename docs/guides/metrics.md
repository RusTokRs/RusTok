# 📊 Metrics Dashboard Guide

> **Complete guide to RusToK custom metrics, Grafana dashboards, and alerting**

## 📋 Table of Contents

1. [Overview](#overview)
2. [Custom Metrics](#custom-metrics)
3. [Grafana Dashboards](#grafana-dashboards)
4. [Alert Rules](#alert-rules)
5. [Usage Examples](#usage-examples)
6. [Best Practices](#best-practices)

---

## Overview

RusToK provides comprehensive custom metrics for monitoring application health, performance, and behavior. These metrics are:

- **Exported** via Prometheus format (`/api/_health/metrics`)
- **Collected** by Prometheus every 15s
- **Visualized** in Grafana dashboards
- **Alerted** based on SLO thresholds

### Architecture

```
┌─────────────┐       ┌────────────┐       ┌──────────┐
│   RusToK    │──────▶│ Prometheus │──────▶│ Grafana  │
│   Server    │       │ (scrape)   │       │ (visuali)│
└─────────────┘       └────────────┘       └──────────┘
      │                     │
      │                     ▼
      │               ┌──────────┐
      └──────────────▶│ Jaeger   │
         (traces)     │ (traces) │
                      └──────────┘
```

---

## Custom Metrics

### EventBus Metrics

Track event processing, throughput, and queue health.

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `rustok_event_bus_published_total` | Counter | `event_type`, `tenant_id` | Total events published |
| `rustok_event_bus_dispatched_total` | Counter | `event_type`, `handler` | Total events dispatched to handlers |
| `rustok_event_bus_queue_depth` | Gauge | `transport` | Current event queue depth |
| `rustok_event_bus_processing_duration_seconds` | Histogram | `event_type`, `handler` | Event processing duration |
| `rustok_event_bus_errors_total` | Counter | `event_type`, `error_type` | Event processing errors |
| `rustok_event_bus_lag_seconds` | Histogram | `event_type` | Time between publish and processing |

**Example Usage:**

```rust
use rustok_telemetry::metrics;

// Record event publication
metrics::record_event_published("ProductCreated", &tenant_id.to_string());

// Record event dispatch
metrics::record_event_dispatched("ProductCreated", "index_handler");

// Update queue depth
metrics::update_queue_depth("in_memory", 42);

// Record processing duration
let start = std::time::Instant::now();
// ... process event ...
metrics::record_event_processing_duration(
    "ProductCreated", 
    "index_handler", 
    start.elapsed().as_secs_f64()
);
```

### Circuit Breaker Metrics

Monitor circuit breaker states and health.

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `rustok_circuit_breaker_state` | Gauge | `service` | State (0=closed, 1=open, 2=half-open) |
| `rustok_circuit_breaker_transitions_total` | Counter | `service`, `from_state`, `to_state` | State transitions |
| `rustok_circuit_breaker_calls_total` | Counter | `service`, `result` | Calls (success/failure/rejected) |
| `rustok_circuit_breaker_failures` | Gauge | `service` | Current failure count |

**Example Usage:**

```rust
use rustok_telemetry::metrics;

// Update state (0=closed, 1=open, 2=half-open)
metrics::update_circuit_breaker_state("redis", 0);

// Record transition
metrics::record_circuit_breaker_transition("redis", "closed", "open");

// Record call result
metrics::record_circuit_breaker_call("redis", "success");

// Update failure count
metrics::update_circuit_breaker_failures("redis", 3);
```

### Cache Metrics

Track cache hit rates, size, and evictions.

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `rustok_cache_operations_total` | Counter | `cache`, `operation`, `result` | Cache operations (hit/miss) |
| `rustok_cache_hit_rate` | Gauge | `cache` | Cache hit rate (0.0-1.0) |
| `rustok_cache_size` | Gauge | `cache` | Current cache entries |
| `rustok_cache_evictions_total` | Counter | `cache`, `reason` | Cache evictions |
| `rustok_cache_operation_duration_seconds` | Histogram | `cache`, `operation` | Operation duration |

**Example Usage:**

```rust
use rustok_telemetry::metrics;

// Record cache hit
metrics::record_cache_operation("tenant_cache", "get", "hit");

// Record cache miss
metrics::record_cache_operation("tenant_cache", "get", "miss");

// Update cache size
metrics::update_cache_size("tenant_cache", 1234);

// Record eviction
metrics::record_cache_eviction("tenant_cache", "ttl");
```

### Span/Trace Metrics

Track distributed tracing metrics.

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `rustok_spans_created_total` | Counter | `operation`, `tenant_id` | Total spans created |
| `rustok_span_duration_seconds` | Histogram | `operation` | Span duration |
| `rustok_spans_with_errors_total` | Counter | `operation`, `error_type` | Spans with errors |

### Module Error Metrics

Track errors by module.

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `rustok_module_errors_total` | Counter | `module`, `error_type`, `severity` | Errors by module |
| `rustok_module_error_rate` | Gauge | `module` | Error rate (errors/sec) |

### Module Entrypoint Adoption Metrics

Track whether key module entry points go through rustok libraries or bypass paths.

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `rustok_module_entrypoint_calls_total` | Counter | `module`, `entry_point`, `path` | Calls split by `path=library|bypass` |

**PromQL examples:**

```promql
# Library adoption rate by module (0..100)
100 *
sum by (module) (increase(rustok_module_entrypoint_calls_total{path="library"}[7d]))
/
clamp_min(
  sum by (module) (increase(rustok_module_entrypoint_calls_total{path=~"library|bypass"}[7d])),
  1
)

# New bypass points in last 7 days
sum by (module, entry_point) (increase(rustok_module_entrypoint_calls_total{path="bypass"}[7d])) > 0
```

### Database Metrics

Monitor database performance.

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `rustok_database_query_duration_seconds` | Histogram | `query_type`, `table` | Query duration |
| `rustok_database_connections` | Gauge | `state` | Active/idle connections |
| `rustok_database_query_errors_total` | Counter | `query_type`, `error_type` | Query errors |

### HTTP/API Metrics (Enhanced)

Extended HTTP metrics.

| Metric | Type | Labels | Description |
|--------|------|--------|-------------|
| `rustok_http_requests_by_endpoint` | Counter | `method`, `endpoint`, `status` | Requests by endpoint |
| `rustok_http_request_size_bytes` | Histogram | `method`, `endpoint` | Request size |
| `rustok_http_response_size_bytes` | Histogram | `method`, `endpoint` | Response size |
| `rustok_http_active_connections` | Gauge | - | Active connections |

---

## Grafana Dashboards

### Overview Dashboard

**File:** `grafana/dashboards/rustok-overview.json`

**Panels:**
- HTTP request rate
- Request latency (P50, P95, P99)
- Error rate
- Active connections
- Top endpoints
- Status code distribution
- Response times heatmap

### Advanced Dashboard

**File:** `grafana/dashboards/rustok-advanced.json`

**Panels (13 total):**

1. **Request Rate** (Stat) - Real-time request rate
2. **P95 Latency** (Stat) - 95th percentile latency
3. **Error Rate** (Stat) - HTTP 5xx error rate
4. **Circuit Breaker Status** (Stat) - Circuit breaker states
5. **HTTP Request Rate by Endpoint** (Time Series) - Per-endpoint traffic
6. **HTTP Request Latency** (Time Series) - P50, P95, P99 over time
7. **EventBus Throughput** (Time Series) - Events published per second
8. **EventBus Queue Depth** (Time Series) - Event queue size
9. **Cache Hit Rate** (Time Series) - Cache hit rate by cache
10. **Cache Size** (Time Series) - Cache entries over time
11. **Error Rate by Module** (Time Series) - Errors per module
12. **Database Query Duration** (Time Series) - P95 query latency
13. **Database Connections** (Time Series) - Active/idle connections

**Access:** http://localhost:3000 (admin/admin)

### Creating Custom Dashboards

1. Open Grafana (http://localhost:3000)
2. Click **+ Create → Dashboard**
3. Add Panel → Select Prometheus datasource
4. Enter PromQL query:

```promql
# HTTP request rate
rate(rustok_http_requests_total[5m])

# P95 latency by endpoint
histogram_quantile(0.95, rate(rustok_http_request_duration_seconds_bucket[5m]))

# Cache hit rate
rate(rustok_cache_operations_total{result="hit"}[5m]) / 
(rate(rustok_cache_operations_total{result="hit"}[5m]) + 
 rate(rustok_cache_operations_total{result="miss"}[5m]))

# Event queue depth
rustok_event_bus_queue_depth

# Circuit breaker status
rustok_circuit_breaker_state{service="redis"}
```

---

## Alert Rules

### SLO-Based Alerts

**File:** `prometheus/alert_rules.yml`

#### High Error Rate

```yaml
- alert: HighErrorRate
  expr: |
    (rate(rustok_http_requests_total{status=~"5.."}[5m]) /
     rate(rustok_http_requests_total[5m])) > 0.05
  for: 2m
  labels:
    severity: critical
  annotations:
    summary: "High error rate detected"
    description: "Error rate is {{ $value | humanizePercentage }}"
```

**Threshold:** 5% error rate  
**Duration:** 2 minutes  
**Action:** Critical alert

#### Slow Request Latency

```yaml
- alert: SlowRequestLatency
  expr: |
    histogram_quantile(0.95, 
      rate(rustok_http_request_duration_seconds_bucket[5m])) > 0.5
  for: 5m
  labels:
    severity: warning
```

**Threshold:** 500ms P95 latency  
**Duration:** 5 minutes  
**Action:** Warning

### EventBus Alerts

#### High Queue Depth

```yaml
- alert: HighEventQueueDepth
  expr: rustok_event_bus_queue_depth > 7000
  for: 5m
  labels:
    severity: warning
```

**Threshold:** 7,000 events (70% of max 10,000)  
**Duration:** 5 minutes

#### Critical Queue Depth

```yaml
- alert: CriticalEventQueueDepth
  expr: rustok_event_bus_queue_depth > 10000
  for: 2m
  labels:
    severity: critical
  annotations:
    description: "Risk of memory exhaustion!"
```

**Threshold:** 10,000 events (max capacity)  
**Duration:** 2 minutes

### Circuit Breaker Alerts

#### Circuit Open

```yaml
- alert: CircuitBreakerOpen
  expr: rustok_circuit_breaker_state == 1
  for: 1m
  labels:
    severity: critical
```

**Condition:** Circuit breaker is OPEN  
**Duration:** 1 minute

### Cache Alerts

#### Low Cache Hit Rate

```yaml
- alert: LowCacheHitRate
  expr: |
    (rate(rustok_cache_operations_total{result="hit"}[5m]) /
     (rate(rustok_cache_operations_total{result="hit"}[5m]) +
      rate(rustok_cache_operations_total{result="miss"}[5m]))) < 0.5
  for: 10m
  labels:
    severity: warning
```

**Threshold:** 50% hit rate  
**Duration:** 10 minutes

### Database Alerts

#### Slow Queries

```yaml
- alert: SlowDatabaseQueries
  expr: |
    histogram_quantile(0.95,
      rate(rustok_database_query_duration_seconds_bucket[5m])) > 0.1
  for: 5m
  labels:
    severity: warning
```

**Threshold:** 100ms P95 query duration  
**Duration:** 5 minutes

---

## Usage Examples

### Instrumenting EventBus

```rust
use rustok_telemetry::metrics;
use std::time::Instant;

pub async fn publish_event(&self, event: DomainEvent) -> Result<()> {
    let tenant_id = event.tenant_id().to_string();
    let event_type = event.event_type();
    
    // Record publication
    metrics::record_event_published(event_type, &tenant_id);
    
    // Update queue depth
    let depth = self.queue.len() as i64;
    metrics::update_queue_depth("in_memory", depth);
    
    // Publish event
    self.transport.publish(event).await?;
    
    Ok(())
}

pub async fn dispatch_event(&self, event: DomainEvent) -> Result<()> {
    let event_type = event.event_type();
    let handler_name = "my_handler";
    
    let start = Instant::now();
    
    match self.handler.handle(event).await {
        Ok(_) => {
            metrics::record_event_dispatched(event_type, handler_name);
            metrics::record_event_processing_duration(
                event_type,
                handler_name,
                start.elapsed().as_secs_f64()
            );
        }
        Err(e) => {
            metrics::record_event_error(event_type, "handler_error");
        }
    }
    
    Ok(())
}
```

### Instrumenting Circuit Breaker

```rust
use rustok_telemetry::metrics;

impl CircuitBreaker {
    pub async fn call<F, T>(&self, f: F) -> Result<T>
    where
        F: Future<Output = Result<T>>,
    {
        let service = &self.service_name;
        
        match self.state {
            CircuitState::Closed => {
                metrics::update_circuit_breaker_state(service, 0);
                match f.await {
                    Ok(result) => {
                        metrics::record_circuit_breaker_call(service, "success");
                        Ok(result)
                    }
                    Err(e) => {
                        self.failure_count += 1;
                        metrics::update_circuit_breaker_failures(service, self.failure_count);
                        metrics::record_circuit_breaker_call(service, "failure");
                        
                        if self.failure_count >= self.threshold {
                            self.open_circuit();
                        }
                        Err(e)
                    }
                }
            }
            CircuitState::Open => {
                metrics::update_circuit_breaker_state(service, 1);
                metrics::record_circuit_breaker_call(service, "rejected");
                Err(Error::CircuitBreakerOpen)
            }
            CircuitState::HalfOpen => {
                metrics::update_circuit_breaker_state(service, 2);
                // ... test recovery
            }
        }
    }
    
    fn open_circuit(&mut self) {
        metrics::record_circuit_breaker_transition(
            &self.service_name,
            "closed",
            "open"
        );
        self.state = CircuitState::Open;
    }
}
```

### Instrumenting Cache

```rust
use rustok_telemetry::metrics;
use std::time::Instant;

impl TenantCache {
    pub async fn get(&self, key: &str) -> Option<Arc<Tenant>> {
        let start = Instant::now();
        
        let result = self.cache.get(key).await;
        
        let operation_result = if result.is_some() { "hit" } else { "miss" };
        metrics::record_cache_operation("tenant_cache", "get", operation_result);
        metrics::record_cache_duration(
            "tenant_cache",
            "get",
            start.elapsed().as_secs_f64()
        );
        
        // Update cache size periodically
        if self.should_report_size() {
            let size = self.cache.entry_count() as i64;
            metrics::update_cache_size("tenant_cache", size);
        }
        
        result
    }
    
    pub async fn evict(&self, key: &str, reason: &str) {
        self.cache.invalidate(key).await;
        metrics::record_cache_eviction("tenant_cache", reason);
    }
}
```

---

## Best Practices

### 1. Metric Naming

Follow Prometheus naming conventions:

```
<namespace>_<subsystem>_<name>_<unit>

✅ Good:
- rustok_event_bus_published_total
- rustok_http_request_duration_seconds
- rustok_cache_hit_rate

❌ Bad:
- events_published
- request_time_ms
- cache_hits
```

### 2. Label Cardinality

Keep label cardinality low to avoid memory issues:

```rust
// ✅ Good: Low cardinality
metrics::record_event_published("ProductCreated", &tenant_id);

// ❌ Bad: High cardinality (unique per user)
metrics::record_event_published("ProductCreated", &user_id);
```

**Rule of thumb:** Total unique label combinations < 10,000

### 3. Metric Types

Choose the right metric type:

| Type | Use Case | Example |
|------|----------|---------|
| **Counter** | Cumulative values that only increase | Requests, errors, events |
| **Gauge** | Values that go up and down | Queue depth, connections, cache size |
| **Histogram** | Distribution of values | Latency, request size, duration |

### 4. Recording Frequency

Balance overhead vs accuracy:

```rust
// ✅ Good: Record on every operation (low overhead)
metrics::record_event_published(event_type, tenant_id);

// ⚠️ Caution: Expensive operations (batch or sample)
if sample_rate() {
    metrics::update_cache_size("cache", size);
}
```

### 5. Alert Thresholds

Set thresholds based on SLOs:

```yaml
# P95 latency SLO: 500ms
- alert: SlowRequests
  expr: histogram_quantile(0.95, ...) > 0.5
  
# Error rate SLO: 1%
- alert: HighErrorRate
  expr: error_rate > 0.01
  
# Availability SLO: 99.9%
- alert: ServiceDown
  expr: up{job="rustok-server"} == 0
```

### 6. Dashboard Organization

Structure dashboards by audience:

- **Overview:** Executive summary (4 key metrics)
- **Service:** Per-service deep dive
- **Infrastructure:** Database, cache, queues
- **Debugging:** Traces, logs, errors

---

## Quick Start

### 1. Start Observability Stack

```bash
docker-compose -f docker-compose.observability.yml up -d
```

### 2. Run RusToK Server

```bash
cargo run -p rustok-server
```

### 3. Access Dashboards

- **Grafana:** http://localhost:3000 (admin/admin)
- **Prometheus:** http://localhost:9090
- **Jaeger:** http://localhost:16686

### 4. View Metrics

```bash
# Prometheus metrics endpoint
curl http://localhost:3000/api/_health/metrics

# Example output:
# rustok_event_bus_published_total{event_type="ProductCreated",tenant_id="..."} 42
# rustok_http_requests_total{method="POST",path="/graphql",status="200"} 1234
```

### 5. Check Alerts

Open Prometheus → Alerts: http://localhost:9090/alerts

---

## References

- [Prometheus Best Practices](https://prometheus.io/docs/practices/)
- [Grafana Dashboards](https://grafana.com/docs/grafana/latest/dashboards/)
- [OpenTelemetry Metrics](https://opentelemetry.io/docs/concepts/signals/metrics/)
- [SLO-based Alerting](https://sre.google/workbook/alerting-on-slos/)

---

**Task 3.3: Metrics Dashboard ✅ Complete**

### Weekly module adoption report

Use the helper script to build a weekly report artifact from `/metrics`:

```bash
scripts/module_path_adoption_report.sh \
  --metrics-url http://localhost:5150/metrics \
  --baseline artifacts/verification/module_path_snapshot_prev.json \
  --snapshot-out artifacts/verification/module_path_snapshot_current.json \
  --out artifacts/verification/module_path_weekly.md \
  --window weekly
```

The report includes:
- `% scenarios through rustok libraries` by module
- `library vs bypass` ratio by entry point
- new bypass points vs baseline snapshot

Recommended use: attach this report to the platform verification cycle evidence bundle.
