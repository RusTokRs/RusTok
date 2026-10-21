use std::sync::Arc;

use rustok_api::{PortContext, PortError};
use rustok_marketplace_allocation::{
    ListMarketplaceAllocationsByOrderRequest, ListMarketplaceAllocationsBySellerRequest,
    MarketplaceAllocationListResponse, MarketplaceAllocationReadPort,
    MarketplaceOrderAllocationResponse, ReadMarketplaceAllocationByLineRequest,
};
use uuid::Uuid;

/// Marketplace family consumer over allocation-owned read projections.
///
/// The family root never imports allocation entities or database connections.
pub struct MarketplaceAllocationDirectoryService {
    allocation_reader: Arc<dyn MarketplaceAllocationReadPort>,
}

impl MarketplaceAllocationDirectoryService {
    pub fn new(allocation_reader: Arc<dyn MarketplaceAllocationReadPort>) -> Self {
        Self { allocation_reader }
    }

    pub async fn read_by_order_line(
        &self,
        context: PortContext,
        order_line_item_id: Uuid,
    ) -> Result<MarketplaceOrderAllocationResponse, PortError> {
        if order_line_item_id.is_nil() {
            return Err(PortError::validation(
                "marketplace.allocation.order_line_item_id",
                "order_line_item_id must not be nil",
            ));
        }
        self.allocation_reader
            .read_allocation_by_line(
                context,
                ReadMarketplaceAllocationByLineRequest { order_line_item_id },
            )
            .await
    }

    pub async fn list_by_order(
        &self,
        context: PortContext,
        order_id: Uuid,
    ) -> Result<Vec<MarketplaceOrderAllocationResponse>, PortError> {
        if order_id.is_nil() {
            return Err(PortError::validation(
                "marketplace.allocation.order_id",
                "order_id must not be nil",
            ));
        }
        self.allocation_reader
            .list_allocations_by_order(
                context,
                ListMarketplaceAllocationsByOrderRequest { order_id },
            )
            .await
    }

    pub async fn list_by_seller(
        &self,
        context: PortContext,
        request: ListMarketplaceAllocationsBySellerRequest,
    ) -> Result<MarketplaceAllocationListResponse, PortError> {
        if request.seller_id.is_nil() {
            return Err(PortError::validation(
                "marketplace.allocation.seller_id",
                "seller_id must not be nil",
            ));
        }
        self.allocation_reader
            .list_allocations_by_seller(context, request)
            .await
    }
}
