use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema, Validate)]
pub struct CreateShippingProfileInput {
    #[validate(length(
        min = 1,
        max = 64,
        message = "Shipping profile slug must be 1-64 characters"
    ))]
    pub slug: String,
    #[validate(length(
        min = 1,
        max = 255,
        message = "Shipping profile name must be 1-255 characters"
    ))]
    pub name: String,
    #[validate(length(
        max = 1_024,
        message = "Shipping profile description must be max 1024 characters"
    ))]
    pub description: Option<String>,
    #[serde(default)]
    pub metadata: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema, Validate)]
pub struct UpdateShippingProfileInput {
    #[validate(length(
        min = 1,
        max = 64,
        message = "Shipping profile slug must be 1-64 characters"
    ))]
    pub slug: Option<String>,
    #[validate(length(
        min = 1,
        max = 255,
        message = "Shipping profile name must be 1-255 characters"
    ))]
    pub name: Option<String>,
    #[validate(length(
        max = 1_024,
        message = "Shipping profile description must be max 1024 characters"
    ))]
    pub description: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default, ToSchema)]
pub struct ListShippingProfilesInput {
    #[serde(default = "default_page")]
    pub page: u64,
    #[serde(default = "default_per_page")]
    pub per_page: u64,
    pub active: Option<bool>,
    pub search: Option<String>,
}

fn default_page() -> u64 {
    1
}

fn default_per_page() -> u64 {
    20
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ShippingProfileResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub active: bool,
    pub metadata: serde_json::Value,
    pub created_at: chrono::DateTime<chrono::Utc>,
    pub updated_at: chrono::DateTime<chrono::Utc>,
}
