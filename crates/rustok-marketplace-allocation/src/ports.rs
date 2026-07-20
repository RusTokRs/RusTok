use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
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

#[async_trait]
impl MarketplaceAllocationReadPort for crate::MarketplaceAllocationService {
    async fn read_allocation_by_line(
        &self,
        context: PortContext,
        request: ReadMarketplaceAllocationByLineRequest,
    ) -> Result<MarketplaceOrderAllocationResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
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
    Uuid::parse_str(context.tenant_id.as_str()).map_err(|_| {
        PortError::validation(
            "marketplace_allocation.tenant_id_invalid",
            "PortContext.tenant_id must be a UUID for marketplace allocation ports",
        )
    })
}

fn parse_actor_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.actor.id.as_str()).map_err(|_| {
        PortError::validation(
            "marketplace_allocation.actor_id_invalid",
            "write PortContext.actor.id must be a UUID for marketplace allocation audit",
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
