use std::sync::Arc;

use rustok_api::{PortContext, PortError};
use rustok_marketplace_payout::{
    ListMarketplaceSellerPayoutsRequest, MarketplacePayoutListResponse, MarketplacePayoutReadPort,
    MarketplacePayoutResponse, ReadMarketplacePayoutRequest,
};
use uuid::Uuid;

/// Marketplace family consumer over payout-owned read projections.
///
/// The family root never imports payout entities or database connections.
pub struct MarketplacePayoutDirectoryService {
    payout_reader: Arc<dyn MarketplacePayoutReadPort>,
}

impl MarketplacePayoutDirectoryService {
    pub fn new(payout_reader: Arc<dyn MarketplacePayoutReadPort>) -> Self {
        Self { payout_reader }
    }

    pub async fn read_payout(
        &self,
        context: PortContext,
        payout_id: Uuid,
    ) -> Result<MarketplacePayoutResponse, PortError> {
        if payout_id.is_nil() {
            return Err(PortError::validation(
                "marketplace.payout.payout_id",
                "payout_id must not be nil",
            ));
        }
        self.payout_reader
            .read_payout(context, ReadMarketplacePayoutRequest { payout_id })
            .await
    }

    pub async fn list_seller_payouts(
        &self,
        context: PortContext,
        request: ListMarketplaceSellerPayoutsRequest,
    ) -> Result<MarketplacePayoutListResponse, PortError> {
        if request.seller_id.is_nil() {
            return Err(PortError::validation(
                "marketplace.payout.seller_id",
                "seller_id must not be nil",
            ));
        }
        self.payout_reader
            .list_seller_payouts(context, request)
            .await
    }
}
