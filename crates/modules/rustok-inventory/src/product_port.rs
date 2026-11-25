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

use std::collections::{BTreeSet, HashMap};

use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction, DbBackend, DbErr,
    EntityTrait, QueryFilter, QueryOrder, QuerySelect, Select,
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
/// over network transports via `rustok_product_ports::grpc` (gRPC),
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
    if tenant_id.is_nil() || variant_id.is_nil() {
        return Err(PortError::validation(
            "inventory.variant_identity_invalid",
            "tenant and SKU identities must be non-nil UUIDs",
        ));
    }
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
    read_variant_info(conn, tenant_owned_variant_info_query(tenant_id, variant_id)).await
}

/// New allocations must serialize their policy/identity read with Product edits and
/// deletion before acquiring an Inventory item lock. Release/adoption paths
/// retain nonlocking Product reads, avoiding inversion of their item lock order.
pub(crate) async fn find_variant_info_for_reservation(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    variant_id: Uuid,
) -> Result<Option<InventoryProductVariantInfo>, DbErr> {
    let query = reservation_variant_query(txn.get_database_backend(), tenant_id, variant_id)?;
    let Some(variant) = read_variant_info(txn, query).await? else {
        return Ok(None);
    };

    // INVARIANT: Product edits lock parent before Variant. Keep this parent proof
    // nonlocking after the Variant lock to avoid reversing that global lock order.
    let parent_id = product::Entity::find_by_id(variant.product_id)
        .select_only()
        .column(product::Column::Id)
        .filter(product::Column::TenantId.eq(tenant_id))
        .into_tuple::<Uuid>()
        .one(txn)
        .await?;
    if parent_id != Some(variant.product_id) {
        return Ok(None);
    }
    Ok(Some(variant))
}

fn variant_info_query(tenant_id: Uuid, variant_id: Uuid) -> Select<product_variant::Entity> {
    product_variant::Entity::find_by_id(variant_id)
        .filter(product_variant::Column::TenantId.eq(tenant_id))
        .select_only()
        .column(product_variant::Column::Id)
        .column(product_variant::Column::ProductId)
        .column(product_variant::Column::TenantId)
        .column(product_variant::Column::Sku)
        .column(product_variant::Column::InventoryPolicy)
}

fn tenant_owned_variant_info_query(
    tenant_id: Uuid,
    variant_id: Uuid,
) -> Select<product_variant::Entity> {
    variant_info_query(tenant_id, variant_id)
        .inner_join(product::Entity)
        .filter(product::Column::TenantId.eq(tenant_id))
}

fn reservation_variant_query(
    backend: DbBackend,
    tenant_id: Uuid,
    variant_id: Uuid,
) -> Result<Select<product_variant::Entity>, DbErr> {
    let query = variant_info_query(tenant_id, variant_id);
    match backend {
        DbBackend::Postgres | DbBackend::MySql => Ok(query.lock_exclusive()),
        // SQLite's write transaction/snapshot exclusion rejects conflicting writers.
        DbBackend::Sqlite => Ok(query),
        _ => Err(DbErr::Custom("Inventory reservation isolation is unsupported".into())),
    }
}

async fn read_variant_info<C: ConnectionTrait>(
    conn: &C,
    query: Select<product_variant::Entity>,
) -> Result<Option<InventoryProductVariantInfo>, DbErr> {
    let row = query
        .into_tuple::<(Uuid, Uuid, Uuid, Option<String>, String)>()
        .one(conn)
        .await?;
    if row.as_ref().is_some_and(|row| row.0.is_nil() || row.1.is_nil() || row.2.is_nil()) {
        return Err(DbErr::Custom("Stored Product variant identity is invalid".into()));
    }
    Ok(row.map(|row| InventoryProductVariantInfo {
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
    let rows = find_variants_info_from_conn(conn, tenant_id, variant_ids).await?;
    if rows.len() != variant_ids.len() {
        return Err(PortError::not_found(
            "inventory.variant_not_found",
            "one or more inventory variants were not found in the current tenant",
        ));
    }
    let mut by_id = rows
        .into_iter()
        .map(|variant| (variant.variant_id, variant))
        .collect::<HashMap<_, _>>();
    if by_id.len() != variant_ids.len() {
        return Err(PortError::invariant_violation(
            "inventory.variant_identity_duplicate",
            "stored Product variant identities are not unique",
        ));
    }
    variant_ids
        .iter()
        .map(|variant_id| {
            by_id.remove(variant_id).ok_or_else(|| {
                PortError::not_found(
                    "inventory.variant_not_found",
                    "one or more inventory variants were not found in the current tenant",
                )
            })
        })
        .collect()
}

pub(crate) async fn find_variants_info_from_conn<C: ConnectionTrait>(
    conn: &C,
    tenant_id: Uuid,
    variant_ids: &[Uuid],
) -> Result<Vec<InventoryProductVariantInfo>, PortError> {
    if variant_ids.is_empty() {
        return Ok(Vec::new());
    }
    validate_variant_identity_batch(tenant_id, variant_ids)?;
    let rows = product_variant::Entity::find()
        .inner_join(product::Entity)
        .filter(product_variant::Column::TenantId.eq(tenant_id))
        .filter(product::Column::TenantId.eq(tenant_id))
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
    if rows.iter().any(|row| row.0.is_nil() || row.1.is_nil() || row.2 != tenant_id) {
        return Err(PortError::invariant_violation(
            "inventory.variant_identity_invalid",
            "stored Product variant identity is invalid",
        ));
    }

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

fn validate_variant_identity_batch(
    tenant_id: Uuid,
    variant_ids: &[Uuid],
) -> Result<(), PortError> {
    if tenant_id.is_nil() || variant_ids.iter().any(Uuid::is_nil) {
        return Err(PortError::validation(
            "inventory.variant_identity_invalid",
            "tenant and SKU identities must be non-nil UUIDs",
        ));
    }
    if variant_ids.len() > rustok_product::MAX_VARIANT_IDENTITY_BATCH {
        return Err(PortError::validation(
            "inventory.variant_identity_batch_invalid",
            "request no more than 256 SKU identities",
        ));
    }
    if variant_ids.iter().copied().collect::<BTreeSet<_>>().len() != variant_ids.len() {
        return Err(PortError::validation(
            "inventory.variant_identity_duplicate",
            "request each SKU identity only once",
        ));
    }
    Ok(())
}

/// Helper function to paginate through tenant variant IDs from an active database connection.
/// Returns one extra ID beyond `limit` so the caller can detect another page.
pub async fn list_tenant_variant_ids_from_conn<C: ConnectionTrait>(
    conn: &C,
    tenant_id: Uuid,
    after_variant_id: Option<Uuid>,
    limit: usize,
) -> Result<Vec<Uuid>, PortError> {
    if tenant_id.is_nil()
        || after_variant_id.is_some_and(Uuid::is_nil)
        || limit == 0
        || limit > rustok_product::MAX_VARIANT_IDENTITY_BATCH
    {
        return Err(PortError::validation(
            "inventory.variant_identity_page_invalid",
            "request a non-nil tenant, optional non-nil cursor, and a page size from 1 to 256",
        ));
    }
    let mut query = product_variant::Entity::find()
        .inner_join(product::Entity)
        .select_only()
        .column(product_variant::Column::Id)
        .filter(product_variant::Column::TenantId.eq(tenant_id))
        .filter(product::Column::TenantId.eq(tenant_id))
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

    if ids.iter().any(Uuid::is_nil) {
        return Err(PortError::invariant_violation(
            "inventory.variant_identity_invalid",
            "stored Product variant identity is invalid",
        ));
    }
    Ok(ids)
}

#[cfg(test)]
mod reservation_variant_tests {
    use super::*;
    use sea_orm::QueryTrait;

    #[test]
    fn ordinary_variant_identity_reads_prove_the_parent_tenant_without_locking() {
        let sql = tenant_owned_variant_info_query(Uuid::from_u128(1), Uuid::from_u128(2))
            .build(DbBackend::Postgres)
            .to_string();
        assert!(sql.contains("JOIN"), "{sql}");
        assert!(sql.contains("product_variants") && sql.contains("products"), "{sql}");
        assert!(sql.matches("tenant_id").count() >= 2, "{sql}");
        assert!(!sql.contains("FOR UPDATE"), "{sql}");
    }

    #[tokio::test]
    async fn tenant_variant_reads_reject_cross_tenant_parent_bindings() {
        use sea_orm::{ConnectionTrait, Database, DatabaseBackend, Statement, TransactionTrait};

        let db = Database::connect("sqlite::memory:")
            .await
            .expect("connect Product identity fixture");
        for sql in [
            "CREATE TABLE products (id TEXT PRIMARY KEY NOT NULL, tenant_id TEXT NOT NULL)",
            "CREATE TABLE product_variants (id TEXT PRIMARY KEY NOT NULL, tenant_id TEXT NOT NULL, product_id TEXT NOT NULL, sku TEXT, inventory_policy TEXT NOT NULL)",
        ] {
            db.execute_raw(Statement::from_string(DatabaseBackend::Sqlite, sql))
                .await
                .expect("create Product identity table");
        }
        let tenant_id = Uuid::new_v4();
        let foreign_tenant_id = Uuid::new_v4();
        let foreign_product_id = Uuid::new_v4();
        let variant_id = Uuid::new_v4();
        db.execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO products (id, tenant_id) VALUES (?, ?)",
            [foreign_product_id.into(), foreign_tenant_id.into()],
        ))
        .await
        .expect("insert foreign Product");
        db.execute_raw(Statement::from_sql_and_values(
            DatabaseBackend::Sqlite,
            "INSERT INTO product_variants (id, tenant_id, product_id, sku, inventory_policy) VALUES (?, ?, ?, ?, ?)",
            [variant_id.into(), tenant_id.into(), foreign_product_id.into(), "SKU-X".into(), "deny".into()],
        ))
        .await
        .expect("insert mismatched ProductVariant");

        let error = get_variant_info_from_conn(&db, tenant_id, variant_id)
            .await
            .expect_err("foreign parent must not authorize the SKU");
        assert_eq!(error.code, "inventory.variant_not_found");
        let batch_error = get_variants_info_from_conn(&db, tenant_id, &[variant_id])
            .await
            .expect_err("batch reads must not omit an unowned parent");
        assert_eq!(batch_error.code, "inventory.variant_not_found");

        let txn = db.begin().await.expect("begin reservation identity read");
        assert!(
            find_variant_info_for_reservation(&txn, tenant_id, variant_id)
                .await
                .expect("parent proof query")
                .is_none(),
            "reservation must reject a Variant whose parent belongs to another tenant",
        );
        txn.rollback().await.expect("rollback identity fixture");
    }

    #[test]
    fn variant_identity_batch_rejects_duplicate_and_unbounded_requests() {
        let tenant_id = Uuid::new_v4();
        let variant_id = Uuid::new_v4();
        assert!(validate_variant_identity_batch(tenant_id, &[variant_id, variant_id]).is_err());
        assert!(
            validate_variant_identity_batch(
                tenant_id,
                &vec![variant_id; rustok_product::MAX_VARIANT_IDENTITY_BATCH + 1],
            )
            .is_err()
        );
        assert!(validate_variant_identity_batch(Uuid::nil(), &[variant_id]).is_err());
    }

    #[test]
    fn reservation_policy_query_locks_only_the_tenant_scoped_variant() {
        let tenant = Uuid::from_u128(1);
        let variant = Uuid::from_u128(2);
        for backend in [DbBackend::Postgres, DbBackend::MySql] {
            let sql = reservation_variant_query(backend, tenant, variant)
                .unwrap()
                .build(backend)
                .to_string();
            assert!(sql.contains("FOR UPDATE"), "{sql}");
            assert!(sql.contains("tenant_id") && sql.contains("inventory_policy"), "{sql}");
            assert!(!sql.contains("JOIN"), "{sql}");
            assert!(!variant_info_query(tenant, variant)
                .build(backend)
                .to_string()
                .contains("FOR UPDATE"));
        }
        assert!(!reservation_variant_query(DbBackend::Sqlite, tenant, variant)
            .unwrap()
            .build(DbBackend::Sqlite)
            .to_string()
            .contains("FOR UPDATE"));
    }
}
