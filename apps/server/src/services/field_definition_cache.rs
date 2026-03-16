//! Cache for Flex field definitions schema/list queries.

use std::sync::Arc;
use std::time::Duration;

use loco_rs::app::AppContext;
use moka::future::Cache;
use rustok_core::{DomainEvent, EventBus, EventConsumerRuntime};
use tokio::task::JoinHandle;
use uuid::Uuid;

use crate::services::field_definition_registry::FieldDefinitionView;

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

    pub fn invalidate(&self, tenant_id: Uuid, entity_type: &str) {
        self.inner.invalidate(&(tenant_id, entity_type.to_string()));
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
                        cache_for_task.invalidate(tenant_id, entity_type);
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

#[cfg(test)]
mod tests {
    use super::FieldDefinitionCache;
    use crate::services::field_definition_registry::FieldDefinitionView;
    use serde_json::json;
    use uuid::Uuid;

    fn mock_view(field_key: &str) -> FieldDefinitionView {
        FieldDefinitionView {
            id: Uuid::new_v4(),
            field_key: field_key.to_string(),
            field_type: "text".to_string(),
            label: json!({"en": field_key}),
            description: None,
            is_required: false,
            default_value: None,
            validation: None,
            position: 0,
            is_active: true,
            created_at: chrono::Utc::now().to_rfc3339(),
            updated_at: chrono::Utc::now().to_rfc3339(),
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

        cache.invalidate(tenant_id, entity_type);
        assert!(cache.get(tenant_id, entity_type).await.is_none());
    }
}
