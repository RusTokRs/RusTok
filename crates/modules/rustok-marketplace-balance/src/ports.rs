use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use std::sync::Arc;
use uuid::Uuid;

use crate::dto::{
    ListMarketplaceSellerLedgerEntriesRequest, MarketplaceLedgerEntryListResponse,
    MarketplaceLedgerReversalResponse, MarketplaceLedgerTransactionResponse,
    MarketplaceSellerBalanceResponse, MarketplaceSellerBalanceTransferResponse,
    PostMarketplaceLedgerReversalInput, PostMarketplaceOrderLedgerInput,
    PostMarketplaceSellerBalanceTransferInput, ReadMarketplaceOrderLedgerRequest,
    ReadMarketplaceSellerBalanceRequest, RebuildMarketplaceSellerBalanceInput,
};
use crate::error::MarketplaceLedgerError;

#[async_trait]
pub trait MarketplaceLedgerReadPort: Send + Sync {
    async fn read_order_ledger(
        &self,
        context: PortContext,
        request: ReadMarketplaceOrderLedgerRequest,
    ) -> Result<MarketplaceLedgerTransactionResponse, PortError>;

    async fn list_seller_entries(
        &self,
        context: PortContext,
        request: ListMarketplaceSellerLedgerEntriesRequest,
    ) -> Result<MarketplaceLedgerEntryListResponse, PortError>;

    async fn read_seller_balance(
        &self,
        _context: PortContext,
        _request: ReadMarketplaceSellerBalanceRequest,
    ) -> Result<MarketplaceSellerBalanceResponse, PortError> {
        Err(PortError::validation(
            "marketplace_ledger.seller_balance_not_supported",
            "marketplace ledger provider does not support seller balance projections",
        ))
    }
}

#[async_trait]
pub trait MarketplaceLedgerCommandPort: Send + Sync {
    async fn post_order_commissions(
        &self,
        context: PortContext,
        request: PostMarketplaceOrderLedgerInput,
    ) -> Result<MarketplaceLedgerTransactionResponse, PortError>;

    async fn post_financial_reversal(
        &self,
        _context: PortContext,
        _request: PostMarketplaceLedgerReversalInput,
    ) -> Result<MarketplaceLedgerReversalResponse, PortError> {
        Err(PortError::validation(
            "marketplace_ledger.reversal_not_supported",
            "marketplace ledger provider does not support financial reversals",
        ))
    }

    async fn post_seller_balance_transfer(
        &self,
        _context: PortContext,
        _request: PostMarketplaceSellerBalanceTransferInput,
    ) -> Result<MarketplaceSellerBalanceTransferResponse, PortError> {
        Err(PortError::validation(
            "marketplace_ledger.balance_transfer_not_supported",
            "marketplace ledger provider does not support seller balance transfers",
        ))
    }

    async fn rebuild_seller_balance(
        &self,
        _context: PortContext,
        _request: RebuildMarketplaceSellerBalanceInput,
    ) -> Result<MarketplaceSellerBalanceResponse, PortError> {
        Err(PortError::validation(
            "marketplace_ledger.seller_balance_rebuild_not_supported",
            "marketplace ledger provider does not support seller balance rebuilds",
        ))
    }
}

/// Build the owner-controlled in-process ledger command adapter for checkout composition.
///
/// The factory composes only inside the Ledger owner boundary; Commerce receives the typed port.
pub fn in_process_marketplace_ledger_command_port(
    db: sea_orm::DatabaseConnection,
) -> Arc<dyn MarketplaceLedgerCommandPort> {
    let allocation =
        Arc::new(rustok_marketplace_allocation::MarketplaceAllocationService::new(db.clone()));
    let commission = Arc::new(
        rustok_marketplace_commission::MarketplaceCommissionService::new(db.clone(), allocation),
    );
    Arc::new(crate::MarketplaceLedgerService::new(db, commission))
}

#[async_trait]
impl MarketplaceLedgerReadPort for crate::MarketplaceLedgerService {
    async fn read_order_ledger(
        &self,
        context: PortContext,
        request: ReadMarketplaceOrderLedgerRequest,
    ) -> Result<MarketplaceLedgerTransactionResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        if request.order_id.is_nil() {
            return Err(PortError::validation(
                "marketplace_ledger.order_id_invalid",
                "order_id must not be nil UUID",
            ));
        }
        self.read_order_ledger(parse_tenant_id(&context)?, request.order_id)
            .await
            .map_err(map_owner_error)
    }

    async fn list_seller_entries(
        &self,
        context: PortContext,
        request: ListMarketplaceSellerLedgerEntriesRequest,
    ) -> Result<MarketplaceLedgerEntryListResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        if request.seller_id.is_nil() {
            return Err(PortError::validation(
                "marketplace_ledger.seller_id_invalid",
                "seller_id must not be nil UUID",
            ));
        }
        self.list_seller_entries(parse_tenant_id(&context)?, request)
            .await
            .map_err(map_owner_error)
    }

    async fn read_seller_balance(
        &self,
        context: PortContext,
        mut request: ReadMarketplaceSellerBalanceRequest,
    ) -> Result<MarketplaceSellerBalanceResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        if request.seller_id.is_nil() {
            return Err(PortError::validation(
                "marketplace_ledger.seller_id_invalid",
                "seller_id must not be nil UUID",
            ));
        }
        let currency = request.currency_code.trim();
        if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(PortError::validation(
                "marketplace_ledger.currency_code_invalid",
                "currency_code must be an ASCII 3-letter currency code",
            ));
        }
        request.currency_code = currency.to_ascii_uppercase();
        self.read_seller_balance_projection(parse_tenant_id(&context)?, request)
            .await
            .map_err(map_owner_error)
    }
}

#[async_trait]
impl MarketplaceLedgerCommandPort for crate::MarketplaceLedgerService {
    async fn post_order_commissions(
        &self,
        context: PortContext,
        request: PostMarketplaceOrderLedgerInput,
    ) -> Result<MarketplaceLedgerTransactionResponse, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        if request.order_id.is_nil() {
            return Err(PortError::validation(
                "marketplace_ledger.order_id_invalid",
                "order_id must not be nil UUID",
            ));
        }
        let tenant_id = parse_tenant_id(&context)?;
        let actor_id = parse_actor_id(&context)?;
        let idempotency_key = parse_idempotency_key(&context)?;
        let response = self
            .post_order_with_receipt(context, tenant_id, actor_id, idempotency_key, request)
            .await
            .map_err(map_owner_error)?;
        self.rebuild_seller_balances_for_transaction(tenant_id, &response)
            .await
            .map_err(map_owner_error)?;
        Ok(response)
    }

    async fn post_financial_reversal(
        &self,
        context: PortContext,
        request: PostMarketplaceLedgerReversalInput,
    ) -> Result<MarketplaceLedgerReversalResponse, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        if request.source_id.is_nil() {
            return Err(PortError::validation(
                "marketplace_ledger.source_id_invalid",
                "source_id must not be nil UUID",
            ));
        }
        if request.order_id.is_nil() {
            return Err(PortError::validation(
                "marketplace_ledger.order_id_invalid",
                "order_id must not be nil UUID",
            ));
        }
        let tenant_id = parse_tenant_id(&context)?;
        let actor_id = parse_actor_id(&context)?;
        let idempotency_key = parse_idempotency_key(&context)?;
        self.post_reversal_with_receipt(context, tenant_id, actor_id, idempotency_key, request)
            .await
            .map_err(map_owner_error)
    }

    async fn post_seller_balance_transfer(
        &self,
        context: PortContext,
        request: PostMarketplaceSellerBalanceTransferInput,
    ) -> Result<MarketplaceSellerBalanceTransferResponse, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        if request.source_id.is_nil() {
            return Err(PortError::validation(
                "marketplace_ledger.source_id_invalid",
                "source_id must not be nil UUID",
            ));
        }
        if request.seller_id.is_nil() {
            return Err(PortError::validation(
                "marketplace_ledger.seller_id_invalid",
                "seller_id must not be nil UUID",
            ));
        }
        let tenant_id = parse_tenant_id(&context)?;
        let actor_id = parse_actor_id(&context)?;
        let idempotency_key = parse_idempotency_key(&context)?;
        self.post_balance_transfer_with_receipt(
            context,
            tenant_id,
            actor_id,
            idempotency_key,
            request,
        )
        .await
        .map_err(map_owner_error)
    }

    async fn rebuild_seller_balance(
        &self,
        context: PortContext,
        mut request: RebuildMarketplaceSellerBalanceInput,
    ) -> Result<MarketplaceSellerBalanceResponse, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        if request.seller_id.is_nil() {
            return Err(PortError::validation(
                "marketplace_ledger.seller_id_invalid",
                "seller_id must not be nil UUID",
            ));
        }
        let currency = request.currency_code.trim();
        if currency.len() != 3 || !currency.chars().all(|c| c.is_ascii_alphabetic()) {
            return Err(PortError::validation(
                "marketplace_ledger.currency_code_invalid",
                "currency_code must be an ASCII 3-letter currency code",
            ));
        }
        request.currency_code = currency.to_ascii_uppercase();
        let tenant_id = parse_tenant_id(&context)?;
        parse_actor_id(&context)?;
        parse_idempotency_key(&context)?;
        self.rebuild_seller_balance_projection(tenant_id, request)
            .await
            .map_err(map_owner_error)
    }
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    let id = Uuid::parse_str(context.tenant_id.as_str()).map_err(|_| {
        PortError::validation(
            "marketplace_ledger.tenant_id_invalid",
            "PortContext.tenant_id must be a UUID for marketplace ledger ports",
        )
    })?;
    if id.is_nil() {
        return Err(PortError::validation(
            "marketplace_ledger.tenant_id_invalid",
            "PortContext.tenant_id must not be nil UUID for marketplace ledger ports",
        ));
    }
    Ok(id)
}

fn parse_actor_id(context: &PortContext) -> Result<Uuid, PortError> {
    let id = Uuid::parse_str(context.actor.id.as_str()).map_err(|_| {
        PortError::validation(
            "marketplace_ledger.actor_id_invalid",
            "write PortContext.actor.id must be a UUID for marketplace ledger audit",
        )
    })?;
    if id.is_nil() {
        return Err(PortError::validation(
            "marketplace_ledger.actor_id_invalid",
            "write PortContext.actor.id must not be nil UUID for marketplace ledger audit",
        ));
    }
    Ok(id)
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
                "marketplace_ledger.idempotency_key_required",
                "marketplace ledger write requires an idempotency key",
            )
        })
}

fn map_owner_error(error: MarketplaceLedgerError) -> PortError {
    match error {
        MarketplaceLedgerError::TransactionNotFound(order_id) => PortError::not_found(
            "marketplace_ledger.transaction_not_found",
            format!("marketplace ledger transaction for order {order_id} was not found"),
        ),
        MarketplaceLedgerError::OrderAlreadyPosted(order_id) => PortError::conflict(
            "marketplace_ledger.order_already_posted",
            format!("marketplace ledger transaction for order {order_id} is already posted"),
        ),
        MarketplaceLedgerError::AssessmentAlreadyPosted(assessment_id) => PortError::conflict(
            "marketplace_ledger.assessment_already_posted",
            format!("commission assessment {assessment_id} is already posted"),
        ),
        MarketplaceLedgerError::ReversalAlreadyPosted(source_id) => PortError::conflict(
            "marketplace_ledger.reversal_already_posted",
            format!("marketplace ledger reversal source {source_id} is already posted"),
        ),
        MarketplaceLedgerError::BalanceTransferAlreadyPosted(source_id) => PortError::conflict(
            "marketplace_ledger.balance_transfer_already_posted",
            format!("marketplace seller balance transfer source {source_id} is already posted"),
        ),
        MarketplaceLedgerError::SellerBalanceNotFound {
            seller_id,
            currency_code,
        } => PortError::not_found(
            "marketplace_ledger.seller_balance_not_found",
            format!(
                "marketplace seller balance for seller {seller_id} and currency {currency_code} was not found"
            ),
        ),
        MarketplaceLedgerError::IdempotencyConflict => PortError::conflict(
            "marketplace_ledger.idempotency_conflict",
            "ledger idempotency key is already bound to another request",
        ),
        MarketplaceLedgerError::ReceiptCorrupt => PortError::invariant_violation(
            "marketplace_ledger.receipt_corrupt",
            "ledger receipt requires operator review",
        ),
        MarketplaceLedgerError::Validation(message) => {
            PortError::validation("marketplace_ledger.validation", message)
        }
        MarketplaceLedgerError::CommissionBoundary {
            code,
            message,
            retryable,
            kind,
        } => PortError::new(
            match kind {
                crate::error::MarketplaceCommissionBoundaryKind::Validation => {
                    PortErrorKind::Validation
                }
                crate::error::MarketplaceCommissionBoundaryKind::NotFound => {
                    PortErrorKind::NotFound
                }
                crate::error::MarketplaceCommissionBoundaryKind::Conflict => {
                    PortErrorKind::Conflict
                }
                crate::error::MarketplaceCommissionBoundaryKind::Forbidden => {
                    PortErrorKind::Forbidden
                }
                crate::error::MarketplaceCommissionBoundaryKind::Unavailable => {
                    PortErrorKind::Unavailable
                }
                crate::error::MarketplaceCommissionBoundaryKind::Timeout => PortErrorKind::Timeout,
                crate::error::MarketplaceCommissionBoundaryKind::InvariantViolation => {
                    PortErrorKind::InvariantViolation
                }
            },
            code,
            message,
            retryable,
        ),
        MarketplaceLedgerError::Database(_) => PortError::new(
            PortErrorKind::Unavailable,
            "marketplace_ledger.storage_unavailable",
            "marketplace ledger storage is temporarily unavailable",
            true,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::error::MarketplaceCommissionBoundaryKind;
    use rustok_api::{PortActor, PortErrorKind};

    #[test]
    fn parse_tenant_id_invariants() {
        let valid_uuid = Uuid::new_v4();
        let ctx = PortContext::new(valid_uuid.to_string(), PortActor::system(), "en", "corr-1");
        assert_eq!(parse_tenant_id(&ctx).unwrap(), valid_uuid);

        let invalid_ctx = PortContext::new("not-a-uuid", PortActor::system(), "en", "corr-1");
        assert_eq!(
            parse_tenant_id(&invalid_ctx).unwrap_err().code,
            "marketplace_ledger.tenant_id_invalid"
        );

        let nil_ctx =
            PortContext::new(Uuid::nil().to_string(), PortActor::system(), "en", "corr-1");
        assert_eq!(
            parse_tenant_id(&nil_ctx).unwrap_err().code,
            "marketplace_ledger.tenant_id_invalid"
        );
    }

    #[test]
    fn parse_actor_id_invariants() {
        let valid_uuid = Uuid::new_v4();
        let ctx = PortContext::new(
            Uuid::new_v4().to_string(),
            PortActor::user(valid_uuid.to_string()),
            "en",
            "corr-1",
        );
        assert_eq!(parse_actor_id(&ctx).unwrap(), valid_uuid);

        let invalid_ctx = PortContext::new(
            Uuid::new_v4().to_string(),
            PortActor::user("not-a-uuid"),
            "en",
            "corr-1",
        );
        assert_eq!(
            parse_actor_id(&invalid_ctx).unwrap_err().code,
            "marketplace_ledger.actor_id_invalid"
        );

        let nil_ctx = PortContext::new(
            Uuid::new_v4().to_string(),
            PortActor::user(Uuid::nil().to_string()),
            "en",
            "corr-1",
        );
        assert_eq!(
            parse_actor_id(&nil_ctx).unwrap_err().code,
            "marketplace_ledger.actor_id_invalid"
        );
    }

    #[test]
    fn parse_idempotency_key_invariants() {
        let mut ctx = PortContext::new(
            Uuid::new_v4().to_string(),
            PortActor::system(),
            "en",
            "corr-1",
        );
        assert_eq!(
            parse_idempotency_key(&ctx).unwrap_err().code,
            "marketplace_ledger.idempotency_key_required"
        );

        ctx.idempotency_key = Some("   ".to_string());
        assert_eq!(
            parse_idempotency_key(&ctx).unwrap_err().code,
            "marketplace_ledger.idempotency_key_required"
        );

        ctx.idempotency_key = Some("  idem-123  ".to_string());
        assert_eq!(parse_idempotency_key(&ctx).unwrap(), "idem-123");
    }

    #[test]
    fn map_owner_error_branches() {
        let order_id = Uuid::new_v4();
        let err = map_owner_error(MarketplaceLedgerError::TransactionNotFound(order_id));
        assert_eq!(err.code, "marketplace_ledger.transaction_not_found");
        assert_eq!(err.kind, PortErrorKind::NotFound);

        let err = map_owner_error(MarketplaceLedgerError::OrderAlreadyPosted(order_id));
        assert_eq!(err.code, "marketplace_ledger.order_already_posted");
        assert_eq!(err.kind, PortErrorKind::Conflict);

        let err = map_owner_error(MarketplaceLedgerError::AssessmentAlreadyPosted(order_id));
        assert_eq!(err.code, "marketplace_ledger.assessment_already_posted");
        assert_eq!(err.kind, PortErrorKind::Conflict);

        let err = map_owner_error(MarketplaceLedgerError::ReversalAlreadyPosted(order_id));
        assert_eq!(err.code, "marketplace_ledger.reversal_already_posted");
        assert_eq!(err.kind, PortErrorKind::Conflict);

        let err = map_owner_error(MarketplaceLedgerError::BalanceTransferAlreadyPosted(
            order_id,
        ));
        assert_eq!(
            err.code,
            "marketplace_ledger.balance_transfer_already_posted"
        );
        assert_eq!(err.kind, PortErrorKind::Conflict);

        let err = map_owner_error(MarketplaceLedgerError::SellerBalanceNotFound {
            seller_id: order_id,
            currency_code: "USD".to_string(),
        });
        assert_eq!(err.code, "marketplace_ledger.seller_balance_not_found");
        assert_eq!(err.kind, PortErrorKind::NotFound);

        let err = map_owner_error(MarketplaceLedgerError::IdempotencyConflict);
        assert_eq!(err.code, "marketplace_ledger.idempotency_conflict");
        assert_eq!(err.kind, PortErrorKind::Conflict);

        let err = map_owner_error(MarketplaceLedgerError::ReceiptCorrupt);
        assert_eq!(err.code, "marketplace_ledger.receipt_corrupt");
        assert_eq!(err.kind, PortErrorKind::InvariantViolation);

        let err = map_owner_error(MarketplaceLedgerError::Validation("bad input".to_string()));
        assert_eq!(err.code, "marketplace_ledger.validation");
        assert_eq!(err.kind, PortErrorKind::Validation);

        let err = map_owner_error(MarketplaceLedgerError::CommissionBoundary {
            code: "comm.failed".to_string(),
            message: "commission unavailable".to_string(),
            retryable: true,
            kind: MarketplaceCommissionBoundaryKind::Unavailable,
        });
        assert_eq!(err.code, "comm.failed");
        assert_eq!(err.kind, PortErrorKind::Unavailable);
        assert!(err.retryable);
    }
}
