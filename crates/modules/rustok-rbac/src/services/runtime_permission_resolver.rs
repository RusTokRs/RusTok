use crate::{
    PermissionCache, PermissionCacheResolutionError, PermissionResolution, PermissionResolver,
    RelationPermissionStore, resolve_permissions_with_cache,
};
use async_trait::async_trait;
use std::marker::PhantomData;

/// Runtime adapter for the read-only permission-resolution path.
#[derive(Clone)]
pub struct RuntimePermissionResolver<S, C, E>
where
    S: RelationPermissionStore,
    C: PermissionCache,
{
    store: S,
    cache: C,
    _error: PhantomData<E>,
}

impl<S, C, E> RuntimePermissionResolver<S, C, E>
where
    S: RelationPermissionStore,
    C: PermissionCache,
{
    pub fn new(store: S, cache: C) -> Self {
        Self {
            store,
            cache,
            _error: PhantomData,
        }
    }
}

#[async_trait]
impl<S, C, E> PermissionResolver for RuntimePermissionResolver<S, C, E>
where
    S: RelationPermissionStore + Send + Sync,
    C: PermissionCache + Send + Sync,
    S::Error: Send + Sync,
    E: From<PermissionCacheResolutionError<S::Error>> + Send + Sync,
{
    type Error = E;

    async fn resolve_permissions(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
    ) -> Result<PermissionResolution, Self::Error> {
        resolve_permissions_with_cache(&self.store, &self.cache, tenant_id, user_id)
            .await
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::RuntimePermissionResolver;
    use crate::{
        PermissionCache, PermissionCacheLookup, PermissionCacheResolutionError, PermissionResolver,
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
        fail_load: bool,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum StubStoreError {
        Load,
    }

    #[derive(Debug, Clone, PartialEq, Eq)]
    enum ResolverError {
        Store(StubStoreError),
        Invalidated,
    }

    impl From<PermissionCacheResolutionError<StubStoreError>> for ResolverError {
        fn from(value: PermissionCacheResolutionError<StubStoreError>) -> Self {
            match value {
                PermissionCacheResolutionError::Store(error) => Self::Store(error),
                PermissionCacheResolutionError::Invalidated => Self::Invalidated,
            }
        }
    }

    struct SupersededCache;

    #[async_trait]
    impl PermissionCache for SupersededCache {
        async fn get(&self, _: &uuid::Uuid, _: &uuid::Uuid) -> Option<Vec<Permission>> {
            None
        }
        async fn insert(&self, _: &uuid::Uuid, _: &uuid::Uuid, _: Vec<Permission>) {
            panic!("generation-aware resolver must use conditional publication");
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
            _: Vec<Permission>,
        ) -> bool {
            false
        }
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
        type Error = StubStoreError;

        async fn load_user_role_ids(
            &self,
            _user_id: &uuid::Uuid,
        ) -> Result<Vec<uuid::Uuid>, Self::Error> {
            if self.fail_load {
                return Err(StubStoreError::Load);
            }
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
    async fn resolve_permissions_delegates_to_relation_and_cache_layer() {
        let role_id = uuid::Uuid::new_v4();
        let tenant_id = uuid::Uuid::new_v4();
        let user_id = uuid::Uuid::new_v4();
        let resolver: RuntimePermissionResolver<_, _, ResolverError> =
            RuntimePermissionResolver::new(
                StubStore {
                    role_ids: vec![role_id],
                    tenant_role_ids: vec![role_id],
                    permissions: vec![Permission::USERS_READ],
                    fail_load: false,
                },
                StubCache::default(),
            );

        let first = resolver
            .resolve_permissions(&tenant_id, &user_id)
            .await
            .unwrap();
        let second = resolver
            .resolve_permissions(&tenant_id, &user_id)
            .await
            .unwrap();

        assert!(!first.cache_hit);
        assert!(second.cache_hit);
        assert_eq!(second.permissions, vec![Permission::USERS_READ]);
    }

    #[tokio::test]
    async fn authorization_propagates_continuous_invalidation_without_a_decision() {
        let role_id = uuid::Uuid::new_v4();
        let resolver: RuntimePermissionResolver<_, _, ResolverError> =
            RuntimePermissionResolver::new(
                StubStore {
                    role_ids: vec![role_id],
                    tenant_role_ids: vec![role_id],
                    permissions: vec![Permission::USERS_MANAGE],
                    fail_load: false,
                },
                SupersededCache,
            );
        let tenant_id = uuid::Uuid::new_v4();
        let user_id = uuid::Uuid::new_v4();
        let permission = Permission::USERS_READ;
        let required = [permission];
        assert_eq!(
            crate::authorize_permission(&resolver, &tenant_id, &user_id, &permission)
                .await
                .err(),
            Some(ResolverError::Invalidated)
        );
        assert_eq!(
            crate::authorize_any_permission(&resolver, &tenant_id, &user_id, &required)
                .await
                .err(),
            Some(ResolverError::Invalidated)
        );
        assert_eq!(
            crate::authorize_all_permissions(&resolver, &tenant_id, &user_id, &required)
                .await
                .err(),
            Some(ResolverError::Invalidated)
        );
    }

    #[tokio::test]
    async fn relation_store_error_is_mapped_to_runtime_resolver_error_type() {
        let resolver: RuntimePermissionResolver<_, _, ResolverError> =
            RuntimePermissionResolver::new(
                StubStore {
                    role_ids: vec![],
                    tenant_role_ids: vec![],
                    permissions: vec![],
                    fail_load: true,
                },
                StubCache::default(),
            );

        let result = resolver
            .resolve_permissions(&uuid::Uuid::new_v4(), &uuid::Uuid::new_v4())
            .await;

        assert_eq!(
            result.err(),
            Some(ResolverError::Store(StubStoreError::Load))
        );
    }
}
