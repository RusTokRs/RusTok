use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use uuid::Uuid;

use crate::dto::{
    ListMarketplaceSellerLedgerEntriesRequest, MarketplaceLedgerEntryListResponse,
    MarketplaceLedgerReversalResponse, MarketplaceLedgerTransactionResponse,
    MarketplaceSellerBalanceResponse, PostMarketplaceLedgerReversalInput,
    PostMarketplaceOrderLedgerInput, ReadMarketplaceOrderLedgerRequest,
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

#[async_trait]
impl MarketplaceLedgerReadPort for crate::MarketplaceLedgerService {
    async fn read_order_ledger(
        &self,
        context: PortContext,
        request: ReadMarketplaceOrderLedgerRequest,
    ) -> Result<MarketplaceLedgerTransactionResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
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
        self.list_seller_entries(parse_tenant_id(&context)?, request)
            .await
            .map_err(map_owner_error)
    }

    async fn read_seller_balance(
        &self,
        context: PortContext,
        request: ReadMarketplaceSellerBalanceRequest,
    ) -> Result<MarketplaceSellerBalanceResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
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
        let tenant_id = parse_tenant_id(&context)?;
        let actor_id = parse_actor_id(&context)?;
        let idempotency_key = parse_idempotency_key(&context)?;
        self.post_reversal_with_receipt(context, tenant_id, actor_id, idempotency_key, request)
            .await
            .map_err(map_owner_error)
    }

    async fn rebuild_seller_balance(
        &self,
        context: PortContext,
        request: RebuildMarketplaceSellerBalanceInput,
    ) -> Result<MarketplaceSellerBalanceResponse, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        let tenant_id = parse_tenant_id(&context)?;
        parse_actor_id(&context)?;
        parse_idempotency_key(&context)?;
        self.rebuild_seller_balance_projection(tenant_id, request)
            .await
            .map_err(map_owner_error)
    }
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.tenant_id.as_str()).map_err(|_| {
        PortError::validation(
            "marketplace_ledger.tenant_id_invalid",
            "PortContext.tenant_id must be a UUID for marketplace ledger ports",
        )
    })
}

fn parse_actor_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.actor.id.as_str()).map_err(|_| {
        PortError::validation(
            "marketplace_ledger.actor_id_invalid",
            "write PortContext.actor.id must be a UUID for marketplace ledger audit",
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
        MarketplaceLedgerError::Database(_) => PortError::new(
            PortErrorKind::Unavailable,
            "marketplace_ledger.storage_unavailable",
            "marketplace ledger storage is temporarily unavailable",
            true,
        ),
    }
}
