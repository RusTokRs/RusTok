use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{
    CreateFlexEntryCommand, CreateFlexSchemaCommand, FieldDefinitionsConfigParseError,
    FlexEntryView, FlexSchemaView, UpdateFlexEntryCommand, UpdateFlexSchemaCommand,
    parse_field_definitions_config,
};

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateFlexSchemaRequest {
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub fields_config: serde_json::Value,
    pub settings: Option<serde_json::Value>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UpdateFlexSchemaRequest {
    pub name: Option<String>,
    pub description: Option<String>,
    pub fields_config: Option<serde_json::Value>,
    pub settings: Option<serde_json::Value>,
    pub is_active: Option<bool>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct CreateFlexEntryRequest {
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub data: serde_json::Value,
    pub status: Option<String>,
}

#[derive(Debug, Deserialize, Serialize, ToSchema)]
pub struct UpdateFlexEntryRequest {
    pub data: Option<serde_json::Value>,
    pub status: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FlexSchemaResponse {
    pub id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub fields_config: serde_json::Value,
    pub settings: serde_json::Value,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct FlexEntryResponse {
    pub id: Uuid,
    pub schema_id: Uuid,
    pub entity_type: Option<String>,
    pub entity_id: Option<Uuid>,
    pub data: serde_json::Value,
    pub status: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct DeleteFlexResponse {
    pub success: bool,
}

impl DeleteFlexResponse {
    pub fn success() -> Self {
        Self { success: true }
    }
}

impl CreateFlexSchemaRequest {
    pub fn into_command(self) -> Result<CreateFlexSchemaCommand, FieldDefinitionsConfigParseError> {
        Ok(CreateFlexSchemaCommand {
            slug: self.slug,
            name: self.name,
            description: self.description,
            fields_config: parse_field_definitions_config(self.fields_config)?,
            settings: self.settings,
            is_active: self.is_active,
        })
    }
}

impl UpdateFlexSchemaRequest {
    pub fn into_command(self) -> Result<UpdateFlexSchemaCommand, FieldDefinitionsConfigParseError> {
        Ok(UpdateFlexSchemaCommand {
            name: self.name,
            description: self.description,
            fields_config: self
                .fields_config
                .map(parse_field_definitions_config)
                .transpose()?,
            settings: self.settings,
            is_active: self.is_active,
        })
    }
}

impl CreateFlexEntryRequest {
    pub fn into_command(self, schema_id: Uuid) -> CreateFlexEntryCommand {
        CreateFlexEntryCommand {
            schema_id,
            entity_type: self.entity_type,
            entity_id: self.entity_id,
            data: self.data,
            status: self.status,
        }
    }
}

impl UpdateFlexEntryRequest {
    pub fn into_command(self) -> UpdateFlexEntryCommand {
        UpdateFlexEntryCommand {
            data: self.data,
            status: self.status,
        }
    }
}

impl From<FlexSchemaView> for FlexSchemaResponse {
    fn from(view: FlexSchemaView) -> Self {
        Self {
            id: view.id,
            slug: view.slug,
            name: view.name,
            description: view.description,
            fields_config: serde_json::to_value(view.fields_config)
                .unwrap_or_else(|_| serde_json::Value::Array(Vec::new())),
            settings: view.settings,
            is_active: view.is_active,
            created_at: view.created_at,
            updated_at: view.updated_at,
        }
    }
}

impl From<FlexEntryView> for FlexEntryResponse {
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
