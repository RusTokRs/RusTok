//! Field-definition routing by `entity_type` for Flex APIs.
//! Registry is generic and does not depend on concrete domain modules.

use std::{collections::HashMap, sync::Arc};

use async_trait::async_trait;
use sea_orm::DatabaseConnection;
use serde_json::Value as JsonValue;
use uuid::Uuid;

use rustok_core::field_schema::FlexError;

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
    pub is_required: bool,
    pub default_value: Option<JsonValue>,
    pub validation: Option<JsonValue>,
    pub position: i32,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

/// Transport-agnostic input for creating a field definition.
#[derive(Debug, Clone)]
pub struct CreateFieldDefinitionCommand {
    pub field_key: String,
    pub field_type: rustok_core::field_schema::FieldType,
    pub label: std::collections::HashMap<String, String>,
    pub description: Option<std::collections::HashMap<String, String>>,
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
    pub is_required: Option<bool>,
    pub default_value: Option<JsonValue>,
    pub validation: Option<rustok_core::field_schema::ValidationRule>,
    pub position: Option<i32>,
    pub is_active: Option<bool>,
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
