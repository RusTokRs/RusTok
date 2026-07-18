use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

pub const MAX_COMMISSION_RULES_PER_PAGE: u64 = 200;
pub const MAX_COMMISSION_ASSESSMENTS_PER_PAGE: u64 = 200;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceCommissionRuleStatus {
    Active,
    Inactive,
}

impl MarketplaceCommissionRuleStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Active => "active",
            Self::Inactive => "inactive",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "active" => Some(Self::Active),
            "inactive" => Some(Self::Inactive),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceCommissionAssessmentStatus {
    Assessed,
    Reversed,
}

impl MarketplaceCommissionAssessmentStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Assessed => "assessed",
            Self::Reversed => "reversed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "assessed" => Some(Self::Assessed),
            "reversed" => Some(Self::Reversed),
            _ => None,
        }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct CreateMarketplaceCommissionRuleVersionInput {
    pub rule_key: Uuid,
    pub seller_id: Option<Uuid>,
    pub listing_id: Option<Uuid>,
    pub rate_bps: i32,
    pub fixed_amount: i64,
    pub currency_code: Option<String>,
    pub priority: i32,
    pub effective_from: DateTime<FixedOffset>,
    pub effective_until: Option<DateTime<FixedOffset>>,
    pub status: MarketplaceCommissionRuleStatus,
    #[serde(default = "empty_object")]
    pub metadata: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct MarketplaceCommissionRuleResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub rule_key: Uuid,
    pub version: i32,
    pub seller_id: Option<Uuid>,
    pub listing_id: Option<Uuid>,
    pub rate_bps: i32,
    pub fixed_amount: i64,
    pub currency_code: Option<String>,
    pub priority: i32,
    pub effective_from: DateTime<FixedOffset>,
    pub effective_until: Option<DateTime<FixedOffset>>,
    pub status: MarketplaceCommissionRuleStatus,
    pub metadata: serde_json::Value,
    pub created_at: DateTime<FixedOffset>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct AssessMarketplaceOrderCommissionsInput {
    pub order_id: Uuid,
    pub assessed_at: DateTime<FixedOffset>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct MarketplaceCommissionAssessmentResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub allocation_id: Uuid,
    pub order_id: Uuid,
    pub order_line_item_id: Uuid,
    pub seller_id: Uuid,
    pub listing_id: Uuid,
    pub rule_id: Uuid,
    pub rule_key: Uuid,
    pub rule_version: i32,
    pub currency_code: String,
    pub allocation_total_amount: i64,
    pub rate_bps: i32,
    pub fixed_amount: i64,
    pub commission_amount: i64,
    pub seller_proceeds_amount: i64,
    pub status: MarketplaceCommissionAssessmentStatus,
    pub metadata: serde_json::Value,
    pub assessed_at: DateTime<FixedOffset>,
    pub created_at: DateTime<FixedOffset>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct AssessMarketplaceOrderCommissionsResponse {
    pub order_id: Uuid,
    pub assessments: Vec<MarketplaceCommissionAssessmentResponse>,
    pub commission_total_amount: i64,
    pub seller_proceeds_total_amount: i64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct ReadMarketplaceCommissionAssessmentRequest {
    pub allocation_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct ListMarketplaceCommissionAssessmentsByOrderRequest {
    pub order_id: Uuid,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct ListMarketplaceCommissionAssessmentsBySellerRequest {
    pub seller_id: Uuid,
    pub page: u64,
    pub per_page: u64,
    pub status: Option<MarketplaceCommissionAssessmentStatus>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct MarketplaceCommissionAssessmentListResponse {
    pub items: Vec<MarketplaceCommissionAssessmentResponse>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct ListMarketplaceCommissionRulesRequest {
    pub rule_key: Option<Uuid>,
    pub seller_id: Option<Uuid>,
    pub listing_id: Option<Uuid>,
    pub status: Option<MarketplaceCommissionRuleStatus>,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq, ToSchema)]
pub struct MarketplaceCommissionRuleListResponse {
    pub items: Vec<MarketplaceCommissionRuleResponse>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

fn empty_object() -> serde_json::Value {
    serde_json::json!({})
}
