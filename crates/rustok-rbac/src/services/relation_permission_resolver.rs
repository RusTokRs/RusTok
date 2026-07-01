use crate::services::permission_normalization::normalize_permissions;
use rustok_api::Permission;

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
    C: PermissionCache,
{
    if let Some(cached_permissions) = cache.get(tenant_id, user_id).await {
        return Ok(crate::PermissionResolution {
            permissions: normalize_permissions(cached_permissions),
            cache_hit: true,
        });
    }

    let resolved_permissions =
        resolve_permissions_from_relations(store, tenant_id, user_id).await?;
    cache
        .insert(tenant_id, user_id, resolved_permissions.clone())
        .await;

    Ok(crate::PermissionResolution {
        permissions: resolved_permissions,
        cache_hit: false,
    })
}

pub async fn invalidate_cached_permissions<C: PermissionCache>(
    cache: &C,
    tenant_id: &uuid::Uuid,
    user_id: &uuid::Uuid,
) {
    cache.invalidate(tenant_id, user_id).await;
}

#[cfg(test)]
mod tests {
    use super::{
        resolve_permissions_from_relations, resolve_permissions_with_cache, PermissionCache,
        RelationPermissionStore,
    };
    use async_trait::async_trait;
    use rustok_api::Permission;
    use std::collections::HashMap;
    use std::sync::Arc;
    use tokio::sync::Mutex;

    type PermissionCacheKey = (uuid::Uuid, uuid::Uuid);
    type PermissionCacheMap = HashMap<PermissionCacheKey, Vec<Permission>>;

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
