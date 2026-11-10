/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Product-owned port boundary for Inventory consumption.
//!
//! Provides transport-neutral and persistence-neutral contracts so that Inventory logic
//! never queries Product-owned tables (`products`, `product_variants`) directly via SQL.

use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError, PortErrorKind};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use uuid::Uuid;

use rustok_commerce_foundation::entities::{product, product_variant};

/// Validated product variant metadata required by Inventory business operations.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InventoryProductVariantInfo {
    pub variant_id: Uuid,
    pub product_id: Uuid,
    pub tenant_id: Uuid,
    pub sku: Option<String>,
    pub inventory_policy: String,
}

impl InventoryProductVariantInfo {
    pub fn id(&self) -> Uuid {
        self.variant_id
    }

    pub fn allows_backorder(&self) -> bool {
        crate::inventory_policy_allows_backorder(&self.inventory_policy)
    }
}

/// Hexagonal port decoupling Inventory from Product persistence.
///
/// Implementations may execute in-process via `rustok_product`,
/// over network transports via `rustok_product_transport` (gRPC),
/// or fall back to local database queries when running in single-database mode.
#[async_trait]
pub trait InventoryProductPort: Send + Sync {
    /// Validates tenant ownership of a variant and returns its identity and inventory policy.
    async fn get_variant_info(
        &self,
        context: &PortContext,
        tenant_id: Uuid,
        variant_id: Uuid,
    ) -> Result<InventoryProductVariantInfo, PortError>;

    /// Validates a batch of variants for a tenant and returns their parent product IDs and policies.
    async fn get_variants_info(
        &self,
        context: &PortContext,
        tenant_id: Uuid,
        variant_ids: &[Uuid],
    ) -> Result<Vec<InventoryProductVariantInfo>, PortError>;

    /// Paginates through variant IDs for a given tenant.
    async fn list_tenant_variant_ids(
        &self,
        context: &PortContext,
        tenant_id: Uuid,
        after_variant_id: Option<Uuid>,
        limit: usize,
    ) -> Result<Vec<Uuid>, PortError>;
}

/// Fallback adapter using local SeaORM database connection when no separate Product service is configured.
#[derive(Clone)]
pub struct DatabaseFallbackProductAdapter {
    db: DatabaseConnection,
}

impl DatabaseFallbackProductAdapter {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl InventoryProductPort for DatabaseFallbackProductAdapter {
    async fn get_variant_info(
        &self,
        _context: &PortContext,
        tenant_id: Uuid,
        variant_id: Uuid,
    ) -> Result<InventoryProductVariantInfo, PortError> {
        get_variant_info_from_conn(&self.db, tenant_id, variant_id).await
    }

    async fn get_variants_info(
        &self,
        _context: &PortContext,
        tenant_id: Uuid,
        variant_ids: &[Uuid],
    ) -> Result<Vec<InventoryProductVariantInfo>, PortError> {
        get_variants_info_from_conn(&self.db, tenant_id, variant_ids).await
    }

    async fn list_tenant_variant_ids(
        &self,
        _context: &PortContext,
        tenant_id: Uuid,
        after_variant_id: Option<Uuid>,
        limit: usize,
    ) -> Result<Vec<Uuid>, PortError> {
        list_tenant_variant_ids_from_conn(&self.db, tenant_id, after_variant_id, limit).await
    }
}

/// Helper function to load variant metadata from an active database connection or transaction.
pub async fn get_variant_info_from_conn<C: ConnectionTrait>(
    conn: &C,
    tenant_id: Uuid,
    variant_id: Uuid,
) -> Result<InventoryProductVariantInfo, PortError> {
    let variant = product_variant::Entity::find_by_id(variant_id)
        .filter(product_variant::Column::TenantId.eq(tenant_id))
        .one(conn)
        .await
        .map_err(|err| PortError::unavailable("inventory.storage_unavailable", err.to_string()))?
        .ok_or_else(|| {
            PortError::not_found(
                "inventory.variant_not_found",
                "inventory variant was not found",
            )
        })?;

    Ok(InventoryProductVariantInfo {
        variant_id: variant.id,
        product_id: variant.product_id,
        tenant_id: variant.tenant_id,
        sku: variant.sku,
        inventory_policy: variant.inventory_policy,
    })
}

/// Helper function to load batch variant metadata from an active database connection or transaction.
pub async fn get_variants_info_from_conn<C: ConnectionTrait>(
    conn: &C,
    tenant_id: Uuid,
    variant_ids: &[Uuid],
) -> Result<Vec<InventoryProductVariantInfo>, PortError> {
    if variant_ids.is_empty() {
        return Ok(Vec::new());
    }
    let variants = product_variant::Entity::find()
        .filter(product_variant::Column::TenantId.eq(tenant_id))
        .filter(product_variant::Column::Id.is_in(variant_ids.iter().copied()))
        .all(conn)
        .await
        .map_err(|err| PortError::unavailable("inventory.storage_unavailable", err.to_string()))?;

    Ok(variants
        .into_iter()
        .map(|v| InventoryProductVariantInfo {
            variant_id: v.id,
            product_id: v.product_id,
            tenant_id: v.tenant_id,
            sku: v.sku,
            inventory_policy: v.inventory_policy,
        })
        .collect())
}

/// Helper function to paginate through tenant variant IDs from an active database connection.
pub async fn list_tenant_variant_ids_from_conn<C: ConnectionTrait>(
    conn: &C,
    tenant_id: Uuid,
    after_variant_id: Option<Uuid>,
    limit: usize,
) -> Result<Vec<Uuid>, PortError> {
    let mut query = product_variant::Entity::find()
        .select_only()
        .column(product_variant::Column::Id)
        .filter(product_variant::Column::TenantId.eq(tenant_id))
        .order_by_asc(product_variant::Column::Id)
        .limit((limit + 1) as u64);

    if let Some(after) = after_variant_id {
        query = query.filter(product_variant::Column::Id.gt(after));
    }

    let ids = query
        .into_tuple::<Uuid>()
        .all(conn)
        .await
        .map_err(|err| PortError::unavailable("inventory.storage_unavailable", err.to_string()))?;

    Ok(ids)
}
