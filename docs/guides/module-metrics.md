# Module Metrics Guide

This guide explains the Prometheus metrics implementation in RusToK.

## Overview

RusToK uses Prometheus metrics for observability and monitoring of all modules. The `rustok-telemetry` crate provides a centralized metrics registry.

## Available Metrics

### Content Module (`rustok-content`)

#### Operations Counter
```
rustok_content_operations_total{operation, kind, status}
```
Tracks all content operations (create, update, delete, publish).

- `operation`: The type of operation (create, update, delete, publish, unpublish)
- `kind`: The content kind (post, page, article, comment)
- `status`: success or error

#### Operation Duration
```
rustok_content_operation_duration_seconds{operation, kind}
```
Histogram of operation durations in seconds.

#### Total Nodes Gauge
```
rustok_content_nodes_total
```
Current total number of content nodes.

### Commerce Module (`rustok-commerce`)

#### Operations Counter
```
rustok_commerce_operations_total{operation, kind, status}
```
Tracks all commerce operations (create_product, update_order, etc.).

- `operation`: The type of operation (create_product, update_order, place_order)
- `kind`: The commerce entity type (product, variant, order, price)
- `status`: success or error

#### Operation Duration
```
rustok_commerce_operation_duration_seconds{operation, kind}
```
Histogram of commerce operation durations.

#### Products Gauge
```
rustok_commerce_products_total
```
Current total number of products.

#### Orders Gauge
```
rustok_commerce_orders_total
```
Current total number of orders.

### HTTP Metrics

#### Requests Counter
```
rustok_http_requests_total{method, path, status}
```
- `method`: HTTP method (GET, POST, PUT, DELETE)
- `path`: Request path (/api/content/nodes, /store/products)
- `status`: HTTP status code (200, 201, 400, 404, 500)

#### Request Duration
```
rustok_http_request_duration_seconds{method, path}
```
Histogram of HTTP request durations.

## Usage in Services

### Recording Operations

```rust
use rustok_telemetry::{
    CONTENT_OPERATIONS_TOTAL, CONTENT_OPERATION_DURATION_SECONDS,
    CONTENT_NODES_TOTAL
};

pub async fn create_node(&self, ...) -> ContentResult<NodeResponse> {
    // Start timer
    let timer = CONTENT_OPERATION_DURATION_SECONDS
        .with_label_values(&["create", &input.kind])
        .start_timer();
    
    let result = self.create_node_impl(...).await;
    
    // Drop timer to record duration
    drop(timer);
    
    // Count operation
    let status = result.is_ok();
    CONTENT_OPERATIONS_TOTAL
        .with_label_values(&["create", &input.kind, if status { "success" } else { "error" }])
        .inc();
    
    // Update gauge on success
    if result.is_ok() {
        CONTENT_NODES_TOTAL.inc();
    }
    
    result
}
```

### Exporting Metrics

The metrics endpoint is exposed by the server:

```bash
curl http://localhost:3000/metrics
```

Returns Prometheus format:

```
# HELP rustok_content_operations_total Total content operations
# TYPE rustok_content_operations_total counter
rustok_content_operations_total{operation="create",kind="post",status="success"} 42
rustok_content_operations_total{operation="create",kind="page",status="success"} 15
rustok_content_operations_total{operation="create",kind="post",status="error"} 2

# HELP rustok_content_operation_duration_seconds Duration of content operations
# TYPE rustok_content_operation_duration_seconds histogram
rustok_content_operation_duration_seconds{operation="create",kind="post"} 0.05 0.02 0.01 0.005

# HELP rustok_content_nodes_total Total number of content nodes
# TYPE rustok_content_nodes_total gauge
rustok_content_nodes_total 57
```

## Grafana Dashboard Example

Create a new dashboard in Grafana with the following panels:

### 1. Content Operations Rate
- Query: `rate(rustok_content_operations_total[5m])`
- Visualization: Time series graph

### 2. Content Operation Duration (P95)
- Query: `histogram_quantile(0.95, rate(rustok_content_operation_duration_seconds_bucket[5m]))`
- Visualization: Single stat / gauge

### 3. Content Nodes Total
- Query: `rustok_content_nodes_total`
- Visualization: Gauge

### 4. Error Rate
- Query: `sum(rate(rustok_content_operations_total{status="error"}[5m])) by (operation, kind)`
- Visualization: Heatmap

## Initialization

The telemetry system is initialized in the server app:

```rust
use rustok_telemetry::{TelemetryConfig, LogFormat};

let telemetry_handles = TelemetryConfig {
    service_name: "rustok-server".to_string(),
    log_format: LogFormat::Json, // or LogFormat::Pretty for development
    metrics: true,
}.init()?;
```

## Best Practices

1. **Always record both counters and histograms** - Counters give volume, histograms give latency
2. **Use consistent label values** - Define enums for operation types and use them
3. **Don't create high-cardinality labels** - Avoid using user IDs or request IDs as labels
4. **Update gauges atomically** - Gauges should reflect current state, not increments
5. **Label all operations** - Every database/external call should be measured

## Future Enhancements

- [ ] Add RED method metrics (Rate, Errors, Duration) for each endpoint
- [ ] Add SLA/SLO alerts based on metrics
- [ ] Add business metrics (revenue, conversion rate)
- [ ] Add custom dashboards for each module
