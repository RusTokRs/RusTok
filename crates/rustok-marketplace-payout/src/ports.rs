use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use uuid::Uuid;

use crate::dto::{
    ListMarketplaceSellerPayoutsRequest, MarketplacePayoutListResponse, MarketplacePayoutResponse,
    ReadMarketplacePayoutRequest, ScheduleMarketplacePayoutInput,
};
use crate::error::MarketplacePayoutError;

#[async_trait]
pub trait MarketplacePayoutReadPort: Send + Sync {
    async fn read_payout(
        &self,
        context: PortContext,
        request: ReadMarketplacePayoutRequest,
    ) -> Result<MarketplacePayoutResponse, PortError>;

    async fn list_seller_payouts(
        &self,
        context: PortContext,
        request: ListMarketplaceSellerPayoutsRequest,
    ) -> Result<MarketplacePayoutListResponse, PortError>;
}

#[async_trait]
pub trait MarketplacePayoutCommandPort: Send + Sync {
    async fn schedule_payout(
        &self,
        context: PortContext,
        request: ScheduleMarketplacePayoutInput,
    ) -> Result<MarketplacePayoutResponse, PortError>;
}

#[async_trait]
impl MarketplacePayoutReadPort for crate::MarketplacePayoutService {
    async fn read_payout(
        &self,
        context: PortContext,
        request: ReadMarketplacePayoutRequest,
    ) -> Result<MarketplacePayoutResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        self.read_payout(parse_tenant_id(&context)?, request.payout_id)
            .await
            .map_err(map_owner_error)
    }

    async fn list_seller_payouts(
        &self,
        context: PortContext,
        request: ListMarketplaceSellerPayoutsRequest,
    ) -> Result<MarketplacePayoutListResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        self.list_seller_payouts(parse_tenant_id(&context)?, request)
            .await
            .map_err(map_owner_error)
    }
}

#[async_trait]
impl MarketplacePayoutCommandPort for crate::MarketplacePayoutService {
    async fn schedule_payout(
        &self,
        context: PortContext,
        request: ScheduleMarketplacePayoutInput,
    ) -> Result<MarketplacePayoutResponse, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        let tenant_id = parse_tenant_id(&context)?;
        let actor_id = parse_actor_id(&context)?;
        let idempotency_key = parse_idempotency_key(&context)?;
        self.schedule_with_operation(context, tenant_id, actor_id, idempotency_key, request)
            .await
            .map_err(map_owner_error)
    }
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.tenant_id.as_str()).map_err(|_| {
        PortError::validation(
            "marketplace_payout.tenant_id_invalid",
            "PortContext.tenant_id must be a UUID for marketplace payout ports",
        )
    })
}

fn parse_actor_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.actor.id.as_str()).map_err(|_| {
        PortError::validation(
            "marketplace_payout.actor_id_invalid",
            "write PortContext.actor.id must be a UUID for marketplace payout audit",
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
                "marketplace_payout.idempotency_key_required",
                "marketplace payout write requires an idempotency key",
            )
        })
}

fn map_owner_error(error: MarketplacePayoutError) -> PortError {
    match error {
        MarketplacePayoutError::PayoutNotFound(payout_id) => PortError::not_found(
            "marketplace_payout.not_found",
            format!("marketplace payout {payout_id} was not found"),
        ),
        MarketplacePayoutError::LedgerEntryNotFound(entry_id) => PortError::not_found(
            "marketplace_payout.ledger_entry_not_found",
            format!("ledger entry {entry_id} was not found in the seller payable projection"),
        ),
        MarketplacePayoutError::LedgerEntryAlreadyAssigned(entry_id) => PortError::conflict(
            "marketplace_payout.ledger_entry_already_assigned",
            format!("ledger entry {entry_id} is already assigned to a payout"),
        ),
        MarketplacePayoutError::IdempotencyConflict => PortError::conflict(
            "marketplace_payout.idempotency_conflict",
            "payout idempotency key is already bound to another request",
        ),
        MarketplacePayoutError::ReceiptCorrupt => PortError::invariant_violation(
            "marketplace_payout.receipt_corrupt",
            "payout receipt requires operator review",
        ),
        MarketplacePayoutError::LedgerWriterNotConfigured => PortError::new(
            PortErrorKind::Unavailable,
            "marketplace_payout.ledger_writer_not_configured",
            "marketplace payout ledger writer is not configured",
            false,
        ),
        MarketplacePayoutError::OperationInProgress(operation_id) => PortError::new(
            PortErrorKind::Unavailable,
            "marketplace_payout.operation_in_progress",
            format!("marketplace payout operation {operation_id} is already being processed"),
            true,
        ),
        MarketplacePayoutError::OperationFailed { operation_id, code } => PortError::conflict(
            "marketplace_payout.operation_failed",
            format!("marketplace payout operation {operation_id} failed with code {code:?}"),
        ),
        MarketplacePayoutError::CompensationRequired(operation_id) => PortError::new(
            PortErrorKind::Unavailable,
            "marketplace_payout.compensation_required",
            format!("marketplace payout operation {operation_id} requires compensation retry"),
            true,
        ),
        MarketplacePayoutError::ReconciliationRequired(operation_id) => {
            PortError::invariant_violation(
                "marketplace_payout.reconciliation_required",
                format!("marketplace payout operation {operation_id} requires reconciliation"),
            )
        }
        MarketplacePayoutError::OperationCorrupt(operation_id) => PortError::invariant_violation(
            "marketplace_payout.operation_corrupt",
            format!("marketplace payout operation {operation_id} is corrupt"),
        ),
        MarketplacePayoutError::Validation(message) => {
            PortError::validation("marketplace_payout.validation", message)
        }
        MarketplacePayoutError::LedgerBoundary {
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
        MarketplacePayoutError::Database(_) => PortError::new(
            PortErrorKind::Unavailable,
            "marketplace_payout.storage_unavailable",
            "marketplace payout storage is temporarily unavailable",
            true,
        ),
    }
}
