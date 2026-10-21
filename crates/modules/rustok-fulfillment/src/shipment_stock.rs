//! Stock-movement port for physical shipment.
//!
//! The fulfillment module owns *when* goods leave the warehouse (ship, cancel of a shipped
//! fulfillment, reopen). It does not own stock. This module declares the consumer-defined port
//! through which the fulfillment owner asks the stock owner to move stock **inside the
//! fulfillment transaction**, so the shipment counters and the stock ledger commit or roll back
//! together. The inventory-backed adapter is composed in `rustok-commerce`.
//!
//! The default port of [`FulfillmentService`](crate::FulfillmentService) is
//! [`UnconfiguredShipmentStockPort`], which fails every movement: a deployment that forgets to
//! wire stock must not silently ship without consuming it. Deployments without stock (digital
//! only) and tests opt out explicitly with [`NoopShipmentStockPort`].

use async_trait::async_trait;
use sea_orm::{DatabaseTransaction, DbErr};
use thiserror::Error;
use uuid::Uuid;

/// One stock movement for a fulfilled order line.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ShipmentStockRequest {
    pub tenant_id: Uuid,
    pub order_line_item_id: Uuid,
    /// Strictly positive number of units.
    pub quantity: i32,
}

#[derive(Debug, Error)]
pub enum ShipmentStockError {
    /// The stock owner refused the movement (missing reservation, quantity above the
    /// reservation, inconsistent ledger, ...). Rolls the fulfillment operation back.
    #[error("{0}")]
    Rejected(String),
    /// No stock port is wired into this `FulfillmentService`.
    #[error("shipment stock port is not configured")]
    NotConfigured,
    #[error(transparent)]
    Database(#[from] DbErr),
}

/// Stock movements that must be atomic with fulfillment shipment state.
///
/// Every method runs on the caller's transaction and must not commit or roll it back.
#[async_trait]
pub trait ShipmentStockPort: Send + Sync {
    /// Units physically left the warehouse: consume them from the order line's reservation.
    async fn consume_shipped(
        &self,
        txn: &DatabaseTransaction,
        request: ShipmentStockRequest,
    ) -> Result<(), ShipmentStockError>;

    /// A shipped fulfillment was cancelled: the undelivered units are back on the shelf.
    async fn restore_unshipped(
        &self,
        txn: &DatabaseTransaction,
        request: ShipmentStockRequest,
    ) -> Result<(), ShipmentStockError>;

    /// A cancelled shipped fulfillment was reopened: take the units off the shelf again.
    /// Inverse of [`restore_unshipped`](Self::restore_unshipped).
    async fn reconsume_restored(
        &self,
        txn: &DatabaseTransaction,
        request: ShipmentStockRequest,
    ) -> Result<(), ShipmentStockError>;
}

/// Default port: refuses every movement so an unwired runtime fails loudly.
#[derive(Debug, Default, Clone, Copy)]
pub struct UnconfiguredShipmentStockPort;

#[async_trait]
impl ShipmentStockPort for UnconfiguredShipmentStockPort {
    async fn consume_shipped(
        &self,
        _txn: &DatabaseTransaction,
        _request: ShipmentStockRequest,
    ) -> Result<(), ShipmentStockError> {
        Err(ShipmentStockError::NotConfigured)
    }

    async fn restore_unshipped(
        &self,
        _txn: &DatabaseTransaction,
        _request: ShipmentStockRequest,
    ) -> Result<(), ShipmentStockError> {
        Err(ShipmentStockError::NotConfigured)
    }

    async fn reconsume_restored(
        &self,
        _txn: &DatabaseTransaction,
        _request: ShipmentStockRequest,
    ) -> Result<(), ShipmentStockError> {
        Err(ShipmentStockError::NotConfigured)
    }
}

/// Explicit opt-out for deployments and tests that do not track stock.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopShipmentStockPort;

#[async_trait]
impl ShipmentStockPort for NoopShipmentStockPort {
    async fn consume_shipped(
        &self,
        _txn: &DatabaseTransaction,
        _request: ShipmentStockRequest,
    ) -> Result<(), ShipmentStockError> {
        Ok(())
    }

    async fn restore_unshipped(
        &self,
        _txn: &DatabaseTransaction,
        _request: ShipmentStockRequest,
    ) -> Result<(), ShipmentStockError> {
        Ok(())
    }

    async fn reconsume_restored(
        &self,
        _txn: &DatabaseTransaction,
        _request: ShipmentStockRequest,
    ) -> Result<(), ShipmentStockError> {
        Ok(())
    }
}
