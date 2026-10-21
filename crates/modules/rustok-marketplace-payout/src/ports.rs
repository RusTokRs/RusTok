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
        if request.payout_id.is_nil() {
            return Err(PortError::validation(
                "marketplace_payout.payout_id_invalid",
                "request.payout_id must not be nil UUID",
            ));
        }
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
        if request.seller_id.is_nil() {
            return Err(PortError::validation(
                "marketplace_payout.seller_id_invalid",
                "request.seller_id must not be nil UUID",
            ));
        }
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
        if request.seller_id.is_nil() {
            return Err(PortError::validation(
                "marketplace_payout.seller_id_invalid",
                "request.seller_id must not be nil UUID",
            ));
        }
        self.schedule_with_receipt(context, tenant_id, actor_id, idempotency_key, request)
            .await
            .map_err(map_owner_error)
    }
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.tenant_id.as_str())
        .map_err(|_| {
            PortError::validation(
                "marketplace_payout.tenant_id_invalid",
                "PortContext.tenant_id must be a UUID for marketplace payout ports",
            )
        })
        .and_then(|id| {
            if id.is_nil() {
                Err(PortError::validation(
                    "marketplace_payout.tenant_id_invalid",
                    "PortContext.tenant_id must not be nil UUID",
                ))
            } else {
                Ok(id)
            }
        })
}

fn parse_actor_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.actor.id.as_str())
        .map_err(|_| {
            PortError::validation(
                "marketplace_payout.actor_id_invalid",
                "write PortContext.actor.id must be a UUID for marketplace payout audit",
            )
        })
        .and_then(|id| {
            if id.is_nil() {
                Err(PortError::validation(
                    "marketplace_payout.actor_id_invalid",
                    "write PortContext.actor.id must not be nil UUID",
                ))
            } else {
                Ok(id)
            }
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
        MarketplacePayoutError::Validation(message) => {
            PortError::validation("marketplace_payout.validation", message)
        }
        MarketplacePayoutError::LedgerBoundary {
            code,
            message,
            retryable,
            kind,
        } => PortError::new(
            match kind {
                crate::error::MarketplaceLedgerBoundaryKind::Validation => {
                    PortErrorKind::Validation
                }
                crate::error::MarketplaceLedgerBoundaryKind::NotFound => PortErrorKind::NotFound,
                crate::error::MarketplaceLedgerBoundaryKind::Conflict => PortErrorKind::Conflict,
                crate::error::MarketplaceLedgerBoundaryKind::Forbidden => PortErrorKind::Forbidden,
                crate::error::MarketplaceLedgerBoundaryKind::Unavailable => {
                    PortErrorKind::Unavailable
                }
                crate::error::MarketplaceLedgerBoundaryKind::Timeout => PortErrorKind::Timeout,
                crate::error::MarketplaceLedgerBoundaryKind::InvariantViolation => {
                    PortErrorKind::InvariantViolation
                }
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

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::PortActor;

    #[test]
    fn parse_tenant_id_validates_uuid() {
        let valid_id = Uuid::new_v4();
        let context = PortContext::new(
            valid_id.to_string(),
            PortActor::system(),
            "en",
            "correlation-id",
        );
        assert_eq!(parse_tenant_id(&context).unwrap(), valid_id);

        let invalid_context = PortContext::new(
            "not-a-uuid",
            PortActor::system(),
            "en",
            "correlation-id",
        );
        assert_eq!(
            parse_tenant_id(&invalid_context).unwrap_err().code,
            "marketplace_payout.tenant_id_invalid"
        );

        let nil_context = PortContext::new(
            Uuid::nil().to_string(),
            PortActor::system(),
            "en",
            "correlation-id",
        );
        assert_eq!(
            parse_tenant_id(&nil_context).unwrap_err().code,
            "marketplace_payout.tenant_id_invalid"
        );
    }

    #[test]
    fn parse_actor_id_validates_uuid() {
        let valid_id = Uuid::new_v4();
        let context = PortContext::new(
            Uuid::new_v4().to_string(),
            PortActor::user(valid_id.to_string()),
            "en",
            "correlation-id",
        );
        assert_eq!(parse_actor_id(&context).unwrap(), valid_id);

        let invalid_context = PortContext::new(
            Uuid::new_v4().to_string(),
            PortActor::user("not-a-uuid"),
            "en",
            "correlation-id",
        );
        assert_eq!(
            parse_actor_id(&invalid_context).unwrap_err().code,
            "marketplace_payout.actor_id_invalid"
        );

        let nil_context = PortContext::new(
            Uuid::new_v4().to_string(),
            PortActor::user(Uuid::nil().to_string()),
            "en",
            "correlation-id",
        );
        assert_eq!(
            parse_actor_id(&nil_context).unwrap_err().code,
            "marketplace_payout.actor_id_invalid"
        );
    }

    #[test]
    fn parse_idempotency_key_requires_non_empty_trimmed_value() {
        let mut context = PortContext::new(
            Uuid::new_v4().to_string(),
            PortActor::system(),
            "en",
            "correlation-id",
        );
        assert_eq!(
            parse_idempotency_key(&context).unwrap_err().code,
            "marketplace_payout.idempotency_key_required"
        );

        context = context.with_idempotency_key("   ");
        assert_eq!(
            parse_idempotency_key(&context).unwrap_err().code,
            "marketplace_payout.idempotency_key_required"
        );

        context = context.with_idempotency_key("  valid-key  ");
        assert_eq!(parse_idempotency_key(&context).unwrap(), "valid-key");
    }

    #[test]
    fn map_owner_error_covers_all_error_variants() {
        let id = Uuid::new_v4();
        let not_found = map_owner_error(MarketplacePayoutError::PayoutNotFound(id));
        assert_eq!(not_found.code, "marketplace_payout.not_found");
        assert_eq!(not_found.kind, PortErrorKind::NotFound);

        let ledger_not_found = map_owner_error(MarketplacePayoutError::LedgerEntryNotFound(id));
        assert_eq!(
            ledger_not_found.code,
            "marketplace_payout.ledger_entry_not_found"
        );
        assert_eq!(ledger_not_found.kind, PortErrorKind::NotFound);

        let assigned = map_owner_error(MarketplacePayoutError::LedgerEntryAlreadyAssigned(id));
        assert_eq!(
            assigned.code,
            "marketplace_payout.ledger_entry_already_assigned"
        );
        assert_eq!(assigned.kind, PortErrorKind::Conflict);

        let conflict = map_owner_error(MarketplacePayoutError::IdempotencyConflict);
        assert_eq!(conflict.code, "marketplace_payout.idempotency_conflict");
        assert_eq!(conflict.kind, PortErrorKind::Conflict);

        let corrupt = map_owner_error(MarketplacePayoutError::ReceiptCorrupt);
        assert_eq!(corrupt.code, "marketplace_payout.receipt_corrupt");
        assert_eq!(corrupt.kind, PortErrorKind::InvariantViolation);

        let val = map_owner_error(MarketplacePayoutError::Validation("test err".into()));
        assert_eq!(val.code, "marketplace_payout.validation");
        assert_eq!(val.kind, PortErrorKind::Validation);

        let db_err = map_owner_error(MarketplacePayoutError::Database(sea_orm::DbErr::Custom(
            "db error".into(),
        )));
        assert_eq!(db_err.code, "marketplace_payout.storage_unavailable");
        assert_eq!(db_err.kind, PortErrorKind::Unavailable);
        assert!(db_err.retryable);

        for (kind, expected) in [
            (
                crate::error::MarketplaceLedgerBoundaryKind::Validation,
                PortErrorKind::Validation,
            ),
            (
                crate::error::MarketplaceLedgerBoundaryKind::NotFound,
                PortErrorKind::NotFound,
            ),
            (
                crate::error::MarketplaceLedgerBoundaryKind::Conflict,
                PortErrorKind::Conflict,
            ),
            (
                crate::error::MarketplaceLedgerBoundaryKind::Forbidden,
                PortErrorKind::Forbidden,
            ),
            (
                crate::error::MarketplaceLedgerBoundaryKind::Unavailable,
                PortErrorKind::Unavailable,
            ),
            (
                crate::error::MarketplaceLedgerBoundaryKind::Timeout,
                PortErrorKind::Timeout,
            ),
            (
                crate::error::MarketplaceLedgerBoundaryKind::InvariantViolation,
                PortErrorKind::InvariantViolation,
            ),
        ] {
            let mapped = map_owner_error(MarketplacePayoutError::LedgerBoundary {
                code: "test.boundary".to_string(),
                message: "boundary message".to_string(),
                retryable: true,
                kind,
            });
            assert_eq!(mapped.code, "test.boundary");
            assert_eq!(mapped.kind, expected);
            assert!(mapped.retryable);
        }
    }
}
