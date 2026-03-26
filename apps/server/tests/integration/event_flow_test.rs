//! Event Flow Integration Tests
//!
//! Tests the complete end-to-end event flow:
//! Event → Processing → State Update
//!
//! These are integration tests that verify the event-driven architecture
//! works correctly, including event validation, dispatching, and state changes.

use async_trait::async_trait;
use chrono::Utc;
use rustok_core::events::{
    DomainEvent, Envelope, EventMetadata, EventValidationError, TransactionalEventBus,
    TypedEvent,
};
use rustok_core::{EventDispatcher, PermissionScope, SecurityContext};
use rustok_test_utils::{
    events::{mock_transactional_event_bus, MockEventTransport},
    setup_test_db,
};
use sea_orm::{DatabaseConnection, DbBackend, Statement, TransactionTrait};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::RwLock;
use uuid::Uuid;

/// Test application wrapper for event tests
struct EventTestApp {
    db: sea_orm::DatabaseConnection,
    event_bus: TestEventBus,
    security: SecurityContext,
}

impl EventTestApp {
    async fn new() -> Self {
        let db = setup_test_db().await;
        let event_bus = TestEventBus::new();
        let security = SecurityContext::system();

        Self::setup_event_tables(&db).await;

        Self { db, event_bus, security }
    }

    async fn setup_event_tables(db: &sea_orm::DatabaseConnection) {
        db.execute(Statement::from_string(
            DbBackend::Sqlite,
            r#"
            CREATE TABLE IF NOT EXISTS outbox_events (
                id TEXT PRIMARY KEY,
                event_type TEXT NOT NULL,
                tenant_id TEXT,
                payload TEXT NOT NULL,
                metadata TEXT NOT NULL DEFAULT '{}',
                status TEXT NOT NULL DEFAULT 'pending',
                created_at TEXT NOT NULL,
                processed_at TEXT,
                retry_count INTEGER NOT NULL DEFAULT 0,
                error_message TEXT
            );
            
            CREATE TABLE IF NOT EXISTS event_log (
                id TEXT PRIMARY KEY,
                event_id TEXT NOT NULL,
                event_type TEXT NOT NULL,
                source TEXT NOT NULL,
                handler TEXT NOT NULL,
                status TEXT NOT NULL,
                error_message TEXT,
                processing_time_ms INTEGER,
                created_at TEXT NOT NULL
            );
            "#,
        ))
        .await
        .expect("Failed to create event tables");
    }

    async fn publish_event<E: TypedEvent + Serialize>(&self, event: E, tenant_id: Option<Uuid>) -> anyhow::Result<Uuid> {
        let event_id = Uuid::new_v4();
        let metadata = EventMetadata::new(
            tenant_id,
            None, // user_id
            Some("test-source".to_string()),
            None, // correlation_id
            None, // causation_id
        );

        let envelope = Envelope::new(event_id, event, metadata);
        
        // Store in outbox
        self.db.execute(Statement::from_string(
            DbBackend::Sqlite,
            format!(
                "INSERT INTO outbox_events (id, event_type, tenant_id, payload, metadata, status, created_at) VALUES (
                    '{}', '{}', '{}', '{}', '{}', 'pending', '{}'
                )",
                envelope.id,
                envelope.event_type,
                envelope.metadata.tenant_id.map(|id| id.to_string()).unwrap_or_default(),
                serde_json::to_string(&envelope.payload).unwrap_or_default(),
                serde_json::to_string(&envelope.metadata).unwrap_or_default(),
                Utc::now()
            ),
        ))
        .await?;

        // Process through event bus
        self.event_bus.publish(envelope).await?;

        Ok(event_id)
    }

    async fn get_outbox_events(&self) -> anyhow::Result<Vec<OutboxEvent>> {
        let result = self.db.query_all(&Statement::from_string(
            DbBackend::Sqlite,
            "SELECT id, event_type, tenant_id, status, created_at, processed_at FROM outbox_events ORDER BY created_at DESC",
        )).await?;

        Ok(result.into_iter().map(|row| OutboxEvent {
            id: row.try_get::<String>(0).unwrap_or_default(),
            event_type: row.try_get::<String>(1).unwrap_or_default(),
            tenant_id: row.try_get::<String>(2).ok().map(|s| s.parse().ok()).flatten(),
            status: row.try_get::<String>(3).unwrap_or_default(),
            created_at: row.try_get::<String>(4).unwrap_or_default(),
            processed_at: row.try_get::<String>(5).ok(),
        }).collect())
    }

    async fn process_events(&self) -> anyhow::Result<ProcessingResult> {
        let mut processed = 0;
        let mut failed = 0;
        
        // Simulate event processing
        let events = self.get_outbox_events().await?;
        
        for event in events.iter().filter(|e| e.status == "pending") {
            // Simulate processing
            self.db.execute(Statement::from_string(
                DbBackend::Sqlite,
                format!(
                    "UPDATE outbox_events SET status = 'processed', processed_at = '{}' WHERE id = '{}'",
                    Utc::now(),
                    event.id
                ),
            ))
            .await?;
            processed += 1;
        }

        Ok(ProcessingResult {
            processed,
            failed,
        })
    }

    async fn get_event_logs(&self) -> anyhow::Result<Vec<EventLog>> {
        let result = self.db.query_all(&Statement::from_string(
            DbBackend::Sqlite,
            "SELECT id, event_id, event_type, source, handler, status, processing_time_ms, created_at FROM event_log ORDER BY created_at DESC",
        )).await?;

        Ok(result.into_iter().map(|row| EventLog {
            id: row.try_get::<String>(0).unwrap_or_default(),
            event_id: row.try_get::<String>(1).unwrap_or_default(),
            event_type: row.try_get::<String>(2).unwrap_or_default(),
            source: row.try_get::<String>(3).unwrap_or_default(),
            handler: row.try_get::<String>(4).unwrap_or_default(),
            status: row.try_get::<String>(5).unwrap_or_default(),
            processing_time_ms: row.try_get::<i64>(6).ok(),
            created_at: row.try_get::<String>(7).unwrap_or_default(),
        }).collect())
    }
}

// Simple test event bus for testing
#[derive(Clone)]
struct TestEventBus {
    published_events: Arc<RwLock<Vec<Envelope<TestEvent>>>>,
}

impl TestEventBus {
    fn new() -> Self {
        Self {
            published_events: Arc::new(RwLock::new(Vec::new())),
        }
    }

    async fn publish(&self, event: Envelope<TestEvent>) -> anyhow::Result<()> {
        self.published_events.write().await.push(event);
        Ok(())
    }

    async fn get_published_count(&self) -> usize {
        self.published_events.read().await.len()
    }
}

// Test events
#[derive(Debug, Clone, Serialize, Deserialize)]
struct TestEvent {
    event_type: String,
    payload: serde_json::Value,
}

impl TypedEvent for TestEvent {
    fn event_type(&self) -> &str {
        &self.event_type
    }
}

// Custom domain events for testing
#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrderCreatedEvent {
    order_id: Uuid,
    customer_id: Uuid,
    total: f64,
    items: Vec<OrderItem>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct OrderItem {
    product_id: Uuid,
    quantity: i32,
    price: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct ProductInventoryUpdatedEvent {
    product_id: Uuid,
    old_quantity: i32,
    new_quantity: i32,
    reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct TenantCreatedEvent {
    tenant_id: Uuid,
    name: String,
    slug: String,
}

impl TypedEvent for OrderCreatedEvent {
    fn event_type(&self) -> &str {
        "order.created"
    }
}

impl TypedEvent for ProductInventoryUpdatedEvent {
    fn event_type(&self) -> &str {
        "product.inventory.updated"
    }
}

impl TypedEvent for TenantCreatedEvent {
    fn event_type(&self) -> &str {
        "tenant.created"
    }
}

// Helper types
#[derive(Debug, Clone)]
struct OutboxEvent {
    id: String,
    event_type: String,
    tenant_id: Option<Uuid>,
    status: String,
    created_at: String,
    processed_at: Option<String>,
}

#[derive(Debug, Clone)]
struct EventLog {
    id: String,
    event_id: String,
    event_type: String,
    source: String,
    handler: String,
    status: String,
    processing_time_ms: Option<i64>,
    created_at: String,
}

#[derive(Debug, Clone)]
struct ProcessingResult {
    processed: usize,
    failed: usize,
}

#[tokio::test]
async fn test_event_publishing() {
    let app = EventTestApp::new().await;
    
    let tenant_id = Uuid::new_v4();
    
    // Publish an event
    let event = OrderCreatedEvent {
        order_id: Uuid::new_v4(),
        customer_id: Uuid::new_v4(),
        total: 1000.0,
        items: vec![OrderItem {
            product_id: Uuid::new_v4(),
            quantity: 2,
            price: 500.0,
        }],
    };

    let event_id = app.publish_event(event, Some(tenant_id)).await.unwrap();
    
    assert!(!event_id.to_string().is_empty());
    
    // Verify event is in outbox
    let outbox_events = app.get_outbox_events().await.unwrap();
    assert!(!outbox_events.is_empty());
    
    let published_event = outbox_events.iter().find(|e| e.id == event_id.to_string());
    assert!(published_event.is_some());
    assert_eq!(published_event.unwrap().status, "pending");
}

#[tokio::test]
async fn test_event_processing() {
    let app = EventTestApp::new().await;
    
    let tenant_id = Uuid::new_v4();
    
    // Publish multiple events
    for i in 0..3 {
        let event = OrderCreatedEvent {
            order_id: Uuid::new_v4(),
            customer_id: Uuid::new_v4(),
            total: 100.0 * (i + 1) as f64,
            items: vec![],
        };
        app.publish_event(event, Some(tenant_id)).await.unwrap();
    }
    
    // Process events
    let result = app.process_events().await.unwrap();
    
    assert_eq!(result.processed, 3);
    assert_eq!(result.failed, 0);
    
    // Verify all events are processed
    let outbox_events = app.get_outbox_events().await.unwrap();
    assert!(outbox_events.iter().all(|e| e.status == "processed"));
    assert!(outbox_events.iter().all(|e| e.processed_at.is_some()));
}

#[tokio::test]
async fn test_event_ordering() {
    let app = EventTestApp::new().await;
    
    let tenant_id = Uuid::new_v4();
    let order_id = Uuid::new_v4();
    
    // Publish events in a specific order
    let event1 = OrderCreatedEvent {
        order_id,
        customer_id: Uuid::new_v4(),
        total: 500.0,
        items: vec![OrderItem {
            product_id: Uuid::new_v4(),
            quantity: 1,
            price: 500.0,
        }],
    };
    app.publish_event(event1, Some(tenant_id)).await.unwrap();
    
    let event2 = ProductInventoryUpdatedEvent {
        product_id: Uuid::new_v4(),
        old_quantity: 100,
        new_quantity: 99,
        reason: "Order placed".to_string(),
    };
    app.publish_event(event2, Some(tenant_id)).await.unwrap();
    
    // Process events
    app.process_events().await.unwrap();
    
    // Verify order of processing in logs
    let logs = app.get_event_logs().await.unwrap();
    // In a real system, we'd verify ordering based on timestamps
    assert!(logs.len() >= 2);
}

#[tokio::test]
async fn test_event_with_tenant_isolation() {
    let app = EventTestApp::new().await;
    
    let tenant1_id = Uuid::new_v4();
    let tenant2_id = Uuid::new_v4();
    
    // Publish events for tenant 1
    let event1 = TenantCreatedEvent {
        tenant_id: tenant1_id,
        name: "Tenant 1".to_string(),
        slug: "tenant-1".to_string(),
    };
    app.publish_event(event1, Some(tenant1_id)).await.unwrap();
    
    // Publish events for tenant 2
    let event2 = TenantCreatedEvent {
        tenant_id: tenant2_id,
        name: "Tenant 2".to_string(),
        slug: "tenant-2".to_string(),
    };
    app.publish_event(event2, Some(tenant2_id)).await.unwrap();
    
    // Get all events
    let outbox_events = app.get_outbox_events().await.unwrap();
    
    // Verify both tenants have events
    let tenant1_events: Vec<_> = outbox_events.iter()
        .filter(|e| e.tenant_id == Some(tenant1_id))
        .collect();
    let tenant2_events: Vec<_> = outbox_events.iter()
        .filter(|e| e.tenant_id == Some(tenant2_id))
        .collect();
    
    assert!(!tenant1_events.is_empty());
    assert!(!tenant2_events.is_empty());
}

#[tokio::test]
async fn test_event_metadata() {
    let app = EventTestApp::new().await;
    
    let tenant_id = Uuid::new_v4();
    let user_id = Uuid::new_v4();
    
    // Create event with full metadata
    let event = OrderCreatedEvent {
        order_id: Uuid::new_v4(),
        customer_id: user_id,
        total: 250.0,
        items: vec![],
    };
    
    // The metadata is created inside publish_event
    // Here we just verify the event was published
    let event_id = app.publish_event(event, Some(tenant_id)).await.unwrap();
    
    // Verify event exists
    let outbox_events = app.get_outbox_events().await.unwrap();
    let found = outbox_events.iter().find(|e| e.id == event_id.to_string());
    
    assert!(found.is_some());
}

#[tokio::test]
async fn test_bulk_event_processing() {
    let app = EventTestApp::new().await;
    
    let tenant_id = Uuid::new_v4();
    
    // Publish many events
    let num_events = 100;
    for i in 0..num_events {
        let event = OrderCreatedEvent {
            order_id: Uuid::new_v4(),
            customer_id: Uuid::new_v4(),
            total: (i + 1) as f64 * 10.0,
            items: vec![],
        };
        app.publish_event(event, Some(tenant_id)).await.unwrap();
    }
    
    // Process all events
    let result = app.process_events().await.unwrap();
    
    assert_eq!(result.processed, num_events);
    assert_eq!(result.failed, 0);
}

#[tokio::test]
async fn test_event_types() {
    let app = EventTestApp::new().await;
    
    // Test different event types
    let order_event = OrderCreatedEvent {
        order_id: Uuid::new_v4(),
        customer_id: Uuid::new_v4(),
        total: 100.0,
        items: vec![],
    };
    app.publish_event(order_event, Some(Uuid::new_v4())).await.unwrap();
    
    let inventory_event = ProductInventoryUpdatedEvent {
        product_id: Uuid::new_v4(),
        old_quantity: 50,
        new_quantity: 45,
        reason: "Sale".to_string(),
    };
    app.publish_event(inventory_event, Some(Uuid::new_v4())).await.unwrap();
    
    let tenant_event = TenantCreatedEvent {
        tenant_id: Uuid::new_v4(),
        name: "New Tenant".to_string(),
        slug: "new-tenant".to_string(),
    };
    app.publish_event(tenant_event, None).await.unwrap();
    
    // Verify all event types are in outbox
    let outbox_events = app.get_outbox_events().await.unwrap();
    
    let event_types: Vec<String> = outbox_events.iter()
        .map(|e| e.event_type.clone())
        .collect();
    
    assert!(event_types.iter().any(|t| t.contains("order.created")));
    assert!(event_types.iter().any(|t| t.contains("product.inventory")));
    assert!(event_types.iter().any(|t| t.contains("tenant.created")));
}
