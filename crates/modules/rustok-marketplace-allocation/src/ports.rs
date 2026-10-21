use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use std::sync::Arc;
use uuid::Uuid;

use crate::dto::{
    AllocateMarketplaceOrderLinesInput, AllocateMarketplaceOrderLinesResponse,
    ListMarketplaceAllocationsByOrderRequest, ListMarketplaceAllocationsBySellerRequest,
    MarketplaceAllocationListResponse, MarketplaceOrderAllocationResponse,
    ReadMarketplaceAllocationByLineRequest,
};
use crate::error::MarketplaceAllocationError;

#[async_trait]
pub trait MarketplaceAllocationReadPort: Send + Sync {
    async fn read_allocation_by_line(
        &self,
        context: PortContext,
        request: ReadMarketplaceAllocationByLineRequest,
    ) -> Result<MarketplaceOrderAllocationResponse, PortError>;

    async fn list_allocations_by_order(
        &self,
        context: PortContext,
        request: ListMarketplaceAllocationsByOrderRequest,
    ) -> Result<Vec<MarketplaceOrderAllocationResponse>, PortError>;

    async fn list_allocations_by_seller(
        &self,
        context: PortContext,
        request: ListMarketplaceAllocationsBySellerRequest,
    ) -> Result<MarketplaceAllocationListResponse, PortError>;
}

#[async_trait]
pub trait MarketplaceAllocationCommandPort: Send + Sync {
    async fn allocate_order_lines(
        &self,
        context: PortContext,
        request: AllocateMarketplaceOrderLinesInput,
    ) -> Result<AllocateMarketplaceOrderLinesResponse, PortError>;
}

/// Build the owner-controlled in-process command adapter for checkout composition.
///
/// Consumers receive the typed command port and do not construct the allocation service directly.
pub fn in_process_marketplace_allocation_command_port(
    db: sea_orm::DatabaseConnection,
) -> Arc<dyn MarketplaceAllocationCommandPort> {
    Arc::new(crate::MarketplaceAllocationService::new(db))
}

#[async_trait]
impl MarketplaceAllocationReadPort for crate::MarketplaceAllocationService {
    async fn read_allocation_by_line(
        &self,
        context: PortContext,
        request: ReadMarketplaceAllocationByLineRequest,
    ) -> Result<MarketplaceOrderAllocationResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        if request.order_line_item_id.is_nil() {
            return Err(PortError::validation(
                "marketplace_allocation.order_line_item_id_invalid",
                "order_line_item_id must not be nil",
            ));
        }
        self.get_by_order_line(parse_tenant_id(&context)?, request.order_line_item_id)
            .await
            .map_err(map_owner_error)
    }

    async fn list_allocations_by_order(
        &self,
        context: PortContext,
        request: ListMarketplaceAllocationsByOrderRequest,
    ) -> Result<Vec<MarketplaceOrderAllocationResponse>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        if request.order_id.is_nil() {
            return Err(PortError::validation(
                "marketplace_allocation.order_id_invalid",
                "order_id must not be nil",
            ));
        }
        self.list_by_order(parse_tenant_id(&context)?, request.order_id)
            .await
            .map_err(map_owner_error)
    }

    async fn list_allocations_by_seller(
        &self,
        context: PortContext,
        request: ListMarketplaceAllocationsBySellerRequest,
    ) -> Result<MarketplaceAllocationListResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        if request.seller_id.is_nil() {
            return Err(PortError::validation(
                "marketplace_allocation.seller_id_invalid",
                "seller_id must not be nil",
            ));
        }
        self.list_by_seller(parse_tenant_id(&context)?, request)
            .await
            .map_err(map_owner_error)
    }
}

#[async_trait]
impl MarketplaceAllocationCommandPort for crate::MarketplaceAllocationService {
    async fn allocate_order_lines(
        &self,
        context: PortContext,
        request: AllocateMarketplaceOrderLinesInput,
    ) -> Result<AllocateMarketplaceOrderLinesResponse, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        self.allocate_order_lines_with_receipt(
            parse_tenant_id(&context)?,
            parse_actor_id(&context)?,
            parse_idempotency_key(&context)?,
            request,
        )
        .await
        .map_err(map_owner_error)
    }
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    let id = Uuid::parse_str(context.tenant_id.as_str()).map_err(|_| {
        PortError::validation(
            "marketplace_allocation.tenant_id_invalid",
            "PortContext.tenant_id must be a UUID for marketplace allocation ports",
        )
    })?;
    if id.is_nil() {
        return Err(PortError::validation(
            "marketplace_allocation.tenant_id_invalid",
            "PortContext.tenant_id must not be nil for marketplace allocation ports",
        ));
    }
    Ok(id)
}

fn parse_actor_id(context: &PortContext) -> Result<Uuid, PortError> {
    let id = Uuid::parse_str(context.actor.id.as_str()).map_err(|_| {
        PortError::validation(
            "marketplace_allocation.actor_id_invalid",
            "write PortContext.actor.id must be a UUID for marketplace allocation audit",
        )
    })?;
    if id.is_nil() {
        return Err(PortError::validation(
            "marketplace_allocation.actor_id_invalid",
            "write PortContext.actor.id must not be nil for marketplace allocation audit",
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
                "marketplace_allocation.idempotency_key_required",
                "marketplace allocation write requires an idempotency key",
            )
        })
}

fn map_owner_error(error: MarketplaceAllocationError) -> PortError {
    match error {
        MarketplaceAllocationError::AllocationNotFound(line_id) => PortError::not_found(
            "marketplace_allocation.not_found",
            format!("marketplace allocation for order line {line_id} was not found"),
        ),
        MarketplaceAllocationError::LineAlreadyAllocated(line_id) => PortError::conflict(
            "marketplace_allocation.line_already_allocated",
            format!("order line {line_id} is already allocated"),
        ),
        MarketplaceAllocationError::DuplicateLine(line_id) => PortError::validation(
            "marketplace_allocation.duplicate_line",
            format!("order line {line_id} appears more than once"),
        ),
        MarketplaceAllocationError::IdempotencyConflict => PortError::conflict(
            "marketplace_allocation.idempotency_conflict",
            "allocation idempotency key is already bound to another request",
        ),
        MarketplaceAllocationError::ReceiptCorrupt => PortError::invariant_violation(
            "marketplace_allocation.receipt_corrupt",
            "allocation receipt requires operator review",
        ),
        MarketplaceAllocationError::Validation(message) => {
            PortError::validation("marketplace_allocation.validation", message)
        }
        MarketplaceAllocationError::Database(_) => PortError::new(
            PortErrorKind::Unavailable,
            "marketplace_allocation.storage_unavailable",
            "marketplace allocation storage is temporarily unavailable",
            true,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::PortActor;

    #[test]
    fn parse_tenant_id_requires_valid_non_nil_uuid() {
        let nil_context = PortContext::new(
            Uuid::nil().to_string(),
            PortActor::system(),
            "en",
            "contract",
        );
        let err = parse_tenant_id(&nil_context).unwrap_err();
        assert_eq!(err.code, "marketplace_allocation.tenant_id_invalid");

        let invalid_context = PortContext::new(
            "not-a-uuid",
            PortActor::system(),
            "en",
            "contract",
        );
        let err2 = parse_tenant_id(&invalid_context).unwrap_err();
        assert_eq!(err2.code, "marketplace_allocation.tenant_id_invalid");

        let valid_id = Uuid::new_v4();
        let valid_context = PortContext::new(
            valid_id.to_string(),
            PortActor::system(),
            "en",
            "contract",
        );
        assert_eq!(parse_tenant_id(&valid_context).unwrap(), valid_id);
    }

    #[test]
    fn parse_actor_id_requires_valid_non_nil_uuid() {
        let nil_context = PortContext::new(
            Uuid::new_v4().to_string(),
            PortActor::user(Uuid::nil().to_string()),
            "en",
            "contract",
        );
        let err = parse_actor_id(&nil_context).unwrap_err();
        assert_eq!(err.code, "marketplace_allocation.actor_id_invalid");

        let invalid_context = PortContext::new(
            Uuid::new_v4().to_string(),
            PortActor::user("not-a-uuid"),
            "en",
            "contract",
        );
        let err2 = parse_actor_id(&invalid_context).unwrap_err();
        assert_eq!(err2.code, "marketplace_allocation.actor_id_invalid");

        let valid_id = Uuid::new_v4();
        let valid_context = PortContext::new(
            Uuid::new_v4().to_string(),
            PortActor::user(valid_id.to_string()),
            "en",
            "contract",
        );
        assert_eq!(parse_actor_id(&valid_context).unwrap(), valid_id);
    }

    #[test]
    fn parse_idempotency_key_requires_non_empty() {
        let base = PortContext::new(
            Uuid::new_v4().to_string(),
            PortActor::system(),
            "en",
            "contract",
        );
        let err = parse_idempotency_key(&base).unwrap_err();
        assert_eq!(err.code, "marketplace_allocation.idempotency_key_required");

        let whitespace = base.clone().with_idempotency_key("   ");
        let err2 = parse_idempotency_key(&whitespace).unwrap_err();
        assert_eq!(err2.code, "marketplace_allocation.idempotency_key_required");

        let valid = base.with_idempotency_key("key-123");
        assert_eq!(parse_idempotency_key(&valid).unwrap(), "key-123");
    }

    #[test]
    fn map_owner_error_maps_correctly() {
        let err = map_owner_error(MarketplaceAllocationError::AllocationNotFound(Uuid::nil()));
        assert_eq!(err.code, "marketplace_allocation.not_found");

        let err2 = map_owner_error(MarketplaceAllocationError::LineAlreadyAllocated(Uuid::nil()));
        assert_eq!(err.code, "marketplace_allocation.not_found");
        assert_eq!(err2.code, "marketplace_allocation.line_already_allocated");

        let err3 = map_owner_error(MarketplaceAllocationError::Validation("err".into()));
        assert_eq!(err3.code, "marketplace_allocation.validation");

        let err4 = map_owner_error(MarketplaceAllocationError::IdempotencyConflict);
        assert_eq!(err4.code, "marketplace_allocation.idempotency_conflict");
    }
}
