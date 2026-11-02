//! Fulfillment-owned consumer contract. Peer preparation is outside the owner transaction;
//! a prepared plan is per-call state, never authorization, a receipt or external-outcome proof.

use async_trait::async_trait;
use chrono::{DateTime, FixedOffset};
use rustok_api::{PortContext, PortError};
use sea_orm::{DatabaseTransaction, DbErr};
use std::sync::Arc;
use thiserror::Error;
use uuid::Uuid;

pub const MAX_SHIPMENT_STOCK_ITEMS: usize = 1024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ShipmentStockAction {
    Ship,
    Restore,
    Reconsume,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShipmentStockItemScope {
    pub fulfillment_item_id: Uuid,
    pub order_line_item_id: Uuid,
    pub quantity: i32,
    pub shipped_quantity: i32,
    pub delivered_quantity: i32,
    pub movement_quantity: i32,
}

/// Stock-relevant saved owner facts; no prices, publication eligibility or caller metadata.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ShipmentStockScope {
    pub tenant_id: Uuid,
    pub order_id: Uuid,
    pub fulfillment_id: Uuid,
    pub status: String,
    pub updated_at: DateTime<FixedOffset>,
    pub action: ShipmentStockAction,
    pub items: Vec<ShipmentStockItemScope>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ShipmentStockRequest {
    pub fulfillment_item_id: Uuid,
    pub order_line_item_id: Uuid,
    pub quantity: i32,
}

#[derive(Debug, Error)]
pub enum ShipmentStockError {
    #[error("shipment stock port is not configured")]
    NotConfigured,
    #[error("shipment stock operation was rejected")]
    Rejected(String),
    #[error("shipment stock owner refused the operation")]
    Owner(PortError),
    #[error("shipment stock storage is unavailable")]
    Database(#[from] DbErr),
}

#[async_trait]
pub trait PreparedShipmentStock: Send + Sync {
    /// One complete deterministic batch. Native implementations use only this transaction,
    /// not a peer connection, and must not commit it. Consumer rolls back on every error.
    async fn apply(
        &self,
        txn: &DatabaseTransaction,
        requests: &[ShipmentStockRequest],
    ) -> Result<(), ShipmentStockError>;
}

#[async_trait]
pub trait ShipmentStockPort: Send + Sync {
    /// Called before opening the Fulfillment write transaction or executing a fresh carrier
    /// effect. Preserve the canonical actor/claims/scope/deadline and immutable recovery key.
    async fn prepare(
        &self,
        context: &PortContext,
        scope: &ShipmentStockScope,
    ) -> Result<Arc<dyn PreparedShipmentStock>, ShipmentStockError>;
}

/// Opaque owner-correlated token: callers cannot manufacture scopes, change context or use a
/// plan for another fulfillment. Even an explicit no-stock plan must match the locked snapshot.
pub struct PreparedFulfillmentStock {
    pub(crate) context: PortContext,
    pub(crate) scope: ShipmentStockScope,
    pub(crate) plan: Option<Arc<dyn PreparedShipmentStock>>,
    pub(crate) started: std::time::Instant,
}

impl PreparedFulfillmentStock {
    /// Remaining original per-call budget; never refresh the lease while a carrier runs.
    pub fn remaining_context(&self) -> Result<PortContext, PortError> {
        remaining_stock_context(&self.context, self.started)
    }
}

#[derive(Debug, Default, Clone, Copy)]
pub struct UnconfiguredShipmentStockPort;
#[async_trait]
impl ShipmentStockPort for UnconfiguredShipmentStockPort {
    async fn prepare(
        &self,
        _context: &PortContext,
        _scope: &ShipmentStockScope,
    ) -> Result<Arc<dyn PreparedShipmentStock>, ShipmentStockError> {
        Err(ShipmentStockError::NotConfigured)
    }
}

/// Explicit deployment/test opt-out, not an unavailable selected dependency or nullable-SKU fact.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopShipmentStockPort;
#[async_trait]
impl ShipmentStockPort for NoopShipmentStockPort {
    async fn prepare(
        &self,
        _context: &PortContext,
        _scope: &ShipmentStockScope,
    ) -> Result<Arc<dyn PreparedShipmentStock>, ShipmentStockError> {
        Ok(Arc::new(Self))
    }
}
#[async_trait]
impl PreparedShipmentStock for NoopShipmentStockPort {
    async fn apply(
        &self,
        _txn: &DatabaseTransaction,
        _requests: &[ShipmentStockRequest],
    ) -> Result<(), ShipmentStockError> {
        Ok(())
    }
}

/// A deadline is a remaining call budget, never reset after local reads or admission locks.
pub(crate) fn remaining_stock_context(
    context: &PortContext,
    started: std::time::Instant,
) -> Result<PortContext, PortError> {
    let budget = context
        .deadline_ms
        .filter(|value| *value > 0)
        .ok_or_else(|| {
            PortError::timeout(
                "fulfillment.shipment_stock_deadline_expired",
                "shipment preparation requires a positive budget",
            )
        })?;
    let remaining = std::time::Duration::from_millis(budget.min(30_000))
        .checked_sub(started.elapsed())
        .filter(|value| value.as_millis() > 0)
        .ok_or_else(|| {
            PortError::timeout(
                "fulfillment.shipment_stock_deadline_expired",
                "shipment preparation budget has expired",
            )
        })?;
    Ok(context.clone().with_deadline(remaining))
}
