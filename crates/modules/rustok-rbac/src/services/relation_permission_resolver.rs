use crate::services::permission_normalization::normalize_permissions;
use rustok_api::Permission;
use sea_orm::ConnectionTrait;

const MAX_PERMISSION_CACHE_RESOLUTION_ATTEMPTS: usize = 4;

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

    async fn role_ids(
        &self,
        statement: sea_orm::Statement,
    ) -> Result<Vec<uuid::Uuid>, sea_orm::DbErr> {
        self.db
            .query_all_raw(statement)
            .await?
            .iter()
            .map(|row| row.try_get("", "id"))
            .collect()
    }
}

#[async_trait::async_trait]
impl<C: ConnectionTrait> RelationPermissionStore for ConnectionRelationPermissionStore<'_, C> {
    type Error = sea_orm::DbErr;

    async fn load_user_role_ids(
        &self,
        user_id: &uuid::Uuid,
    ) -> Result<Vec<uuid::Uuid>, Self::Error> {
        self.role_ids(self.statement(
            "SELECT DISTINCT role.id FROM roles role JOIN user_roles membership ON membership.role_id = role.id JOIN users subject ON subject.id = membership.user_id AND subject.tenant_id = role.tenant_id WHERE subject.id = ?",
            vec![(*user_id).into()],
        )?).await
    }

    async fn load_tenant_role_ids(
        &self,
        tenant_id: &uuid::Uuid,
        role_ids: &[uuid::Uuid],
    ) -> Result<Vec<uuid::Uuid>, Self::Error> {
        if role_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut values = vec![(*tenant_id).into()];
        values.extend(role_ids.iter().map(|id| sea_orm::Value::from(*id)));
        self.role_ids(self.statement(
            &format!(
                "SELECT id FROM roles WHERE tenant_id = ? AND id IN ({})",
                vec!["?"; role_ids.len()].join(", ")
            ),
            values,
        )?)
        .await
    }

    async fn load_permissions_for_roles(
        &self,
        tenant_id: &uuid::Uuid,
        role_ids: &[uuid::Uuid],
    ) -> Result<Vec<Permission>, Self::Error> {
        if role_ids.is_empty() {
            return Ok(Vec::new());
        }
        let mut values = vec![(*tenant_id).into()];
        values.extend(role_ids.iter().map(|id| sea_orm::Value::from(*id)));
        let rows = self.db.query_all_raw(self.statement(
            &format!("SELECT DISTINCT permission.resource, permission.action FROM permissions permission JOIN role_permissions grant_row ON grant_row.permission_id = permission.id JOIN roles role ON role.id = grant_row.role_id AND role.tenant_id = permission.tenant_id WHERE permission.tenant_id = ? AND role.id IN ({})", vec!["?"; role_ids.len()].join(", ")),
            values,
        )?).await?;
        rows.iter()
            .map(|row| {
                let resource: String = row.try_get("", "resource")?;
                let action: String = row.try_get("", "action")?;
                Ok(Permission::new(
                    resource.parse().map_err(sea_orm::DbErr::Type)?,
                    action.parse().map_err(sea_orm::DbErr::Type)?,
                ))
            })
            .collect()
    }
}

#[async_trait::async_trait]
impl RelationPermissionStore for SeaOrmRelationPermissionStore {
    type Error = sea_orm::DbErr;

    async fn load_user_role_ids(
        &self,
        user_id: &uuid::Uuid,
    ) -> Result<Vec<uuid::Uuid>, Self::Error> {
        ConnectionRelationPermissionStore { db: &self.db }
            .load_user_role_ids(user_id)
            .await
    }

    async fn load_tenant_role_ids(
        &self,
        tenant_id: &uuid::Uuid,
        role_ids: &[uuid::Uuid],
    ) -> Result<Vec<uuid::Uuid>, Self::Error> {
        ConnectionRelationPermissionStore { db: &self.db }
            .load_tenant_role_ids(tenant_id, role_ids)
            .await
    }

    async fn load_permissions_for_roles(
        &self,
        tenant_id: &uuid::Uuid,
        role_ids: &[uuid::Uuid],
    ) -> Result<Vec<Permission>, Self::Error> {
        ConnectionRelationPermissionStore { db: &self.db }
            .load_permissions_for_roles(tenant_id, role_ids)
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
impl crate::PermissionResolver for SeaOrmRelationPermissionStore {
    type Error = sea_orm::DbErr;

    async fn resolve_permissions(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
    ) -> Result<crate::PermissionResolution, Self::Error> {
        Ok(crate::PermissionResolution {
            permissions: resolve_persisted_permissions_on(&self.db, tenant_id, user_id).await?,
            cache_hit: false,
        })
    }
}

/// Evaluate current persisted grants through the canonical tenant policy engine,
/// without request snapshots or a cache. This is not a revocation fence.
pub async fn authorize_current_permission(
    db: &sea_orm::DatabaseConnection,
    tenant_id: &uuid::Uuid,
    user_id: &uuid::Uuid,
    required_permission: &Permission,
) -> Result<crate::AuthorizationDecision, sea_orm::DbErr> {
    crate::authorize_permission(
        &SeaOrmRelationPermissionStore::new(db.clone()),
        tenant_id,
        user_id,
        required_permission,
    )
    .await
}

#[async_trait::async_trait]
pub trait RelationPermissionStore {
    type Error;

    async fn load_user_role_ids(
        &self,
        user_id: &uuid::Uuid,
    ) -> Result<Vec<uuid::Uuid>, Self::Error>;

    async fn load_tenant_role_ids(
        &self,
        tenant_id: &uuid::Uuid,
        role_ids: &[uuid::Uuid],
    ) -> Result<Vec<uuid::Uuid>, Self::Error>;

    async fn load_permissions_for_roles(
        &self,
        tenant_id: &uuid::Uuid,
        role_ids: &[uuid::Uuid],
    ) -> Result<Vec<Permission>, Self::Error>;
}

/// Cache lookup result carrying an opaque token for conditional publication.
///
/// Cache adapters that do not need race protection can use token `0` through the default trait
/// methods. Generation-aware adapters override `lookup` and `insert_if_current` so a value loaded
/// before an invalidation cannot be published or returned after that invalidation completes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionCacheLookup {
    permissions: Option<Vec<Permission>>,
    token: u64,
}

impl PermissionCacheLookup {
    pub fn new(permissions: Option<Vec<Permission>>, token: u64) -> Self {
        Self { permissions, token }
    }

    pub fn into_parts(self) -> (Option<Vec<Permission>>, u64) {
        (self.permissions, self.token)
    }
}

#[async_trait::async_trait]
pub trait PermissionCache {
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

    /// Publish a cache fill only if the lookup token still represents the current generation.
    ///
    /// Returns `true` when the resolved permissions are safe to return to the caller. The default
    /// implementation preserves compatibility for adapters without generation tracking.
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
}

pub async fn resolve_permissions_from_relations<S: RelationPermissionStore>(
    store: &S,
    tenant_id: &uuid::Uuid,
    user_id: &uuid::Uuid,
) -> Result<Vec<Permission>, S::Error> {
    let user_role_ids = store.load_user_role_ids(user_id).await?;
    if user_role_ids.is_empty() {
        return Ok(vec![]);
    }

    let tenant_role_ids = store
        .load_tenant_role_ids(tenant_id, &user_role_ids)
        .await?;
    if tenant_role_ids.is_empty() {
        return Ok(vec![]);
    }

    let permissions = store
        .load_permissions_for_roles(tenant_id, &tenant_role_ids)
        .await?;

    Ok(normalize_permissions(permissions))
}

pub async fn resolve_permissions_with_cache<S, C>(
    store: &S,
    cache: &C,
    tenant_id: &uuid::Uuid,
    user_id: &uuid::Uuid,
) -> Result<crate::PermissionResolution, S::Error>
where
    S: RelationPermissionStore,
    C: PermissionCache + Sync,
{
    for _ in 0..MAX_PERMISSION_CACHE_RESOLUTION_ATTEMPTS {
        let (cached_permissions, token) = cache.lookup(tenant_id, user_id).await.into_parts();
        if let Some(cached_permissions) = cached_permissions {
            return Ok(crate::PermissionResolution {
                permissions: normalize_permissions(cached_permissions),
                cache_hit: true,
            });
        }

        let resolved_permissions =
            resolve_permissions_from_relations(store, tenant_id, user_id).await?;
        if cache
            .insert_if_current(tenant_id, user_id, token, resolved_permissions.clone())
            .await
        {
            return Ok(crate::PermissionResolution {
                permissions: resolved_permissions,
                cache_hit: false,
            });
        }
    }

    // Continuous invalidation means no permission snapshot can be proven current. Deny by returning
    // an empty set instead of exposing a value loaded from a superseded generation.
    Ok(crate::PermissionResolution {
        permissions: Vec::new(),
        cache_hit: false,
    })
}

pub async fn invalidate_cached_permissions<C: PermissionCache + Sync>(
    cache: &C,
    tenant_id: &uuid::Uuid,
    user_id: &uuid::Uuid,
) {
    cache.invalidate(tenant_id, user_id).await;
}

#[cfg(test)]
mod tests {
    use super::{
        PermissionCache, PermissionCacheLookup, RelationPermissionStore,
        resolve_permissions_from_relations, resolve_permissions_with_cache,
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
            "CREATE TABLE roles (id TEXT PRIMARY KEY, tenant_id TEXT NOT NULL)",
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
                "INSERT INTO roles VALUES (?1, ?2)",
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
                super::resolve_persisted_permissions_on(&transaction, &tenant_id, &user_id)
                    .await
                    .expect("owner must use the supplied transaction")
                    .is_empty()
            );
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
        db.execute_raw(Statement::from_string(
            DbBackend::Sqlite,
            "DROP TABLE user_roles",
        ))
        .await
        .expect("remove relation availability");
        assert!(check().await.is_err());
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

    struct AlwaysSupersededCache;

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
            false
        }
    }

    #[async_trait]
    impl RelationPermissionStore for StubStore {
        type Error = String;

        async fn load_user_role_ids(
            &self,
            _user_id: &uuid::Uuid,
        ) -> Result<Vec<uuid::Uuid>, Self::Error> {
            Ok(self.role_ids.clone())
        }

        async fn load_tenant_role_ids(
            &self,
            _tenant_id: &uuid::Uuid,
            _role_ids: &[uuid::Uuid],
        ) -> Result<Vec<uuid::Uuid>, Self::Error> {
            Ok(self.tenant_role_ids.clone())
        }

        async fn load_permissions_for_roles(
            &self,
            _tenant_id: &uuid::Uuid,
            _role_ids: &[uuid::Uuid],
        ) -> Result<Vec<Permission>, Self::Error> {
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
    async fn continuous_invalidation_fails_closed() {
        let role_id = uuid::Uuid::new_v4();
        let store = StubStore {
            role_ids: vec![role_id],
            tenant_role_ids: vec![role_id],
            permissions: vec![Permission::USERS_MANAGE],
        };

        let result = resolve_permissions_with_cache(
            &store,
            &AlwaysSupersededCache,
            &uuid::Uuid::new_v4(),
            &uuid::Uuid::new_v4(),
        )
        .await
        .unwrap();

        assert!(result.permissions.is_empty());
        assert!(!result.cache_hit);
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
            role_ids: vec![],
            tenant_role_ids: vec![],
            permissions: vec![],
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
}
