use std::sync::Arc;

use rustok_api::PortContext;
use rustok_marketplace_allocation::MarketplaceAllocationReadPort;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::allocation_filter::AssessableAllocationReader;
use crate::dto::{
    AssessMarketplaceOrderCommissionsInput, AssessMarketplaceOrderCommissionsResponse,
    CreateMarketplaceCommissionRuleVersionInput, ListMarketplaceCommissionAssessmentsBySellerRequest,
    ListMarketplaceCommissionRulesRequest, MarketplaceCommissionAssessmentListResponse,
    MarketplaceCommissionAssessmentResponse, MarketplaceCommissionRuleListResponse,
    MarketplaceCommissionRuleResponse,
};
use crate::error::MarketplaceCommissionResult;

pub struct MarketplaceCommissionService {
    inner: crate::service::MarketplaceCommissionService,
}

impl MarketplaceCommissionService {
    pub fn new(
        db: DatabaseConnection,
        allocation_reader: Arc<dyn MarketplaceAllocationReadPort>,
    ) -> Self {
        let allocation_reader = Arc::new(AssessableAllocationReader::new(allocation_reader));
        Self {
            inner: crate::service::MarketplaceCommissionService::new(db, allocation_reader),
        }
    }

    pub fn database(&self) -> &DatabaseConnection {
        self.inner.database()
    }

    pub async fn create_rule_version_with_receipt(
        &self,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        input: CreateMarketplaceCommissionRuleVersionInput,
    ) -> MarketplaceCommissionResult<MarketplaceCommissionRuleResponse> {
        self.inner
            .create_rule_version_with_receipt(
                tenant_id,
                actor_id,
                idempotency_key,
                input,
            )
            .await
    }

    pub async fn assess_order_with_receipt(
        &self,
        context: PortContext,
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: impl Into<String>,
        input: AssessMarketplaceOrderCommissionsInput,
    ) -> MarketplaceCommissionResult<AssessMarketplaceOrderCommissionsResponse> {
        self.inner
            .assess_order_with_receipt(
                context,
                tenant_id,
                actor_id,
                idempotency_key,
                input,
            )
            .await
    }

    pub async fn get_assessment_by_allocation(
        &self,
        tenant_id: Uuid,
        allocation_id: Uuid,
    ) -> MarketplaceCommissionResult<MarketplaceCommissionAssessmentResponse> {
        self.inner
            .get_assessment_by_allocation(tenant_id, allocation_id)
            .await
    }

    pub async fn list_assessments_by_order(
        &self,
        tenant_id: Uuid,
        order_id: Uuid,
    ) -> MarketplaceCommissionResult<Vec<MarketplaceCommissionAssessmentResponse>> {
        self.inner
            .list_assessments_by_order(tenant_id, order_id)
            .await
    }

    pub async fn list_assessments_by_seller(
        &self,
        tenant_id: Uuid,
        request: ListMarketplaceCommissionAssessmentsBySellerRequest,
    ) -> MarketplaceCommissionResult<MarketplaceCommissionAssessmentListResponse> {
        self.inner
            .list_assessments_by_seller(tenant_id, request)
            .await
    }

    pub async fn list_rules(
        &self,
        tenant_id: Uuid,
        request: ListMarketplaceCommissionRulesRequest,
    ) -> MarketplaceCommissionResult<MarketplaceCommissionRuleListResponse> {
        self.inner.list_rules(tenant_id, request).await
    }
}
