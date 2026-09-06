use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

pub const MAX_PAYOUT_ITEMS_PER_BATCH: usize = 500;
pub const MAX_PAYOUTS_PER_PAGE: u64 = 200;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarketplacePayoutStatus {
    Scheduled,
    Processing,
    Paid,
    Failed,
    Cancelled,
}

impl MarketplacePayoutStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Scheduled => "scheduled",
            Self::Processing => "processing",
            Self::Paid => "paid",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "scheduled" => Some(Self::Scheduled),
            "processing" => Some(Self::Processing),
            "paid" => Some(Self::Paid),
            "failed" => Some(Self::Failed),
            "cancelled" => Some(Self::Cancelled),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct ScheduleMarketplacePayoutInput {
    pub seller_id: Uuid,
    pub currency_code: String,
    pub ledger_entry_ids: Vec<Uuid>,
    pub scheduled_for: DateTime<FixedOffset>,
    pub destination_reference: Option<String>,
    #[serde(default = "empty_object")]
    pub metadata: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct MarketplacePayoutItemResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub payout_id: Uuid,
    pub ledger_entry_id: Uuid,
    pub amount: i64,
    pub created_at: DateTime<FixedOffset>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct MarketplacePayoutResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub seller_id: Uuid,
    pub currency_code: String,
    pub total_amount: i64,
    pub status: MarketplacePayoutStatus,
    pub scheduled_for: DateTime<FixedOffset>,
    pub destination_reference: Option<String>,
    pub external_reference: Option<String>,
    pub failure_code: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub paid_at: Option<DateTime<FixedOffset>>,
    pub items: Vec<MarketplacePayoutItemResponse>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct ReadMarketplacePayoutRequest {
    pub payout_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct ListMarketplaceSellerPayoutsRequest {
    pub seller_id: Uuid,
    pub currency_code: Option<String>,
    pub status: Option<MarketplacePayoutStatus>,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct MarketplacePayoutListResponse {
    pub items: Vec<MarketplacePayoutResponse>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

fn empty_object() -> serde_json::Value {
    serde_json::json!({})
}
