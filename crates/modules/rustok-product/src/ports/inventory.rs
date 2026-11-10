/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Product-owned port boundary for inventory bootstrap collaboration.
//!
//! Provides transport-neutral and persistence-neutral contracts for initial variant
//! stock bootstrap, quantity updates, and variant deletion cleanup within active write transactions.

use async_trait::async_trait;
use sea_orm::DatabaseTransaction;
use uuid::Uuid;

use crate::CommerceError;

/// Input for creating initial inventory state for a newly persisted product variant.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProductInitialInventory {
    pub variant_id: Uuid,
    pub sku: Option<String>,
    pub available_quantity: i32,
}

/// Transactional port for variant inventory bootstrapping and deletion.
#[async_trait]
pub trait ProductInventoryBootstrapPort: Send + Sync {
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

/// In-process adapter delegating to `rustok_inventory::BootstrapService`.
#[derive(Clone, Copy, Debug, Default)]
pub struct EmbeddedInventoryBootstrapAdapter;

#[async_trait]
impl ProductInventoryBootstrapPort for EmbeddedInventoryBootstrapAdapter {
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
        let default_stock_location =
            rustok_inventory::BootstrapService::ensure_default_location_in_tx(txn, tenant_id)
                .await
                .map_err(CommerceError::Database)?;

        for item in items {
            rustok_inventory::BootstrapService::create_initial_records_in_tx(
                txn,
                &default_stock_location,
                actor_id,
                rustok_inventory::InitialInventory {
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
        rustok_inventory::BootstrapService::update_initial_quantity_in_tx(
            txn, tenant_id, actor_id, variant_id, quantity,
        )
        .await
        .map_err(Into::into)
    }

    async fn delete_records_for_variants_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        actor_id: Uuid,
        variant_ids: &[Uuid],
    ) -> Result<(), CommerceError> {
        rustok_inventory::BootstrapService::delete_records_for_variants_in_tx(
            txn,
            tenant_id,
            actor_id,
            variant_ids,
        )
        .await
        .map_err(Into::into)
    }
}
