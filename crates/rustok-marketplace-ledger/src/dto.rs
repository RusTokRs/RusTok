use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

pub const MAX_LEDGER_ENTRIES_PER_PAGE: u64 = 200;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceLedgerTransactionStatus {
    Posted,
    Reversed,
}

impl MarketplaceLedgerTransactionStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Posted => "posted",
            Self::Reversed => "reversed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "posted" => Some(Self::Posted),
            "reversed" => Some(Self::Reversed),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceLedgerEntryDirection {
    Debit,
    Credit,
}

impl MarketplaceLedgerEntryDirection {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Debit => "debit",
            Self::Credit => "credit",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "debit" => Some(Self::Debit),
            "credit" => Some(Self::Credit),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceLedgerAccountCode {
    MarketplaceClearing,
    PlatformCommissionRevenue,
    SellerPayable,
}

impl MarketplaceLedgerAccountCode {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::MarketplaceClearing => "marketplace_clearing",
            Self::PlatformCommissionRevenue => "platform_commission_revenue",
            Self::SellerPayable => "seller_payable",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "marketplace_clearing" => Some(Self::MarketplaceClearing),
            "platform_commission_revenue" => Some(Self::PlatformCommissionRevenue),
            "seller_payable" => Some(Self::SellerPayable),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct PostMarketplaceOrderLedgerInput {
    pub order_id: Uuid,
    pub posted_at: DateTime<FixedOffset>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct MarketplaceLedgerEntryResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub transaction_id: Uuid,
    pub order_id: Uuid,
    pub assessment_id: Uuid,
    pub allocation_id: Uuid,
    pub order_line_item_id: Uuid,
    pub seller_id: Option<Uuid>,
    pub account_code: MarketplaceLedgerAccountCode,
    pub direction: MarketplaceLedgerEntryDirection,
    pub currency_code: String,
    pub amount: i64,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<FixedOffset>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct MarketplaceLedgerTransactionResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub source_kind: String,
    pub source_id: Uuid,
    pub order_id: Uuid,
    pub currency_code: String,
    pub debit_total_amount: i64,
    pub credit_total_amount: i64,
    pub status: MarketplaceLedgerTransactionStatus,
    pub posted_at: DateTime<FixedOffset>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<FixedOffset>,
    pub entries: Vec<MarketplaceLedgerEntryResponse>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct ReadMarketplaceOrderLedgerRequest {
    pub order_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct ListMarketplaceSellerLedgerEntriesRequest {
    pub seller_id: Uuid,
    pub currency_code: Option<String>,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct MarketplaceLedgerEntryListResponse {
    pub items: Vec<MarketplaceLedgerEntryResponse>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct MarketplaceSellerPayableBalanceResponse {
    pub seller_id: Uuid,
    pub currency_code: String,
    pub credit_total_amount: i64,
    pub debit_total_amount: i64,
    pub balance_amount: i64,
}
