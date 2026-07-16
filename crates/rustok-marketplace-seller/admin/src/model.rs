use serde::{Deserialize, Serialize};

#[derive(Clone, Debug, Default, Deserialize, Serialize, PartialEq, Eq)]
pub struct MarketplaceSellerAdminListItem {
    pub id: String,
    pub handle: String,
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
pub struct MarketplaceSellerAdminShell {
    pub title: String,
    pub subtitle: String,
    pub empty_state: String,
    pub transport_profile: String,
}
