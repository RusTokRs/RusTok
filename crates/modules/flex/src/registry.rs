//! Field-definition routing by `entity_type` for Flex APIs.
//! Registry is generic and does not depend on concrete domain modules.

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use rustok_core::field_schema::{
    FieldDefinition, FieldType, FlexError, ValidationRule, is_valid_field_key,
};
use rustok_events::{DomainEvent, EventEnvelope};

/// Service-layer representation of a field definition.
///
/// Kept outside GraphQL types so registry contracts stay transport-agnostic.
#[derive(Debug, Clone)]
pub struct FieldDefinitionView {
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

/// Source shape for converting persisted field-definition rows into Flex views.
pub trait FieldDefinitionViewSource {
    fn id(&self) -> Uuid;
    fn field_key(&self) -> &str;
    fn field_type(&self) -> &str;
    fn label(&self) -> &JsonValue;
    fn description(&self) -> Option<&JsonValue>;
    fn is_localized(&self) -> bool;
    fn is_required(&self) -> bool;
    fn default_value(&self) -> Option<&JsonValue>;
    fn validation(&self) -> Option<&JsonValue>;
    fn position(&self) -> i32;
    fn is_active(&self) -> bool;
    fn created_at(&self) -> String;
    fn updated_at(&self) -> String;
}

/// Source shape for converting persisted rows into portable core field definitions.
pub trait FieldDefinitionSource {
    fn field_key(&self) -> &str;
    fn field_type(&self) -> &str;
    fn label(&self) -> &JsonValue;
    fn description(&self) -> Option<&JsonValue>;
    fn is_localized(&self) -> bool;
    fn is_required(&self) -> bool;
    fn default_value(&self) -> Option<&JsonValue>;
    fn validation(&self) -> Option<&JsonValue>;
    fn position(&self) -> i32;
    fn is_active(&self) -> bool;
}

pub fn field_definition_from_source<T: FieldDefinitionSource>(
    source: &T,
) -> Option<FieldDefinition> {
    let field_type: FieldType =
        serde_json::from_value(JsonValue::String(source.field_type().to_string())).ok()?;

    let label: HashMap<String, String> =
        serde_json::from_value(source.label().clone()).unwrap_or_default();

    let description: Option<HashMap<String, String>> = source
        .description()
        .and_then(|value| serde_json::from_value(value.clone()).ok());

    let validation: Option<ValidationRule> = source
        .validation()
        .and_then(|value| serde_json::from_value(value.clone()).ok());

    Some(FieldDefinition {
        field_key: source.field_key().to_string(),
        field_type,
        label,
        description,
        is_localized: source.is_localized(),
        is_required: source.is_required(),
        default_value: source.default_value().cloned(),
        validation,
        position: source.position(),
        is_active: source.is_active(),
    })
}

#[macro_export]
macro_rules! impl_field_definition_source {
    ($model:ty) => {
        impl $crate::FieldDefinitionSource for $model {
            fn field_key(&self) -> &str {
                &self.field_key
            }

            fn field_type(&self) -> &str {
                &self.field_type
            }

            fn label(&self) -> &serde_json::Value {
                &self.label
            }

            fn description(&self) -> Option<&serde_json::Value> {
                self.description.as_ref()
            }

            fn is_localized(&self) -> bool {
                self.is_localized
            }

            fn is_required(&self) -> bool {
                self.is_required
            }

            fn default_value(&self) -> Option<&serde_json::Value> {
                self.default_value.as_ref()
            }

            fn validation(&self) -> Option<&serde_json::Value> {
                self.validation.as_ref()
            }

            fn position(&self) -> i32 {
                self.position
            }

            fn is_active(&self) -> bool {
                self.is_active
            }
        }
    };
}

impl FieldDefinitionView {
    pub fn from_source<T: FieldDefinitionViewSource>(source: &T) -> Self {
        Self {
            id: source.id(),
            field_key: source.field_key().to_string(),
            field_type: source.field_type().to_string(),
            label: source.label().clone(),
            description: source.description().cloned(),
            is_localized: source.is_localized(),
            is_required: source.is_required(),
            default_value: source.default_value().cloned(),
            validation: source.validation().cloned(),
            position: source.position(),
            is_active: source.is_active(),
            created_at: source.created_at(),
            updated_at: source.updated_at(),
        }
    }
}

/// Transport-agnostic input for creating a field definition.
#[derive(Debug, Clone)]
pub struct CreateFieldDefinitionCommand {
    pub field_key: String,
    pub field_type: rustok_core::field_schema::FieldType,
    pub label: std::collections::HashMap<String, String>,
    pub description: Option<std::collections::HashMap<String, String>>,
    pub is_localized: bool,
    pub is_required: bool,
    pub default_value: Option<JsonValue>,
    pub validation: Option<rustok_core::field_schema::ValidationRule>,
    pub position: Option<i32>,
}

/// Transport-agnostic input for updating a field definition.
#[derive(Debug, Clone, Default)]
pub struct UpdateFieldDefinitionCommand {
    pub label: Option<std::collections::HashMap<String, String>>,
    pub description: Option<std::collections::HashMap<String, String>>,
    pub is_localized: Option<bool>,
    pub is_required: Option<bool>,
    pub default_value: Option<JsonValue>,
    pub validation: Option<rustok_core::field_schema::ValidationRule>,
    pub position: Option<i32>,
    pub is_active: Option<bool>,
}

/// Validate owner-owned create guardrails for attached field definitions.
pub fn validate_field_definition_create(
    entity_type: &str,
    field_key: &str,
    has_duplicate_key: bool,
    active_count: u64,
    max_fields: usize,
) -> Result<(), FlexError> {
    if !is_valid_field_key(field_key) {
        return Err(FlexError::InvalidFieldKey(field_key.to_string()));
    }

    if has_duplicate_key {
        return Err(FlexError::DuplicateFieldKey(field_key.to_string()));
    }

    if active_count >= max_fields as u64 {
        return Err(FlexError::TooManyFields {
            entity_type: entity_type.to_string(),
            max: max_fields,
        });
    }

    Ok(())
}

/// Resolve explicit position or append after current active definitions.
pub fn field_definition_position_or_next(position: Option<i32>, active_count: u64) -> i32 {
    position.unwrap_or(active_count as i32)
}

/// Stable persisted string form for a field definition type.
pub fn field_definition_type_name(field_type: FieldType) -> String {
    serde_json::to_value(field_type)
        .ok()
        .and_then(|v| v.as_str().map(ToOwned::to_owned))
        .unwrap_or_default()
}

pub fn field_definition_label_json(label: &HashMap<String, String>) -> JsonValue {
    serde_json::to_value(label).unwrap_or_default()
}

pub fn field_definition_description_json(description: &HashMap<String, String>) -> JsonValue {
    serde_json::to_value(description).unwrap_or_default()
}

pub fn field_definition_validation_json(validation: &ValidationRule) -> JsonValue {
    serde_json::to_value(validation).unwrap_or_default()
}

pub fn field_definition_cache_invalidation_target(event: &DomainEvent) -> Option<(Uuid, &str)> {
    match event {
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
        } => Some((*tenant_id, entity_type.as_str())),
        _ => None,
    }
}

pub fn field_definition_created_event(
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    entity_type: &str,
    field_key: String,
    field_type: String,
) -> EventEnvelope {
    EventEnvelope::new(
        tenant_id,
        actor_id,
        DomainEvent::FieldDefinitionCreated {
            tenant_id,
            entity_type: entity_type.to_string(),
            field_key,
            field_type,
        },
    )
}

pub fn field_definition_updated_event(
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    entity_type: &str,
    field_key: String,
) -> EventEnvelope {
    EventEnvelope::new(
        tenant_id,
        actor_id,
        DomainEvent::FieldDefinitionUpdated {
            tenant_id,
            entity_type: entity_type.to_string(),
            field_key,
        },
    )
}

pub fn field_definition_deleted_event(
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    entity_type: &str,
    field_key: String,
) -> EventEnvelope {
    EventEnvelope::new(
        tenant_id,
        actor_id,
        DomainEvent::FieldDefinitionDeleted {
            tenant_id,
            entity_type: entity_type.to_string(),
            field_key,
        },
    )
}

/// Implement conversion from Flex field-definition commands into an adapter input pair.
#[macro_export]
macro_rules! impl_field_definition_command_conversions {
    ($create:ty, $update:ty) => {
        impl From<$crate::CreateFieldDefinitionCommand> for $create {
            fn from(input: $crate::CreateFieldDefinitionCommand) -> Self {
                Self {
                    field_key: input.field_key,
                    field_type: input.field_type,
                    label: input.label,
                    description: input.description,
                    is_localized: input.is_localized,
                    is_required: input.is_required,
                    default_value: input.default_value,
                    validation: input.validation,
                    position: input.position,
                }
            }
        }

        impl From<$crate::UpdateFieldDefinitionCommand> for $update {
            fn from(input: $crate::UpdateFieldDefinitionCommand) -> Self {
                Self {
                    label: input.label,
                    description: input.description,
                    is_localized: input.is_localized,
                    is_required: input.is_required,
                    default_value: input.default_value,
                    validation: input.validation,
                    position: input.position,
                    is_active: input.is_active,
                }
            }
        }
    };
}

/// Runtime contract for read/reorder operations on field definitions.
#[async_trait]
pub trait FieldDefinitionService: Send + Sync {
    /// Entity type key (for example: `"user"`, `"product"`).
    fn entity_type(&self) -> &'static str;

    async fn list_all(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
    ) -> Result<Vec<FieldDefinitionView>, FlexError>;

    async fn find_by_id(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        id: Uuid,
    ) -> Result<Option<FieldDefinitionView>, FlexError>;

    async fn reorder(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        ids: &[Uuid],
    ) -> Result<Vec<FieldDefinitionView>, FlexError>;

    async fn create(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        input: CreateFieldDefinitionCommand,
    ) -> Result<(FieldDefinitionView, rustok_events::EventEnvelope), FlexError>;

    async fn update(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        id: Uuid,
        input: UpdateFieldDefinitionCommand,
    ) -> Result<(FieldDefinitionView, rustok_events::EventEnvelope), FlexError>;

    async fn deactivate(
        &self,
        db: &DatabaseConnection,
        tenant_id: Uuid,
        actor_id: Option<Uuid>,
        id: Uuid,
    ) -> Result<rustok_events::EventEnvelope, FlexError>;
}

/// Registry that resolves `entity_type -> service`.
pub struct FieldDefRegistry {
    services: HashMap<&'static str, Arc<dyn FieldDefinitionService>>,
}

impl Default for FieldDefRegistry {
    fn default() -> Self {
        Self::new()
    }
}

impl FieldDefRegistry {
    pub fn new() -> Self {
        Self {
            services: HashMap::new(),
        }
    }

    pub fn register(&mut self, service: Arc<dyn FieldDefinitionService>) {
        self.services.insert(service.entity_type(), service);
    }

    pub fn get(&self, entity_type: &str) -> Result<Arc<dyn FieldDefinitionService>, FlexError> {
        self.services
            .get(entity_type)
            .cloned()
            .ok_or_else(|| FlexError::UnknownEntityType(entity_type.to_string()))
    }
}
