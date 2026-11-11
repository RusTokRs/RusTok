/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Product-owned port boundary for inventory collaboration.
//!
//! Provides transport-neutral and persistence-neutral contracts for initial variant
//! stock bootstrap, quantity updates, variant deletion cleanup, and inventory projections.
//! No Inventory tables or SeaORM entities are imported here.

use std::collections::HashMap;

use async_trait::async_trait;
use sea_orm::{DatabaseConnection, DatabaseTransaction};
use uuid::Uuid;

use crate::CommerceError;

/// Bound for atomic Product/Inventory deletion collaboration.
pub const MAX_CATALOG_DELETE_VARIANTS: usize = 256;

/// Input for creating initial inventory state for a newly persisted product variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductInitialInventory {
    pub variant_id: Uuid,
    pub sku: Option<String>,
    pub available_quantity: i32,
}

/// Reasons why inventory deletion may be refused by the inventory provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductInventoryDeletionBlocker {
    InvalidRequest,
    LimitExceeded,
    IdentityUnavailable,
    HistoryPresent,
    StockPresent,
    StateUnverified,
    IsolationUnsupported,
    ConcurrentChange,
}

impl ProductInventoryDeletionBlocker {
    pub const ALL: [Self; 8] = [
        Self::InvalidRequest,
        Self::LimitExceeded,
        Self::IdentityUnavailable,
        Self::HistoryPresent,
        Self::StockPresent,
        Self::StateUnverified,
        Self::IsolationUnsupported,
        Self::ConcurrentChange,
    ];

    pub const fn message(self) -> &'static str {
        match self {
            Self::InvalidRequest => {
                "Inventory deletion requires a nonnil tenant and distinct nonnil SKU identities"
            }
            Self::LimitExceeded => {
                "Inventory deletion exceeds the bounded maximum of 256 variants per catalog operation"
            }
            Self::IdentityUnavailable => {
                "Variant identities for inventory deletion could not be verified against catalog ownership"
            }
            Self::HistoryPresent => {
                "Inventory history records exist; variants cannot be deleted while ledger events remain"
            }
            Self::StockPresent => {
                "Inventory stock levels are non-zero; variants cannot be deleted while positive quantity remains"
            }
            Self::StateUnverified => {
                "Inventory records could not be verified within the active product write transaction"
            }
            Self::IsolationUnsupported => {
                "Inventory deletion requires a transactional database connection supporting read isolation"
            }
            Self::ConcurrentChange => {
                "Inventory levels or ledger records were modified concurrently during variant deletion"
            }
        }
    }
}

/// Reasons why quantity mutations may be refused by the inventory provider.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProductInventoryQuantityBlocker {
    InvalidRequest,
    LimitExceeded,
    IdentityUnavailable,
    StateUnverified,
    LocationUnavailable,
    QuantityUnrepresentable,
    InsufficientStock,
    ConcurrentChange,
    IsolationUnsupported,
    EventUnrepresentable,
    HistoryPresent,
    PublicationUnverified,
}

impl ProductInventoryQuantityBlocker {
    pub const ALL: [Self; 12] = [
        Self::InvalidRequest,
        Self::LimitExceeded,
        Self::IdentityUnavailable,
        Self::StateUnverified,
        Self::LocationUnavailable,
        Self::QuantityUnrepresentable,
        Self::InsufficientStock,
        Self::ConcurrentChange,
        Self::IsolationUnsupported,
        Self::EventUnrepresentable,
        Self::HistoryPresent,
        Self::PublicationUnverified,
    ];

    pub const fn kind(self) -> rustok_api::PortErrorKind {
        match self {
            Self::InvalidRequest | Self::LimitExceeded => rustok_api::PortErrorKind::Validation,
            Self::LocationUnavailable
            | Self::InsufficientStock
            | Self::ConcurrentChange
            | Self::HistoryPresent => rustok_api::PortErrorKind::Conflict,
            Self::IdentityUnavailable
            | Self::StateUnverified
            | Self::QuantityUnrepresentable
            | Self::IsolationUnsupported
            | Self::EventUnrepresentable
            | Self::PublicationUnverified => rustok_api::PortErrorKind::InvariantViolation,
        }
    }

    pub const fn message(self) -> &'static str {
        match self {
            Self::InvalidRequest => {
                "Inventory quantity writes require nonnil tenant, actor and SKU identities and valid quantity input"
            }
            Self::LimitExceeded => {
                "Inventory quantity exceeds the complete level bound; reconcile through a bounded Inventory operation"
            }
            Self::IdentityUnavailable => {
                "Inventory quantity cannot prove the saved item and levels; reconcile without recreating stock identity"
            }
            Self::StateUnverified => {
                "Inventory quantity cannot prove all original location bindings and counters; reconcile without moving stock"
            }
            Self::LocationUnavailable => {
                "Inventory quantity needs an active saved owner-default level; configure it explicitly without relocating reservations"
            }
            Self::QuantityUnrepresentable => {
                "Inventory quantity cannot fit the exact storage or response range; reconcile without clamping counters"
            }
            Self::InsufficientStock => {
                "Inventory quantity would violate physical stock or deny-policy availability; reconcile stock and reservations first"
            }
            Self::ConcurrentChange => {
                "Inventory quantity state changed; recover the original command outcome before issuing a new stock instruction"
            }
            Self::IsolationUnsupported => {
                "Inventory quantity requires verified READ COMMITTED on PostgreSQL/MySQL and enabled current-transaction instrumentation on MySQL"
            }
            Self::EventUnrepresentable => {
                "Inventory quantity cannot be represented by the existing stock notification; reconcile without changing reservation history"
            }
            Self::HistoryPresent => {
                "Inventory quantity cannot be updated because active transaction history exists"
            }
            Self::PublicationUnverified => {
                "Inventory quantity notification could not be verified; preserve the original operation and reconcile its outcome"
            }
        }
    }
}

/// Either an open database transaction or a connection pool handle.
#[derive(Copy, Clone)]
pub enum InventoryConnection<'a> {
    Connection(&'a DatabaseConnection),
    Transaction(&'a DatabaseTransaction),
}

impl<'a> From<&'a DatabaseConnection> for InventoryConnection<'a> {
    fn from(c: &'a DatabaseConnection) -> Self {
        InventoryConnection::Connection(c)
    }
}

impl<'a> From<&'a DatabaseTransaction> for InventoryConnection<'a> {
    fn from(tx: &'a DatabaseTransaction) -> Self {
        InventoryConnection::Transaction(tx)
    }
}

pub trait AsInventoryConnection {
    fn as_inventory_connection(&self) -> InventoryConnection<'_>;
}

impl AsInventoryConnection for DatabaseConnection {
    fn as_inventory_connection(&self) -> InventoryConnection<'_> {
        InventoryConnection::Connection(self)
    }
}

impl AsInventoryConnection for DatabaseTransaction {
    fn as_inventory_connection(&self) -> InventoryConnection<'_> {
        InventoryConnection::Transaction(self)
    }
}

impl AsInventoryConnection for crate::services::write_transaction::ProductWriteTransaction {
    fn as_inventory_connection(&self) -> InventoryConnection<'_> {
        InventoryConnection::Transaction(self)
    }
}

/// Transactional and projection port for variant inventory.
#[async_trait]
pub trait ProductInventoryPort: Send + Sync {
    /// Loads available stock quantities for a batch of variants.
    async fn load_available_quantities(
        &self,
        conn: InventoryConnection<'_>,
        variant_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, i32>, CommerceError>;

    /// Loads available stock quantities for variants scoped to a public sales channel.
    async fn load_available_inventory_by_variant_for_public_channel(
        &self,
        conn: InventoryConnection<'_>,
        tenant_id: Uuid,
        public_channel_slug: Option<&str>,
        variant_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, i32>, CommerceError>;

    /// Bootstraps default stock location and initial inventory records for newly created variants in transaction.
    async fn bootstrap_variants_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Uuid,
        items: Vec<ProductInitialInventory>,
    ) -> Result<(), CommerceError>;

    /// Updates the initial quantity of a variant in transaction.
    async fn update_initial_quantity_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        quantity: i32,
    ) -> Result<(), CommerceError>;

    /// Deletes inventory records for specified variants in transaction.
    async fn delete_records_for_variants_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_ids: &[Uuid],
    ) -> Result<(), CommerceError>;
}

/// Default no-op inventory implementation used when running Product in standalone microservice mode
/// or in tests without an inventory service attached.
#[derive(Debug, Default, Clone, Copy)]
pub struct NoopProductInventoryPort;

#[async_trait]
impl ProductInventoryPort for NoopProductInventoryPort {
    async fn load_available_quantities(
        &self,
        _conn: InventoryConnection<'_>,
        _variant_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, i32>, CommerceError> {
        Ok(HashMap::new())
    }

    async fn load_available_inventory_by_variant_for_public_channel(
        &self,
        _conn: InventoryConnection<'_>,
        _tenant_id: Uuid,
        _public_channel_slug: Option<&str>,
        _variant_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, i32>, CommerceError> {
        Ok(HashMap::new())
    }

    async fn bootstrap_variants_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _actor_id: Uuid,
        items: Vec<ProductInitialInventory>,
    ) -> Result<(), CommerceError> {
        if items.iter().any(|item| item.available_quantity != 0) {
            return Err(CommerceError::InventoryQuantityBlocked(
                ProductInventoryQuantityBlocker::LocationUnavailable,
            ));
        }
        Ok(())
    }

    async fn update_initial_quantity_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _actor_id: Uuid,
        _variant_id: Uuid,
        quantity: i32,
    ) -> Result<(), CommerceError> {
        if quantity != 0 {
            return Err(CommerceError::InventoryQuantityBlocked(
                ProductInventoryQuantityBlocker::LocationUnavailable,
            ));
        }
        Ok(())
    }

    async fn delete_records_for_variants_in_tx(
        &self,
        _txn: &DatabaseTransaction,
        _tenant_id: Uuid,
        _actor_id: Uuid,
        _variant_ids: &[Uuid],
    ) -> Result<(), CommerceError> {
        Ok(())
    }
}
