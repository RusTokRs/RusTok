use crate::core::MarketplaceSellerAdminTransportProfile;
use crate::model::MarketplaceSellerAdminDirectory;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MarketplaceSellerAdminTransportError {
    pub code: String,
    pub message: String,
    pub retryable: bool,
}

pub trait MarketplaceSellerAdminTransport: Send + Sync {
    fn profile(&self) -> MarketplaceSellerAdminTransportProfile;

    fn load_directory(
        &self,
        page: u64,
        per_page: u64,
    ) -> Result<MarketplaceSellerAdminDirectory, MarketplaceSellerAdminTransportError>;
}

/// Explicit placeholder used until the host mounts native or GraphQL adapters.
///
/// It never falls back to another transport and never fabricates seller data.
pub struct UnmountedMarketplaceSellerAdminTransport {
    profile: MarketplaceSellerAdminTransportProfile,
}

impl UnmountedMarketplaceSellerAdminTransport {
    pub const fn new(profile: MarketplaceSellerAdminTransportProfile) -> Self {
        Self { profile }
    }
}

impl MarketplaceSellerAdminTransport for UnmountedMarketplaceSellerAdminTransport {
    fn profile(&self) -> MarketplaceSellerAdminTransportProfile {
        self.profile
    }

    fn load_directory(
        &self,
        _page: u64,
        _per_page: u64,
    ) -> Result<MarketplaceSellerAdminDirectory, MarketplaceSellerAdminTransportError> {
        Err(MarketplaceSellerAdminTransportError {
            code: "marketplace_seller_admin.transport_unmounted".to_string(),
            message: "marketplace seller admin transport is not mounted".to_string(),
            retryable: false,
        })
    }
}
