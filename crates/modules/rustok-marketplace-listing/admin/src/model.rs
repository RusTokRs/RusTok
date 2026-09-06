use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MarketplaceListingAdminListItem {
    pub id: String,
    pub seller_id: String,
    pub master_variant_id: String,
    pub seller_sku: String,
    pub market_slug: String,
    pub channel_slug: String,
    pub status: String,
    pub approval_status: String,
    pub current_terms_version: i32,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MarketplaceListingAdminDirectory {
    pub items: Vec<MarketplaceListingAdminListItem>,
    pub total: u64,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MarketplaceListingAdminFilters {
    pub seller_id: Option<String>,
    pub master_variant_id: Option<String>,
    pub market_slug: Option<String>,
    pub channel_slug: Option<String>,
    pub status: Option<String>,
    pub approval_status: Option<String>,
    pub search: Option<String>,
    pub page: u64,
    pub per_page: u64,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MarketplaceListingAdminTerms {
    pub id: String,
    pub listing_id: String,
    pub version: i32,
    pub pricing_reference: Option<String>,
    pub inventory_reference: Option<String>,
    pub fulfillment_profile_slug: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MarketplaceListingAdminRecord {
    pub id: String,
    pub tenant_id: String,
    pub seller_id: String,
    pub master_product_id: String,
    pub master_variant_id: String,
    pub seller_sku: String,
    pub market_slug: String,
    pub channel_slug: String,
    pub status: String,
    pub approval_status: String,
    pub current_terms_version: i32,
    pub current_terms: MarketplaceListingAdminTerms,
    pub metadata: serde_json::Value,
    pub published_at: Option<String>,
    pub approved_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MarketplaceListingAdminEvent {
    pub id: String,
    pub listing_id: String,
    pub actor_id: Option<String>,
    pub event_kind: String,
    pub locale: Option<String>,
    pub provenance: String,
    pub note: Option<String>,
    pub metadata: serde_json::Value,
    pub created_at: String,
}

impl MarketplaceListingAdminEvent {
    pub fn has_unknown_attribution(&self) -> bool {
        self.provenance == "legacy_snapshot" && self.actor_id.is_none() && self.locale.is_none()
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MarketplaceListingAdminDetail {
    pub listing: MarketplaceListingAdminRecord,
    pub events: Vec<MarketplaceListingAdminEvent>,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MarketplaceListingCreateDraft {
    pub seller_id: String,
    pub master_variant_id: String,
    pub seller_sku: String,
    pub market_slug: String,
    pub channel_slug: String,
    pub pricing_reference: Option<String>,
    pub inventory_reference: Option<String>,
    pub fulfillment_profile_slug: Option<String>,
    pub metadata: serde_json::Value,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MarketplaceListingTermsDraft {
    pub pricing_reference: Option<String>,
    pub inventory_reference: Option<String>,
    pub fulfillment_profile_slug: Option<String>,
    pub metadata: serde_json::Value,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum MarketplaceListingAdminCommand {
    Create {
        draft: MarketplaceListingCreateDraft,
    },
    UpdateTerms {
        listing_id: String,
        draft: MarketplaceListingTermsDraft,
    },
    SubmitForReview {
        listing_id: String,
    },
    Review {
        listing_id: String,
        approved: bool,
        note: Option<String>,
    },
    Publish {
        listing_id: String,
    },
    Suspend {
        listing_id: String,
        reason: String,
    },
    Reactivate {
        listing_id: String,
    },
    Archive {
        listing_id: String,
    },
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum MarketplaceListingAdminAction {
    List,
    Read,
    Create,
    Update,
    Moderate,
    Publish,
    Manage,
}

impl MarketplaceListingAdminAction {
    pub const fn permission(self) -> rustok_api::Permission {
        match self {
            Self::List => rustok_api::Permission::MARKETPLACE_LISTINGS_LIST,
            Self::Read => rustok_api::Permission::MARKETPLACE_LISTINGS_READ,
            Self::Create => rustok_api::Permission::MARKETPLACE_LISTINGS_CREATE,
            Self::Update => rustok_api::Permission::MARKETPLACE_LISTINGS_UPDATE,
            Self::Moderate => rustok_api::Permission::MARKETPLACE_LISTINGS_MODERATE,
            Self::Publish => rustok_api::Permission::MARKETPLACE_LISTINGS_PUBLISH,
            Self::Manage => rustok_api::Permission::MARKETPLACE_LISTINGS_MANAGE,
        }
    }
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MarketplaceListingAdminCommandResult {
    pub listing: MarketplaceListingAdminRecord,
}

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MarketplaceListingAdminShell {
    pub title: String,
    pub subtitle: String,
    pub empty_state: String,
    pub legacy_attribution_label: String,
    pub transport_profile: String,
}
