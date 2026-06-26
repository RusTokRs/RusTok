---
id: doc://docs/standards/instrumentation-examples.md
kind: project_overview
language: markdown
last_verified_snapshot: snap_jsonl_00000021
source_language: markdown
status: verified
---
# 🔍 OpenTelemetry Instrumentation Examples

> **Date:** 2026-02-13  
> **Sprint:** 3 - Task 3.1

This document provides practical examples of instrumenting RusToK services with OpenTelemetry.

## 📋 Table of Contents

1. [Service Layer Instrumentation](#service-layer-instrumentation)
2. [Repository/Database Instrumentation](#repositorydatabase-instrumentation)
3. [Event Bus Instrumentation](#event-bus-instrumentation)
4. [HTTP Handler Instrumentation](#http-handler-instrumentation)
5. [Cache Instrumentation](#cache-instrumentation)
6. [Background Task Instrumentation](#background-task-instrumentation)

---

## Service Layer Instrumentation

### Example: Content Node Service

```rust
use tracing::instrument;
use uuid::Uuid;
use rustok_content::dto::{CreateNodeInput, NodeResponse};

pub struct NodeService {
    db: DatabaseConnection,
    event_bus: Arc<EventBus>,
}

impl NodeService {
    /// Create a new content node
    #[instrument(
        name = "content.create_node",
        skip(self, input),
        fields(
            tenant_id = %tenant_id,
            actor_id = %actor_id,
            node_kind = %input.kind,
            otel.kind = "internal"
        )
    )]
    pub async fn create_node(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: CreateNodeInput,
    ) -> Result<NodeResponse> {
        let span = tracing::Span::current();
        
        // Record additional attributes
        span.record("node_title", &input.title);
        if let Some(parent_id) = input.parent_id {
            span.record("parent_id", &parent_id.to_string());
        }
        
        // Business logic
        tracing::info!("Creating content node");
        let node = self.repository.create(tenant_id, input).await?;
        
        // Record result
        span.record("node_id", &node.id.to_string());
        tracing::info!(node_id = %node.id, "Content node created successfully");
        
        // Publish event (this will create a child span)
        self.publish_node_created_event(tenant_id, &node).await?;
        
        Ok(node.into())
    }

    /// Update a content node
    #[instrument(
        name = "content.update_node",
        skip(self, input),
        fields(
            tenant_id = %tenant_id,
            actor_id = %actor_id,
            node_id = %node_id,
            otel.kind = "internal"
        )
    )]
    pub async fn update_node(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        node_id: Uuid,
        input: UpdateNodeInput,
    ) -> Result<NodeResponse> {
        tracing::info!("Updating content node");
        
        // Check permissions (creates child span)
        self.check_update_permission(tenant_id, actor_id, node_id).await?;
        
        // Update in database (creates child span)
        let node = self.repository.update(tenant_id, node_id, input).await?;
        
        tracing::info!("Content node updated successfully");
        
        // Publish event
        self.publish_node_updated_event(tenant_id, &node).await?;
        
        Ok(node.into())
    }

    /// Delete a content node
    #[instrument(
        name = "content.delete_node",
        skip(self),
        fields(
            tenant_id = %tenant_id,
            actor_id = %actor_id,
            node_id = %node_id,
            otel.kind = "internal"
        )
    )]
    pub async fn delete_node(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        node_id: Uuid,
    ) -> Result<()> {
        tracing::info!("Deleting content node");
        
        // Soft delete
        self.repository.soft_delete(tenant_id, node_id).await?;
        
        tracing::info!("Content node deleted successfully");
        
        // Publish event
        self.publish_node_deleted_event(tenant_id, node_id).await?;
        
        Ok(())
    }
}
```

### Example: Commerce Product Service

```rust
use rustok_commerce::dto::{CreateProductInput, ProductResponse};

pub struct ProductService {
    db: DatabaseConnection,
    event_bus: Arc<EventBus>,
}

impl ProductService {
    #[instrument(
        name = "commerce.create_product",
        skip(self, input),
        fields(
            tenant_id = %tenant_id,
            actor_id = %actor_id,
            product_sku = %input.sku,
            otel.kind = "internal"
        )
    )]
    pub async fn create_product(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        input: CreateProductInput,
    ) -> Result<ProductResponse> {
        let span = tracing::Span::current();
        
        // Validate SKU uniqueness (child span)
        self.validate_sku_unique(tenant_id, &input.sku).await?;
        
        // Create product
        tracing::info!("Creating product");
        let product = self.repository.create(tenant_id, input).await?;
        
        span.record("product_id", &product.id.to_string());
        tracing::info!(product_id = %product.id, sku = %product.sku, "Product created");
        
        // Publish event
        self.publish_product_created_event(tenant_id, &product).await?;
        
        Ok(product.into())
    }

    #[instrument(
        name = "commerce.validate_sku_unique",
        skip(self),
        fields(
            tenant_id = %tenant_id,
            sku = %sku,
            otel.kind = "internal"
        )
    )]
    async fn validate_sku_unique(&self, tenant_id: Uuid, sku: &str) -> Result<()> {
        let exists = self.repository.sku_exists(tenant_id, sku).await?;
        
        if exists {
            tracing::warn!(sku = %sku, "SKU already exists");
            return Err(Error::DuplicateSku(sku.to_string()));
        }
        
        tracing::debug!("SKU is unique");
        Ok(())
    }
}
```

---

## Repository/Database Instrumentation

### Example: Database Query with SQL Statement

```rust
use sea_orm::{EntityTrait, QueryFilter, ColumnTrait};
use tracing::{instrument, Span};

pub struct NodeRepository {
    db: DatabaseConnection,
}

impl NodeRepository {
    #[instrument(
        name = "db.query.find_node",
        skip(self),
        fields(
            db.system = "postgresql",
            db.statement = tracing::field::Empty,
            tenant_id = %tenant_id,
            node_id = %node_id,
            otel.kind = "client"
        )
    )]
    pub async fn find_by_id(
        &self,
        tenant_id: Uuid,
        node_id: Uuid,
    ) -> Result<Option<node::Model>> {
        let span = Span::current();
        
        // Record SQL statement (simplified)
        span.record("db.statement", "SELECT * FROM nodes WHERE tenant_id = $1 AND id = $2");
        
        let node = node::Entity::find()
            .filter(node::Column::TenantId.eq(tenant_id))
            .filter(node::Column::Id.eq(node_id))
            .one(&self.db)
            .await
            .map_err(|e| {
                tracing::error!(error = %e, "Database query failed");
                Error::Database(e)
            })?;
        
        if node.is_some() {
            tracing::debug!("Node found");
        } else {
            tracing::debug!("Node not found");
        }
        
        Ok(node)
    }

    #[instrument(
        name = "db.query.list_nodes",
        skip(self),
        fields(
            db.system = "postgresql",
            db.statement = "SELECT * FROM nodes WHERE tenant_id = $1 AND kind = $2 ORDER BY created_at DESC",
            tenant_id = %tenant_id,
            kind = %kind,
            otel.kind = "client"
        )
    )]
    pub async fn list_by_kind(
        &self,
        tenant_id: Uuid,
        kind: &str,
    ) -> Result<Vec<node::Model>> {
        let nodes = node::Entity::find()
            .filter(node::Column::TenantId.eq(tenant_id))
            .filter(node::Column::Kind.eq(kind))
            .order_by_desc(node::Column::CreatedAt)
            .all(&self.db)
            .await?;
        
        tracing::info!(count = nodes.len(), "Nodes retrieved");
        
        Ok(nodes)
    }

    #[instrument(
        name = "db.mutation.create_node",
        skip(self, input),
        fields(
            db.system = "postgresql",
            db.operation = "INSERT",
            tenant_id = %tenant_id,
            otel.kind = "client"
        )
    )]
    pub async fn create(
        &self,
        tenant_id: Uuid,
        input: CreateNodeInput,
    ) -> Result<node::Model> {
        let active_model = node::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            kind: Set(input.kind),
            title: Set(input.title),
            // ... other fields
        };
        
        let node = active_model.insert(&self.db).await?;
        
        tracing::info!(node_id = %node.id, "Node inserted into database");
        
        Ok(node)
    }
}
```

---

## Event Bus Instrumentation

### Example: Publishing Events with Trace Context

```rust
use rustok_core::events::{DomainEvent, EventEnvelope, EventMetadata};

pub struct EventPublisher {
    event_bus: Arc<dyn EventBus>,
}

impl EventPublisher {
    #[instrument(
        name = "event_bus.publish",
        skip(self, event),
        fields(
            event_type = ?event,
            otel.kind = "producer"
        )
    )]
    pub async fn publish(&self, event: DomainEvent) -> Result<()> {
        let span = Span::current();
        
        // Extract trace context
        let trace_id = Self::extract_trace_id(&span);
        
        // Create envelope with trace context
        let envelope = EventEnvelope {
            event_id: Uuid::new_v4(),
            event,
            metadata: EventMetadata {
                trace_id: Some(trace_id.clone()),
                span_id: Self::extract_span_id(&span),
                timestamp: Utc::now(),
            },
        };
        
        tracing::info!(
            event_id = %envelope.event_id,
            trace_id = %trace_id,
            "Publishing event"
        );
        
        self.event_bus.publish(envelope).await?;
        
        tracing::debug!("Event published successfully");
        
        Ok(())
    }

    fn extract_trace_id(span: &Span) -> String {
        // Extract OpenTelemetry trace ID
        use opentelemetry::trace::TraceContextExt;
        
        let context = span.context();
        let span_context = context.span().span_context();
        span_context.trace_id().to_string()
    }

    fn extract_span_id(span: &Span) -> Option<String> {
        use opentelemetry::trace::TraceContextExt;
        
        let context = span.context();
        let span_context = context.span().span_context();
        Some(span_context.span_id().to_string())
    }
}
```

### Example: Handling Events with Trace Context

```rust
pub struct NodeCreatedHandler {
    index_service: Arc<IndexService>,
}

impl NodeCreatedHandler {
    #[instrument(
        name = "event_handler.node_created",
        skip(self, envelope),
        fields(
            event_id = %envelope.event_id,
            trace_id = %envelope.metadata.trace_id.as_ref().unwrap_or(&"none".to_string()),
            otel.kind = "consumer"
        )
    )]
    pub async fn handle(&self, envelope: EventEnvelope) -> Result<()> {
        // Extract event data
        let DomainEvent::NodeCreated { node_id, tenant_id, .. } = envelope.event else {
            return Err(Error::WrongEventType);
        };
        
        tracing::info!(
            node_id = %node_id,
            tenant_id = %tenant_id,
            "Handling NodeCreated event"
        );
        
        // Update index (this creates a child span)
        self.index_service.index_node(tenant_id, node_id).await?;
        
        tracing::info!("NodeCreated event handled successfully");
        
        Ok(())
    }
}
```

---

## HTTP Handler Instrumentation

### Example: Axum Handler

```rust
use axum::{extract::{Path, State}, Json};
use tower_http::trace::TraceLayer;

#[instrument(
    name = "http.handler.create_product",
    skip(state, input),
    fields(
        http.method = "POST",
        http.route = "/api/products",
        tenant_id = tracing::field::Empty,
        actor_id = tracing::field::Empty,
        otel.kind = "server"
    )
)]
pub async fn create_product_handler(
    State(state): State<AppState>,
    tenant_id: Extension<TenantId>,
    actor_id: Extension<ActorId>,
    Json(input): Json<CreateProductInput>,
) -> Result<Json<ProductResponse>, ApiError> {
    let span = Span::current();
    
    // Record tenant and actor
    span.record("tenant_id", &tenant_id.0.to_string());
    span.record("actor_id", &actor_id.0.to_string());
    
    tracing::info!(
        sku = %input.sku,
        "Handling create product request"
    );
    
    // Call service (creates child span)
    let product = state
        .product_service
        .create_product(tenant_id.0, actor_id.0, input)
        .await
        .map_err(ApiError::from)?;
    
    tracing::info!(product_id = %product.id, "Product created via API");
    
    Ok(Json(product))
}

/// Configure tracing middleware for Axum
pub fn tracing_middleware() -> TraceLayer<SharedClassifier> {
    TraceLayer::new_for_http()
        .make_span_with(|request: &Request<_>| {
            info_span!(
                "http.request",
                http.method = %request.method(),
                http.route = %request.uri().path(),
                http.target = %request.uri(),
                otel.kind = "server"
            )
        })
        .on_response(|response: &Response<_>, latency: Duration, _span: &Span| {
            tracing::info!(
                http.status_code = response.status().as_u16(),
                latency_ms = latency.as_millis(),
                "Request completed"
            );
        })
}
```

---

## Cache Instrumentation

### Example: Redis Cache Operations

```rust
use rustok_core::cache::CacheBackend;

pub struct RedisCacheBackend {
    client: redis::Client,
    circuit_breaker: CircuitBreaker,
}

impl RedisCacheBackend {
    #[instrument(
        name = "cache.get",
        skip(self),
        fields(
            cache.system = "redis",
            cache.key = %key,
            cache.hit = tracing::field::Empty,
            otel.kind = "client"
        )
    )]
    pub async fn get(&self, key: &str) -> Result<Option<String>> {
        let span = Span::current();
        
        let result = self.circuit_breaker
            .call(|| {
                let client = self.client.clone();
                let key = key.to_string();
                Box::pin(async move {
                    let mut conn = client.get_async_connection().await?;
                    let value: Option<String> = conn.get(&key).await?;
                    Ok(value)
                })
            })
            .await?;
        
        // Record cache hit/miss
        span.record("cache.hit", result.is_some());
        
        if result.is_some() {
            tracing::debug!("Cache hit");
        } else {
            tracing::debug!("Cache miss");
        }
        
        Ok(result)
    }

    #[instrument(
        name = "cache.set",
        skip(self, value),
        fields(
            cache.system = "redis",
            cache.key = %key,
            cache.ttl_seconds = ttl_seconds,
            otel.kind = "client"
        )
    )]
    pub async fn set(&self, key: &str, value: &str, ttl_seconds: u64) -> Result<()> {
        self.circuit_breaker
            .call(|| {
                let client = self.client.clone();
                let key = key.to_string();
                let value = value.to_string();
                Box::pin(async move {
                    let mut conn = client.get_async_connection().await?;
                    redis::cmd("SETEX")
                        .arg(&key)
                        .arg(ttl_seconds)
                        .arg(&value)
                        .query_async(&mut conn)
                        .await?;
                    Ok(())
                })
            })
            .await?;
        
        tracing::debug!("Value cached");
        
        Ok(())
    }
}
```

---

## Background Task Instrumentation

### Example: Spawning Background Tasks

```rust
use tracing::Instrument;
use tokio::task;

pub struct BackgroundJobService {
    db: DatabaseConnection,
}

impl BackgroundJobService {
    #[instrument(
        name = "background.schedule_cleanup",
        skip(self),
        fields(
            tenant_id = %tenant_id,
            otel.kind = "internal"
        )
    )]
    pub async fn schedule_cleanup(&self, tenant_id: Uuid) {
        tracing::info!("Scheduling cleanup job");
        
        let db = self.db.clone();
        let current_span = Span::current();
        
        // Spawn background task with trace context
        task::spawn(
            async move {
                tracing::info!("Starting cleanup job");
                
                match Self::run_cleanup(db, tenant_id).await {
                    Ok(count) => {
                        tracing::info!(deleted_count = count, "Cleanup completed");
                    }
                    Err(e) => {
                        tracing::error!(error = %e, "Cleanup failed");
                    }
                }
            }
            .instrument(info_span!(
                parent: &current_span,
                "background.run_cleanup",
                tenant_id = %tenant_id
            ))
        );
        
        tracing::info!("Cleanup job scheduled");
    }

    #[instrument(
        name = "background.run_cleanup",
        skip(db),
        fields(
            tenant_id = %tenant_id,
            otel.kind = "internal"
        )
    )]
    async fn run_cleanup(db: DatabaseConnection, tenant_id: Uuid) -> Result<u64> {
        // Delete old records
        let result = node::Entity::delete_many()
            .filter(node::Column::TenantId.eq(tenant_id))
            .filter(node::Column::DeletedAt.is_not_null())
            .filter(node::Column::DeletedAt.lt(Utc::now() - Duration::days(30)))
            .exec(&db)
            .await?;
        
        Ok(result.rows_affected)
    }
}
```

---

## 🎯 Summary

Key patterns for instrumentation:

1. **Use `#[instrument]`** on all public service methods
2. **Add semantic attributes** (`otel.kind`, `db.system`, etc.)
3. **Record dynamic values** with `span.record()`
4. **Propagate context** through event metadata
5. **Use `.instrument()`** for background tasks
6. **Log important milestones** within spans
7. **Record errors** with `tracing::error!`

---

**Next:** See [OPENTELEMETRY_INTEGRATION.md](./opentelemetry-integration.md) for configuration and deployment.
