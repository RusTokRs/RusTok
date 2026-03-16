//! Server-side registration of field-definition services.
//! This file wires concrete module implementations into generic registry.

use std::sync::Arc;

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use rustok_core::field_schema::FlexError;

use crate::models::user_field_definitions::Model;
use crate::services::field_definition_registry::{
    FieldDefRegistry, FieldDefinitionService, FieldDefinitionView,
};
use crate::services::user_field_service::UserFieldService;

struct UserFieldDefinitionService;

#[async_trait]
impl FieldDefinitionService for UserFieldDefinitionService {
    fn entity_type(&self) -> &'static str {
        "user"
    }

    async fn list_all(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<Vec<FieldDefinitionView>, FlexError> {
        let rows = UserFieldService::list_all(db, tenant_id).await?;
        Ok(rows.into_iter().map(FieldDefinitionView::from).collect())
    }

    async fn find_by_id(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<FieldDefinitionView>, FlexError> {
        let row = UserFieldService::find_by_id(db, tenant_id, id).await?;
        Ok(row.map(FieldDefinitionView::from))
    }

    async fn reorder(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        ids: &[Uuid],
    ) -> Result<Vec<FieldDefinitionView>, FlexError> {
        let rows = UserFieldService::reorder(db, tenant_id, ids).await?;
        Ok(rows.into_iter().map(FieldDefinitionView::from).collect())
    }
}

impl From<Model> for FieldDefinitionView {
    fn from(m: Model) -> Self {
        Self {
            id: m.id,
            field_key: m.field_key,
            field_type: m.field_type,
            label: m.label,
            description: m.description,
            is_required: m.is_required,
            default_value: m.default_value,
            validation: m.validation,
            position: m.position,
            is_active: m.is_active,
            created_at: m.created_at.to_rfc3339(),
            updated_at: m.updated_at.to_rfc3339(),
        }
    }
}

pub fn build_field_def_registry() -> FieldDefRegistry {
    let mut registry = FieldDefRegistry::new();
    registry.register(Arc::new(UserFieldDefinitionService));
    registry
}
