/*
 * Copyright (c) 2026 RusTokRs.
 *
 * This file is part of RusTok.
 * Licensed under the Business Source License 1.1 with RusTok Additional Use Grant.
 * See the LICENSE file in the project root for full license terms.
 *
 * You may not remove or alter this copyright notice or license header.
 */

//! Variant identity boundary for Inventory consumption.
//!
//! The current adapter reads the co-located Product schema. Transactional write paths
//! keep their own locking identity checks within the shared database transaction.

use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbErr, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect,
};
use uuid::Uuid;

use rustok_commerce_foundation::entities::product_variant;

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
    find_variant_info_from_conn(conn, tenant_id, variant_id)
        .await
        .map_err(|_| crate::ports_impl::storage_unavailable())?
        .ok_or_else(|| {
            PortError::not_found(
                "inventory.variant_not_found",
                "inventory variant was not found",
            )
        })
}

pub(crate) async fn find_variant_info_from_conn<C: ConnectionTrait>(
    conn: &C,
    tenant_id: Uuid,
    variant_id: Uuid,
) -> Result<Option<InventoryProductVariantInfo>, DbErr> {
    Ok(product_variant::Entity::find_by_id(variant_id)
        .filter(product_variant::Column::TenantId.eq(tenant_id))
        .select_only()
        .column(product_variant::Column::Id)
        .column(product_variant::Column::ProductId)
        .column(product_variant::Column::TenantId)
        .column(product_variant::Column::Sku)
        .column(product_variant::Column::InventoryPolicy)
        .into_tuple::<(Uuid, Uuid, Uuid, Option<String>, String)>()
        .one(conn)
        .await?
        .map(|row| InventoryProductVariantInfo {
            variant_id: row.0,
            product_id: row.1,
            tenant_id: row.2,
            sku: row.3,
            inventory_policy: row.4,
        }))
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
    let rows = product_variant::Entity::find()
        .filter(product_variant::Column::TenantId.eq(tenant_id))
        .filter(product_variant::Column::Id.is_in(variant_ids.iter().copied()))
        .select_only()
        .column(product_variant::Column::Id)
        .column(product_variant::Column::ProductId)
        .column(product_variant::Column::TenantId)
        .column(product_variant::Column::Sku)
        .column(product_variant::Column::InventoryPolicy)
        .into_tuple::<(Uuid, Uuid, Uuid, Option<String>, String)>()
        .all(conn)
        .await
        .map_err(|_| crate::ports_impl::storage_unavailable())?;

    Ok(rows
        .into_iter()
        .map(|row| InventoryProductVariantInfo {
            variant_id: row.0,
            product_id: row.1,
            tenant_id: row.2,
            sku: row.3,
            inventory_policy: row.4,
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
        .map_err(|_| crate::ports_impl::storage_unavailable())?;

    Ok(ids)
}
