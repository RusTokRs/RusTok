use std::sync::Arc;

use rustok_api::{PortContext, PortError};
use rustok_marketplace_listing::{
    ListMarketplaceListingsInput, MarketplaceListingEligibilityProjection,
    MarketplaceListingEligibilityRequest, MarketplaceListingListResponse,
    MarketplaceListingReadPort, MarketplaceListingResponse, ReadMarketplaceListingRequest,
};
use uuid::Uuid;

/// Marketplace family consumer over listing-owned read projections.
///
/// The family root composes listing directory and eligibility reads without
/// importing listing entities or database connections.
pub struct MarketplaceListingDirectoryService {
    listing_reader: Arc<dyn MarketplaceListingReadPort>,
}

impl MarketplaceListingDirectoryService {
    pub fn new(listing_reader: Arc<dyn MarketplaceListingReadPort>) -> Self {
        Self { listing_reader }
    }

    pub async fn read_listing(
        &self,
        context: PortContext,
        listing_id: Uuid,
    ) -> Result<MarketplaceListingResponse, PortError> {
        self.listing_reader
            .read_listing(context, ReadMarketplaceListingRequest { listing_id })
            .await
    }

    pub async fn list_listings(
        &self,
        context: PortContext,
        request: ListMarketplaceListingsInput,
    ) -> Result<MarketplaceListingListResponse, PortError> {
        self.listing_reader.list_listings(context, request).await
    }

    pub async fn list_eligibility(
        &self,
        context: PortContext,
        request: MarketplaceListingEligibilityRequest,
    ) -> Result<Vec<MarketplaceListingEligibilityProjection>, PortError> {
        self.listing_reader.list_eligibility(context, request).await
    }
}
