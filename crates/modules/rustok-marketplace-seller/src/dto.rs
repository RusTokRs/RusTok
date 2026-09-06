use chrono::{DateTime, FixedOffset};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use utoipa::ToSchema;
use uuid::Uuid;
use validator::Validate;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceSellerStatus {
    Draft,
    Active,
    Suspended,
    Closed,
}

impl MarketplaceSellerStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Active => "active",
            Self::Suspended => "suspended",
            Self::Closed => "closed",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "draft" => Some(Self::Draft),
            "active" => Some(Self::Active),
            "suspended" => Some(Self::Suspended),
            "closed" => Some(Self::Closed),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceSellerOnboardingStatus {
    Draft,
    Submitted,
    Approved,
    Rejected,
}

impl MarketplaceSellerOnboardingStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Draft => "draft",
            Self::Submitted => "submitted",
            Self::Approved => "approved",
            Self::Rejected => "rejected",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "draft" => Some(Self::Draft),
            "submitted" => Some(Self::Submitted),
            "approved" => Some(Self::Approved),
            "rejected" => Some(Self::Rejected),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceSellerMemberRole {
    Owner,
    Admin,
    Operations,
    Finance,
    Member,
}

impl MarketplaceSellerMemberRole {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Owner => "owner",
            Self::Admin => "admin",
            Self::Operations => "operations",
            Self::Finance => "finance",
            Self::Member => "member",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "owner" => Some(Self::Owner),
            "admin" => Some(Self::Admin),
            "operations" => Some(Self::Operations),
            "finance" => Some(Self::Finance),
            "member" => Some(Self::Member),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceSellerMemberStatus {
    Invited,
    Active,
    Disabled,
}

impl MarketplaceSellerMemberStatus {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Invited => "invited",
            Self::Active => "active",
            Self::Disabled => "disabled",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "invited" => Some(Self::Invited),
            "active" => Some(Self::Active),
            "disabled" => Some(Self::Disabled),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceSellerEventKind {
    Created,
    ProfileUpdated,
    OnboardingSubmitted,
    OnboardingApproved,
    OnboardingRejected,
    Suspended,
    Reactivated,
    MemberAdded,
    MemberUpdated,
    LegacyOnboardingSnapshot,
    LegacySuspensionSnapshot,
}

impl MarketplaceSellerEventKind {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Created => "created",
            Self::ProfileUpdated => "profile_updated",
            Self::OnboardingSubmitted => "onboarding_submitted",
            Self::OnboardingApproved => "onboarding_approved",
            Self::OnboardingRejected => "onboarding_rejected",
            Self::Suspended => "suspended",
            Self::Reactivated => "reactivated",
            Self::MemberAdded => "member_added",
            Self::MemberUpdated => "member_updated",
            Self::LegacyOnboardingSnapshot => "legacy_onboarding_snapshot",
            Self::LegacySuspensionSnapshot => "legacy_suspension_snapshot",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "created" => Some(Self::Created),
            "profile_updated" => Some(Self::ProfileUpdated),
            "onboarding_submitted" => Some(Self::OnboardingSubmitted),
            "onboarding_approved" => Some(Self::OnboardingApproved),
            "onboarding_rejected" => Some(Self::OnboardingRejected),
            "suspended" => Some(Self::Suspended),
            "reactivated" => Some(Self::Reactivated),
            "member_added" => Some(Self::MemberAdded),
            "member_updated" => Some(Self::MemberUpdated),
            "legacy_onboarding_snapshot" => Some(Self::LegacyOnboardingSnapshot),
            "legacy_suspension_snapshot" => Some(Self::LegacySuspensionSnapshot),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceSellerEventProvenance {
    Command,
    LegacySnapshot,
}

impl MarketplaceSellerEventProvenance {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Command => "command",
            Self::LegacySnapshot => "legacy_snapshot",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value {
            "command" => Some(Self::Command),
            "legacy_snapshot" => Some(Self::LegacySnapshot),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct CreateMarketplaceSellerInput {
    #[validate(length(min = 2, max = 80))]
    pub handle: String,
    #[validate(length(min = 1, max = 160))]
    pub display_name: String,
    #[validate(length(max = 240))]
    pub legal_name: Option<String>,
    pub owner_user_id: Uuid,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct UpdateMarketplaceSellerProfileInput {
    #[validate(length(min = 1, max = 160))]
    pub display_name: Option<String>,
    #[validate(length(max = 240))]
    pub legal_name: Option<String>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct SubmitMarketplaceSellerOnboardingInput {
    #[validate(length(max = 2000))]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct ReviewMarketplaceSellerOnboardingInput {
    pub approved: bool,
    #[validate(length(max = 2000))]
    pub note: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Validate, ToSchema)]
pub struct SuspendMarketplaceSellerInput {
    #[validate(length(min = 1, max = 1000))]
    pub reason: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct AddMarketplaceSellerMemberInput {
    pub user_id: Uuid,
    pub role: MarketplaceSellerMemberRole,
    #[serde(default)]
    pub metadata: Value,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct UpdateMarketplaceSellerMemberInput {
    pub role: Option<MarketplaceSellerMemberRole>,
    pub status: Option<MarketplaceSellerMemberStatus>,
    pub metadata: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct ListMarketplaceSellersInput {
    pub page: u64,
    pub per_page: u64,
    pub status: Option<MarketplaceSellerStatus>,
    pub onboarding_status: Option<MarketplaceSellerOnboardingStatus>,
    pub search: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MarketplaceSellerResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub handle: String,
    pub resolved_locale: String,
    pub display_name: String,
    pub legal_name: Option<String>,
    pub status: MarketplaceSellerStatus,
    pub onboarding_status: MarketplaceSellerOnboardingStatus,
    pub onboarding_note: Option<String>,
    pub suspension_reason: Option<String>,
    pub metadata: Value,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
    pub activated_at: Option<DateTime<FixedOffset>>,
    pub suspended_at: Option<DateTime<FixedOffset>>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MarketplaceSellerMemberResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub seller_id: Uuid,
    pub user_id: Uuid,
    pub role: MarketplaceSellerMemberRole,
    pub status: MarketplaceSellerMemberStatus,
    pub invited_by_actor_id: Option<Uuid>,
    pub accepted_at: Option<DateTime<FixedOffset>>,
    pub metadata: Value,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, ToSchema)]
pub struct MarketplaceSellerEventResponse {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub seller_id: Uuid,
    pub actor_id: Option<Uuid>,
    pub event_kind: MarketplaceSellerEventKind,
    pub locale: Option<String>,
    pub provenance: MarketplaceSellerEventProvenance,
    pub note: Option<String>,
    pub metadata: Value,
    pub created_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadMarketplaceSellerRequest {
    pub seller_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ReadMarketplaceSellerMembershipRequest {
    pub seller_id: Uuid,
    pub user_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListMarketplaceSellerMembersRequest {
    pub seller_id: Uuid,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ListMarketplaceSellerEventsRequest {
    pub seller_id: Uuid,
    pub limit: u64,
}
