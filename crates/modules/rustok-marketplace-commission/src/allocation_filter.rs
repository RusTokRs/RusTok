use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use rustok_marketplace_allocation::{
    ListMarketplaceAllocationsByOrderRequest, ListMarketplaceAllocationsBySellerRequest,
    MarketplaceAllocationListResponse, MarketplaceAllocationReadPort, MarketplaceAllocationStatus,
    MarketplaceOrderAllocationResponse, ReadMarketplaceAllocationByLineRequest,
};

pub(crate) struct AssessableAllocationReader {
    inner: Arc<dyn MarketplaceAllocationReadPort>,
}

impl AssessableAllocationReader {
    pub(crate) fn new(inner: Arc<dyn MarketplaceAllocationReadPort>) -> Self {
        Self { inner }
    }
}

#[async_trait]
impl MarketplaceAllocationReadPort for AssessableAllocationReader {
    async fn read_allocation_by_line(
        &self,
        context: PortContext,
        request: ReadMarketplaceAllocationByLineRequest,
    ) -> Result<MarketplaceOrderAllocationResponse, PortError> {
        self.inner.read_allocation_by_line(context, request).await
    }

    async fn list_allocations_by_order(
        &self,
        context: PortContext,
        request: ListMarketplaceAllocationsByOrderRequest,
    ) -> Result<Vec<MarketplaceOrderAllocationResponse>, PortError> {
        let allocations = self
            .inner
            .list_allocations_by_order(context, request)
            .await?;
        if let Some(allocation) = allocations
            .iter()
            .find(|allocation| allocation.status != MarketplaceAllocationStatus::Allocated)
        {
            return Err(PortError::conflict(
                "marketplace_commission.allocation_not_assessable",
                format!(
                    "allocation {} is `{}` and cannot be assessed",
                    allocation.id,
                    allocation.status.as_str()
                ),
            ));
        }
        Ok(allocations)
    }

    async fn list_allocations_by_seller(
        &self,
        context: PortContext,
        request: ListMarketplaceAllocationsBySellerRequest,
    ) -> Result<MarketplaceAllocationListResponse, PortError> {
        self.inner
            .list_allocations_by_seller(context, request)
            .await
    }
}
