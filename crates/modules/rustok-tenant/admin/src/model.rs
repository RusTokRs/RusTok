use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantAdminBootstrap {
    pub tenant: TenantAdminTenant,
    pub modules: Vec<TenantAdminModule>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantAdminTenant {
    pub id: String,
    pub slug: String,
    pub name: String,
    pub domain: Option<String>,
    pub is_active: bool,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TenantAdminModule {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub enabled: bool,
    pub source: String,
}
