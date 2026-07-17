use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceListingStatus {
    Draft,
    PendingReview,
    Active,
    Suspended,
    Archived,
}

impl MarketplaceListingStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::PendingReview => "pending_review",
            Self::Active => "active",
            Self::Suspended => "suspended",
            Self::Archived => "archived",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "draft" => Some(Self::Draft),
            "pending_review" => Some(Self::PendingReview),
            "active" => Some(Self::Active),
            "suspended" => Some(Self::Suspended),
            "archived" => Some(Self::Archived),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceListingApprovalStatus {
    Draft,
    Pending,
    Approved,
    Rejected,
}

impl MarketplaceListingApprovalStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Pending => "pending",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "draft" => Some(Self::Draft),
            "pending" => Some(Self::Pending),
            "approved" => Some(Self::Approved),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateMarketplaceListingInput {
    pub seller_id: Uuid,
    pub master_variant_id: Uuid,
    #[validate(length(min = 1, max = 120))]
    pub seller_sku: String,
    #[validate(length(min = 1, max = 80))]
    pub market_slug: String,
    #[validate(length(min = 1, max = 80))]
    pub channel_slug: String,
    #[validate(length(max = 191))]
    pub pricing_reference: Option<String>,
    #[validate(length(max = 191))]
    pub inventory_reference: Option<String>,
    #[validate(length(max = 120))]
    pub fulfillment_profile_slug: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct UpdateMarketplaceListingTermsInput {
    pub listing_id: Uuid,
    #[validate(length(max = 191))]
    pub pricing_reference: Option<String>,
    #[validate(length(max = 191))]
    pub inventory_reference: Option<String>,
    #[validate(length(max = 120))]
    pub fulfillment_profile_slug: Option<String>,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct ReviewMarketplaceListingInput {
    pub listing_id: Uuid,
    pub approved: bool,
    #[validate(length(max = 2000))]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct SuspendMarketplaceListingInput {
    pub listing_id: Uuid,
    #[validate(length(min = 1, max = 1000))]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MarketplaceListingTermsResponse {
    pub id: Uuid,
    pub listing_id: Uuid,
    pub version: i32,
    pub pricing_reference: Option<String>,
    pub inventory_reference: Option<String>,
    pub fulfillment_profile_slug: Option<String>,
    pub metadata: Value,
    pub created_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MarketplaceListingResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub seller_id: Uuid,
    pub master_product_id: Uuid,
    pub master_variant_id: Uuid,
    pub seller_sku: String,
    pub market_slug: String,
    pub channel_slug: String,
    pub status: MarketplaceListingStatus,
    pub approval_status: MarketplaceListingApprovalStatus,
    pub approval_note: Option<String>,
    pub suspension_reason: Option<String>,
    pub current_terms_version: i32,
    pub current_terms: MarketplaceListingTermsResponse,
    pub metadata: Value,
    pub published_at: Option<DateTime<FixedOffset>>,
    pub approved_at: Option<DateTime<FixedOffset>>,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ListMarketplaceListingsInput {
    pub page: u64,
    pub per_page: u64,
    pub seller_id: Option<Uuid>,
    pub master_variant_id: Option<Uuid>,
    pub market_slug: Option<String>,
    pub channel_slug: Option<String>,
    pub status: Option<MarketplaceListingStatus>,
    pub approval_status: Option<MarketplaceListingApprovalStatus>,
    pub search: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadMarketplaceListingRequest {
    pub listing_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct MarketplaceListingEligibilityRequest {
    pub master_variant_id: Uuid,
    pub market_slug: String,
    pub channel_slug: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MarketplaceListingEligibilityProjection {
    pub listing: MarketplaceListingResponse,
    pub eligible: bool,
    pub reason_codes: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MarketplaceListingListResponse {
    pub items: Vec<MarketplaceListingResponse>,
    pub total: u64,
}
