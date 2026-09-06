use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

pub const MAX_LEDGER_ENTRIES_PER_PAGE: u64 = 200;
pub const MAX_LEDGER_REVERSAL_LINES: usize = 500;
pub const MAX_LEDGER_BALANCE_TRANSFER_LINES: usize = 500;

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

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash, ToSchema)]
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

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceLedgerReversalKind {
    Refund,
    Chargeback,
}

impl MarketplaceLedgerReversalKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Refund => "refund",
            Self::Chargeback => "chargeback",
        }
    }

    pub const fn source_kind(self) -> &'static str {
        match self {
            Self::Refund => "refund_reversal",
            Self::Chargeback => "chargeback_reversal",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "refund" => Some(Self::Refund),
            "chargeback" => Some(Self::Chargeback),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceSellerBalanceBucket {
    Pending,
    Available,
    Reserved,
    Paid,
}

impl MarketplaceSellerBalanceBucket {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Pending => "pending",
            Self::Available => "available",
            Self::Reserved => "reserved",
            Self::Paid => "paid",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pending" => Some(Self::Pending),
            "available" => Some(Self::Available),
            "reserved" => Some(Self::Reserved),
            "paid" => Some(Self::Paid),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, Hash, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceSellerBalanceTransferKind {
    PendingRelease,
    ReserveHold,
    ReserveRelease,
    PayoutSettlement,
    PayoutReversal,
}

impl MarketplaceSellerBalanceTransferKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::PendingRelease => "pending_release",
            Self::ReserveHold => "reserve_hold",
            Self::ReserveRelease => "reserve_release",
            Self::PayoutSettlement => "payout_settlement",
            Self::PayoutReversal => "payout_reversal",
        }
    }

    pub const fn source_kind(self) -> &'static str {
        match self {
            Self::PendingRelease => "seller_balance_pending_release",
            Self::ReserveHold => "seller_balance_reserve_hold",
            Self::ReserveRelease => "seller_balance_reserve_release",
            Self::PayoutSettlement => "seller_balance_payout_settlement",
            Self::PayoutReversal => "seller_balance_payout_reversal",
        }
    }

    pub const fn buckets(
        self,
    ) -> (
        MarketplaceSellerBalanceBucket,
        MarketplaceSellerBalanceBucket,
    ) {
        match self {
            Self::PendingRelease => (
                MarketplaceSellerBalanceBucket::Pending,
                MarketplaceSellerBalanceBucket::Available,
            ),
            Self::ReserveHold => (
                MarketplaceSellerBalanceBucket::Available,
                MarketplaceSellerBalanceBucket::Reserved,
            ),
            Self::ReserveRelease => (
                MarketplaceSellerBalanceBucket::Reserved,
                MarketplaceSellerBalanceBucket::Available,
            ),
            Self::PayoutSettlement => (
                MarketplaceSellerBalanceBucket::Reserved,
                MarketplaceSellerBalanceBucket::Paid,
            ),
            Self::PayoutReversal => (
                MarketplaceSellerBalanceBucket::Paid,
                MarketplaceSellerBalanceBucket::Available,
            ),
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "pending_release" => Some(Self::PendingRelease),
            "reserve_hold" => Some(Self::ReserveHold),
            "reserve_release" => Some(Self::ReserveRelease),
            "payout_settlement" => Some(Self::PayoutSettlement),
            "payout_reversal" => Some(Self::PayoutReversal),
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
pub struct MarketplaceLedgerReversalLineInput {
    pub assessment_id: Uuid,
    pub allocation_id: Uuid,
    pub order_line_item_id: Uuid,
    pub seller_id: Uuid,
    pub commission_amount: i64,
    pub seller_amount: i64,
    #[serde(default = "default_pending_bucket")]
    pub seller_balance_bucket: MarketplaceSellerBalanceBucket,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct PostMarketplaceLedgerReversalInput {
    pub kind: MarketplaceLedgerReversalKind,
    pub source_id: Uuid,
    pub order_id: Uuid,
    pub currency_code: String,
    pub reversed_at: DateTime<FixedOffset>,
    pub lines: Vec<MarketplaceLedgerReversalLineInput>,
    #[serde(default = "empty_object")]
    pub metadata: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct MarketplaceLedgerReversalEntryResponse {
    pub entry: MarketplaceLedgerEntryResponse,
    pub reversed_entry_id: Uuid,
    pub seller_balance_bucket: Option<MarketplaceSellerBalanceBucket>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct MarketplaceLedgerReversalResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub transaction_id: Uuid,
    pub kind: MarketplaceLedgerReversalKind,
    pub source_id: Uuid,
    pub order_id: Uuid,
    pub currency_code: String,
    pub total_amount: i64,
    pub reversed_transaction_id: Uuid,
    pub reversed_at: DateTime<FixedOffset>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<FixedOffset>,
    pub transaction: MarketplaceLedgerTransactionResponse,
    pub entries: Vec<MarketplaceLedgerReversalEntryResponse>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct MarketplaceSellerBalanceTransferLineInput {
    pub reference_entry_id: Uuid,
    pub amount: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct PostMarketplaceSellerBalanceTransferInput {
    pub kind: MarketplaceSellerBalanceTransferKind,
    pub source_id: Uuid,
    pub seller_id: Uuid,
    pub currency_code: String,
    pub transferred_at: DateTime<FixedOffset>,
    pub lines: Vec<MarketplaceSellerBalanceTransferLineInput>,
    #[serde(default = "empty_object")]
    pub metadata: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct MarketplaceSellerBalanceTransferLineResponse {
    pub reference_entry_id: Uuid,
    pub amount: i64,
    pub from_bucket: MarketplaceSellerBalanceBucket,
    pub to_bucket: MarketplaceSellerBalanceBucket,
    pub debit_entry: MarketplaceLedgerEntryResponse,
    pub credit_entry: MarketplaceLedgerEntryResponse,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct MarketplaceSellerBalanceTransferResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub transaction_id: Uuid,
    pub kind: MarketplaceSellerBalanceTransferKind,
    pub source_id: Uuid,
    pub seller_id: Uuid,
    pub currency_code: String,
    pub from_bucket: MarketplaceSellerBalanceBucket,
    pub to_bucket: MarketplaceSellerBalanceBucket,
    pub total_amount: i64,
    pub transferred_at: DateTime<FixedOffset>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<FixedOffset>,
    pub transaction: MarketplaceLedgerTransactionResponse,
    pub lines: Vec<MarketplaceSellerBalanceTransferLineResponse>,
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
pub struct ReadMarketplaceSellerBalanceRequest {
    pub seller_id: Uuid,
    pub currency_code: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct RebuildMarketplaceSellerBalanceInput {
    pub seller_id: Uuid,
    pub currency_code: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct MarketplaceSellerBalanceResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub seller_id: Uuid,
    pub currency_code: String,
    pub pending_amount: i64,
    pub available_amount: i64,
    pub reserved_amount: i64,
    pub paid_amount: i64,
    pub negative_amount: i64,
    pub source_entry_count: u64,
    pub last_entry_id: Option<Uuid>,
    pub last_entry_created_at: Option<DateTime<FixedOffset>>,
    pub rebuilt_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

/// Legacy aggregate retained while callers migrate to the bucketed balance projection.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct MarketplaceSellerPayableBalanceResponse {
    pub seller_id: Uuid,
    pub currency_code: String,
    pub credit_total_amount: i64,
    pub debit_total_amount: i64,
    pub balance_amount: i64,
}

fn default_pending_bucket() -> MarketplaceSellerBalanceBucket {
    MarketplaceSellerBalanceBucket::Pending
}

fn empty_object() -> serde_json::Value {
    serde_json::json!({})
}
