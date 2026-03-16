//! Transport-agnostic orchestration helpers for Flex attached mode.

use sea_orm::DatabaseConnection;
use uuid::Uuid;

use rustok_core::field_schema::FlexError;
use rustok_events::EventEnvelope;

use crate::registry::{
    CreateFieldDefinitionCommand, FieldDefRegistry, FieldDefinitionView,
    UpdateFieldDefinitionCommand,
};

/// Optional cache abstraction used by orchestration helpers.
pub trait FieldDefinitionCachePort {
    fn get(&self, tenant_id: Uuid, entity_type: &str) -> Option<Vec<FieldDefinitionView>>;
    fn set(&self, tenant_id: Uuid, entity_type: &str, rows: Vec<FieldDefinitionView>);
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
