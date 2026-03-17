//! Transport-agnostic orchestration helpers for Flex attached mode.

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use rustok_core::field_schema::FlexError;
use rustok_events::EventEnvelope;

use crate::registry::{
    CreateFieldDefinitionCommand, FieldDefRegistry, FieldDefinitionView,
    UpdateFieldDefinitionCommand,
};

/// Optional cache abstraction used by orchestration helpers.
#[async_trait]
pub trait FieldDefinitionCachePort: Send + Sync {
    async fn get(&self, tenant_id: Uuid, entity_type: &str) -> Option<Vec<FieldDefinitionView>>;
    async fn set(&self, tenant_id: Uuid, entity_type: &str, rows: Vec<FieldDefinitionView>);
    fn invalidate(&self, tenant_id: Uuid, entity_type: &str);
}

pub async fn list_field_definitions(
    registry: &FieldDefRegistry,
    db: &DatabaseConnection,
    tenant_id: Uuid,
    entity_type: &str,
) -> Result<Vec<FieldDefinitionView>, FlexError> {
    let service = registry.get(entity_type)?;
    service.list_all(db, tenant_id).await
}

pub async fn list_field_definitions_with_cache(
    registry: &FieldDefRegistry,
    db: &DatabaseConnection,
    cache: &dyn FieldDefinitionCachePort,
    tenant_id: Uuid,
    entity_type: &str,
) -> Result<Vec<FieldDefinitionView>, FlexError> {
    if let Some(rows) = cache.get(tenant_id, entity_type).await {
        return Ok(rows);
    }

    let rows = list_field_definitions(registry, db, tenant_id, entity_type).await?;
    cache.set(tenant_id, entity_type, rows.clone()).await;

    Ok(rows)
}

pub fn invalidate_field_definition_cache(
    cache: &dyn FieldDefinitionCachePort,
    tenant_id: Uuid,
    entity_type: &str,
) {
    cache.invalidate(tenant_id, entity_type);
}

pub async fn find_field_definition(
    registry: &FieldDefRegistry,
    db: &DatabaseConnection,
    tenant_id: Uuid,
    entity_type: &str,
    id: Uuid,
) -> Result<Option<FieldDefinitionView>, FlexError> {
    let service = registry.get(entity_type)?;
    service.find_by_id(db, tenant_id, id).await
}

pub async fn reorder_field_definitions(
    registry: &FieldDefRegistry,
    db: &DatabaseConnection,
    tenant_id: Uuid,
    entity_type: &str,
    ids: &[Uuid],
) -> Result<Vec<FieldDefinitionView>, FlexError> {
    let service = registry.get(entity_type)?;
    service.reorder(db, tenant_id, ids).await
}

pub async fn create_field_definition(
    registry: &FieldDefRegistry,
    db: &DatabaseConnection,
    tenant_id: Uuid,
    entity_type: &str,
    actor_id: Option<Uuid>,
    input: CreateFieldDefinitionCommand,
) -> Result<(FieldDefinitionView, EventEnvelope), FlexError> {
    let service = registry.get(entity_type)?;
    service.create(db, tenant_id, actor_id, input).await
}

pub async fn update_field_definition(
    registry: &FieldDefRegistry,
    db: &DatabaseConnection,
    tenant_id: Uuid,
    entity_type: &str,
    actor_id: Option<Uuid>,
    id: Uuid,
    input: UpdateFieldDefinitionCommand,
) -> Result<(FieldDefinitionView, EventEnvelope), FlexError> {
    let service = registry.get(entity_type)?;
    service.update(db, tenant_id, actor_id, id, input).await
}

pub async fn deactivate_field_definition(
    registry: &FieldDefRegistry,
    db: &DatabaseConnection,
    tenant_id: Uuid,
    entity_type: &str,
    actor_id: Option<Uuid>,
    id: Uuid,
) -> Result<EventEnvelope, FlexError> {
    let service = registry.get(entity_type)?;
    service.deactivate(db, tenant_id, actor_id, id).await
}

#[cfg(test)]
mod tests {
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    use async_trait::async_trait;
    use sea_orm::{Database, DatabaseConnection};
    use serde_json::json;
    use uuid::Uuid;

    use rustok_core::field_schema::FlexError;
    use rustok_events::EventEnvelope;

    use crate::registry::{
        CreateFieldDefinitionCommand, FieldDefRegistry, FieldDefinitionService,
        FieldDefinitionView, UpdateFieldDefinitionCommand,
    };

    use super::{list_field_definitions_with_cache, FieldDefinitionCachePort};

    #[derive(Default)]
    struct InMemoryCache {
        rows: std::sync::Mutex<Option<Vec<FieldDefinitionView>>>,
    }

    #[async_trait]
    impl FieldDefinitionCachePort for InMemoryCache {
        async fn get(
            &self,
            _tenant_id: Uuid,
            _entity_type: &str,
        ) -> Option<Vec<FieldDefinitionView>> {
            self.rows.lock().expect("cache lock").clone()
        }

        async fn set(&self, _tenant_id: Uuid, _entity_type: &str, rows: Vec<FieldDefinitionView>) {
            *self.rows.lock().expect("cache lock") = Some(rows);
        }

        fn invalidate(&self, _tenant_id: Uuid, _entity_type: &str) {
            *self.rows.lock().expect("cache lock") = None;
        }
    }

    struct MockService {
        calls: Arc<AtomicUsize>,
        rows: Vec<FieldDefinitionView>,
    }

    #[async_trait]
    impl FieldDefinitionService for MockService {
        fn entity_type(&self) -> &'static str {
            "user"
        }

        async fn list_all(
            &self,
            _db: &DatabaseConnection,
            _tenant_id: Uuid,
        ) -> Result<Vec<FieldDefinitionView>, FlexError> {
            self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.rows.clone())
        }

        async fn find_by_id(
            &self,
            _db: &DatabaseConnection,
            _tenant_id: Uuid,
            _id: Uuid,
        ) -> Result<Option<FieldDefinitionView>, FlexError> {
            Ok(None)
        }

        async fn reorder(
            &self,
            _db: &DatabaseConnection,
            _tenant_id: Uuid,
            _ids: &[Uuid],
        ) -> Result<Vec<FieldDefinitionView>, FlexError> {
            Ok(Vec::new())
        }

        async fn create(
            &self,
            _db: &DatabaseConnection,
            _tenant_id: Uuid,
            _actor_id: Option<Uuid>,
            _input: CreateFieldDefinitionCommand,
        ) -> Result<(FieldDefinitionView, EventEnvelope), FlexError> {
            Err(FlexError::Database("not used in test".to_string()))
        }

        async fn update(
            &self,
            _db: &DatabaseConnection,
            _tenant_id: Uuid,
            _actor_id: Option<Uuid>,
            _id: Uuid,
            _input: UpdateFieldDefinitionCommand,
        ) -> Result<(FieldDefinitionView, EventEnvelope), FlexError> {
            Err(FlexError::Database("not used in test".to_string()))
        }

        async fn deactivate(
            &self,
            _db: &DatabaseConnection,
            _tenant_id: Uuid,
            _actor_id: Option<Uuid>,
            _id: Uuid,
        ) -> Result<EventEnvelope, FlexError> {
            Err(FlexError::Database("not used in test".to_string()))
        }
    }

    fn sample_view() -> FieldDefinitionView {
        FieldDefinitionView {
            id: Uuid::new_v4(),
            field_key: "nickname".to_string(),
            field_type: "text".to_string(),
            label: json!({"en": "Nickname"}),
            description: None,
            is_required: false,
            default_value: None,
            validation: None,
            position: 0,
            is_active: true,
            created_at: "2026-01-01T00:00:00Z".to_string(),
            updated_at: "2026-01-01T00:00:00Z".to_string(),
        }
    }

    async fn test_db() -> DatabaseConnection {
        Database::connect("sqlite::memory:")
            .await
            .expect("sqlite memory db")
    }

    #[tokio::test]
    async fn list_with_cache_hits_service_once_and_uses_cache_afterwards() {
        let db = test_db().await;
        let tenant_id = Uuid::new_v4();
        let mut registry = FieldDefRegistry::new();
        let calls = Arc::new(AtomicUsize::new(0));
        registry.register(Arc::new(MockService {
            calls: calls.clone(),
            rows: vec![sample_view()],
        }));
        let cache = InMemoryCache::default();

        let first = list_field_definitions_with_cache(&registry, &db, &cache, tenant_id, "user")
            .await
            .expect("first call");
        let second = list_field_definitions_with_cache(&registry, &db, &cache, tenant_id, "user")
            .await
            .expect("second call");

        assert_eq!(first.len(), 1);
        assert_eq!(second.len(), 1);
        assert_eq!(calls.load(Ordering::SeqCst), 1);
    }

    #[tokio::test]
    async fn list_with_cache_returns_unknown_entity_type_for_missing_route() {
        let db = test_db().await;
        let tenant_id = Uuid::new_v4();
        let registry = FieldDefRegistry::new();
        let cache = InMemoryCache::default();

        let err = list_field_definitions_with_cache(&registry, &db, &cache, tenant_id, "unknown")
            .await
            .expect_err("unknown entity type should fail");

        assert!(matches!(err, FlexError::UnknownEntityType(_)));
    }
}
