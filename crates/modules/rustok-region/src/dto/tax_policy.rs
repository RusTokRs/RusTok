use rust_decimal::Decimal;

use super::RegionCountryTaxPolicyResponse;

/// Region-owned tax configuration for an already selected cart region.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegionTaxPolicy {
    pub provider_id: Option<String>,
    pub channel_provider_id: Option<String>,
    pub tax_rate: Decimal,
    pub tax_included: bool,
    pub country_rules: Vec<RegionCountryTaxPolicyResponse>,
}
