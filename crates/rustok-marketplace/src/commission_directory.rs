use std::sync::Arc;

use rustok_api::{PortContext, PortError};
use rustok_marketplace_commission::{
    ListMarketplaceCommissionAssessmentsByOrderRequest,
    ListMarketplaceCommissionAssessmentsBySellerRequest,
    ListMarketplaceCommissionRulesRequest,
    MarketplaceCommissionAssessmentListResponse,
    MarketplaceCommissionAssessmentResponse,
    MarketplaceCommissionReadPort,
    MarketplaceCommissionRuleListResponse,
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
