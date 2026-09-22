use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateRegionInput {
    #[validate(length(min = 1, message = "At least one translation required"))]
    #[validate(nested)]
    pub translations: Vec<RegionTranslationInput>,
    #[validate(length(equal = 3))]
    pub currency_code: String,
    pub tax_provider_id: Option<String>,
    pub tax_rate: Decimal,
    pub tax_included: bool,
    #[validate(nested)]
    pub country_tax_policies: Option<Vec<RegionCountryTaxPolicyInput>>,
    pub countries: Vec<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, Default, ToSchema)]
pub struct UpdateRegionInput {
    #[validate(nested)]
    pub translations: Option<Vec<RegionTranslationInput>>,
    #[validate(length(equal = 3))]
    pub currency_code: Option<String>,
    pub tax_provider_id: Option<Option<String>>,
    pub tax_rate: Option<Decimal>,
    pub tax_included: Option<bool>,
    #[validate(nested)]
    pub country_tax_policies: Option<Vec<RegionCountryTaxPolicyInput>>,
    pub countries: Option<Vec<String>>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct RegionTranslationInput {
    #[validate(length(min = 2, max = 5))]
    pub locale: String,
    #[validate(length(min = 1, max = 100))]
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct RegionCountryTaxPolicyInput {
    #[validate(length(equal = 2))]
    pub country_code: String,
    pub tax_rate: Decimal,
    pub tax_included: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RegionResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub currency_code: String,
    pub tax_provider_id: Option<String>,
    pub tax_rate: Decimal,
    pub tax_included: bool,
    pub country_tax_policies: Vec<RegionCountryTaxPolicyResponse>,
    pub countries: Vec<String>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub requested_locale: Option<String>,
    pub effective_locale: Option<String>,
    pub available_locales: Vec<String>,
    pub translations: Vec<RegionTranslationResponse>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RegionTranslationResponse {
    pub locale: String,
    pub name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RegionCountryTaxPolicyResponse {
    pub country_code: String,
    pub tax_rate: Decimal,
    pub tax_included: bool,
}
