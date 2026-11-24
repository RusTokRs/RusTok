use sea_orm::sea_query::LockType;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseBackend, DatabaseTransaction, DbErr, EntityTrait,
    QueryFilter, QuerySelect, Select, Statement,
};
use uuid::Uuid;

use crate::entities::{product, product_variant};

fn product_in_tenant(tenant_id: Uuid, product_id: Uuid) -> Select<product::Entity> {
    product::Entity::find_by_id(product_id).filter(product::Column::TenantId.eq(tenant_id))
}

/// Checks a local Product reference without loading the aggregate or its lifecycle details.
pub async fn product_exists_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
) -> Result<bool, DbErr> {
    Ok(product_in_tenant(tenant_id, product_id)
        .select_only()
        .column(product::Column::Id)
        .into_tuple::<Uuid>()
        .one(txn)
        .await?
        .is_some())
}

/// Returns the parent of a Variant in the caller's tenant and transaction.
pub async fn variant_parent_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    variant_id: Uuid,
) -> Result<Option<Uuid>, DbErr> {
    product_variant::Entity::find_by_id(variant_id)
        .filter(product_variant::Column::TenantId.eq(tenant_id))
        .select_only()
        .column(product_variant::Column::ProductId)
        .into_tuple::<Uuid>()
        .one(txn)
        .await
}

/// Serializes co-located extension writes on an existing Product until transaction completion.
/// The boolean is false when the Product is absent from the tenant.
pub async fn lock_product_reference_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
) -> Result<bool, DbErr> {
    if txn.get_database_backend() == DatabaseBackend::Sqlite {
        return lock_sqlite_product_in_tx(txn, tenant_id, product_id).await;
    }
    let query = product_in_tenant(tenant_id, product_id)
        .select_only()
        .column(product::Column::Id);
    // Keep foreign-key checks on the related Product compatible with reciprocal creates.
    let query = match txn.get_database_backend() {
        DatabaseBackend::Postgres => query.lock(LockType::NoKeyUpdate),
        DatabaseBackend::MySql => query.lock_exclusive(),
        backend => {
            return Err(DbErr::Custom(format!(
                "Unsupported Product lock backend {backend:?}"
            )));
        }
    };
    Ok(query.into_tuple::<Uuid>().one(txn).await?.is_some())
}

pub(super) async fn lock_sqlite_product_in_tx(
    txn: &DatabaseTransaction,
    tenant_id: Uuid,
    product_id: Uuid,
) -> Result<bool, DbErr> {
    // SQLite has no row-level lock. A no-op write acquires its transaction write lock.
    let statement = Statement::from_sql_and_values(
        DatabaseBackend::Sqlite,
        "UPDATE products SET updated_at = updated_at WHERE tenant_id = ?1 AND id = ?2",
        [tenant_id.into(), product_id.into()],
    );
    Ok(txn.execute_raw(statement).await?.rows_affected() == 1)
}
