use std::sync::Arc;

use rustok_api::{PortContext, PortError};
use rustok_marketplace_seller::{
    ListMarketplaceSellersInput, MarketplaceSellerListResponse, MarketplaceSellerReadPort,
    MarketplaceSellerResponse, ReadMarketplaceSellerRequest,
};
use uuid::Uuid;

/// Root-owned consumer facade over the seller owner provider port.
///
/// The Marketplace root receives only typed projections and never imports seller
/// entities or a seller database connection.
pub struct MarketplaceSellerDirectoryService {
    seller_port: Arc<dyn MarketplaceSellerReadPort>,
}

impl MarketplaceSellerDirectoryService {
    pub fn new(seller_port: Arc<dyn MarketplaceSellerReadPort>) -> Self {
        Self { seller_port }
    }

    pub async fn read_seller(
        &self,
        context: PortContext,
        seller_id: Uuid,
    ) -> Result<MarketplaceSellerResponse, PortError> {
        self.seller_port
            .read_seller(context, ReadMarketplaceSellerRequest { seller_id })
            .await
    }

    pub async fn list_sellers(
        &self,
        context: PortContext,
        input: ListMarketplaceSellersInput,
    ) -> Result<MarketplaceSellerListResponse, PortError> {
        self.seller_port.list_sellers(context, input).await
    }
}
