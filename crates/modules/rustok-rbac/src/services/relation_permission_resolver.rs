use crate::role_grant_revision::{
    STATIC_ACTOR_REVISION_SELECT, StaticActorAuthorityRevision,
    decode_static_actor_authority_revision, read_optional_static_actor_authority_revision_on,
};
use crate::services::permission_normalization::normalize_permissions;
use std::sync::Arc;
use rustok_api::Permission;
use sea_orm::ConnectionTrait;

const MAX_PERMISSION_CACHE_RESOLUTION_ATTEMPTS: usize = 4;

// One graph definition for the current read and cache-hit validation.
const PERSISTED_GRANTS_SQL: &str = "SELECT DISTINCT permission.resource, permission.action \
             FROM users subject \
             JOIN user_roles membership ON membership.user_id = subject.id \
             JOIN roles role ON role.id = membership.role_id AND role.tenant_id = subject.tenant_id \
             JOIN role_permissions grant_row ON grant_row.role_id = role.id \
             JOIN permissions permission ON permission.id = grant_row.permission_id AND permission.tenant_id = role.tenant_id \
             WHERE subject.tenant_id = ? AND subject.id = ?";

/// Canonical persisted relation reader shared by cached and current decisions.
#[derive(Clone)]
pub struct SeaOrmRelationPermissionStore {
    db: sea_orm::DatabaseConnection,
}

impl SeaOrmRelationPermissionStore {
    pub fn new(db: sea_orm::DatabaseConnection) -> Self {
        Self { db }
    }
}

struct ConnectionRelationPermissionStore<'a, C: ConnectionTrait> {
    db: &'a C,
}

impl<C: ConnectionTrait> ConnectionRelationPermissionStore<'_, C> {
    fn statement(
        &self,
        sql: &str,
        values: Vec<sea_orm::Value>,
    ) -> Result<sea_orm::Statement, sea_orm::DbErr> {
        let backend = self.db.get_database_backend();
        if !matches!(
            backend,
            sea_orm::DbBackend::Postgres | sea_orm::DbBackend::Sqlite
        ) {
            return Err(sea_orm::DbErr::Custom(
                "RBAC relation reader requires PostgreSQL or SQLite".to_string(),
            ));
        }
        let mut index = 0;
        let sql = sql
            .chars()
            .map(|character| {
                if character == '?' {
                    index += 1;
                    match backend {
                        sea_orm::DbBackend::Postgres => format!("${index}"),
                        _ => format!("?{index}"),
                    }
                } else {
                    character.to_string()
                }
            })
            .collect::<String>();
        Ok(sea_orm::Statement::from_sql_and_values(
            backend, sql, values,
        ))
    }

    // Compare exact vocabulary pairs at one snapshot, with no generation
    // write or domain lock. Different raw spellings fall back to typed reads.
    async fn cached_grants_match(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        permissions: &[Permission],
        revision: Option<&StaticActorAuthorityRevision>,
    ) -> Result<bool, sea_orm::DbErr> {
        let (collation, expected_sql) = match self.db.get_database_backend() {
            sea_orm::DbBackend::Postgres => (
                "\"C\"",
                "SELECT resource COLLATE \"C\" AS resource, action COLLATE \"C\" AS action \
                 FROM jsonb_to_recordset(CAST(CAST(? AS text) AS jsonb)) AS pair(resource text, action text)",
            ),
            sea_orm::DbBackend::Sqlite => (
                "BINARY",
                "SELECT CAST(json_extract(value, '$.resource') AS TEXT) COLLATE BINARY AS resource, \
                        CAST(json_extract(value, '$.action') AS TEXT) COLLATE BINARY AS action \
                 FROM json_each(?)",
            ),
            _ => return Err(sea_orm::DbErr::Custom(
                "RBAC relation reader requires PostgreSQL or SQLite".to_string(),
            )),
        };
        let expected = permissions.iter().map(|permission| serde_json::json!({
            "resource": permission.resource.to_string(),
            "action": permission.action.to_string(),
        })).collect::<Vec<_>>();
        let mut sql = format!(
            "WITH persisted AS ({PERSISTED_GRANTS_SQL}), \
             current_grants AS (SELECT CAST(resource AS TEXT) COLLATE {collation} AS resource, \
                                       CAST(action AS TEXT) COLLATE {collation} AS action FROM persisted), \
             expected_grants AS ({expected_sql}), \
             checks AS (SELECT NOT EXISTS (SELECT resource, action FROM current_grants EXCEPT SELECT resource, action FROM expected_grants) \
                AND NOT EXISTS (SELECT resource, action FROM expected_grants EXCEPT SELECT resource, action FROM current_grants) AS grants_match)"
        );
        let mut values = vec![
            (*tenant_id).into(),
            (*user_id).into(),
            serde_json::Value::Array(expected).to_string().into(),
        ];
        if let Some(revision) = revision {
            // One observation for both exact vocabulary and durable revision.
            // A missing actor yields no rows and cannot validate a bound hit.
            sql.push_str(&format!(
                ", revision AS ({STATIC_ACTOR_REVISION_SELECT} WHERE u.tenant_id = ? AND u.id = ?) \
                 SELECT checks.grants_match, revision.* FROM checks CROSS JOIN revision"
            ));
            values.extend([(*tenant_id).into(), (*user_id).into()]);
            let rows = self.db.query_all_raw(self.statement(&sql, values)?).await?;
            let Some(row) = rows.first() else {
                return Ok(false);
            };
            let matches: bool = row.try_get("", "grants_match")?;
            let current = decode_static_actor_authority_revision(&rows)?;
            return Ok(matches && current.as_ref() == Some(revision));
        }
        sql.push_str(" SELECT grants_match FROM checks");
        let row = self.db.query_one_raw(self.statement(&sql, values)?).await?.ok_or_else(|| {
            sea_orm::DbErr::Custom("RBAC cache validation returned no result".to_string())
        })?;
        row.try_get("", "grants_match")
    }
}

#[async_trait::async_trait]
impl<C: ConnectionTrait> RelationPermissionStore for ConnectionRelationPermissionStore<'_, C> {
    type Error = sea_orm::DbErr;

    async fn load_permissions_for_user(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
    ) -> Result<Vec<Permission>, Self::Error> {
        // One statement observes membership and grants at the same database snapshot.
        let rows = self.db.query_all_raw(self.statement(
            PERSISTED_GRANTS_SQL,
            vec![(*tenant_id).into(), (*user_id).into()],
        )?).await?;
        rows.iter()
            .map(|row| {
                let resource: String = row.try_get("", "resource")?;
                let action: String = row.try_get("", "action")?;
                Ok(Permission::new(
                    resource
                        .parse()
                        .map_err(|err: rustok_api::ResourceParseError| {
                            sea_orm::DbErr::Type(err.to_string())
                        })?,
                    action
                        .parse()
                        .map_err(|err: rustok_api::ActionParseError| {
                            sea_orm::DbErr::Type(err.to_string())
                        })?,
                ))
            })
            .collect()
    }
}

#[async_trait::async_trait]
impl RelationPermissionStore for SeaOrmRelationPermissionStore {
    type Error = sea_orm::DbErr;

    async fn load_permissions_for_user(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
    ) -> Result<Vec<Permission>, Self::Error> {
        ConnectionRelationPermissionStore { db: &self.db }
            .load_permissions_for_user(tenant_id, user_id)
            .await
    }

    async fn cached_permissions_are_current(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        permissions: &[Permission],
    ) -> Result<bool, Self::Error> {
        ConnectionRelationPermissionStore { db: &self.db }
            .cached_grants_match(tenant_id, user_id, permissions, None)
            .await
    }

    async fn read_static_revision(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
    ) -> Result<Option<StaticActorAuthorityRevision>, Self::Error> {
        read_optional_static_actor_authority_revision_on(&self.db, *tenant_id, *user_id).await
    }

    async fn cached_permissions_match_revision(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        permissions: &[Permission],
        revision: &StaticActorAuthorityRevision,
    ) -> Result<bool, Self::Error> {
        ConnectionRelationPermissionStore { db: &self.db }
            .cached_grants_match(tenant_id, user_id, permissions, Some(revision))
            .await
    }
}

/// Resolve persisted grants using the supplied connection or transaction.
/// Hosts retain their serialization locks; no cache or request scope is used.
pub async fn resolve_persisted_permissions_on<C: ConnectionTrait>(
    db: &C,
    tenant_id: &uuid::Uuid,
    user_id: &uuid::Uuid,
) -> Result<Vec<Permission>, sea_orm::DbErr> {
    resolve_permissions_from_relations(
        &ConnectionRelationPermissionStore { db },
        tenant_id,
        user_id,
    )
    .await
}

#[async_trait::async_trait]
impl<C: ConnectionTrait> crate::PermissionResolver for ConnectionRelationPermissionStore<'_, C> {
    type Error = sea_orm::DbErr;

    async fn resolve_permissions(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
    ) -> Result<crate::PermissionResolution, Self::Error> {
        Ok(crate::PermissionResolution {
            permissions: resolve_permissions_from_relations(self, tenant_id, user_id).await?,
            cache_hit: false,
        })
    }
}

/// Read exact persisted role membership, excluding subjects from other tenants.
pub async fn load_role_user_ids_on<C: ConnectionTrait>(
    db: &C,
    tenant_id: uuid::Uuid,
    role: rustok_core::UserRole,
) -> Result<Vec<uuid::Uuid>, sea_orm::DbErr> {
    if tenant_id.is_nil() {
        return Ok(Vec::new());
    }
    tracing::debug!(
        tenant_id = %tenant_id,
        role = %role,
        "Loading role user IDs"
    );
    let store = ConnectionRelationPermissionStore { db };
    db.query_all_raw(store.statement(
        "SELECT DISTINCT subject.id FROM users subject JOIN user_roles membership ON membership.user_id = subject.id JOIN roles role ON role.id = membership.role_id AND role.tenant_id = subject.tenant_id WHERE role.tenant_id = ? AND role.slug = ? ORDER BY subject.id",
        vec![tenant_id.into(), role.to_string().into()],
    )?)
    .await?
    .iter()
    .map(|row| row.try_get("", "id"))
    .collect()
}

#[async_trait::async_trait]
impl crate::PermissionResolver for SeaOrmRelationPermissionStore {
    type Error = sea_orm::DbErr;

    async fn resolve_permissions(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
    ) -> Result<crate::PermissionResolution, Self::Error> {
        crate::PermissionResolver::resolve_permissions(
            &ConnectionRelationPermissionStore { db: &self.db },
            tenant_id,
            user_id,
        )
        .await
    }
}

/// Evaluate current persisted grants through the canonical tenant policy engine,
/// without request snapshots or a cache. This is not a revocation fence.
pub async fn authorize_current_permission<C: ConnectionTrait>(
    db: &C,
    tenant_id: &uuid::Uuid,
    user_id: &uuid::Uuid,
    required_permission: &Permission,
) -> Result<crate::AuthorizationDecision, sea_orm::DbErr> {
    crate::authorize_permission(
        &ConnectionRelationPermissionStore { db },
        tenant_id,
        user_id,
        required_permission,
    )
    .await
}

#[async_trait::async_trait]
pub trait RelationPermissionStore {
    type Error;

    /// Read the exact tenant/user relation graph from one coherent snapshot.
    /// Implementations must not compose separately observed memberships and grants.
    async fn load_permissions_for_user(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
    ) -> Result<Vec<Permission>, Self::Error>;

    /// A cache hit is a candidate, not persisted authority. The default
    /// compares a fresh coherent typed read. Optimized stores must prove
    /// exact snapshot equivalence and propagate observation failures.
    async fn cached_permissions_are_current(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        permissions: &[Permission],
    ) -> Result<bool, Self::Error>
    where
        Self: Sync,
    {
        Ok(normalize_permissions(self.load_permissions_for_user(tenant_id, user_id).await?) == permissions)
    }

    /// Optional durable static relation observation. Legacy stores retain
    /// exact validation and do not produce revision-bound cache entries.
    /// For a revision-aware cache/store pair, None means the subject is
    /// absent; it is not an authority revision for a nonempty grant set.
    async fn read_static_revision(
        &self,
        _tenant_id: &uuid::Uuid,
        _user_id: &uuid::Uuid,
    ) -> Result<Option<StaticActorAuthorityRevision>, Self::Error> {
        Ok(None)
    }

    /// A bound hit needs both exact grant equivalence and the original
    /// revision. Stores must observe both coherently; the default cannot
    /// prove that contract and therefore never accepts a bound hit.
    async fn cached_permissions_match_revision(
        &self,
        _tenant_id: &uuid::Uuid,
        _user_id: &uuid::Uuid,
        _permissions: &[Permission],
        _revision: &StaticActorAuthorityRevision,
    ) -> Result<bool, Self::Error> {
        Ok(false)
    }
}

/// Cache lookup result carrying an opaque token for conditional publication.
///
/// Cache adapters that do not need race protection can use token `0` through the default trait
/// methods. Generation-aware adapters override `lookup` and `insert_if_current` to reject a value
/// when invalidation is observed before conditional publication. This is not a fence against
/// invalidation after validation, at commit, or in another replica.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionCacheLookup {
    permissions: Option<Vec<Permission>>,
    token: u64,
    revision: Option<Arc<StaticActorAuthorityRevision>>,
}

impl PermissionCacheLookup {
    pub fn new(permissions: Option<Vec<Permission>>, token: u64) -> Self {
        Self {
            permissions,
            token,
            revision: None,
        }
    }

    pub fn with_revision(
        permissions: Vec<Permission>,
        token: u64,
        revision: Option<Arc<StaticActorAuthorityRevision>>,
    ) -> Self {
        Self {
            permissions: Some(permissions),
            token,
            revision,
        }
    }

    pub fn into_parts(self) -> (Option<Vec<Permission>>, u64) {
        (self.permissions, self.token)
    }
}

#[async_trait::async_trait]
pub trait PermissionCache {
    /// Opt in only when the adapter preserves the revision and permissions
    /// together in one immutable cache entry and the store supports revisions.
    fn supports_static_revisions(&self) -> bool {
        false
    }

    async fn get(&self, tenant_id: &uuid::Uuid, user_id: &uuid::Uuid) -> Option<Vec<Permission>>;

    async fn insert(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        permissions: Vec<Permission>,
    );

    async fn invalidate(&self, tenant_id: &uuid::Uuid, user_id: &uuid::Uuid);

    async fn lookup(&self, tenant_id: &uuid::Uuid, user_id: &uuid::Uuid) -> PermissionCacheLookup {
        PermissionCacheLookup::new(self.get(tenant_id, user_id).await, 0)
    }

    /// Recheck the local lookup token after asynchronous authority validation.
    async fn is_lookup_current(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        token: u64,
    ) -> bool {
        self.lookup(tenant_id, user_id).await.into_parts().1 == token
    }

    /// Publish a cache fill only if the lookup token still represents the current generation.
    ///
    /// Returns `true` when the adapter accepts the fill under its validation policy. This does not
    /// prove current authority after that validation point. The default implementation preserves
    /// compatibility for adapters without generation tracking.
    async fn insert_if_current(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        _token: u64,
        permissions: Vec<Permission>,
    ) -> bool {
        self.insert(tenant_id, user_id, permissions).await;
        true
    }

    async fn insert_bound_if_current(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        token: u64,
        permissions: Vec<Permission>,
        _revision: Option<Arc<StaticActorAuthorityRevision>>,
    ) -> bool {
        self.insert_if_current(tenant_id, user_id, token, permissions).await
    }
}

pub async fn resolve_permissions_from_relations<S: RelationPermissionStore>(
    store: &S,
    tenant_id: &uuid::Uuid,
    user_id: &uuid::Uuid,
) -> Result<Vec<Permission>, S::Error> {
    if tenant_id.is_nil() || user_id.is_nil() {
        return Ok(vec![]);
    }
    tracing::debug!(
        tenant_id = %tenant_id,
        user_id = %user_id,
        "Resolving permissions from relations"
    );
    let permissions = store.load_permissions_for_user(tenant_id, user_id).await?;

    Ok(normalize_permissions(permissions))
}

/// Failure to resolve a permission set suitable for a cached authorization decision.
#[derive(Debug, thiserror::Error)]
pub enum PermissionCacheResolutionError<E> {
    #[error("permission relation read failed: {0}")]
    Store(E),
    #[error("permission resolution invalidated during every attempt")]
    Invalidated,
}

pub async fn resolve_permissions_with_cache<S, C>(
    store: &S,
    cache: &C,
    tenant_id: &uuid::Uuid,
    user_id: &uuid::Uuid,
) -> Result<crate::PermissionResolution, PermissionCacheResolutionError<S::Error>>
where
    S: RelationPermissionStore + Sync,
    C: PermissionCache + Sync,
{
    if tenant_id.is_nil() || user_id.is_nil() {
        return Ok(crate::PermissionResolution {
            permissions: Vec::new(),
            cache_hit: false,
        });
    }
    tracing::debug!(
        tenant_id = %tenant_id,
        user_id = %user_id,
        "Resolving permissions with cache"
    );
    for _ in 0..MAX_PERMISSION_CACHE_RESOLUTION_ATTEMPTS {
        let PermissionCacheLookup { permissions: cached_permissions, token, revision } =
            cache.lookup(tenant_id, user_id).await;
        if let Some(cached_permissions) = cached_permissions {
            let permissions = normalize_permissions(cached_permissions);
            let matches = match revision.as_deref() {
                Some(revision) => store
                    .cached_permissions_match_revision(tenant_id, user_id, &permissions, revision)
                    .await,
                None => store.cached_permissions_are_current(tenant_id, user_id, &permissions).await,
            }.map_err(PermissionCacheResolutionError::Store)?;
            if matches {
                if cache.is_lookup_current(tenant_id, user_id, token).await {
                    return Ok(crate::PermissionResolution {
                        permissions,
                        cache_hit: true,
                    });
                }
                // The local token was superseded while the read was in flight.
                continue;
            }
            // Direct SQL or a missed notification changed persisted authority.
            // Refresh through the canonical reader and conditional publication.
        }

        let revision_aware = cache.supports_static_revisions();
        let revision = if revision_aware {
            store.read_static_revision(tenant_id, user_id).await
                .map_err(PermissionCacheResolutionError::Store)?
                .map(Arc::new)
        } else {
            None
        };
        let resolved_permissions = resolve_permissions_from_relations(store, tenant_id, user_id)
            .await
            .map_err(PermissionCacheResolutionError::Store)?;
        // Absence is not a revision: a subject could be created and deleted
        // (or move into and out of this tenant) between the two observations.
        // Never publish or return grants without a subject revision in the
        // revision-aware path, even if both observations would be None.
        if revision_aware && revision.is_none() && !resolved_permissions.is_empty() {
            continue;
        }
        if cache
            .insert_bound_if_current(tenant_id, user_id, token, resolved_permissions.clone(), revision.clone())
            .await
        {
            if revision_aware {
                // The original revision must bracket the grant read AND
                // asynchronous publication; raw SQL may not bump local epochs.
                let current = store.read_static_revision(tenant_id, user_id).await
                    .map_err(PermissionCacheResolutionError::Store)?;
                if current.as_ref() != revision.as_deref()
                    || !cache.is_lookup_current(tenant_id, user_id, token).await
                {
                    continue;
                }
            }
            return Ok(crate::PermissionResolution {
                permissions: resolved_permissions,
                cache_hit: false,
            });
        }
    }

    // Every read was superseded before conditional publication. It is neither a
    // verified grant set nor evidence of an empty set; make no decision from it.
    Err(PermissionCacheResolutionError::Invalidated)
}

pub async fn invalidate_cached_permissions<C: PermissionCache + Sync>(
    cache: &C,
    tenant_id: &uuid::Uuid,
    user_id: &uuid::Uuid,
) {
    if tenant_id.is_nil() || user_id.is_nil() {
        return;
    }
    tracing::debug!(
        tenant_id = %tenant_id,
        user_id = %user_id,
        "Invalidating cached permissions"
    );
    cache.invalidate(tenant_id, user_id).await;
}

#[cfg(test)]
mod tests {
    use super::{
        PermissionCache, PermissionCacheLookup, PermissionCacheResolutionError,
        RelationPermissionStore, resolve_permissions_from_relations,
        resolve_permissions_with_cache,
    };
    use async_trait::async_trait;
    use rustok_api::Permission;
    use std::collections::HashMap;
    use std::sync::Arc;
    use std::sync::atomic::{AtomicUsize, Ordering};
    use tokio::sync::Mutex;

    type PermissionCacheKey = (uuid::Uuid, uuid::Uuid);
    type PermissionCacheMap = HashMap<PermissionCacheKey, Vec<Permission>>;

    #[tokio::test]
    async fn persisted_policy_reader_is_tenant_scoped_and_observes_revocation() {
        use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("sqlite database");
        for sql in [
            "CREATE TABLE users (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL)",
            "CREATE TABLE roles (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, slug TEXT NOT NULL)",
            "CREATE TABLE user_roles (user_id TEXT NOT NULL, role_id TEXT NOT NULL)",
            "CREATE TABLE permissions (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL, resource TEXT NOT NULL, action TEXT NOT NULL)",
            "CREATE TABLE role_permissions (role_id TEXT NOT NULL, permission_id TEXT NOT NULL)",
        ] {
            db.execute_raw(Statement::from_string(DbBackend::Sqlite, sql))
                .await
                .expect("relation schema");
        }
        let tenant_id = uuid::Uuid::new_v4();
        let user_id = uuid::Uuid::new_v4();
        let role_id = uuid::Uuid::new_v4();
        let permission_id = uuid::Uuid::new_v4();
        for (sql, values) in [
            (
                "INSERT INTO users VALUES (?1, ?2)",
                vec![user_id.into(), tenant_id.into()],
            ),
            (
                "INSERT INTO roles VALUES (?1, ?2, 'admin')",
                vec![role_id.into(), tenant_id.into()],
            ),
            (
                "INSERT INTO user_roles VALUES (?1, ?2)",
                vec![user_id.into(), role_id.into()],
            ),
            (
                "INSERT INTO permissions VALUES (?1, ?2, 'modules', 'manage')",
                vec![permission_id.into(), tenant_id.into()],
            ),
            (
                "INSERT INTO role_permissions VALUES (?1, ?2)",
                vec![role_id.into(), permission_id.into()],
            ),
        ] {
            db.execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                sql,
                values,
            ))
            .await
            .expect("persist grants");
        }
        let check = || {
            super::authorize_current_permission(
                &db,
                &tenant_id,
                &user_id,
                &Permission::MODULES_MANAGE,
            )
        };
        let decision = check().await.expect("current decision");
        assert!(decision.allowed);
        assert!(!decision.cache_hit);
        assert_eq!(decision.engine, crate::AuthzEngine::Policy);

        let foreign_user_id = uuid::Uuid::new_v4();
        let foreign_tenant_id = uuid::Uuid::new_v4();
        for (sql, values) in [
            (
                "INSERT INTO users VALUES (?1, ?2)",
                vec![foreign_user_id.into(), foreign_tenant_id.into()],
            ),
            (
                "INSERT INTO user_roles VALUES (?1, ?2)",
                vec![foreign_user_id.into(), role_id.into()],
            ),
        ] {
            db.execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                sql,
                values,
            ))
            .await
            .expect("foreign membership fixture");
        }
        assert_eq!(
            super::load_role_user_ids_on(&db, tenant_id, rustok_core::UserRole::Admin)
                .await
                .expect("owner role membership"),
            vec![user_id]
        );
        assert!(
            super::load_role_user_ids_on(&db, foreign_tenant_id, rustok_core::UserRole::Admin)
                .await
                .expect("isolated role membership")
                .is_empty()
        );
        // Malformed foreign grant: the reader must enforce tenant isolation
        // even in this deliberately unconstrained fixture.
        let foreign_permission_id = uuid::Uuid::new_v4();
        for (sql, values) in [
            (
                "INSERT INTO permissions VALUES (?1, ?2, 'users', 'manage')",
                vec![foreign_permission_id.into(), foreign_tenant_id.into()],
            ),
            (
                "INSERT INTO role_permissions VALUES (?1, ?2)",
                vec![role_id.into(), foreign_permission_id.into()],
            ),
        ] {
            db.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite, sql, values))
                .await
                .expect("foreign grant fixture");
        }
        let store = super::SeaOrmRelationPermissionStore::new(db.clone());
        let cache = StubCache::default();
        let local = resolve_permissions_with_cache(&store, &cache, &tenant_id, &user_id)
            .await
            .expect("cached relation resolution");
        assert_eq!(local.permissions, vec![Permission::MODULES_MANAGE]);
        assert!(!local.cache_hit);
        let hit = resolve_permissions_with_cache(&store, &cache, &tenant_id, &user_id)
            .await.expect("validate warm cache");
        assert!(hit.cache_hit);
        assert_eq!(hit.permissions, vec![Permission::MODULES_MANAGE]);

        // Revoke the static grant without touching membership or notifying
        // the cache, then restore it while the empty result is cached.
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "DELETE FROM role_permissions WHERE permission_id = ?1",
            vec![permission_id.into()],
        )).await.unwrap();
        let grant_revoked = resolve_permissions_with_cache(&store, &cache, &tenant_id, &user_id)
            .await.expect("refresh after direct grant removal");
        assert!(!grant_revoked.cache_hit);
        assert!(grant_revoked.permissions.is_empty());
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO role_permissions VALUES (?1, ?2)",
            vec![role_id.into(), permission_id.into()],
        )).await.unwrap();
        let grant_restored = resolve_permissions_with_cache(&store, &cache, &tenant_id, &user_id)
            .await.expect("refresh after direct grant insertion");
        assert!(!grant_restored.cache_hit);
        assert_eq!(grant_restored.permissions, vec![Permission::MODULES_MANAGE]);

        for (tenant, user) in [
            (tenant_id, foreign_user_id),
            (foreign_tenant_id, foreign_user_id),
            (foreign_tenant_id, user_id),
        ] {
            let resolved = resolve_permissions_with_cache(&store, &cache, &tenant, &user)
                .await
                .expect("isolated relation resolution");
            assert!(resolved.permissions.is_empty());
        }
        {
            use sea_orm::TransactionTrait;

            let transaction = db.begin().await.expect("caller transaction");
            transaction
                .execute_raw(Statement::from_string(
                    DbBackend::Sqlite,
                    "DELETE FROM user_roles",
                ))
                .await
                .expect("transaction-local revocation");
            assert!(
                super::load_role_user_ids_on(&transaction, tenant_id, rustok_core::UserRole::Admin)
                    .await
                    .expect("membership must use the supplied transaction")
                    .is_empty()
            );
            assert!(
                super::resolve_persisted_permissions_on(&transaction, &tenant_id, &user_id)
                    .await
                    .expect("owner must use the supplied transaction")
                    .is_empty()
            );
            let revoked = super::authorize_current_permission(
                &transaction,
                &tenant_id,
                &user_id,
                &Permission::MODULES_MANAGE,
            )
            .await
            .expect("current policy must use the supplied transaction");
            assert!(!revoked.allowed);
            assert!(!revoked.cache_hit);
            assert_eq!(revoked.engine, crate::AuthzEngine::Policy);
            transaction.rollback().await.expect("rollback revocation");
            assert!(check().await.expect("rolled-back grants remain").allowed);
        }
        assert!(
            !super::authorize_current_permission(
                &db,
                &uuid::Uuid::new_v4(),
                &user_id,
                &Permission::MODULES_MANAGE
            )
            .await
            .expect("foreign tenant decision")
            .allowed
        );
        db.execute_raw(Statement::from_string(
            DbBackend::Sqlite,
            "DELETE FROM user_roles",
        ))
        .await
        .expect("revoke membership");
        assert!(!check().await.expect("revoked decision").allowed);
        let revoked = resolve_permissions_with_cache(&store, &cache, &tenant_id, &user_id)
            .await.expect("refresh after direct SQL without invalidation");
        assert!(!revoked.cache_hit);
        assert!(revoked.permissions.is_empty());
        db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite, "INSERT INTO user_roles VALUES (?1, ?2)",
            vec![user_id.into(), role_id.into()],
        )).await.unwrap();
        let restored = resolve_permissions_with_cache(&store, &cache, &tenant_id, &user_id)
            .await.expect("empty cache is not authority either");
        assert!(!restored.cache_hit);
        assert_eq!(restored.permissions, vec![Permission::MODULES_MANAGE]);
        db.execute_raw(Statement::from_string(
            DbBackend::Sqlite,
            "DROP TABLE user_roles",
        ))
        .await
        .expect("remove relation availability");
        assert!(check().await.is_err());
        assert!(matches!(
            resolve_permissions_with_cache(&store, &cache, &tenant_id, &user_id).await,
            Err(PermissionCacheResolutionError::Store(_))
        ), "a warm cache must not hide unavailable persisted authority");
    }

    #[tokio::test]
    #[ignore = "requires RUSTOK_RBAC_TEST_DATABASE_URL and PostgreSQL schema creation privileges"]
    async fn postgres_cached_grant_equivalence_observes_raw_writes_and_cascades() {
        use sea_orm::{ConnectionTrait, Database, DbBackend, TransactionTrait};
        let db = Database::connect(std::env::var("RUSTOK_RBAC_TEST_DATABASE_URL").unwrap())
            .await.unwrap();
        assert_eq!(db.get_database_backend(), DbBackend::Postgres);
        let tx = db.begin().await.unwrap();
        let schema = format!("rbac_cache_equivalence_{}", uuid::Uuid::new_v4().simple());
        tx.execute_unprepared(&format!("CREATE SCHEMA {schema}; SET LOCAL search_path TO {schema}"))
            .await.unwrap();
        tx.execute_unprepared(
            "CREATE TABLE users (id UUID PRIMARY KEY, tenant_id UUID NOT NULL);
             CREATE TABLE roles (id UUID PRIMARY KEY, tenant_id UUID NOT NULL);
             CREATE TABLE permissions (id UUID PRIMARY KEY, tenant_id UUID NOT NULL, resource TEXT NOT NULL, action TEXT NOT NULL);
             CREATE TABLE user_roles (user_id UUID REFERENCES users(id) ON DELETE CASCADE, role_id UUID REFERENCES roles(id) ON DELETE CASCADE);
             CREATE TABLE role_permissions (role_id UUID REFERENCES roles(id) ON DELETE CASCADE, permission_id UUID REFERENCES permissions(id) ON DELETE CASCADE)"
        ).await.unwrap();
        let tenant = uuid::Uuid::new_v4();
        let user = uuid::Uuid::new_v4();
        let role = uuid::Uuid::new_v4();
        let permission = uuid::Uuid::new_v4();
        tx.execute_unprepared(&format!(
            "INSERT INTO users VALUES ('{user}', '{tenant}');
             INSERT INTO roles VALUES ('{role}', '{tenant}');
             INSERT INTO permissions VALUES ('{permission}', '{tenant}', 'modules', 'manage');
             INSERT INTO user_roles VALUES ('{user}', '{role}');
             INSERT INTO role_permissions VALUES ('{role}', '{permission}')"
        )).await.unwrap();
        let store = super::ConnectionRelationPermissionStore { db: &tx };
        let expected = [Permission::MODULES_MANAGE];
        assert!(store.cached_grants_match(&tenant, &user, &expected, None).await.unwrap());
        assert!(!store.cached_grants_match(&tenant, &user, &[], None).await.unwrap());
        assert!(!store.cached_grants_match(&tenant, &user, &[Permission::USERS_READ], None).await.unwrap());
        assert!(!store.cached_grants_match(&uuid::Uuid::new_v4(), &user, &expected, None).await.unwrap());

        tx.execute_unprepared(&format!("DELETE FROM permissions WHERE id = '{permission}'"))
            .await.unwrap();
        assert!(!store.cached_grants_match(&tenant, &user, &expected, None).await.unwrap());
        assert!(store.cached_grants_match(&tenant, &user, &[], None).await.unwrap());
        tx.execute_unprepared(&format!(
            "INSERT INTO permissions VALUES ('{permission}', '{tenant}', 'modules', 'manage');
             INSERT INTO role_permissions VALUES ('{role}', '{permission}')"
        )).await.unwrap();
        assert!(store.cached_grants_match(&tenant, &user, &expected, None).await.unwrap());
        tx.execute_unprepared(&format!("DELETE FROM roles WHERE id = '{role}'")).await.unwrap();
        assert!(!store.cached_grants_match(&tenant, &user, &expected, None).await.unwrap());
        assert!(store.cached_grants_match(&tenant, &user, &[], None).await.unwrap());
        // Transactional DDL keeps the disposable test schema out of the database.
        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn cache_hit_superseded_during_authority_validation_is_not_returned() {
        struct SupersededHit;
        #[async_trait]
        impl PermissionCache for SupersededHit {
            async fn get(&self, _: &uuid::Uuid, _: &uuid::Uuid) -> Option<Vec<Permission>> {
                Some(vec![Permission::USERS_MANAGE])
            }
            async fn insert(&self, _: &uuid::Uuid, _: &uuid::Uuid, _: Vec<Permission>) {
                panic!("a superseded hit must not be published");
            }
            async fn invalidate(&self, _: &uuid::Uuid, _: &uuid::Uuid) {}
            async fn lookup(&self, _: &uuid::Uuid, _: &uuid::Uuid) -> PermissionCacheLookup {
                PermissionCacheLookup::new(Some(vec![Permission::USERS_MANAGE]), 7)
            }
            async fn is_lookup_current(&self, _: &uuid::Uuid, _: &uuid::Uuid, _: u64) -> bool {
                false
            }
        }
        let store = StubStore {
            role_ids: vec![uuid::Uuid::new_v4()],
            tenant_role_ids: vec![uuid::Uuid::new_v4()],
            permissions: vec![Permission::USERS_MANAGE],
        };
        assert!(matches!(
            resolve_permissions_with_cache(&store, &SupersededHit, &uuid::Uuid::new_v4(), &uuid::Uuid::new_v4()).await,
            Err(PermissionCacheResolutionError::Invalidated)
        ));
    }

    struct StubStore {
        role_ids: Vec<uuid::Uuid>,
        tenant_role_ids: Vec<uuid::Uuid>,
        permissions: Vec<Permission>,
    }

    #[derive(Default)]
    struct StubCache {
        values: Arc<Mutex<PermissionCacheMap>>,
    }

    #[async_trait]
    impl PermissionCache for StubCache {
        async fn get(
            &self,
            tenant_id: &uuid::Uuid,
            user_id: &uuid::Uuid,
        ) -> Option<Vec<Permission>> {
            self.values
                .lock()
                .await
                .get(&(*tenant_id, *user_id))
                .cloned()
        }

        async fn insert(
            &self,
            tenant_id: &uuid::Uuid,
            user_id: &uuid::Uuid,
            permissions: Vec<Permission>,
        ) {
            self.values
                .lock()
                .await
                .insert((*tenant_id, *user_id), permissions);
        }

        async fn invalidate(&self, tenant_id: &uuid::Uuid, user_id: &uuid::Uuid) {
            self.values.lock().await.remove(&(*tenant_id, *user_id));
        }
    }

    struct TokenAwareCache {
        publication_attempts: AtomicUsize,
        values: Arc<Mutex<PermissionCacheMap>>,
    }

    impl Default for TokenAwareCache {
        fn default() -> Self {
            Self {
                publication_attempts: AtomicUsize::new(0),
                values: Arc::new(Mutex::new(HashMap::new())),
            }
        }
    }

    #[async_trait]
    impl PermissionCache for TokenAwareCache {
        async fn get(
            &self,
            _tenant_id: &uuid::Uuid,
            _user_id: &uuid::Uuid,
        ) -> Option<Vec<Permission>> {
            panic!("generation-aware resolver must use lookup")
        }

        async fn insert(
            &self,
            _tenant_id: &uuid::Uuid,
            _user_id: &uuid::Uuid,
            _permissions: Vec<Permission>,
        ) {
            panic!("generation-aware resolver must use insert_if_current")
        }

        async fn invalidate(&self, tenant_id: &uuid::Uuid, user_id: &uuid::Uuid) {
            self.values.lock().await.remove(&(*tenant_id, *user_id));
        }

        async fn lookup(
            &self,
            _tenant_id: &uuid::Uuid,
            _user_id: &uuid::Uuid,
        ) -> PermissionCacheLookup {
            PermissionCacheLookup::new(None, 41)
        }

        async fn insert_if_current(
            &self,
            tenant_id: &uuid::Uuid,
            user_id: &uuid::Uuid,
            token: u64,
            permissions: Vec<Permission>,
        ) -> bool {
            assert_eq!(token, 41);
            if self.publication_attempts.fetch_add(1, Ordering::AcqRel) == 0 {
                return false;
            }
            self.values
                .lock()
                .await
                .insert((*tenant_id, *user_id), permissions);
            true
        }
    }

    #[derive(Default)]
    struct AlwaysSupersededCache {
        attempts: AtomicUsize,
    }

    #[async_trait]
    impl PermissionCache for AlwaysSupersededCache {
        async fn get(
            &self,
            _tenant_id: &uuid::Uuid,
            _user_id: &uuid::Uuid,
        ) -> Option<Vec<Permission>> {
            panic!("generation-aware resolver must use lookup")
        }

        async fn insert(
            &self,
            _tenant_id: &uuid::Uuid,
            _user_id: &uuid::Uuid,
            _permissions: Vec<Permission>,
        ) {
            panic!("generation-aware resolver must use insert_if_current")
        }

        async fn invalidate(&self, _tenant_id: &uuid::Uuid, _user_id: &uuid::Uuid) {}

        async fn lookup(
            &self,
            _tenant_id: &uuid::Uuid,
            _user_id: &uuid::Uuid,
        ) -> PermissionCacheLookup {
            PermissionCacheLookup::new(None, 7)
        }

        async fn insert_if_current(
            &self,
            _tenant_id: &uuid::Uuid,
            _user_id: &uuid::Uuid,
            _token: u64,
            _permissions: Vec<Permission>,
        ) -> bool {
            self.attempts.fetch_add(1, Ordering::SeqCst);
            false
        }
    }

    #[async_trait]
    impl RelationPermissionStore for StubStore {
        type Error = String;

        async fn load_permissions_for_user(
            &self,
            _tenant_id: &uuid::Uuid,
            _user_id: &uuid::Uuid,
        ) -> Result<Vec<Permission>, Self::Error> {
            if self.role_ids.is_empty() || self.tenant_role_ids.is_empty() {
                return Ok(Vec::new());
            }
            Ok(self.permissions.clone())
        }
    }

    #[tokio::test]
    async fn returns_empty_when_user_has_no_roles() {
        let store = StubStore {
            role_ids: vec![],
            tenant_role_ids: vec![uuid::Uuid::new_v4()],
            permissions: vec![Permission::USERS_READ],
        };

        let resolved = resolve_permissions_from_relations(
            &store,
            &uuid::Uuid::new_v4(),
            &uuid::Uuid::new_v4(),
        )
        .await
        .unwrap();

        assert!(resolved.is_empty());
    }

    #[tokio::test]
    async fn returns_stable_sorted_permissions() {
        let role_id = uuid::Uuid::new_v4();
        let store = StubStore {
            role_ids: vec![role_id],
            tenant_role_ids: vec![role_id],
            permissions: vec![
                Permission::USERS_UPDATE,
                Permission::USERS_READ,
                Permission::USERS_MANAGE,
            ],
        };

        let resolved = resolve_permissions_from_relations(
            &store,
            &uuid::Uuid::new_v4(),
            &uuid::Uuid::new_v4(),
        )
        .await
        .unwrap();

        assert_eq!(
            resolved,
            vec![
                Permission::USERS_MANAGE,
                Permission::USERS_READ,
                Permission::USERS_UPDATE,
            ]
        );
    }

    #[tokio::test]
    async fn deduplicates_permissions() {
        let role_id = uuid::Uuid::new_v4();
        let store = StubStore {
            role_ids: vec![role_id],
            tenant_role_ids: vec![role_id],
            permissions: vec![Permission::USERS_READ, Permission::USERS_READ],
        };

        let resolved = resolve_permissions_from_relations(
            &store,
            &uuid::Uuid::new_v4(),
            &uuid::Uuid::new_v4(),
        )
        .await
        .unwrap();

        assert_eq!(resolved, vec![Permission::USERS_READ]);
    }

    #[tokio::test]
    async fn resolve_permissions_with_cache_reports_hit_on_second_lookup() {
        let role_id = uuid::Uuid::new_v4();
        let tenant_id = uuid::Uuid::new_v4();
        let user_id = uuid::Uuid::new_v4();
        let store = StubStore {
            role_ids: vec![role_id],
            tenant_role_ids: vec![role_id],
            permissions: vec![Permission::USERS_READ],
        };
        let cache = StubCache::default();

        let first = resolve_permissions_with_cache(&store, &cache, &tenant_id, &user_id)
            .await
            .unwrap();
        let second = resolve_permissions_with_cache(&store, &cache, &tenant_id, &user_id)
            .await
            .unwrap();

        assert!(!first.cache_hit);
        assert!(second.cache_hit);
        assert_eq!(second.permissions, vec![Permission::USERS_READ]);
    }

    #[tokio::test]
    async fn resolver_retries_generation_checked_cache_publication() {
        let role_id = uuid::Uuid::new_v4();
        let tenant_id = uuid::Uuid::new_v4();
        let user_id = uuid::Uuid::new_v4();
        let store = StubStore {
            role_ids: vec![role_id],
            tenant_role_ids: vec![role_id],
            permissions: vec![Permission::USERS_READ],
        };
        let cache = TokenAwareCache::default();

        let result = resolve_permissions_with_cache(&store, &cache, &tenant_id, &user_id)
            .await
            .unwrap();

        assert_eq!(result.permissions, vec![Permission::USERS_READ]);
        assert_eq!(cache.publication_attempts.load(Ordering::Acquire), 2);
        assert_eq!(
            cache
                .values
                .lock()
                .await
                .get(&(tenant_id, user_id))
                .cloned(),
            Some(vec![Permission::USERS_READ])
        );
    }

    #[tokio::test]
    async fn continuous_invalidation_fails_closed_with_typed_error() {
        let role_id = uuid::Uuid::new_v4();
        let store = StubStore {
            role_ids: vec![role_id],
            tenant_role_ids: vec![role_id],
            permissions: vec![Permission::USERS_MANAGE],
        };
        let cache = AlwaysSupersededCache::default();
        let result = resolve_permissions_with_cache(
            &store,
            &cache,
            &uuid::Uuid::new_v4(),
            &uuid::Uuid::new_v4(),
        )
        .await;
        assert!(matches!(
            result,
            Err(PermissionCacheResolutionError::Invalidated)
        ));
        assert_eq!(
            cache.attempts.load(Ordering::SeqCst),
            super::MAX_PERMISSION_CACHE_RESOLUTION_ATTEMPTS
        );
    }

    #[tokio::test]
    async fn revision_aware_fill_never_publishes_grants_for_absent_subject() {
        struct AbsentRevisionStore {
            reads: AtomicUsize,
            transient_reads: usize,
        }

        #[async_trait]
        impl RelationPermissionStore for AbsentRevisionStore {
            type Error = String;

            async fn load_permissions_for_user(
                &self,
                _: &uuid::Uuid,
                _: &uuid::Uuid,
            ) -> Result<Vec<Permission>, Self::Error> {
                let read = self.reads.fetch_add(1, Ordering::SeqCst);
                Ok(if read < self.transient_reads {
                    vec![Permission::USERS_MANAGE]
                } else {
                    Vec::new()
                })
            }

            async fn read_static_revision(
                &self,
                _: &uuid::Uuid,
                _: &uuid::Uuid,
            ) -> Result<Option<super::StaticActorAuthorityRevision>, Self::Error> {
                Ok(None)
            }
        }

        #[derive(Default)]
        struct RevisionAwareCache {
            publications: AtomicUsize,
        }

        #[async_trait]
        impl PermissionCache for RevisionAwareCache {
            fn supports_static_revisions(&self) -> bool {
                true
            }

            async fn get(&self, _: &uuid::Uuid, _: &uuid::Uuid) -> Option<Vec<Permission>> {
                None
            }

            async fn insert(
                &self,
                _: &uuid::Uuid,
                _: &uuid::Uuid,
                permissions: Vec<Permission>,
            ) {
                assert!(permissions.is_empty(), "absent subject cannot have grants");
                self.publications.fetch_add(1, Ordering::SeqCst);
            }

            async fn invalidate(&self, _: &uuid::Uuid, _: &uuid::Uuid) {}
        }

        let tenant = uuid::Uuid::new_v4();
        let actor = uuid::Uuid::new_v4();

        // Model a transient subject visible to the first coherent grant
        // read but absent at the revision observations.
        let store = AbsentRevisionStore {
            reads: AtomicUsize::new(0),
            transient_reads: 1,
        };
        let cache = RevisionAwareCache::default();
        let result = resolve_permissions_with_cache(&store, &cache, &tenant, &actor)
            .await
            .unwrap();
        assert!(result.permissions.is_empty());
        assert!(!result.cache_hit);
        assert_eq!(store.reads.load(Ordering::SeqCst), 2);
        assert_eq!(cache.publications.load(Ordering::SeqCst), 1);

        // Repeated contradictory evidence must exhaust the retry budget,
        // not publish an empty success or the last unbound grant set.
        let store = AbsentRevisionStore {
            reads: AtomicUsize::new(0),
            transient_reads: usize::MAX,
        };
        let cache = RevisionAwareCache::default();
        assert!(matches!(
            resolve_permissions_with_cache(&store, &cache, &tenant, &actor).await,
            Err(PermissionCacheResolutionError::Invalidated)
        ));
        assert_eq!(
            store.reads.load(Ordering::SeqCst),
            super::MAX_PERMISSION_CACHE_RESOLUTION_ATTEMPTS
        );
        assert_eq!(cache.publications.load(Ordering::SeqCst), 0);
    }

    #[tokio::test]
    async fn revoked_final_read_is_not_returned_when_publication_is_rejected() {
        use tokio::sync::Notify;

        struct MutableStore {
            grants: Arc<Mutex<Vec<Permission>>>,
        }
        #[async_trait]
        impl RelationPermissionStore for MutableStore {
            type Error = String;
            async fn load_permissions_for_user(
                &self,
                _: &uuid::Uuid,
                _: &uuid::Uuid,
            ) -> Result<Vec<Permission>, String> {
                Ok(self.grants.lock().await.clone())
            }
        }
        struct RevocationCache {
            attempts: AtomicUsize,
            final_read: Notify,
            resume: Notify,
        }
        #[async_trait]
        impl PermissionCache for RevocationCache {
            async fn get(&self, _: &uuid::Uuid, _: &uuid::Uuid) -> Option<Vec<Permission>> {
                None
            }
            async fn insert(&self, _: &uuid::Uuid, _: &uuid::Uuid, _: Vec<Permission>) {
                panic!("conditional publication required");
            }
            async fn invalidate(&self, _: &uuid::Uuid, _: &uuid::Uuid) {}
            async fn lookup(&self, _: &uuid::Uuid, _: &uuid::Uuid) -> PermissionCacheLookup {
                PermissionCacheLookup::new(None, 1)
            }
            async fn insert_if_current(
                &self,
                _: &uuid::Uuid,
                _: &uuid::Uuid,
                _: u64,
                permissions: Vec<Permission>,
            ) -> bool {
                let attempt = self.attempts.fetch_add(1, Ordering::SeqCst) + 1;
                if attempt == super::MAX_PERMISSION_CACHE_RESOLUTION_ATTEMPTS {
                    assert_eq!(permissions, vec![Permission::USERS_MANAGE]);
                    self.final_read.notify_one();
                    self.resume.notified().await;
                }
                false
            }
        }

        let grants = Arc::new(Mutex::new(vec![Permission::USERS_MANAGE]));
        let store = MutableStore {
            grants: grants.clone(),
        };
        let cache = Arc::new(RevocationCache {
            attempts: AtomicUsize::new(0),
            final_read: Notify::new(),
            resume: Notify::new(),
        });
        let resolving_cache = cache.clone();
        let task = tokio::spawn(async move {
            resolve_permissions_with_cache(
                &store,
                resolving_cache.as_ref(),
                &uuid::Uuid::new_v4(),
                &uuid::Uuid::new_v4(),
            )
            .await
        });
        tokio::time::timeout(
            std::time::Duration::from_secs(2),
            cache.final_read.notified(),
        )
        .await
        .expect("final relation read must reach publication");
        // Model a committed revocation and its rejected cache-fill generation.
        grants.lock().await.clear();
        cache.resume.notify_one();
        let result = tokio::time::timeout(std::time::Duration::from_secs(2), task)
            .await
            .unwrap()
            .unwrap();
        assert!(matches!(
            result,
            Err(PermissionCacheResolutionError::Invalidated)
        ));
        assert!(grants.lock().await.is_empty());
    }

    #[tokio::test]
    async fn invalidate_cached_permissions_evicts_entry() {
        let tenant_id = uuid::Uuid::new_v4();
        let user_id = uuid::Uuid::new_v4();
        let cache = StubCache::default();

        cache
            .insert(&tenant_id, &user_id, vec![Permission::USERS_READ])
            .await;

        super::invalidate_cached_permissions(&cache, &tenant_id, &user_id).await;

        let cached = cache.get(&tenant_id, &user_id).await;
        assert!(cached.is_none());
    }

    #[tokio::test]
    async fn resolve_permissions_with_cache_normalizes_cached_permissions() {
        let tenant_id = uuid::Uuid::new_v4();
        let user_id = uuid::Uuid::new_v4();
        let store = StubStore {
            role_ids: vec![uuid::Uuid::new_v4()],
            tenant_role_ids: vec![uuid::Uuid::new_v4()],
            permissions: vec![Permission::USERS_MANAGE, Permission::USERS_READ],
        };
        let cache = StubCache::default();

        cache
            .insert(
                &tenant_id,
                &user_id,
                vec![
                    Permission::USERS_READ,
                    Permission::USERS_MANAGE,
                    Permission::USERS_READ,
                ],
            )
            .await;

        let resolved = resolve_permissions_with_cache(&store, &cache, &tenant_id, &user_id)
            .await
            .unwrap();

        assert!(resolved.cache_hit);
        assert_eq!(
            resolved.permissions,
            vec![Permission::USERS_MANAGE, Permission::USERS_READ]
        );
    }

    #[tokio::test]
    async fn nil_uuids_fast_path_to_empty_permissions() {
        let store = StubStore {
            role_ids: vec![uuid::Uuid::new_v4()],
            tenant_role_ids: vec![uuid::Uuid::new_v4()],
            permissions: vec![Permission::USERS_READ],
        };
        let cache = StubCache::default();

        let nil_tenant = uuid::Uuid::nil();
        let valid_user = uuid::Uuid::new_v4();
        let perms = resolve_permissions_from_relations(&store, &nil_tenant, &valid_user)
            .await
            .unwrap();
        assert!(perms.is_empty());

        let res = resolve_permissions_with_cache(&store, &cache, &nil_tenant, &valid_user)
            .await
            .unwrap();
        assert!(res.permissions.is_empty());
        assert!(!res.cache_hit);

        let valid_tenant = uuid::Uuid::new_v4();
        let nil_user = uuid::Uuid::nil();
        let perms = resolve_permissions_from_relations(&store, &valid_tenant, &nil_user)
            .await
            .unwrap();
        assert!(perms.is_empty());

        let res = resolve_permissions_with_cache(&store, &cache, &valid_tenant, &nil_user)
            .await
            .unwrap();
        assert!(res.permissions.is_empty());
        assert!(!res.cache_hit);
    }
}
