use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use sea_orm::DatabaseConnection;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    AddMarketplaceCartLineItemInput, AddMarketplaceCartLineItemResponse, CartError,
    CartMarketplaceLineSnapshot, CartMarketplaceSnapshotService, MarketplaceCartLineSnapshotInput,
};

const LIST_MARKETPLACE_LINE_SNAPSHOTS_OPERATION: &str = "list_marketplace_line_snapshots";
const ADD_MARKETPLACE_LINE_ITEM_OPERATION: &str = "add_marketplace_line_item";
const BIND_MARKETPLACE_LINE_SNAPSHOT_OPERATION: &str = "bind_marketplace_line_snapshot";

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct AddMarketplaceCartLineRequest {
    pub cart_id: Uuid,
    pub input: AddMarketplaceCartLineItemInput,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct BindMarketplaceCartLineSnapshotRequest {
    pub cart_id: Uuid,
    pub cart_line_item_id: Uuid,
    pub snapshot: MarketplaceCartLineSnapshotInput,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct ListMarketplaceCartLineSnapshotsRequest {
    pub cart_id: Uuid,
}

#[async_trait]
pub trait MarketplaceCartSnapshotReadPort: Send + Sync {
    async fn list_marketplace_line_snapshots(
        &self,
        context: PortContext,
        request: ListMarketplaceCartLineSnapshotsRequest,
    ) -> Result<Vec<CartMarketplaceLineSnapshot>, PortError>;
}

#[async_trait]
pub trait MarketplaceCartSnapshotCommandPort: Send + Sync {
    async fn add_marketplace_line_item(
        &self,
        context: PortContext,
        request: AddMarketplaceCartLineRequest,
    ) -> Result<AddMarketplaceCartLineItemResponse, PortError>;

    async fn bind_marketplace_line_snapshot(
        &self,
        context: PortContext,
        request: BindMarketplaceCartLineSnapshotRequest,
    ) -> Result<CartMarketplaceLineSnapshot, PortError>;
}

pub fn in_process_marketplace_cart_snapshot_read_port(
    db: DatabaseConnection,
) -> Arc<dyn MarketplaceCartSnapshotReadPort> {
    Arc::new(CartMarketplaceSnapshotService::new(db))
}

pub fn in_process_marketplace_cart_snapshot_command_port(
    db: DatabaseConnection,
) -> Arc<dyn MarketplaceCartSnapshotCommandPort> {
    Arc::new(CartMarketplaceSnapshotService::new(db))
}

#[async_trait]
impl MarketplaceCartSnapshotReadPort for CartMarketplaceSnapshotService {
    async fn list_marketplace_line_snapshots(
        &self,
        context: PortContext,
        request: ListMarketplaceCartLineSnapshotsRequest,
    ) -> Result<Vec<CartMarketplaceLineSnapshot>, PortError> {
        let owner_operation = LIST_MARKETPLACE_LINE_SNAPSHOTS_OPERATION;
        require_marketplace_snapshot_policy(&context, owner_operation, PortCallPolicy::read())?;
        let tenant_id = parse_tenant_id(&context, owner_operation)?;
        self.list_cart_snapshots(tenant_id, request.cart_id)
            .await
            .map_err(|error| map_cart_error(&context, owner_operation, error))
    }
}

#[async_trait]
impl MarketplaceCartSnapshotCommandPort for CartMarketplaceSnapshotService {
    async fn add_marketplace_line_item(
        &self,
        context: PortContext,
        request: AddMarketplaceCartLineRequest,
    ) -> Result<AddMarketplaceCartLineItemResponse, PortError> {
        let owner_operation = ADD_MARKETPLACE_LINE_ITEM_OPERATION;
        require_marketplace_snapshot_policy(&context, owner_operation, PortCallPolicy::write())?;
        let tenant_id = parse_tenant_id(&context, owner_operation)?;
        self.add_marketplace_line_item(tenant_id, request.cart_id, request.input)
            .await
            .map_err(|error| map_cart_error(&context, owner_operation, error))
    }

    async fn bind_marketplace_line_snapshot(
        &self,
        context: PortContext,
        request: BindMarketplaceCartLineSnapshotRequest,
    ) -> Result<CartMarketplaceLineSnapshot, PortError> {
        let owner_operation = BIND_MARKETPLACE_LINE_SNAPSHOT_OPERATION;
        require_marketplace_snapshot_policy(&context, owner_operation, PortCallPolicy::write())?;
        let tenant_id = parse_tenant_id(&context, owner_operation)?;
        self.bind_line_snapshot(
            tenant_id,
            request.cart_id,
            request.cart_line_item_id,
            request.snapshot,
        )
        .await
        .map_err(|error| map_cart_error(&context, owner_operation, error))
    }
}

fn require_marketplace_snapshot_policy(
    context: &PortContext,
    owner_operation: &'static str,
    policy: PortCallPolicy,
) -> Result<(), PortError> {
    context
        .require_policy(policy)
        .map_err(|error| map_context_error(context, owner_operation, error))
}

fn parse_tenant_id(
    context: &PortContext,
    owner_operation: &'static str,
) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.tenant_id.trim()).map_err(|error| {
        tracing::warn!(
            error = ?error,
            internal_tenant_id = %context.tenant_id,
            correlation_id = %context.correlation_id,
            tenant_id = %context.tenant_id,
            operation = owner_operation,
            code = "cart.marketplace_snapshot_tenant_invalid",
            "marketplace cart snapshot tenant context is invalid"
        );
        PortError::validation(
            "cart.marketplace_snapshot_tenant_invalid",
            "marketplace cart snapshot request context is invalid",
        )
    })
}

fn map_context_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: PortError,
) -> PortError {
    tracing::warn!(
        internal_code = %error.code,
        internal_message = %error.message,
        kind = ?error.kind,
        retryable = error.retryable,
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        operation = owner_operation,
        code = "cart.marketplace_snapshot_context_invalid",
        "marketplace cart snapshot call context was rejected"
    );

    let PortError {
        kind,
        code,
        retryable,
        ..
    } = error;
    match kind {
        PortErrorKind::Timeout => {
            PortError::timeout(code, "marketplace cart snapshot request context is invalid")
        }
        PortErrorKind::Validation => {
            PortError::validation(code, "marketplace cart snapshot request context is invalid")
        }
        kind => PortError::new(
            kind,
            "cart.marketplace_snapshot_context_invalid",
            "marketplace cart snapshot request context is invalid",
            retryable,
        ),
    }
}

fn map_cart_error(
    context: &PortContext,
    owner_operation: &'static str,
    error: CartError,
) -> PortError {
    eprintln!("DEBUG MAP MARKETPLACE CART ERROR: {error:?}");
    let code = cart_error_code(&error);
    tracing::error!(
        error = ?error,
        correlation_id = %context.correlation_id,
        tenant_id = %context.tenant_id,
        operation = owner_operation,
        code,
        "marketplace cart snapshot owner operation failed"
    );

    match error {
        CartError::Validation(_) => PortError::new(
            PortErrorKind::Conflict,
            "cart.marketplace_snapshot_conflict",
            "marketplace cart snapshot conflicts with the current cart state",
            false,
        ),
        CartError::CartNotFound(_) => PortError::not_found("cart.not_found", "cart was not found"),
        CartError::CartLineItemNotFound(_) => {
            PortError::not_found("cart.line_item_not_found", "cart line item was not found")
        }
        CartError::InvalidTransition { .. } => PortError::conflict(
            "cart.invalid_transition",
            "cart lifecycle transition conflicts with the current state",
        ),
        CartError::Database(_) => PortError::unavailable(
            "cart.marketplace_snapshot_storage_unavailable",
            "marketplace cart snapshot storage is temporarily unavailable",
        ),
        CartError::TaxBoundary {
            kind,
            code,
            retryable,
            ..
        } => PortError::new(
            kind,
            code,
            "marketplace cart snapshot tax recalculation failed",
            retryable,
        ),
    }
}

fn cart_error_code(error: &CartError) -> &str {
    match error {
        CartError::Validation(_) => "cart.marketplace_snapshot_conflict",
        CartError::CartNotFound(_) => "cart.not_found",
        CartError::CartLineItemNotFound(_) => "cart.line_item_not_found",
        CartError::InvalidTransition { .. } => "cart.invalid_transition",
        CartError::Database(_) => "cart.marketplace_snapshot_storage_unavailable",
        CartError::TaxBoundary { code, .. } => code.as_str(),
    }
}
