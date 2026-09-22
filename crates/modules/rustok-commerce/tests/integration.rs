use std::collections::HashMap;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use rustok_core::EventBus;
use rustok_core::events::{
    DomainEvent, EventDispatcher, EventEnvelope, EventHandler, HandlerResult,
};
use uuid::Uuid;

#[derive(Clone, Default)]
struct ProductIndexProjection {
    documents: Arc<Mutex<HashMap<Uuid, String>>>,
}

impl ProductIndexProjection {
    fn upsert(&self, product_id: Uuid, status: &str) {
        self.documents
            .lock()
            .expect("product projection lock poisoned")
            .insert(product_id, status.to_string());
    }

    fn get(&self, product_id: Uuid) -> Option<String> {
        self.documents
            .lock()
            .expect("product projection lock poisoned")
            .get(&product_id)
            .cloned()
    }

    fn len(&self) -> usize {
        self.documents
            .lock()
            .expect("product projection lock poisoned")
            .len()
    }
}

#[derive(Clone)]
struct ProductCreatedIndexHandler {
    projection: ProductIndexProjection,
    processed_count: Arc<AtomicUsize>,
}

impl ProductCreatedIndexHandler {
    fn new(projection: ProductIndexProjection, processed_count: Arc<AtomicUsize>) -> Self {
        Self {
            projection,
            processed_count,
        }
    }
}

#[async_trait]
impl EventHandler for ProductCreatedIndexHandler {
    fn name(&self) -> &'static str {
        "product_created_index_handler"
    }

    fn handles(&self, event: &DomainEvent) -> bool {
        matches!(event, DomainEvent::ProductCreated { .. })
    }

    async fn handle(&self, envelope: &EventEnvelope) -> HandlerResult {
        if let DomainEvent::ProductCreated { product_id } = &envelope.event {
            self.projection.upsert(*product_id, "indexed");
            self.processed_count.fetch_add(1, Ordering::Relaxed);
        }
        Ok(())
    }
}

#[tokio::test]
async fn test_product_created_event_updates_index_projection() {
    let tenant_id = Uuid::new_v4();
    let product_id = Uuid::new_v4();

    let bus = EventBus::new();
    let mut event_stream = bus.subscribe();

    let projection = ProductIndexProjection::default();
    let processed_count = Arc::new(AtomicUsize::new(0));

    let mut dispatcher = EventDispatcher::new(bus.clone());
    dispatcher.register(ProductCreatedIndexHandler::new(
        projection.clone(),
        Arc::clone(&processed_count),
    ));
    let running_dispatcher = dispatcher.start();

    bus.publish(tenant_id, None, DomainEvent::ProductCreated { product_id })
        .expect("must publish ProductCreated event");

    let envelope = tokio::time::timeout(std::time::Duration::from_secs(1), event_stream.recv())
        .await
        .expect("must receive published event")
        .expect("event stream should stay open");

    assert!(matches!(
        envelope.event,
        DomainEvent::ProductCreated { product_id: event_product_id } if event_product_id == product_id
    ));

    wait_until(|| processed_count.load(Ordering::Relaxed) == 1).await;

    assert_eq!(processed_count.load(Ordering::Relaxed), 1);
    assert_eq!(projection.get(product_id).as_deref(), Some("indexed"));
    assert_eq!(projection.len(), 1);

    running_dispatcher.stop();
}

#[tokio::test]
async fn test_product_created_event_repeat_is_idempotent_for_index_projection() {
    let tenant_id = Uuid::new_v4();
    let product_id = Uuid::new_v4();

    let bus = EventBus::new();
    let projection = ProductIndexProjection::default();
    let processed_count = Arc::new(AtomicUsize::new(0));

    let mut dispatcher = EventDispatcher::new(bus.clone());
    dispatcher.register(ProductCreatedIndexHandler::new(
        projection.clone(),
        Arc::clone(&processed_count),
    ));
    let running_dispatcher = dispatcher.start();

    bus.publish(tenant_id, None, DomainEvent::ProductCreated { product_id })
        .expect("first ProductCreated publish must succeed");
    bus.publish(tenant_id, None, DomainEvent::ProductCreated { product_id })
        .expect("second ProductCreated publish must succeed");

    wait_until(|| processed_count.load(Ordering::Relaxed) >= 2).await;

    assert_eq!(processed_count.load(Ordering::Relaxed), 2);
    assert_eq!(projection.get(product_id).as_deref(), Some("indexed"));
    assert_eq!(projection.len(), 1, "projection must stay deduplicated");

    running_dispatcher.stop();
}

async fn wait_until(condition: impl Fn() -> bool) {
    for _ in 0..40 {
        if condition() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(25)).await;
    }

    panic!("condition was not met within the expected time");
}
