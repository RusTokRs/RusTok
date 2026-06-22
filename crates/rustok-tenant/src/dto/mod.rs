use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct CreateTenantInput {
    pub name: String,
    pub slug: String,
    pub domain: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UpdateTenantInput {
    pub name: Option<String>,
    pub domain: Option<String>,
    pub is_active: Option<bool>,
    pub settings: Option<serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantResponse {
    pub id: Uuid,
    pub name: String,
    pub slug: String,
    pub domain: Option<String>,
    pub is_active: bool,
    pub default_locale: String,
    pub settings: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantModuleResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub module_slug: String,
    pub enabled: bool,
    pub settings: serde_json::Value,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToggleModuleInput {
    pub module_slug: String,
    pub enabled: bool,
}
