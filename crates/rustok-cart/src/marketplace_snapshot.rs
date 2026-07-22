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
        context.require_policy(PortCallPolicy::read())?;
        self.list_cart_snapshots(parse_tenant_id(&context)?, request.cart_id)
            .await
            .map_err(map_cart_error)
    }
}

#[async_trait]
impl MarketplaceCartSnapshotCommandPort for CartMarketplaceSnapshotService {
    async fn add_marketplace_line_item(
        &self,
        context: PortContext,
        request: AddMarketplaceCartLineRequest,
    ) -> Result<AddMarketplaceCartLineItemResponse, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        self.add_marketplace_line_item(parse_tenant_id(&context)?, request.cart_id, request.input)
            .await
            .map_err(map_cart_error)
    }

    async fn bind_marketplace_line_snapshot(
        &self,
        context: PortContext,
        request: BindMarketplaceCartLineSnapshotRequest,
    ) -> Result<CartMarketplaceLineSnapshot, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        self.bind_line_snapshot(
            parse_tenant_id(&context)?,
            request.cart_id,
            request.cart_line_item_id,
            request.snapshot,
        )
        .await
        .map_err(map_cart_error)
    }
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    Uuid::parse_str(context.tenant_id.trim()).map_err(|_| {
        PortError::validation(
            "cart.marketplace_snapshot_tenant_invalid",
            "marketplace cart snapshot requires a UUID tenant_id",
        )
    })
}

fn map_cart_error(error: CartError) -> PortError {
    match error {
        CartError::Validation(message) => PortError::new(
            PortErrorKind::Conflict,
            "cart.marketplace_snapshot_conflict",
            message,
            false,
        ),
        CartError::CartNotFound(cart_id) => {
            PortError::not_found("cart.not_found", format!("cart {cart_id} not found"))
        }
        CartError::CartLineItemNotFound(line_item_id) => PortError::not_found(
            "cart.line_item_not_found",
            format!("cart line item {line_item_id} not found"),
        ),
        CartError::InvalidTransition { from, to } => PortError::conflict(
            "cart.invalid_transition",
            format!("invalid cart status transition: {from} -> {to}"),
        ),
        CartError::Database(_) => PortError::unavailable(
            "cart.marketplace_snapshot_storage_unavailable",
            "marketplace cart snapshot storage is unavailable",
        ),
        CartError::TaxBoundary {
            kind,
            code,
            message,
            retryable,
        } => PortError::new(kind, code, message, retryable),
    }
}
