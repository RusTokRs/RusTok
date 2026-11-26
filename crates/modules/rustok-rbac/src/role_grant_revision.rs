use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseTransaction, DbErr, Statement};
use uuid::Uuid;

/// Caller holds the issuer's user mutation fence (SQLite: write fence).
/// Lock every relevant role before any permission reads. A missing target is
/// not a seed fallback: there is no persisted identity to bind. The sorted
/// exact role identities and epochs are a stable, non-secret ledger binding.
pub async fn lock_invite_role_grant_binding_on(
    tx: &DatabaseTransaction,
    tenant_id: Uuid,
    issuer_id: Uuid,
    target_slug: &str,
) -> Result<Option<String>, DbErr> {
    let backend = tx.get_database_backend();
    let sql = match backend {
        DatabaseBackend::Postgres => "SELECT r.id, r.slug, r.grant_revision FROM roles r WHERE r.tenant_id = $1 AND (r.slug = $2 OR EXISTS (SELECT 1 FROM user_roles ur WHERE ur.user_id = $3 AND ur.role_id = r.id)) ORDER BY r.id FOR UPDATE OF r",
        DatabaseBackend::Sqlite => "SELECT r.id, r.slug, r.grant_revision FROM roles r WHERE r.tenant_id = ? AND (r.slug = ? OR EXISTS (SELECT 1 FROM user_roles ur WHERE ur.user_id = ? AND ur.role_id = r.id)) ORDER BY r.id",
        _ => return Err(DbErr::Custom("unsupported RBAC grant revision backend".into())),
    };
    let rows = tx.query_all_raw(Statement::from_sql_and_values(
        backend, sql, [tenant_id.into(), target_slug.to_string().into(), issuer_id.into()],
    )).await?;
    let mut target = None;
    let mut revisions = Vec::with_capacity(rows.len());
    for row in rows {
        let id: Uuid = row.try_get("", "id")?;
        let slug: String = row.try_get("", "slug")?;
        let revision: i64 = row.try_get("", "grant_revision")?;
        if revision < 0 || id.is_nil() {
            return Err(DbErr::Custom("invalid RBAC role grant revision".into()));
        }
        if slug == target_slug {
            target = Some(id);
        }
        revisions.push((id, revision));
    }
    let Some(target) = target else { return Ok(None); };
    // Sort decoded UUIDs as well so SQL collation/driver representation does
    // not affect the persisted canonical encoding.
    revisions.sort_unstable();
    let mut binding = format!("v1:{target}");
    for (id, revision) in revisions {
        use std::fmt::Write;
        write!(&mut binding, ";{id}:{revision}")
            .map_err(|_| DbErr::Custom("RBAC binding encoding failed".into()))?;
    }
    Ok(Some(binding))
}
