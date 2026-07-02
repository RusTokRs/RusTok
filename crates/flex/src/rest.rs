use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::{FlexEntryView, FlexSchemaView};

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
