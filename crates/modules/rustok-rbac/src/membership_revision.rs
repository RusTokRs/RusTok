use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseTransaction, DbErr, Statement};
use uuid::Uuid;

/// Read the membership epoch within the caller's existing user mutation fence.
/// SQLite callers must already own the write fence.
pub async fn read_user_membership_revision_on(
    tx: &DatabaseTransaction,
    tenant_id: Uuid,
    user_id: Uuid,
) -> Result<i64, DbErr> {
    let backend = tx.get_database_backend();
    let sql = match backend {
        DatabaseBackend::Postgres => "SELECT rbac_membership_revision FROM users WHERE tenant_id = $1 AND id = $2 FOR UPDATE",
        DatabaseBackend::Sqlite => "SELECT rbac_membership_revision FROM users WHERE tenant_id = ? AND id = ?",
        _ => return Err(DbErr::Custom("unsupported RBAC membership revision backend".into())),
    };
    let row = tx.query_one_raw(Statement::from_sql_and_values(
        backend, sql, [tenant_id.into(), user_id.into()],
    )).await?.ok_or_else(|| DbErr::Custom("RBAC membership subject is missing".into()))?;
    let revision: i64 = row.try_get("", "rbac_membership_revision")?;
    if revision < 0 {
        return Err(DbErr::Custom("invalid RBAC membership revision".into()));
    }
    Ok(revision)
}
