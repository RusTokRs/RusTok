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
    #[serde(rename = "taxProviderId")]
    pub tax_provider_id: Option<String>,
    #[serde(rename = "taxRate")]
    pub tax_rate: String,
    #[serde(rename = "taxIncluded")]
    pub tax_included: bool,
    #[serde(rename = "countryTaxPolicies")]
    pub country_tax_policies: Vec<StorefrontRegionCountryTaxPolicy>,
    pub countries: Vec<String>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct StorefrontRegionCountryTaxPolicy {
    #[serde(rename = "countryCode")]
    pub country_code: String,
    #[serde(rename = "taxRate")]
    pub tax_rate: String,
    #[serde(rename = "taxIncluded")]
    pub tax_included: bool,
}
