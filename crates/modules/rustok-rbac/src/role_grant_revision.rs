use sea_orm::{ConnectionTrait, DatabaseBackend, DatabaseTransaction, DbErr, Statement};
use uuid::Uuid;

/// Shared role-grant fence. The caller holds the actor's user mutation
/// fence (SQLite: write fence). An optional target adds a role to the same
/// UUID-ordered lock set; NULL selects only the actor's membership roles.
async fn lock_role_grant_rows_on(
    tx: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Uuid,
    target_slug: Option<&str>,
) -> Result<Vec<(Uuid, String, i64)>, DbErr> {
    let backend = tx.get_database_backend();
    let sql = match backend {
        DatabaseBackend::Postgres => {
            "SELECT r.id, r.slug, r.grant_revision FROM roles r WHERE r.tenant_id = $1 AND (r.slug = $2 OR EXISTS (SELECT 1 FROM user_roles ur WHERE ur.user_id = $3 AND ur.role_id = r.id)) ORDER BY r.id FOR UPDATE OF r"
        }
        DatabaseBackend::Sqlite => {
            "SELECT r.id, r.slug, r.grant_revision FROM roles r WHERE r.tenant_id = ? AND (r.slug = ? OR EXISTS (SELECT 1 FROM user_roles ur WHERE ur.user_id = ? AND ur.role_id = r.id)) ORDER BY r.id"
        }
        _ => {
            return Err(DbErr::Custom(
                "unsupported RBAC grant revision backend".into(),
            ));
        }
    };
    let rows = tx
        .query_all_raw(Statement::from_sql_and_values(
            backend,
            sql,
            [
                tenant_id.into(),
                target_slug.map(str::to_owned).into(),
                actor_id.into(),
            ],
        ))
        .await?;
    let mut locked = Vec::with_capacity(rows.len());
    for row in rows {
        let id: Uuid = row.try_get("", "id")?;
        let slug: String = row.try_get("", "slug")?;
        let revision: i64 = row.try_get("", "grant_revision")?;
        if revision < 0 || id.is_nil() {
            return Err(DbErr::Custom("invalid RBAC role grant revision".into()));
        }
        locked.push((id, slug, revision));
    }
    Ok(locked)
}

/// Lock only the current actor's static membership roles, for immediate
/// authority checks that do not delegate a destination role.
pub async fn lock_actor_role_grants_on(
    tx: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Uuid,
) -> Result<(), DbErr> {
    lock_role_grant_rows_on(tx, tenant_id, actor_id, None).await?;
    Ok(())
}

/// Lock issuer and destination roles before any permission reads. A missing
/// destination cannot be bound. Preserve the exact canonical ledger encoding.
pub async fn lock_invite_role_grant_binding_on(
    tx: &DatabaseTransaction,
    tenant_id: Uuid,
    issuer_id: Uuid,
    target_slug: &str,
) -> Result<Option<String>, DbErr> {
    let rows = lock_role_grant_rows_on(tx, tenant_id, issuer_id, Some(target_slug)).await?;
    let mut target = None;
    let mut revisions = Vec::with_capacity(rows.len());
    for (id, slug, revision) in rows {
        if slug == target_slug {
            target = Some(id);
        }
        revisions.push((id, revision));
    }
    let Some(target) = target else {
        return Ok(None);
    };
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

/// A read-only, single-statement static RBAC observation. Membership and
/// role grant epochs are read from one database snapshot, including users
/// with no roles. Compare before and after multi-query authority resolution;
/// unlike the cache generation this also detects direct relation writes.
#[derive(Debug, PartialEq, Eq)]
pub struct StaticActorAuthorityRevision {
    membership: i64,
    grants: Vec<(Uuid, i64)>,
}

impl StaticActorAuthorityRevision {
    /// Include owned role revision storage when weighting bounded cache entries.
    pub fn estimated_heap_bytes(&self) -> usize {
        std::mem::size_of::<Self>()
            .saturating_add(self.grants.capacity().saturating_mul(std::mem::size_of::<(Uuid, i64)>()))
    }
}

pub(crate) const STATIC_ACTOR_REVISION_SELECT: &str =
    "SELECT u.rbac_membership_revision, r.id AS role_id, r.grant_revision FROM users u LEFT JOIN (user_roles ur JOIN roles r ON r.id = ur.role_id) ON ur.user_id = u.id AND r.tenant_id = u.tenant_id";

pub async fn read_static_actor_authority_revision_on(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    actor_id: Uuid,
) -> Result<StaticActorAuthorityRevision, DbErr> {
    read_optional_static_actor_authority_revision_on(db, tenant_id, actor_id)
        .await?
        .ok_or_else(|| DbErr::Custom("invalid or missing RBAC authority revision".into()))
}

/// A missing subject is an empty relation graph for unscoped cache lookups,
/// not a reusable revision. Malformed revisions and storage errors still fail.
pub(crate) async fn read_optional_static_actor_authority_revision_on(
    db: &impl ConnectionTrait,
    tenant_id: Uuid,
    actor_id: Uuid,
) -> Result<Option<StaticActorAuthorityRevision>, DbErr> {
    let backend = db.get_database_backend();
    let predicate = match backend {
        DatabaseBackend::Postgres => "u.tenant_id = $1 AND u.id = $2",
        DatabaseBackend::Sqlite => "u.tenant_id = ?1 AND u.id = ?2",
        _ => {
            return Err(DbErr::Custom(
                "unsupported RBAC authority revision backend".into(),
            ));
        }
    };
    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            backend,
            format!("{STATIC_ACTOR_REVISION_SELECT} WHERE {predicate} ORDER BY r.id"),
            [tenant_id.into(), actor_id.into()],
        ))
        .await?;
    decode_static_actor_authority_revision(&rows)
}

pub(crate) fn decode_static_actor_authority_revision(
    rows: &[sea_orm::QueryResult],
) -> Result<Option<StaticActorAuthorityRevision>, DbErr> {
    let missing = || DbErr::Custom("invalid or missing RBAC authority revision".into());
    let Some(first) = rows.first() else {
        return Ok(None);
    };
    let membership: i64 = first.try_get("", "rbac_membership_revision")?;
    if membership < 0 {
        return Err(missing());
    }
    let mut grants = Vec::new();
    for row in rows {
        let revision: i64 = row.try_get("", "rbac_membership_revision")?;
        let role: Option<Uuid> = row.try_get("", "role_id")?;
        let grant: Option<i64> = row.try_get("", "grant_revision")?;
        if revision != membership {
            return Err(missing());
        }
        match (role, grant) {
            (Some(id), Some(epoch)) if !id.is_nil() && epoch >= 0 => grants.push((id, epoch)),
            (None, None) => {}
            _ => return Err(missing()),
        }
    }
    grants.sort_unstable();
    grants.dedup();
    Ok(Some(StaticActorAuthorityRevision { membership, grants }))
}

/// Lock the union of actor and managed-user memberships, requested role and
/// continuity role in one deterministic order. Both existing users must be
/// locked first by the transaction owner (SQLite: write fence).
pub async fn lock_user_admin_role_grants_on(
    tx: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Uuid,
    target_id: Option<Uuid>,
    requested_slug: Option<&str>,
) -> Result<bool, DbErr> {
    let backend = tx.get_database_backend();
    let continuity_slug = target_id.map(|_| rustok_core::UserRole::SuperAdmin.to_string());
    let sql = match backend {
        DatabaseBackend::Postgres => {
            "SELECT r.id, r.slug, r.grant_revision FROM roles r WHERE r.tenant_id = $1 AND (r.slug = $2 OR r.slug = $5 OR EXISTS (SELECT 1 FROM user_roles ur WHERE (ur.user_id = $3 OR ur.user_id = $4) AND ur.role_id = r.id)) ORDER BY r.id FOR UPDATE OF r"
        }
        DatabaseBackend::Sqlite => {
            "SELECT r.id, r.slug, r.grant_revision FROM roles r WHERE r.tenant_id = ?1 AND (r.slug = ?2 OR r.slug = ?5 OR EXISTS (SELECT 1 FROM user_roles ur WHERE (ur.user_id = ?3 OR ur.user_id = ?4) AND ur.role_id = r.id)) ORDER BY r.id"
        }
        _ => {
            return Err(DbErr::Custom(
                "unsupported RBAC grant revision backend".into(),
            ));
        }
    };
    let rows = tx
        .query_all_raw(Statement::from_sql_and_values(
            backend,
            sql,
            [
                tenant_id.into(),
                requested_slug.map(str::to_owned).into(),
                actor_id.into(),
                target_id.into(),
                continuity_slug.into(),
            ],
        ))
        .await?;
    let mut found_requested = requested_slug.is_none();
    for row in rows {
        let id: Uuid = row.try_get("", "id")?;
        let revision: i64 = row.try_get("", "grant_revision")?;
        let slug: String = row.try_get("", "slug")?;
        found_requested |= requested_slug == Some(slug.as_str());
        if id.is_nil() || revision < 0 {
            return Err(DbErr::Custom("invalid RBAC role grant revision".into()));
        }
    }
    Ok(found_requested)
}

/// Acquire the actor's grants and the exact destination role as one UUID-
/// ordered set. A preflight slug lookup cannot substitute a recreated role.
pub async fn lock_role_definition_authority_on(
    tx: &DatabaseTransaction,
    tenant_id: Uuid,
    actor_id: Uuid,
    target_role_id: Option<Uuid>,
) -> Result<bool, DbErr> {
    let backend = tx.get_database_backend();
    let sql = match backend {
        DatabaseBackend::Postgres => {
            "SELECT r.id, r.grant_revision FROM roles r WHERE r.tenant_id = $1 AND (r.id = $2 OR EXISTS (SELECT 1 FROM user_roles ur WHERE ur.user_id = $3 AND ur.role_id = r.id)) ORDER BY r.id FOR UPDATE OF r"
        }
        DatabaseBackend::Sqlite => {
            "SELECT r.id, r.grant_revision FROM roles r WHERE r.tenant_id = ?1 AND (r.id = ?2 OR EXISTS (SELECT 1 FROM user_roles ur WHERE ur.user_id = ?3 AND ur.role_id = r.id)) ORDER BY r.id"
        }
        _ => {
            return Err(DbErr::Custom(
                "unsupported RBAC grant revision backend".into(),
            ));
        }
    };
    let rows = tx
        .query_all_raw(Statement::from_sql_and_values(
            backend,
            sql,
            [tenant_id.into(), target_role_id.into(), actor_id.into()],
        ))
        .await?;
    let mut found = target_role_id.is_none();
    for row in rows {
        let id: Uuid = row.try_get("", "id")?;
        let revision: i64 = row.try_get("", "grant_revision")?;
        if id.is_nil() || revision < 0 {
            return Err(DbErr::Custom("invalid RBAC role grant revision".into()));
        }
        found |= target_role_id == Some(id);
    }
    Ok(found)
}
