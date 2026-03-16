//! High-level model helpers for `product_field_definitions`.
//!
//! Provides conversion from a DB row to [`FieldDefinition`], the portable
//! DTO used by [`CustomFieldsSchema`].

use sea_orm::prelude::*;
use std::collections::HashMap;

use rustok_core::field_schema::{FieldDefinition, FieldType, ValidationRule};

pub use super::_entities::product_field_definitions::{
    ActiveModel, Column, Entity, Model, Relation,
};

// Maximum number of field definitions per entity type per tenant.
// Enforced in [`ProductFieldService::create`].
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

impl Model {
    /// Convert a DB row into the portable [`FieldDefinition`] DTO.
    ///
    /// Returns `None` if `field_type` contains an unknown string (forward
    /// compatibility — unknown types are silently skipped by callers).
    pub fn into_field_definition(self) -> Option<FieldDefinition> {
        let field_type: FieldType =
            serde_json::from_value(serde_json::Value::String(self.field_type.clone())).ok()?;

        let label: HashMap<String, String> = serde_json::from_value(self.label).unwrap_or_default();

        let description: Option<HashMap<String, String>> = self
            .description
            .and_then(|v| serde_json::from_value(v).ok());

        let validation: Option<ValidationRule> =
            self.validation.and_then(|v| serde_json::from_value(v).ok());

        Some(FieldDefinition {
            field_key: self.field_key,
            field_type,
            label,
            description,
            is_required: self.is_required,
            default_value: self.default_value,
            validation,
            position: self.position,
            is_active: self.is_active,
        })
    }
}

/// Input for creating a new field definition.
#[derive(Debug, Clone)]
pub struct CreateFieldDefinitionInput {
    pub field_key: String,
    pub field_type: FieldType,
    pub label: HashMap<String, String>,
    pub description: Option<HashMap<String, String>>,
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
    pub is_required: Option<bool>,
    pub default_value: Option<serde_json::Value>,
    pub validation: Option<ValidationRule>,
    pub position: Option<i32>,
    pub is_active: Option<bool>,
}
