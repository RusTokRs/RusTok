//! GraphQL types for the Flex custom fields system.

use async_graphql::{InputObject, SimpleObject};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::models::user_field_definitions::Model;
use crate::services::field_definition_registry::FieldDefinitionView;

/// GraphQL representation of a field definition.
#[derive(Debug, Clone, SimpleObject)]
pub struct FieldDefinitionObject {
    pub id: Uuid,
    /// Tenant-scoped unique key (snake_case, `^[a-z][a-z0-9_]{0,127}$`).
    pub field_key: String,
    /// Serialised [`FieldType`] value, e.g. `"text"`, `"select"`.
    pub field_type: String,
    /// Localised labels as JSON object: `{"en": "Phone", "ru": "Телефон"}`.
    pub label: JsonValue,
    /// Optional localised description.
    pub description: Option<JsonValue>,
    pub is_required: bool,
    /// Default value applied by `apply_defaults()`.
    pub default_value: Option<JsonValue>,
    /// Validation constraints as JSON (min, max, pattern, options, …).
    pub validation: Option<JsonValue>,
    /// Display order (ascending).
    pub position: i32,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<Model> for FieldDefinitionObject {
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

impl From<FieldDefinitionView> for FieldDefinitionObject {
    fn from(m: FieldDefinitionView) -> Self {
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
            created_at: m.created_at,
            updated_at: m.updated_at,
        }
    }
}

// ── Inputs ───────────────────────────────────────────────────────────────────

/// Input for `createFieldDefinition`.
#[derive(Debug, Clone, InputObject)]
pub struct CreateFieldDefinitionInput {
    pub field_key: String,
    /// Serialised field type, e.g. `"text"`, `"select"`, `"integer"`.
    pub field_type: String,
    /// Localised labels JSON: `{"en": "Phone"}`.
    pub label: JsonValue,
    pub description: Option<JsonValue>,
    #[graphql(default)]
    pub is_required: bool,
    pub default_value: Option<JsonValue>,
    pub validation: Option<JsonValue>,
    pub position: Option<i32>,
}

/// Input for `updateFieldDefinition`.
#[derive(Debug, Clone, InputObject)]
pub struct UpdateFieldDefinitionInput {
    pub label: Option<JsonValue>,
    pub description: Option<JsonValue>,
    pub is_required: Option<bool>,
    pub default_value: Option<JsonValue>,
    pub validation: Option<JsonValue>,
    pub position: Option<i32>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct DeleteFieldDefinitionPayload {
    pub success: bool,
}
