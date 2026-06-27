//! Cache for Flex field definitions schema/list queries.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use loco_rs::app::AppContext;
use moka::future::Cache;
use rustok_core::{DomainEvent, EventBus, EventConsumerRuntime};
use tokio::task::JoinHandle;
use uuid::Uuid;

use flex::FieldDefinitionView;

const FIELD_DEFINITION_CACHE_TTL: Duration = Duration::from_secs(30);

#[derive(Clone)]
pub struct FieldDefinitionCache {
    inner: Cache<(Uuid, String), Vec<FieldDefinitionView>>,
}

#[derive(Clone)]
pub struct SharedFieldDefinitionCache(pub Arc<FieldDefinitionCache>);

pub struct FieldDefinitionCacheInvalidationHandle {
    _handle: JoinHandle<()>,
}

impl Default for FieldDefinitionCache {
    fn default() -> Self {
        Self::new()
    }
}

impl FieldDefinitionCache {
    pub fn new() -> Self {
        let inner = Cache::builder()
            .time_to_live(FIELD_DEFINITION_CACHE_TTL)
            .max_capacity(10_000)
            .build();

        Self { inner }
    }

    pub async fn get(
        &self,
        tenant_id: Uuid,
        entity_type: &str,
    ) -> Option<Vec<FieldDefinitionView>> {
        self.inner.get(&(tenant_id, entity_type.to_string())).await
    }

    pub async fn set(&self, tenant_id: Uuid, entity_type: &str, rows: Vec<FieldDefinitionView>) {
        self.inner
            .insert((tenant_id, entity_type.to_string()), rows)
            .await;
    }

    pub async fn invalidate(&self, tenant_id: Uuid, entity_type: &str) {
        self.inner
            .invalidate(&(tenant_id, entity_type.to_string()))
            .await;
    }
}

pub fn field_definition_cache_from_context(
    ctx: &AppContext,
    bus: EventBus,
) -> FieldDefinitionCache {
    if let Some(shared) = ctx.shared_store.get::<SharedFieldDefinitionCache>() {
        return (*shared.0).clone();
    }

    let cache = Arc::new(FieldDefinitionCache::new());

    let mut receiver = bus.subscribe();
    let cache_for_task = cache.clone();
    let consumer_runtime = EventConsumerRuntime::new("field_definition_cache_invalidator");
    let handle = tokio::spawn(async move {
        consumer_runtime.restarted("startup");
        loop {
            match receiver.recv().await {
                Ok(envelope) => match envelope.event {
                    DomainEvent::FieldDefinitionCreated {
                        tenant_id,
                        ref entity_type,
                        ..
                    }
                    | DomainEvent::FieldDefinitionUpdated {
                        tenant_id,
                        ref entity_type,
                        ..
                    }
                    | DomainEvent::FieldDefinitionDeleted {
                        tenant_id,
                        ref entity_type,
                        ..
                    } => {
                        cache_for_task.invalidate(tenant_id, entity_type).await;
                    }
                    _ => {}
                },
                Err(tokio::sync::broadcast::error::RecvError::Lagged(skipped)) => {
                    consumer_runtime.lagged(skipped);
                }
                Err(tokio::sync::broadcast::error::RecvError::Closed) => {
                    consumer_runtime.closed();
                    break;
                }
            }
        }
    });

    ctx.shared_store
        .insert(FieldDefinitionCacheInvalidationHandle { _handle: handle });
    ctx.shared_store
        .insert(SharedFieldDefinitionCache(cache.clone()));

    (*cache).clone()
}

#[async_trait]
impl flex::FieldDefinitionCachePort for FieldDefinitionCache {
    async fn get(&self, tenant_id: Uuid, entity_type: &str) -> Option<Vec<FieldDefinitionView>> {
        FieldDefinitionCache::get(self, tenant_id, entity_type).await
    }

    async fn set(&self, tenant_id: Uuid, entity_type: &str, rows: Vec<FieldDefinitionView>) {
        FieldDefinitionCache::set(self, tenant_id, entity_type, rows).await;
    }

    async fn invalidate(&self, tenant_id: Uuid, entity_type: &str) {
        FieldDefinitionCache::invalidate(self, tenant_id, entity_type).await;
    }
}

#[cfg(test)]
mod tests {
    use super::{field_definition_cache_from_context, FieldDefinitionCache};
    use flex::FieldDefinitionView;
    use loco_rs::{
        app::{AppContext, SharedStore},
        cache,
        environment::Environment,
        storage::{self, Storage},
        tests_cfg::config::test_config,
    };
    use rustok_core::EventBus;
    use rustok_events::{DomainEvent, EventEnvelope};
    use sea_orm::{Database, DatabaseConnection};
    use serde_json::json;
    use std::{sync::Arc, time::Duration};
    use tokio::time::sleep;
    use uuid::Uuid;

    fn mock_view(field_key: &str) -> FieldDefinitionView {
        FieldDefinitionView {
            id: Uuid::new_v4(),
            field_key: field_key.to_string(),
            field_type: "text".to_string(),
            label: json!({"en": field_key}),
            description: None,
            is_localized: false,
            is_required: false,
            default_value: None,
            validation: None,
            position: 0,
            is_active: true,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
        }
    }

    fn test_app_context(db: DatabaseConnection) -> AppContext {
        AppContext {
            environment: Environment::Test,
            db,
            queue_provider: None,
            config: test_config(),
            mailer: None,
            storage: Storage::single(storage::drivers::mem::new()).into(),
            cache: Arc::new(cache::Cache::new(cache::drivers::null::new())),
            shared_store: Arc::new(SharedStore::default()),
        }
    }

    #[tokio::test]
    async fn cache_set_get_and_invalidate() {
        let cache = FieldDefinitionCache::new();
        let tenant_id = Uuid::new_v4();
        let entity_type = "user";

        cache
            .set(tenant_id, entity_type, vec![mock_view("nickname")])
            .await;

        let cached = cache.get(tenant_id, entity_type).await;
        assert!(cached.is_some());
        assert_eq!(cached.expect("cache entry")[0].field_key, "nickname");

        cache.invalidate(tenant_id, entity_type).await;
        assert!(cache.get(tenant_id, entity_type).await.is_none());
    }

    async fn assert_event_bus_invalidation_drops_cached_field_definitions(event: DomainEvent) {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("in-memory sqlite db should connect");
        let ctx = test_app_context(db);
        let bus = EventBus::default();
        let cache = field_definition_cache_from_context(&ctx, bus.clone());
        let (tenant_id, entity_type) = match &event {
            DomainEvent::FieldDefinitionCreated {
                tenant_id,
                entity_type,
                ..
            }
            | DomainEvent::FieldDefinitionUpdated {
                tenant_id,
                entity_type,
                ..
            }
            | DomainEvent::FieldDefinitionDeleted {
                tenant_id,
                entity_type,
                ..
            } => (*tenant_id, entity_type.clone()),
            _ => panic!("test helper expects a FieldDefinition event"),
        };

        cache
            .set(tenant_id, &entity_type, vec![mock_view("nickname")])
            .await;
        assert!(cache.get(tenant_id, &entity_type).await.is_some());

        bus.publish_envelope(EventEnvelope::new(tenant_id, None, event))
            .expect("field definition event should publish");

        for _ in 0..20 {
            if cache.get(tenant_id, &entity_type).await.is_none() {
                return;
            }
            sleep(Duration::from_millis(10)).await;
        }

        panic!("cache entry should be invalidated after field definition event");
    }

    #[tokio::test]
    async fn event_bus_invalidation_drops_cached_field_definitions_on_create() {
        let tenant_id = Uuid::new_v4();
        assert_event_bus_invalidation_drops_cached_field_definitions(
            DomainEvent::FieldDefinitionCreated {
                tenant_id,
                entity_type: "user".to_string(),
                field_key: "nickname".to_string(),
                field_type: "text".to_string(),
            },
        )
        .await;
    }

    #[tokio::test]
    async fn event_bus_invalidation_drops_cached_field_definitions_on_update() {
        let tenant_id = Uuid::new_v4();
        assert_event_bus_invalidation_drops_cached_field_definitions(
            DomainEvent::FieldDefinitionUpdated {
                tenant_id,
                entity_type: "user".to_string(),
                field_key: "nickname".to_string(),
            },
        )
        .await;
    }

    #[tokio::test]
    async fn event_bus_invalidation_drops_cached_field_definitions_on_delete() {
        let tenant_id = Uuid::new_v4();
        assert_event_bus_invalidation_drops_cached_field_definitions(
            DomainEvent::FieldDefinitionDeleted {
                tenant_id,
                entity_type: "user".to_string(),
                field_key: "nickname".to_string(),
            },
        )
        .await;
    }
}
