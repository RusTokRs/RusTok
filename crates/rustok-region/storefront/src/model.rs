use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontRegionsData {
    pub regions: Vec<StorefrontRegion>,
    pub selected_region: Option<StorefrontRegion>,
    pub selected_region_id: Option<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontRegion {
    pub id: String,
    pub name: String,
    #[serde(rename = "currencyCode")]
    pub currency_code: String,
    #[serde(rename = "taxRate")]
    pub tax_rate: String,
    #[serde(rename = "taxIncluded")]
    pub tax_included: bool,
    pub countries: Vec<String>,
}
