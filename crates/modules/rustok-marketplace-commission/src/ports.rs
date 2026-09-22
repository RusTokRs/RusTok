use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use uuid::Uuid;

use crate::dto::{
    AssessMarketplaceOrderCommissionsInput, AssessMarketplaceOrderCommissionsResponse,
    CreateMarketplaceCommissionRuleVersionInput,
    ListMarketplaceCommissionAssessmentsByOrderRequest,
    ListMarketplaceCommissionAssessmentsBySellerRequest, ListMarketplaceCommissionRulesRequest,
    MarketplaceCommissionAssessmentListResponse, MarketplaceCommissionAssessmentResponse,
    MarketplaceCommissionRuleListResponse, MarketplaceCommissionRuleResponse,
    ReadMarketplaceCommissionAssessmentRequest,
};
use crate::error::MarketplaceCommissionError;

#[async_trait]
pub trait MarketplaceCommissionReadPort: Send + Sync {
    async fn read_assessment(
        &self,
        context: PortContext,
        request: ReadMarketplaceCommissionAssessmentRequest,
    ) -> Result<MarketplaceCommissionAssessmentResponse, PortError>;

    async fn list_assessments_by_order(
        &self,
        context: PortContext,
        request: ListMarketplaceCommissionAssessmentsByOrderRequest,
    ) -> Result<Vec<MarketplaceCommissionAssessmentResponse>, PortError>;

    async fn list_assessments_by_seller(
        &self,
        context: PortContext,
        request: ListMarketplaceCommissionAssessmentsBySellerRequest,
    ) -> Result<MarketplaceCommissionAssessmentListResponse, PortError>;

    async fn list_rules(
        &self,
        context: PortContext,
        request: ListMarketplaceCommissionRulesRequest,
    ) -> Result<MarketplaceCommissionRuleListResponse, PortError>;
}

#[async_trait]
pub trait MarketplaceCommissionCommandPort: Send + Sync {
    async fn create_rule_version(
        &self,
        context: PortContext,
        request: CreateMarketplaceCommissionRuleVersionInput,
    ) -> Result<MarketplaceCommissionRuleResponse, PortError>;

    async fn assess_order(
        &self,
        context: PortContext,
        request: AssessMarketplaceOrderCommissionsInput,
    ) -> Result<AssessMarketplaceOrderCommissionsResponse, PortError>;
}

#[async_trait]
impl MarketplaceCommissionReadPort for crate::MarketplaceCommissionService {
    async fn read_assessment(
        &self,
        context: PortContext,
        request: ReadMarketplaceCommissionAssessmentRequest,
    ) -> Result<MarketplaceCommissionAssessmentResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        self.get_assessment_by_allocation(parse_tenant_id(&context)?, request.allocation_id)
            .await
            .map_err(map_owner_error)
    }

    async fn list_assessments_by_order(
        &self,
        context: PortContext,
        request: ListMarketplaceCommissionAssessmentsByOrderRequest,
    ) -> Result<Vec<MarketplaceCommissionAssessmentResponse>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        self.list_assessments_by_order(parse_tenant_id(&context)?, request.order_id)
            .await
            .map_err(map_owner_error)
    }

    async fn list_assessments_by_seller(
        &self,
        context: PortContext,
        request: ListMarketplaceCommissionAssessmentsBySellerRequest,
    ) -> Result<MarketplaceCommissionAssessmentListResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        self.list_assessments_by_seller(parse_tenant_id(&context)?, request)
            .await
            .map_err(map_owner_error)
    }

    async fn list_rules(
        &self,
        context: PortContext,
        request: ListMarketplaceCommissionRulesRequest,
    ) -> Result<MarketplaceCommissionRuleListResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        self.list_rules(parse_tenant_id(&context)?, request)
            .await
            .map_err(map_owner_error)
    }
}

#[async_trait]
impl MarketplaceCommissionCommandPort for crate::MarketplaceCommissionService {
    async fn create_rule_version(
        &self,
        context: PortContext,
        request: CreateMarketplaceCommissionRuleVersionInput,
    ) -> Result<MarketplaceCommissionRuleResponse, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        self.create_rule_version_with_receipt(
            parse_tenant_id(&context)?,
            parse_actor_id(&context)?,
            parse_idempotency_key(&context)?,
            request,
        )
        .await
        .map_err(map_owner_error)
    }

    async fn assess_order(
        &self,
        context: PortContext,
        request: AssessMarketplaceOrderCommissionsInput,
    ) -> Result<AssessMarketplaceOrderCommissionsResponse, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        let tenant_id = parse_tenant_id(&context)?;
        let actor_id = parse_actor_id(&context)?;
        let idempotency_key = parse_idempotency_key(&context)?;
        self.assess_order_with_receipt(context, tenant_id, actor_id, idempotency_key, request)
            .await
            .map_err(map_owner_error)
    }
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.tenant_id.as_str()).map_err(|_| {
        PortError::validation(
            "marketplace_commission.tenant_id_invalid",
            "PortContext.tenant_id must be a UUID for marketplace commission ports",
        )
    })
}

fn parse_actor_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.actor.id.as_str()).map_err(|_| {
        PortError::validation(
            "marketplace_commission.actor_id_invalid",
            "write PortContext.actor.id must be a UUID for marketplace commission audit",
        )
    })
}

fn parse_idempotency_key(context: &PortContext) -> Result<String, PortError> {
    context
        .idempotency_key
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
        .ok_or_else(|| {
            PortError::validation(
                "marketplace_commission.idempotency_key_required",
                "marketplace commission write requires an idempotency key",
            )
        })
}

fn map_owner_error(error: MarketplaceCommissionError) -> PortError {
    match error {
        MarketplaceCommissionError::RuleNotFound(rule_id) => PortError::not_found(
            "marketplace_commission.rule_not_found",
            format!("commission rule {rule_id} was not found"),
        ),
        MarketplaceCommissionError::AssessmentNotFound(allocation_id) => PortError::not_found(
            "marketplace_commission.assessment_not_found",
            format!("commission assessment for allocation {allocation_id} was not found"),
        ),
        MarketplaceCommissionError::RuleNotMatched(allocation_id) => PortError::conflict(
            "marketplace_commission.rule_not_matched",
            format!("no active commission rule matches allocation {allocation_id}"),
        ),
        MarketplaceCommissionError::AllocationAlreadyAssessed(allocation_id) => {
            PortError::conflict(
                "marketplace_commission.allocation_already_assessed",
                format!("allocation {allocation_id} is already assessed"),
            )
        }
        MarketplaceCommissionError::IdempotencyConflict => PortError::conflict(
            "marketplace_commission.idempotency_conflict",
            "commission idempotency key is already bound to another request",
        ),
        MarketplaceCommissionError::ReceiptCorrupt => PortError::invariant_violation(
            "marketplace_commission.receipt_corrupt",
            "commission receipt requires operator review",
        ),
        MarketplaceCommissionError::Validation(message) => {
            PortError::validation("marketplace_commission.validation", message)
        }
        MarketplaceCommissionError::AllocationBoundary {
            code,
            message,
            retryable,
        } => PortError::new(
            if retryable {
                PortErrorKind::Unavailable
            } else {
                PortErrorKind::Conflict
            },
            code,
            message,
            retryable,
        ),
        MarketplaceCommissionError::Database(_) => PortError::new(
            PortErrorKind::Unavailable,
            "marketplace_commission.storage_unavailable",
            "marketplace commission storage is temporarily unavailable",
            true,
        ),
    }
}
