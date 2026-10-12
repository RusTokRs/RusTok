use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum AccountType {
    /// Assets (Активы, e.g. Cash, Bank, Inventory, Accounts Receivable)
    Asset,
    /// Liabilities (Обязательства, e.g. Accounts Payable, Loans)
    Liability,
    /// Equity (Капитал)
    Equity,
    /// Revenue / Income (Выручка, Доходы)
    Revenue,
    /// Expense / COGS (Себестоимость, Расходы)
    Expense,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AccountDefinition {
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
