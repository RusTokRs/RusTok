use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MarketplaceSellerAdminListItem {
    pub id: String,
    pub handle: String,
    pub resolved_locale: String,
    pub display_name: String,
    pub status: String,
    pub onboarding_status: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MarketplaceSellerAdminDirectory {
    pub items: Vec<MarketplaceSellerAdminListItem>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MarketplaceSellerAdminFilters {
    pub search: Option<String>,
    pub status: Option<String>,
    pub onboarding_status: Option<String>,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MarketplaceSellerAdminRecord {
    pub id: String,
    pub tenant_id: String,
    pub handle: String,
    pub resolved_locale: String,
    pub display_name: String,
    pub legal_name: Option<String>,
    pub status: String,
    pub onboarding_status: String,
    pub onboarding_note: Option<String>,
    pub suspension_reason: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
    pub activated_at: Option<String>,
    pub suspended_at: Option<String>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MarketplaceSellerAdminMember {
    pub id: String,
    pub seller_id: String,
    pub user_id: String,
    pub role: String,
    pub status: String,
    pub invited_by_actor_id: Option<String>,
    pub accepted_at: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MarketplaceSellerAdminEvent {
    pub id: String,
    pub seller_id: String,
    pub actor_id: Option<String>,
    pub event_kind: String,
    pub locale: Option<String>,
    pub provenance: String,
    pub note: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MarketplaceSellerAdminEventHistory {
    pub seller_id: String,
    pub items: Vec<MarketplaceSellerAdminEvent>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MarketplaceSellerAdminDetail {
    pub seller: MarketplaceSellerAdminRecord,
    pub members: Vec<MarketplaceSellerAdminMember>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MarketplaceSellerCreateDraft {
    pub handle: String,
    pub display_name: String,
    pub legal_name: Option<String>,
    pub owner_user_id: String,
    pub metadata: serde_json::Value,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MarketplaceSellerProfileDraft {
    pub display_name: Option<String>,
    pub legal_name: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MarketplaceSellerMemberCreateDraft {
    pub user_id: String,
    pub role: String,
    pub metadata: serde_json::Value,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MarketplaceSellerMemberUpdateDraft {
    pub role: Option<String>,
    pub status: Option<String>,
    pub metadata: Option<serde_json::Value>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MarketplaceSellerAdminCommand {
    Create {
        draft: MarketplaceSellerCreateDraft,
    },
    UpdateProfile {
        seller_id: String,
        draft: MarketplaceSellerProfileDraft,
    },
    SubmitOnboarding {
        seller_id: String,
        note: Option<String>,
    },
    ReviewOnboarding {
        seller_id: String,
        approved: bool,
        note: Option<String>,
    },
    Suspend {
        seller_id: String,
        reason: String,
    },
    Reactivate {
        seller_id: String,
    },
    AddMember {
        seller_id: String,
        draft: MarketplaceSellerMemberCreateDraft,
    },
    UpdateMember {
        seller_id: String,
        member_id: String,
        draft: MarketplaceSellerMemberUpdateDraft,
    },
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MarketplaceSellerAdminCommandResult {
    pub seller: Option<MarketplaceSellerAdminRecord>,
    pub member: Option<MarketplaceSellerAdminMember>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MarketplaceSellerAdminShell {
    pub title: String,
    pub subtitle: String,
    pub empty_state: String,
    pub transport_profile: String,
}
