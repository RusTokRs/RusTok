# Grafana Setup Guide for RusToK

This guide explains how to set up Grafana to monitor your RusToK platform using Prometheus metrics.

## Table of Contents
- [Prerequisites](#prerequisites)
- [Quick Start with Docker](#quick-start-with-docker)
- [Prometheus Configuration](#prometheus-configuration)
- [Grafana Setup](#grafana-setup)
- [Importing Dashboard](#importing-dashboard)
- [Available Metrics](#available-metrics)
- [Alert Rules](#alert-rules)

---

## Prerequisites

- RusToK server running with metrics enabled
- Docker and Docker Compose (recommended) OR
- Prometheus and Grafana installed locally

---

## Quick Start with Docker

### 1. Create docker-compose.yml for monitoring stack

```yaml
version: '3.8'

services:
  prometheus:
    image: prom/prometheus:latest
    container_name: rustok-prometheus
    volumes:
      - ./prometheus.yml:/etc/prometheus/prometheus.yml
      - prometheus-data:/prometheus
    command:
      - '--config.file=/etc/prometheus/prometheus.yml'
      - '--storage.tsdb.path=/prometheus'
      - '--web.console.libraries=/usr/share/prometheus/console_libraries'
      - '--web.console.templates=/usr/share/prometheus/consoles'
    ports:
      - "9090:9090"
    restart: unless-stopped

  grafana:
    image: grafana/grafana:latest
    container_name: rustok-grafana
    volumes:
      - grafana-data:/var/lib/grafana
      - ./grafana/provisioning:/etc/grafana/provisioning
    environment:
      - GF_SECURITY_ADMIN_PASSWORD=admin
      - GF_USERS_ALLOW_SIGN_UP=false
    ports:
      - "3000:3000"
    depends_on:
      - prometheus
    restart: unless-stopped

volumes:
  prometheus-data:
  grafana-data:
```

### 2. Start the monitoring stack

```bash
docker-compose up -d
```

Access:
- Prometheus: http://localhost:9090
- Grafana: http://localhost:3000 (admin/admin)

---

## Prometheus Configuration

Create `prometheus.yml` in your project root:

```yaml
global:
  scrape_interval: 15s
  evaluation_interval: 15s
  external_labels:
    cluster: 'rustok-dev'
    environment: 'development'

scrape_configs:
  - job_name: 'rustok'
    metrics_path: '/metrics'
    static_configs:
      - targets: ['host.docker.internal:5150']  # Adjust port if needed
        labels:
          service: 'rustok-server'
          
    # If running multiple instances
    # - targets: 
    #   - 'rustok-1:5150'
    #   - 'rustok-2:5150'
    #   - 'rustok-3:5150'
    
  # Optional: Prometheus self-monitoring
  - job_name: 'prometheus'
    static_configs:
      - targets: ['localhost:9090']
```

**Note**: Replace `host.docker.internal:5150` with your RusToK server address.

---

## Grafana Setup

### 1. Login to Grafana

Navigate to http://localhost:3000 and login with:
- Username: `admin`
- Password: `admin` (change on first login)

### 2. Add Prometheus Data Source

1. Click **Configuration** (⚙️) → **Data Sources**
2. Click **Add data source**
3. Select **Prometheus**
4. Configure:
   - **Name**: RusToK Prometheus
   - **URL**: `http://prometheus:9090` (Docker) or `http://localhost:9090` (local)
   - **Access**: Server (default)
5. Click **Save & Test**

### 3. Import Dashboard

#### Option A: Import from JSON file

1. Click **Dashboards** (📊) → **Import**
2. Click **Upload JSON file**
3. Select `docs/grafana-dashboard-example.json`
4. Select data source: **RusToK Prometheus**
5. Click **Import**

#### Option B: Import from Grafana.com (future)

Once published to Grafana.com:
```
Dashboard ID: [TBD]
```

---

## Importing Dashboard

The example dashboard (`grafana-dashboard-example.json`) includes:

### Panels Overview

1. **Content Operations** - Rate of content CRUD operations by type and status
2. **Content Operation Duration (p95)** - 95th percentile latency for content ops
3. **Commerce Operations** - Rate of product/catalog operations
4. **Commerce Operation Duration (p95)** - 95th percentile latency for commerce ops
5. **HTTP Requests** - Incoming HTTP request rate by method, path, and status
6. **HTTP Request Duration (p95)** - 95th percentile HTTP response time
7. **Tenant Cache Stats** - Cache hit rate as percentage
8. **Tenant Cache Entries** - Number of cached tenant entries
9. **Active Connections** - Current active HTTP connections
10. **Error Rate** - Percentage of 5xx responses

### Dashboard Features

- **Auto-refresh**: 10 second intervals
- **Time range selector**: Adjustable time window
- **Variables**: Can be extended with tenant_id, environment filters
- **Templating ready**: Easy to clone and customize

---

## Available Metrics

### Content Module Metrics

```promql
# Operations counter
rustok_content_operations_total{operation="create_node", kind="post", status="success"}

# Operation duration histogram
rustok_content_operation_duration_seconds_bucket{operation="create_node", kind="post", le="0.1"}
rustok_content_operation_duration_seconds_count{operation="create_node", kind="post"}
rustok_content_operation_duration_seconds_sum{operation="create_node", kind="post"}

# Business metrics
rustok_content_nodes_total
```

### Commerce Module Metrics

```promql
# Operations counter
rustok_commerce_operations_total{operation="create_product", status="success"}

# Operation duration histogram
rustok_commerce_operation_duration_seconds_bucket{operation="create_product", le="0.1"}
rustok_commerce_operation_duration_seconds_count{operation="create_product"}
rustok_commerce_operation_duration_seconds_sum{operation="create_product"}

# Business metrics
rustok_commerce_products_total
rustok_commerce_orders_total
```

### HTTP Metrics

```promql
# Request counter
rustok_http_requests_total{method="GET", path="/api/content", status="200"}

# Request duration histogram
rustok_http_request_duration_seconds_bucket{method="GET", path="/api/content", le="0.1"}

# Active connections gauge
rustok_http_active_connections
```

### Tenant Cache Metrics

```promql
# Cache statistics
rustok_tenant_cache_hits
rustok_tenant_cache_misses
rustok_tenant_cache_evictions
rustok_tenant_cache_entries
rustok_tenant_cache_negative_hits
rustok_tenant_cache_negative_misses
rustok_tenant_cache_negative_evictions
rustok_tenant_cache_negative_entries
rustok_tenant_cache_negative_inserts
```

---

## Useful PromQL Queries

### Request Rate (QPS)

```promql
# Total requests per second
rate(rustok_http_requests_total[5m])

# Requests per second by endpoint
sum(rate(rustok_http_requests_total[5m])) by (path)
```

### Error Rate

```promql
# 5xx error rate as percentage
rate(rustok_http_requests_total{status=~"5.."}[5m]) 
  / 
rate(rustok_http_requests_total[5m]) * 100

# 4xx error rate
rate(rustok_http_requests_total{status=~"4.."}[5m]) 
  / 
rate(rustok_http_requests_total[5m]) * 100
```

### Latency Percentiles

```promql
# p50 (median) latency
histogram_quantile(0.5, rate(rustok_http_request_duration_seconds_bucket[5m]))

# p95 latency
histogram_quantile(0.95, rate(rustok_http_request_duration_seconds_bucket[5m]))

# p99 latency
histogram_quantile(0.99, rate(rustok_http_request_duration_seconds_bucket[5m]))
```

### Cache Performance

```promql
# Cache hit rate
rustok_tenant_cache_hits / (rustok_tenant_cache_hits + rustok_tenant_cache_misses)

# Cache eviction rate
rate(rustok_tenant_cache_evictions[5m])
```

### Business Metrics

```promql
# Content nodes created per hour
increase(rustok_content_nodes_total[1h])

# Products created per day
increase(rustok_commerce_products_total[24h])

# Orders created per hour
increase(rustok_commerce_orders_total[1h])
```

---

## Alert Rules

Create `prometheus-alerts.yml`:

```yaml
groups:
  - name: rustok_alerts
    interval: 30s
    rules:
      # High error rate
      - alert: HighErrorRate
        expr: |
          rate(rustok_http_requests_total{status=~"5.."}[5m]) 
            / 
          rate(rustok_http_requests_total[5m]) > 0.05
        for: 5m
        labels:
          severity: critical
        annotations:
          summary: "High error rate detected"
          description: "Error rate is {{ $value | humanizePercentage }} (threshold: 5%)"
      
      # High latency
      - alert: HighLatency
        expr: |
          histogram_quantile(0.95, 
            rate(rustok_http_request_duration_seconds_bucket[5m])
          ) > 1.0
        for: 10m
        labels:
          severity: warning
        annotations:
          summary: "High p95 latency detected"
          description: "p95 latency is {{ $value }}s (threshold: 1s)"
      
      # Low cache hit rate
      - alert: LowCacheHitRate
        expr: |
          rustok_tenant_cache_hits 
            / 
          (rustok_tenant_cache_hits + rustok_tenant_cache_misses) < 0.7
        for: 15m
        labels:
          severity: warning
        annotations:
          summary: "Low tenant cache hit rate"
          description: "Cache hit rate is {{ $value | humanizePercentage }} (threshold: 70%)"
      
      # Service down
      - alert: ServiceDown
        expr: up{job="rustok"} == 0
        for: 1m
        labels:
          severity: critical
        annotations:
          summary: "RusToK service is down"
          description: "Instance {{ $labels.instance }} is unreachable"
```

Add to `prometheus.yml`:

```yaml
rule_files:
  - 'prometheus-alerts.yml'

alerting:
  alertmanagers:
    - static_configs:
        - targets: ['alertmanager:9093']  # If using Alertmanager
```

---

## Advanced Configuration

### Multi-Environment Setup

Use Prometheus relabeling to add environment labels:

```yaml
scrape_configs:
  - job_name: 'rustok-prod'
    static_configs:
      - targets: ['prod-server-1:5150', 'prod-server-2:5150']
        labels:
          environment: production
          
  - job_name: 'rustok-staging'
    static_configs:
      - targets: ['staging-server:5150']
        labels:
          environment: staging
```

### Grafana Variables

Add template variables to filter by environment/tenant:

```
Name: environment
Type: Query
Query: label_values(rustok_http_requests_total, environment)

Name: tenant_id
Type: Query
Query: label_values(rustok_http_requests_total{environment="$environment"}, tenant_id)
```

### High Availability

For production, run multiple Prometheus instances with:
- Remote write to long-term storage (Thanos, Cortex, VictoriaMetrics)
- Grafana pointing to load-balanced query layer
- Alertmanager cluster for HA alerting

---

## Troubleshooting

### Metrics not showing up

1. Check RusToK server logs for metrics initialization:
   ```bash
   grep -i "metrics" /var/log/rustok/server.log
   ```

2. Verify `/metrics` endpoint is accessible:
   ```bash
   curl http://localhost:5150/metrics
   ```

3. Check Prometheus targets page: http://localhost:9090/targets

### Dashboard shows "No Data"

1. Verify Prometheus data source is configured correctly
2. Check time range selector (try "Last 15 minutes")
3. Verify metrics are being scraped: http://localhost:9090/graph

### High cardinality warnings

If you see "too many time series" warnings:
- Reduce scrape frequency
- Add metric relabeling to drop unused labels
- Implement metric aggregation

---

## Next Steps

1. **Customize Dashboard** - Add panels specific to your use cases
2. **Set Up Alerts** - Configure Alertmanager for notifications
3. **Long-term Storage** - Implement remote write for data retention
4. **Performance Tuning** - Adjust scrape intervals and retention policies
5. **Security** - Enable authentication and TLS for production

---

## Resources

- [Prometheus Documentation](https://prometheus.io/docs/)
- [Grafana Documentation](https://grafana.com/docs/)
- [PromQL Cheat Sheet](https://promlabs.com/promql-cheat-sheet/)
- [RusToK Module Metrics Guide](./guides/module-metrics.md)

---

## Support

For issues or questions:
- Check existing metrics documentation in `docs/guides/module-metrics.md`
- Review structured logging guide in `docs/standards/logging.md`
- Open an issue in the project repository
