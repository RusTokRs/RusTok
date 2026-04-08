use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CurrentTenant {
    pub id: String,
    pub slug: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CommerceAdminBootstrap {
    #[serde(rename = "currentTenant")]
    pub current_tenant: CurrentTenant,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShippingProfileList {
    pub items: Vec<ShippingProfile>,
    pub total: u64,
    pub page: u64,
    #[serde(rename = "perPage")]
    pub per_page: u64,
    #[serde(rename = "hasNext")]
    pub has_next: bool,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ShippingProfile {
    pub id: String,
    #[serde(rename = "tenantId")]
    pub tenant_id: String,
    pub slug: String,
    pub name: String,
    pub description: Option<String>,
    pub active: bool,
    pub metadata: String,
    #[serde(rename = "createdAt")]
    pub created_at: String,
    #[serde(rename = "updatedAt")]
    pub updated_at: String,
}

#[derive(Clone, Debug)]
pub struct ShippingProfileDraft {
    pub slug: String,
    pub name: String,
    pub description: String,
    pub metadata_json: String,
}
