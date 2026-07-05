//! High-level model helpers for `user_field_definitions`.

use sea_orm::prelude::*;
use std::collections::HashMap;

use rustok_core::field_schema::{FieldDefinition, FieldType, ValidationRule};

pub use super::_entities::user_field_definitions::{ActiveModel, Column, Entity, Model, Relation};

// Maximum number of field definitions per entity type per tenant.
// Enforced in `UserFieldService::create`.
pub const MAX_FIELDS_PER_TENANT: usize = 50;

impl Entity {
    /// Load all active definitions for a tenant, ordered by position.
    pub async fn find_active_by_tenant(
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<Vec<Model>, DbErr> {
        use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, QueryOrder};

        Self::find()
            .filter(Column::TenantId.eq(tenant_id))
            .filter(Column::IsActive.eq(true))
            .order_by_asc(Column::Position)
            .all(db)
            .await
    }
}

flex::impl_field_definition_source!(Model);

impl Model {
    /// Convert a DB row into the portable `FieldDefinition` DTO.
    pub fn into_field_definition(self) -> Option<FieldDefinition> {
        flex::field_definition_from_source(&self)
    }
}

/// Input for creating a new field definition.
#[derive(Debug, Clone)]
pub struct CreateFieldDefinitionInput {
    pub field_key: String,
    pub field_type: FieldType,
    pub label: HashMap<String, String>,
    pub description: Option<HashMap<String, String>>,
    pub is_localized: bool,
    pub is_required: bool,
    pub default_value: Option<serde_json::Value>,
    pub validation: Option<ValidationRule>,
    pub position: Option<i32>,
}

/// Input for updating an existing field definition.
#[derive(Debug, Clone, Default)]
pub struct UpdateFieldDefinitionInput {
    pub label: Option<HashMap<String, String>>,
    pub description: Option<HashMap<String, String>>,
    pub is_localized: Option<bool>,
    pub is_required: Option<bool>,
    pub default_value: Option<serde_json::Value>,
    pub validation: Option<ValidationRule>,
    pub position: Option<i32>,
    pub is_active: Option<bool>,
}
