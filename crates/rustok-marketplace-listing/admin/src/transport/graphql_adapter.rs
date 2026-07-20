use crate::model::{
    MarketplaceListingAdminCommand, MarketplaceListingAdminCommandResult,
    MarketplaceListingAdminDetail, MarketplaceListingAdminDirectory,
    MarketplaceListingAdminFilters,
};

pub type GraphqlMarketplaceListingAdminError = String;

const UNMOUNTED: &str = "marketplace listing GraphQL transport is not mounted; the host must provide module-owned listing queries and mutations before selecting the graphql profile";

pub async fn load_directory(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    _filters: MarketplaceListingAdminFilters,
) -> Result<MarketplaceListingAdminDirectory, GraphqlMarketplaceListingAdminError> {
    Err(UNMOUNTED.to_string())
}

pub async fn load_detail(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    _listing_id: String,
) -> Result<MarketplaceListingAdminDetail, GraphqlMarketplaceListingAdminError> {
    Err(UNMOUNTED.to_string())
}

pub async fn execute_command(
    _token: Option<String>,
    _tenant_slug: Option<String>,
    _idempotency_key: String,
    _command: MarketplaceListingAdminCommand,
) -> Result<MarketplaceListingAdminCommandResult, GraphqlMarketplaceListingAdminError> {
    Err(UNMOUNTED.to_string())
}
