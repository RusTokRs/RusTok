use std::sync::Arc;

use rustok_api::{PortContext, PortError};
use rustok_marketplace_commission::{
    ListMarketplaceCommissionAssessmentsByOrderRequest,
    ListMarketplaceCommissionAssessmentsBySellerRequest, ListMarketplaceCommissionRulesRequest,
    MarketplaceCommissionAssessmentListResponse, MarketplaceCommissionAssessmentResponse,
    MarketplaceCommissionReadPort, MarketplaceCommissionRuleListResponse,
    ReadMarketplaceCommissionAssessmentRequest,
};
use uuid::Uuid;

/// Marketplace family consumer over commission-owned read projections.
///
/// The family root never imports commission entities or database connections.
pub struct MarketplaceCommissionDirectoryService {
    commission_reader: Arc<dyn MarketplaceCommissionReadPort>,
}

impl MarketplaceCommissionDirectoryService {
    pub fn new(commission_reader: Arc<dyn MarketplaceCommissionReadPort>) -> Self {
        Self { commission_reader }
    }

    pub async fn read_by_allocation(
        &self,
        context: PortContext,
        allocation_id: Uuid,
    ) -> Result<MarketplaceCommissionAssessmentResponse, PortError> {
        if allocation_id.is_nil() {
            return Err(PortError::validation(
                "marketplace.commission.allocation_id",
                "allocation_id must not be nil",
            ));
        }
        self.commission_reader
            .read_assessment(
                context,
                ReadMarketplaceCommissionAssessmentRequest { allocation_id },
            )
            .await
    }

    pub async fn list_by_order(
        &self,
        context: PortContext,
        order_id: Uuid,
    ) -> Result<Vec<MarketplaceCommissionAssessmentResponse>, PortError> {
        if order_id.is_nil() {
            return Err(PortError::validation(
                "marketplace.commission.order_id",
                "order_id must not be nil",
            ));
        }
        self.commission_reader
            .list_assessments_by_order(
                context,
                ListMarketplaceCommissionAssessmentsByOrderRequest { order_id },
            )
            .await
    }

    pub async fn list_by_seller(
        &self,
        context: PortContext,
        request: ListMarketplaceCommissionAssessmentsBySellerRequest,
    ) -> Result<MarketplaceCommissionAssessmentListResponse, PortError> {
        if request.seller_id.is_nil() {
            return Err(PortError::validation(
                "marketplace.commission.seller_id",
                "seller_id must not be nil",
            ));
        }
        self.commission_reader
            .list_assessments_by_seller(context, request)
            .await
    }

    pub async fn list_rules(
        &self,
        context: PortContext,
        request: ListMarketplaceCommissionRulesRequest,
    ) -> Result<MarketplaceCommissionRuleListResponse, PortError> {
        self.commission_reader.list_rules(context, request).await
    }
}
