/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Adapter implementing `rustok_product::ports::ProductInventoryPort` for the Product catalog.
//!
//! Provides transactional stock bootstrap, quantity updates, variant deletion cleanup,
//! and available stock projections without exposing private Inventory entities or table structures to Product.

use std::collections::HashMap;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_product::CommerceError;
use rustok_product::ports::{
    InventoryConnection, ProductInitialInventory, ProductInventoryDeletionBlocker,
    ProductInventoryPort, ProductInventoryQuantityBlocker,
};
use sea_orm::{DatabaseConnection, DatabaseTransaction};
use uuid::Uuid;

use crate::services::bootstrap::{
    BootstrapService, CatalogDeletionBlocker, CatalogDeletionError, InitialInventory,
};
use crate::services::public_channel::load_available_inventory_by_variant_for_public_channel as public_channel_load;
use crate::{QuantityWriteBlocker, QuantityWriteError};

/// Adapter providing ProductInventoryPort backed by the native Inventory database.
pub struct InventoryProductInventoryPortAdapter {
    _db: DatabaseConnection,
}

impl InventoryProductInventoryPortAdapter {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { _db: db }
    }
}

impl From<CatalogDeletionBlocker> for ProductInventoryDeletionBlocker {
    fn from(b: CatalogDeletionBlocker) -> Self {
        match b {
            CatalogDeletionBlocker::InvalidRequest => Self::InvalidRequest,
            CatalogDeletionBlocker::LimitExceeded => Self::LimitExceeded,
            CatalogDeletionBlocker::IdentityUnavailable => Self::IdentityUnavailable,
            CatalogDeletionBlocker::HistoryPresent => Self::HistoryPresent,
            CatalogDeletionBlocker::StockPresent => Self::StockPresent,
            CatalogDeletionBlocker::StateUnverified => Self::StateUnverified,
            CatalogDeletionBlocker::IsolationUnsupported => Self::IsolationUnsupported,
            CatalogDeletionBlocker::ConcurrentChange => Self::ConcurrentChange,
        }
    }
}

impl From<QuantityWriteBlocker> for ProductInventoryQuantityBlocker {
    fn from(b: QuantityWriteBlocker) -> Self {
        match b {
            QuantityWriteBlocker::InvalidRequest => Self::InvalidRequest,
            QuantityWriteBlocker::LimitExceeded => Self::LimitExceeded,
            QuantityWriteBlocker::IdentityUnavailable => Self::IdentityUnavailable,
            QuantityWriteBlocker::StateUnverified => Self::StateUnverified,
            QuantityWriteBlocker::LocationUnavailable => Self::LocationUnavailable,
            QuantityWriteBlocker::QuantityUnrepresentable => Self::QuantityUnrepresentable,
            QuantityWriteBlocker::InsufficientStock => Self::InsufficientStock,
            QuantityWriteBlocker::ConcurrentChange => Self::ConcurrentChange,
            QuantityWriteBlocker::IsolationUnsupported => Self::IsolationUnsupported,
            QuantityWriteBlocker::EventUnrepresentable => Self::EventUnrepresentable,
            QuantityWriteBlocker::PublicationUnverified => Self::PublicationUnverified,
        }
    }
}

fn map_catalog_deletion_error(err: CatalogDeletionError) -> CommerceError {
    match err {
        CatalogDeletionError::Rejected(b) => CommerceError::InventoryDeletionBlocked(b.into()),
        CatalogDeletionError::Database(err) => CommerceError::Database(err),
    }
}

fn map_quantity_write_error(err: QuantityWriteError) -> CommerceError {
    match err {
        QuantityWriteError::Rejected(b) => CommerceError::InventoryQuantityBlocked(b.into()),
        QuantityWriteError::VariantNotFound(id) => CommerceError::VariantNotFound(id),
        QuantityWriteError::Database(err) => CommerceError::Database(err),
        QuantityWriteError::Publication(rustok_core::error::Error::Database(err)) => {
            CommerceError::Database(err)
        }
        QuantityWriteError::Publication(_) => CommerceError::InventoryQuantityBlocked(
            ProductInventoryQuantityBlocker::PublicationUnverified,
        ),
    }
}

#[async_trait]
impl ProductInventoryPort for InventoryProductInventoryPortAdapter {
    async fn load_available_quantities(
        &self,
        conn: InventoryConnection<'_>,
        variant_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, i32>, CommerceError> {
        match conn {
            InventoryConnection::Connection(c) => {
                BootstrapService::load_available_quantities(c, variant_ids)
                    .await
                    .map_err(CommerceError::from)
            }
            InventoryConnection::Transaction(tx) => {
                BootstrapService::load_available_quantities(tx, variant_ids)
                    .await
                    .map_err(CommerceError::from)
            }
        }
    }

    async fn load_available_inventory_by_variant_for_public_channel(
        &self,
        conn: InventoryConnection<'_>,
        tenant_id: Uuid,
        public_channel_slug: Option<&str>,
        variant_ids: &[Uuid],
    ) -> Result<HashMap<Uuid, i32>, CommerceError> {
        match conn {
            InventoryConnection::Connection(c) => {
                public_channel_load(c, tenant_id, variant_ids, public_channel_slug)
                    .await
                    .map_err(CommerceError::from)
            }
            InventoryConnection::Transaction(tx) => {
                public_channel_load(tx, tenant_id, variant_ids, public_channel_slug)
                    .await
                    .map_err(CommerceError::from)
            }
        }
    }

    async fn bootstrap_variants_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Uuid,
        items: Vec<ProductInitialInventory>,
    ) -> Result<(), CommerceError> {
        if items.is_empty() {
            return Ok(());
        }
        let location = BootstrapService::ensure_default_location_in_tx(txn, tenant_id)
            .await
            .map_err(CommerceError::Database)?;

        for item in items {
            BootstrapService::create_initial_records_in_tx(
                txn,
                &location,
                actor_id,
                InitialInventory {
                    variant_id: item.variant_id,
                    sku: item.sku,
                    available_quantity: item.available_quantity,
                },
            )
            .await
            .map_err(CommerceError::Database)?;
        }
        Ok(())
    }

    async fn update_initial_quantity_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_id: Uuid,
        quantity: i32,
    ) -> Result<(), CommerceError> {
        BootstrapService::update_initial_quantity_in_tx(
            txn,
            tenant_id,
            actor_id,
            variant_id,
            quantity,
        )
        .await
        .map_err(map_quantity_write_error)
    }

    async fn delete_records_for_variants_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_ids: &[Uuid],
    ) -> Result<(), CommerceError> {
        BootstrapService::delete_records_for_variants_in_tx(txn, tenant_id, actor_id, variant_ids)
            .await
            .map_err(map_catalog_deletion_error)
    }
}

/// Creates an in-process adapter implementing `ProductInventoryPort`.
pub fn in_process_product_inventory_port(
    db: DatabaseConnection,
) -> Arc<dyn ProductInventoryPort> {
    Arc::new(InventoryProductInventoryPortAdapter::new(db))
}
