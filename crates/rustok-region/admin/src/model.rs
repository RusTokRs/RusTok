use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct CurrentTenant {
    pub id: String,
    pub slug: String,
    pub name: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RegionAdminBootstrap {
    pub current_tenant: CurrentTenant,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RegionList {
    pub items: Vec<RegionListItem>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RegionListItem {
    pub id: String,
    pub name: String,
    pub currency_code: String,
    pub country_count: usize,
    pub tax_rate: String,
    pub tax_included: bool,
    pub countries_preview: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RegionDetail {
    pub region: RegionRecord,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RegionRecord {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub currency_code: String,
    pub tax_rate: String,
    pub tax_included: bool,
    pub countries: Vec<String>,
    pub metadata_pretty: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct RegionDraft {
    pub name: String,
    pub currency_code: String,
    pub tax_rate: String,
    pub tax_included: bool,
    pub countries: String,
    pub metadata: String,
}
