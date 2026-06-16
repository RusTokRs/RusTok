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
    pub tax_provider_id: Option<String>,
    pub country_count: usize,
    pub tax_rate: String,
    pub tax_included: bool,
    pub countries_preview: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RegionDetail {
    pub region: RegionRecord,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RegionRecord {
    pub id: String,
    pub tenant_id: String,
    pub name: String,
    pub currency_code: String,
    pub tax_provider_id: Option<String>,
    pub tax_rate: String,
    pub tax_included: bool,
    pub country_tax_policies_pretty: String,
    pub countries: Vec<String>,
    pub metadata_pretty: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub struct RegionDraft {
    pub name: String,
    pub locale: String,
    pub currency_code: String,
    pub tax_provider_id: String,
    pub tax_rate: String,
    pub tax_included: bool,
    pub country_tax_policies: String,
    pub countries: String,
    pub metadata: String,
}
