use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateRegionInput {
    #[validate(length(min = 1, max = 100))]
    pub name: String,
    #[validate(length(equal = 3))]
    pub currency_code: String,
    pub tax_rate: Decimal,
    pub tax_included: bool,
    pub countries: Vec<String>,
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, Default, ToSchema)]
pub struct UpdateRegionInput {
    #[validate(length(min = 1, max = 100))]
    pub name: Option<String>,
    #[validate(length(equal = 3))]
    pub currency_code: Option<String>,
    pub tax_rate: Option<Decimal>,
    pub tax_included: Option<bool>,
    pub countries: Option<Vec<String>>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct RegionResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub name: String,
    pub currency_code: String,
    pub tax_rate: Decimal,
    pub tax_included: bool,
    pub countries: Vec<String>,
    pub metadata: Value,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}
