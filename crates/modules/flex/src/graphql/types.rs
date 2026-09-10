use async_graphql::{Enum, InputObject, SimpleObject};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use crate::{
    FieldDefinitionView, FlexAttachedFieldPolicyResolution, FlexDataClassification, FlexEntryView,
    FlexSchemaView,
};

#[derive(Debug, Clone, SimpleObject)]
pub struct FieldDefinitionObject {
    pub id: Uuid,
    pub field_key: String,
    pub field_type: String,
    pub label: JsonValue,
    pub description: Option<JsonValue>,
    pub is_localized: bool,
    pub is_required: bool,
    pub default_value: Option<JsonValue>,
    pub validation: Option<JsonValue>,
    pub position: i32,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<FieldDefinitionView> for FieldDefinitionObject {
    fn from(view: FieldDefinitionView) -> Self {
        Self {
            id: view.id,
            field_key: view.field_key,
            field_type: view.field_type,
            label: view.label,
            description: view.description,
            is_localized: view.is_localized,
            is_required: view.is_required,
            default_value: view.default_value,
            validation: view.validation,
            position: view.position,
            is_active: view.is_active,
            created_at: view.created_at,
            updated_at: view.updated_at,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Enum)]
pub enum AttachedFieldDataClassification {
    Public,
    TenantPrivate,
    Personal,
    Sensitive,
    Secret,
    ImmutableTransaction,
}

impl From<FlexDataClassification> for AttachedFieldDataClassification {
    fn from(value: FlexDataClassification) -> Self {
        match value {
            FlexDataClassification::Public => Self::Public,
            FlexDataClassification::TenantPrivate => Self::TenantPrivate,
            FlexDataClassification::Personal => Self::Personal,
            FlexDataClassification::Sensitive => Self::Sensitive,
            FlexDataClassification::Secret => Self::Secret,
            FlexDataClassification::ImmutableTransaction => Self::ImmutableTransaction,
        }
    }
}

impl From<AttachedFieldDataClassification> for FlexDataClassification {
    fn from(value: AttachedFieldDataClassification) -> Self {
        match value {
            AttachedFieldDataClassification::Public => Self::Public,
            AttachedFieldDataClassification::TenantPrivate => Self::TenantPrivate,
            AttachedFieldDataClassification::Personal => Self::Personal,
            AttachedFieldDataClassification::Sensitive => Self::Sensitive,
            AttachedFieldDataClassification::Secret => Self::Secret,
            AttachedFieldDataClassification::ImmutableTransaction => Self::ImmutableTransaction,
        }
    }
}

#[derive(Debug, Clone, SimpleObject)]
pub struct AttachedFieldPolicyObject {
    pub entity_type: String,
    pub field_key: String,
    pub classification: AttachedFieldDataClassification,
    pub ai_export_allowed: bool,
    /// False means the effective fail-closed default is in force and no policy row exists.
    pub explicit: bool,
}

impl AttachedFieldPolicyObject {
    pub(crate) fn from_resolution(
        entity_type: String,
        field_key: String,
        resolution: FlexAttachedFieldPolicyResolution,
    ) -> Self {
        Self {
            entity_type,
            field_key,
            classification: resolution.policy.classification.into(),
            ai_export_allowed: resolution.policy.ai_export_allowed,
            explicit: resolution.explicit,
        }
    }
}

#[derive(Debug, Clone, InputObject)]
pub struct SetAttachedFieldPolicyInput {
    pub entity_type: Option<String>,
    pub field_key: String,
    pub classification: AttachedFieldDataClassification,
    pub ai_export_allowed: bool,
}

#[derive(Debug, Clone, InputObject)]
pub struct ResetAttachedFieldPolicyInput {
    pub entity_type: Option<String>,
    pub field_key: String,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct AttachedValuesObject {
    pub entity_type: String,
    pub entity_id: Uuid,
    pub values: Option<JsonValue>,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct FlexSchemaObject {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub fields_config: JsonValue,
    pub settings: JsonValue,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

impl From<FlexSchemaView> for FlexSchemaObject {
    fn from(view: FlexSchemaView) -> Self {
        Self {
            id: view.id,
            slug: view.slug,
            name: view.name,
            description: view.description,
            fields_config: serde_json::to_value(view.fields_config)
                .unwrap_or_else(|_| JsonValue::Array(Vec::new())),
            settings: view.settings,
            is_active: view.is_active,
            created_at: view.created_at,
            updated_at: view.updated_at,
        }
    }
}

#[derive(Debug, Clone, SimpleObject)]
pub struct FlexEntryObject {
    pub id: Uuid,
    pub schema_id: Uuid,
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub data: JsonValue,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

impl From<FlexEntryView> for FlexEntryObject {
    fn from(view: FlexEntryView) -> Self {
        Self {
            id: view.id,
            schema_id: view.schema_id,
            entity_type: view.entity_type,
            entity_id: view.entity_id,
            data: view.data,
            status: view.status,
            created_at: view.created_at,
            updated_at: view.updated_at,
        }
    }
}

#[derive(Debug, Clone, InputObject)]
pub struct CreateFlexSchemaInput {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub fields_config: JsonValue,
    pub settings: Option<JsonValue>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, InputObject)]
pub struct UpdateFlexSchemaInput {
    pub name: Option<String>,
    pub description: Option<String>,
    pub fields_config: Option<JsonValue>,
    pub settings: Option<JsonValue>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Clone, InputObject)]
pub struct CreateFlexEntryInput {
    pub schema_id: Uuid,
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub data: JsonValue,
    pub status: Option<String>,
}

#[derive(Debug, Clone, InputObject)]
pub struct UpdateFlexEntryInput {
    pub data: Option<JsonValue>,
    pub status: Option<String>,
}

#[derive(Debug, Clone, InputObject)]
pub struct UpdateAttachedValuesInput {
    pub entity_type: Option<String>,
    pub entity_id: Uuid,
    pub locale: String,
    pub values: Option<JsonValue>,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct DeleteFlexPayload {
    pub success: bool,
}

#[derive(Debug, Clone, SimpleObject)]
pub struct DeleteFieldDefinitionPayload {
    pub success: bool,
}

#[derive(Debug, Clone, InputObject)]
pub struct CreateFieldDefinitionInput {
    pub entity_type: Option<String>,
    pub field_key: String,
    pub field_type: String,
    pub label: JsonValue,
    pub description: Option<JsonValue>,
    #[graphql(default)]
    pub is_localized: bool,
    #[graphql(default)]
    pub is_required: bool,
    pub default_value: Option<JsonValue>,
    pub validation: Option<JsonValue>,
    pub position: Option<i32>,
}

#[derive(Debug, Clone, InputObject)]
pub struct UpdateFieldDefinitionInput {
    pub entity_type: Option<String>,
    pub label: Option<JsonValue>,
    pub description: Option<JsonValue>,
    pub is_localized: Option<bool>,
    pub is_required: Option<bool>,
    pub default_value: Option<JsonValue>,
    pub validation: Option<JsonValue>,
    pub position: Option<i32>,
    pub is_active: Option<bool>,
}
