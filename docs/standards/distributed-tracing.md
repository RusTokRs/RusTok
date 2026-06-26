---
id: doc://docs/standards/distributed-tracing.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# 🔍 Distributed Tracing Guide

> **Sprint 3 Task 3.2**  
> **Status:** Complete ✅  
> **Updated:** 2026-02-12

Complete guide to distributed tracing in RusToK using OpenTelemetry and Jaeger.

---

## 📋 Table of Contents

- [Overview](#overview)
- [Architecture](#architecture)
- [Span Creation](#span-creation)
- [Instrumentation Patterns](#instrumentation-patterns)
- [Correlation](#correlation)
- [Best Practices](#best-practices)
- [Examples](#examples)
- [Troubleshooting](#troubleshooting)

---

## 🎯 Overview

### What is Distributed Tracing?

Distributed tracing tracks requests as they flow through multiple services, creating a complete picture of:
- **Request flow:** Where does each request go?
- **Performance:** How long does each operation take?
- **Errors:** Where do failures occur?
- **Dependencies:** How do services interact?

### Benefits

- **Fast debugging:** Find root causes in seconds, not hours
- **Performance optimization:** Identify bottlenecks visually
- **Service mapping:** Understand system architecture
- **Error tracking:** Full context for every error

---

## 🏗️ Architecture

### Tracing Flow

```
┌─────────────────────────────────────────────────────┐
│                  HTTP Request                       │
│  GET /api/blog/posts/{id}                          │
└────────────────┬────────────────────────────────────┘
                 │
                 ↓
         ┌───────────────┐
         │  HTTP Span    │  ← Entry point
         │  200ms total  │
         └───────┬───────┘
                 │
        ┌────────┼────────┐
        ↓        ↓         ↓
    ┌──────┐ ┌──────┐  ┌──────┐
    │Tenant│ │RBAC  │  │Node  │  ← Child spans
    │ 5ms  │ │ 3ms  │  │150ms │
    └──────┘ └──────┘  └───┬──┘
                           │
                  ┌────────┼────────┐
                  ↓        ↓         ↓
              ┌──────┐ ┌──────┐  ┌──────┐
              │  DB  │ │Event │  │Cache │  ← Grandchild spans
              │100ms │ │ 30ms │  │ 5ms  │
              └──────┘ └──────┘  └──────┘
```

### Span Hierarchy

- **Root Span:** HTTP request (automatically created)
- **Service Spans:** Business logic operations
- **Database Spans:** SQL queries
- **Event Spans:** Event publishing/handling
- **External Spans:** API calls, cache operations

---

## 🛠️ Span Creation

### 1. Using `#[instrument]` Macro (Recommended)

The simplest way to add tracing to functions:

```rust
use tracing::instrument;
use uuid::Uuid;

#[instrument(
    name = "fetch_user",
    skip(db),
    fields(
        tenant_id = %tenant_id,
        user_id = %user_id,
    )
)]
async fn fetch_user(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<User> {
    // Span automatically created with function name
    // tenant_id and user_id are recorded as attributes
    
    tracing::info!("Fetching user from database");
    
    let user = users::Entity::find_by_id(user_id)
        .filter(users::Column::TenantId.eq(tenant_id))
        .one(db)
        .await?;
    
    Ok(user)
}
```

**Parameters:**
- `name`: Span name (default: function name)
- `skip`: Fields to skip (e.g., `db`, `self`)
- `fields`: Additional attributes to record

### 2. Manual Span Creation

For more control:

```rust
use tracing::{info_span, Span};

async fn process_order(order_id: Uuid) -> Result<()> {
    let span = info_span!(
        "process_order",
        order_id = %order_id,
        otel.kind = "internal",
    );
    
    let _guard = span.enter();
    
    // Your code here
    tracing::info!("Processing order");
    
    Ok(())
}
```

### 3. Using RusToK Helpers

```rust
use rustok_core::tracing::{create_span, SpanAttributes};

async fn fetch_data(tenant_id: Uuid, user_id: Uuid) -> Result<Data> {
    let attrs = SpanAttributes::new("fetch_data", "data_service")
        .with_tenant(tenant_id)
        .with_user(user_id);
    
    let span = create_span("fetch_data", attrs);
    let _guard = span.enter();
    
    // Your code here
    
    Ok(Data::default())
}
```

---

## 📐 Instrumentation Patterns

### HTTP Handlers

Already instrumented by Axum middleware:

```rust
// Automatic span creation for all HTTP requests
async fn handler(
    Path(id): Path<Uuid>,
) -> Result<Json<Response>> {
    // Parent span already exists with:
    // - http.method
    // - http.url
    // - http.status_code
    
    fetch_resource(id).await
}
```

### Service Layer

```rust
use tracing::instrument;

impl NodeService {
    #[instrument(
        skip(self, security, input),
        fields(
            tenant_id = %tenant_id,
            kind = %input.kind,
            user_id = ?security.user_id,
        )
    )]
    pub async fn create_node(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateNodeInput,
    ) -> ContentResult<NodeResponse> {
        tracing::info!("Creating node");
        
        // Business logic...
        
        Ok(response)
    }
}
```

### EventBus

Already instrumented:

```rust
// EventBus.publish() creates a span automatically
event_bus.publish(
    tenant_id,
    Some(user_id),
    DomainEvent::NodeCreated(event),
)?;

// Span includes:
// - event.type
// - event.id
// - tenant_id
// - actor_id
// - otel.kind = "producer"
```

### Database Queries

```rust
use rustok_core::tracing::db_span;

async fn query_users(db: &DatabaseConnection) -> Result<Vec<User>> {
    let span = db_span(
        "SELECT * FROM users WHERE active = true",
        "postgres"
    );
    let _guard = span.enter();
    
    let users = users::Entity::find()
        .filter(users::Column::Active.eq(true))
        .all(db)
        .await?;
    
    Ok(users)
}
```

### GraphQL Resolvers

```rust
use async_graphql::{Context, Object, Result};
use tracing::instrument;

#[Object]
impl Query {
    #[instrument(
        skip(ctx),
        fields(
            node_id = %id,
            tenant_id = tracing::field::Empty,
        )
    )]
    async fn node(&self, ctx: &Context<'_>, id: Uuid) -> Result<NodeResponse> {
        let tenant_id = ctx.data::<Uuid>()?;
        
        let span = tracing::Span::current();
        span.record("tenant_id", &tracing::field::display(tenant_id));
        
        tracing::info!("Fetching node");
        
        let service = ctx.data::<Arc<NodeService>>()?;
        let node = service.get_node(*tenant_id, id).await?;
        
        Ok(node.into())
    }
}
```

### External API Calls

```rust
use rustok_core::tracing::http_client_span;

async fn fetch_external_data(url: &str) -> Result<Response> {
    let span = http_client_span("GET", url);
    let _guard = span.enter();
    
    let response = reqwest::get(url).await?;
    
    tracing::info!(
        status = response.status().as_u16(),
        "External API call completed"
    );
    
    Ok(response)
}
```

---

## 🔗 Correlation

### Tenant & User Correlation

Always include tenant and user IDs for multi-tenant tracing:

```rust
#[instrument(
    fields(
        tenant_id = %tenant_id,
        user_id = ?user_id,  // Optional
    )
)]
async fn operation(tenant_id: Uuid, user_id: Option<Uuid>) -> Result<()> {
    // Spans will be filterable by tenant_id and user_id in Jaeger
    Ok(())
}
```

### Event Correlation

Events automatically carry correlation context:

```rust
// Publishing
event_bus.publish(tenant_id, Some(user_id), event)?;
// ↓ Creates span with event.id

// Handling
impl EventHandler for MyHandler {
    async fn handle(&self, envelope: &EventEnvelope) -> HandlerResult {
        // Span includes event.id, linking to publisher
        tracing::info!(
            event_id = %envelope.id,
            "Handling event"
        );
        
        Ok(())
    }
}
```

### Request ID Propagation

HTTP requests automatically propagate trace context via headers:
- `traceparent` (W3C Trace Context)
- `tracestate`

---

## ✅ Best Practices

### 1. Name Spans Descriptively

```rust
// ❌ Bad
#[instrument(name = "do_thing")]
async fn process() -> Result<()> { }

// ✅ Good
#[instrument(name = "order.process")]
async fn process_order() -> Result<()> { }
```

### 2. Skip Large or Sensitive Data

```rust
// ❌ Bad - logs entire request body
#[instrument]
async fn create(input: LargeInput) -> Result<()> { }

// ✅ Good - skip the input
#[instrument(skip(input))]
async fn create(input: LargeInput) -> Result<()> { }
```

### 3. Record Important Fields

```rust
#[instrument(
    skip(self, data),
    fields(
        tenant_id = %tenant_id,
        resource_id = %resource_id,
        operation = "create",
        result = tracing::field::Empty,  // Will record later
    )
)]
async fn create_resource(
    &self,
    tenant_id: Uuid,
    resource_id: Uuid,
    data: Data,
) -> Result<Resource> {
    let resource = /* ... */;
    
    let span = tracing::Span::current();
    span.record("result", "success");
    
    Ok(resource)
}
```

### 4. Record Errors

```rust
use rustok_core::tracing::record_error;

async fn risky_operation() -> Result<()> {
    match do_something().await {
        Ok(result) => Ok(result),
        Err(e) => {
            record_error(&e, "operation_failed");
            Err(e)
        }
    }
}
```

### 5. Use Span Hierarchy

```rust
#[instrument]
async fn parent_operation() -> Result<()> {
    // Parent span
    
    child_operation_1().await?;
    child_operation_2().await?;
    
    Ok(())
}

#[instrument]
async fn child_operation_1() -> Result<()> {
    // Child span - automatically nested under parent
    Ok(())
}

#[instrument]
async fn child_operation_2() -> Result<()> {
    // Another child span
    Ok(())
}
```

---

## 💡 Examples

### Example 1: Complete Request Flow

```rust
// 1. HTTP Handler (automatic span)
async fn create_post_handler(
    State(service): State<Arc<NodeService>>,
    Extension(tenant_id): Extension<Uuid>,
    Extension(user_id): Extension<Uuid>,
    Json(input): Json<CreateNodeInput>,
) -> Result<Json<NodeResponse>> {
    // http.request span (automatic)
    
    let security = SecurityContext::new(user_id, tenant_id, /* roles */);
    let response = service.create_node(tenant_id, security, input).await?;
    
    Ok(Json(response))
}

// 2. Service Layer
impl NodeService {
    #[instrument(skip(self, security, input), fields(tenant_id = %tenant_id))]
    pub async fn create_node(
        &self,
        tenant_id: Uuid,
        security: SecurityContext,
        input: CreateNodeInput,
    ) -> ContentResult<NodeResponse> {
        // node.create span
        
        // 3. Database query
        let node = self.insert_node(tenant_id, input).await?;
        
        // 4. Publish event
        self.event_bus.publish(
            tenant_id,
            Some(security.user_id),
            DomainEvent::NodeCreated(/* ... */),
        )?;
        
        Ok(node.into())
    }
    
    #[instrument(skip(self, input))]
    async fn insert_node(
        &self,
        tenant_id: Uuid,
        input: CreateNodeInput,
    ) -> ContentResult<Node> {
        // db.insert span
        
        let txn = self.db.begin().await?;
        let node = /* insert logic */;
        txn.commit().await?;
        
        Ok(node)
    }
}
```

**Resulting trace in Jaeger:**
```
http.request (200ms)
├── node.create (180ms)
    ├── db.insert (100ms)
    └── eventbus.publish (30ms)
```

### Example 2: Event Flow Tracing

```rust
// Publisher
#[instrument(skip(event_bus), fields(tenant_id = %tenant_id))]
async fn create_order(
    tenant_id: Uuid,
    event_bus: &EventBus,
) -> Result<()> {
    let event = DomainEvent::OrderCreated(/* ... */);
    
    event_bus.publish(tenant_id, None, event)?;
    // Creates span: eventbus.publish
    
    Ok(())
}

// Handler
pub struct OrderIndexHandler;

#[async_trait]
impl EventHandler for OrderIndexHandler {
    fn name(&self) -> &'static str {
        "order_index"
    }
    
    fn handles(&self, event: &DomainEvent) -> bool {
        matches!(event, DomainEvent::OrderCreated(_))
    }
    
    async fn handle(&self, envelope: &EventEnvelope) -> HandlerResult {
        // Span automatically created by dispatcher
        // Links to publisher span via event.id
        
        tracing::info!(
            event_id = %envelope.id,
            "Updating order index"
        );
        
        // Update read model...
        
        Ok(())
    }
}
```

**Trace visualization:**
```
eventbus.publish (30ms) [Producer]
└── event_dispatch (50ms) [Consumer]
    └── order_index.handle (45ms)
        └── db.update (40ms)
```

---

## 🐛 Troubleshooting

### No Traces Appearing

**Check:**
1. Is OpenTelemetry enabled?
   ```bash
   echo $OTEL_ENABLED  # Should be "true"
   ```

2. Is OTLP endpoint correct?
   ```bash
   echo $OTEL_EXPORTER_OTLP_ENDPOINT  # Should be http://localhost:4317
   ```

3. Is Jaeger running?
   ```bash
   docker ps | grep jaeger
   curl http://localhost:16686
   ```

4. Check server logs for OTel errors:
   ```bash
   grep -i "opentelemetry\|otlp" server.log
   ```

### Spans Not Nested Correctly

**Problem:** Child spans appear as separate traces

**Solution:** Ensure async context is preserved:

```rust
// ❌ Bad - spawns task without span context
tokio::spawn(async {
    child_operation().await;
});

// ✅ Good - preserves span context
use tracing::Instrument;

tokio::spawn(
    async {
        child_operation().await;
    }.instrument(tracing::Span::current())
);
```

### Missing Attributes

**Problem:** Fields show as empty in Jaeger

**Solution:** Record fields explicitly:

```rust
#[instrument(fields(user_id = tracing::field::Empty))]
async fn operation(user_id: Option<Uuid>) -> Result<()> {
    if let Some(id) = user_id {
        let span = tracing::Span::current();
        span.record("user_id", &tracing::field::display(id));
    }
    
    Ok(())
}
```

### High Cardinality Fields

**Problem:** Too many unique span names

**Avoid:**
```rust
// ❌ Bad - creates unique span for each ID
let span = info_span!("fetch_user_{}", user_id);
```

**Use:**
```rust
// ✅ Good - same span name, ID as attribute
let span = info_span!("fetch_user", user_id = %user_id);
```

---

## 📊 Querying Traces in Jaeger

### Find All Traces for a Tenant

1. Open Jaeger UI: http://localhost:16686
2. Select Service: `rustok-server`
3. Add Tag: `tenant_id=<uuid>`
4. Click "Find Traces"

### Find Slow Requests

1. Service: `rustok-server`
2. Min Duration: `500ms`
3. Click "Find Traces"

### Find Errors

1. Service: `rustok-server`
2. Tags: `error=true`
3. Click "Find Traces"

### Find Specific Operation

1. Service: `rustok-server`
2. Operation: `node.create`
3. Click "Find Traces"

---

## 📚 Reference

### Common Span Attributes

| Attribute | Description | Example |
|-----------|-------------|---------|
| `tenant_id` | Multi-tenant identifier | `uuid` |
| `user_id` | User identifier | `uuid` |
| `operation` | Operation name | `create`, `update`, `delete` |
| `resource_id` | Resource identifier | `uuid` |
| `otel.kind` | Span kind | `server`, `client`, `producer`, `consumer` |
| `http.method` | HTTP method | `GET`, `POST` |
| `http.url` | Request URL | `/api/nodes` |
| `http.status_code` | Response status | `200`, `404` |
| `db.type` | Database type | `postgres`, `redis` |
| `db.operation` | SQL query | `SELECT * FROM ...` |
| `event.type` | Event type | `NodeCreated` |
| `event.id` | Event identifier | `uuid` |
| `error` | Error message | `string` |
| `error_type` | Error category | `validation`, `not_found` |
| `error_occurred` | Error flag for failed operations | `true`, `false` |

### Span Kinds

- `server`: Entry point (HTTP handler, GraphQL resolver)
- `client`: Outgoing call (HTTP client, database query)
- `producer`: Message/event publisher
- `consumer`: Message/event handler
- `internal`: Internal operation (default)

---

## 🎯 Implementation Status

### ✅ Completed

- [x] EventBus spans (publish, publish_envelope)
- [x] EventDispatcher spans (event_dispatch, handler execution)
- [x] Service layer helpers (SpanAttributes, create_span)
- [x] Database query helpers (db_span)
- [x] HTTP client helpers (http_client_span)
- [x] Event helpers (event_span)
- [x] Error recording (record_error)
- [x] Duration measurement (measure)

### 📋 Existing Instrumentation

Already using `#[instrument]` macro:
- NodeService methods
- HTTP handlers (via Axum middleware)
- GraphQL resolvers (via async-graphql)

---

## 🔜 Next Steps

### Task 3.3: Enhanced Metrics

- Custom Prometheus metrics for:
  - Span count by operation
  - Span duration histograms
  - Error rate by span type
- Grafana dashboards with trace links

---

**Status:** Complete ✅  
**Implementation:** 200+ LOC tracing utilities  
**Documentation:** 6KB guide

For quick start, see [OBSERVABILITY_QUICKSTART.md](../OBSERVABILITY_QUICKSTART.md)
