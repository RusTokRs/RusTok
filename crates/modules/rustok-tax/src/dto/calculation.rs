use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use uuid::Uuid;

use crate::domain::calculation::TaxPolicySnapshot;

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TaxableAmount {
    pub line_item_id: Option<Uuid>,
    pub shipping_option_id: Option<Uuid>,
    pub item_tax_class: Option<String>,
    pub shipping_tax_class: Option<String>,
    pub description: Option<String>,
    pub amount: Decimal,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TaxCalculationInput {
    pub currency_code: String,
    pub channel_id: Option<Uuid>,
    pub customer_tax_exempt: bool,
    pub policy: TaxPolicySnapshot,
    pub taxable_amounts: Vec<TaxableAmount>,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct CalculatedTaxLine {
    pub line_item_id: Option<Uuid>,
    pub shipping_option_id: Option<Uuid>,
    pub description: Option<String>,
    pub provider_id: String,
    pub rate: Decimal,
    pub amount: Decimal,
    pub currency_code: String,
    pub metadata: Value,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq)]
pub struct TaxCalculationResult {
    pub tax_total: Decimal,
    pub tax_included: bool,
    pub lines: Vec<CalculatedTaxLine>,
}
