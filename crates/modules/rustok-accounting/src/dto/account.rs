use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::domain::AccountType;

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct CreateAccountInput {
    pub code: String,
    pub name: String,
    pub account_type: AccountType,
    pub currency_code: Option<String>,
    pub track_party: Option<bool>,
    pub track_contract: Option<bool>,
    pub track_item: Option<bool>,
    pub track_warehouse: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AccountResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub code: String,
    pub name: String,
    pub account_type: AccountType,
    pub currency_code: Option<String>,
    pub is_active: bool,
    pub track_party: bool,
    pub track_contract: bool,
    pub track_item: bool,
    pub track_warehouse: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TrialBalanceQuery {
    pub currency_code: Option<String>,
    pub party_id: Option<Uuid>,
    pub warehouse_id: Option<Uuid>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TrialBalanceLine {
    pub account_code: String,
    pub account_name: String,
    pub account_type: AccountType,
    pub debit_turnover_minor: i64,
    pub credit_turnover_minor: i64,
    pub closing_balance_minor: i64,
    pub currency_code: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct TrialBalanceResponse {
    pub lines: Vec<TrialBalanceLine>,
    pub total_debit_minor: i64,
    pub total_credit_minor: i64,
}
