use std::future::Future;

use rustok_api::Permission;
use rustok_core::UserRole;
use uuid::Uuid;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RbacRequestScope {
    pub tenant_id: Uuid,
    pub actor_id: Uuid,
    pub permissions: Vec<Permission>,
    pub role: UserRole,
}

impl RbacRequestScope {
    pub fn new(
        tenant_id: Uuid,
        actor_id: Uuid,
        mut permissions: Vec<Permission>,
        role: UserRole,
    ) -> Self {
        permissions.sort_by_cached_key(|permission| permission.to_string());
        permissions.dedup();
        Self {
            tenant_id,
            actor_id,
            permissions,
            role,
        }
    }

    fn matches(&self, tenant_id: &Uuid, actor_id: &Uuid) -> bool {
        self.tenant_id == *tenant_id && self.actor_id == *actor_id
    }
}

tokio::task_local! {
    static CURRENT_RBAC_SCOPE: RbacRequestScope;
}

pub async fn with_rbac_request_scope<F>(scope: Option<RbacRequestScope>, future: F) -> F::Output
where
    F: Future,
{
    match scope {
        Some(scope) => CURRENT_RBAC_SCOPE.scope(scope, future).await,
        None => future.await,
    }
}

pub fn permissions_for(tenant_id: &Uuid, actor_id: &Uuid) -> Option<Vec<Permission>> {
    CURRENT_RBAC_SCOPE
        .try_with(|scope| {
            scope
                .matches(tenant_id, actor_id)
                .then(|| scope.permissions.clone())
        })
        .ok()
        .flatten()
}

pub fn role_for(tenant_id: &Uuid, actor_id: &Uuid) -> Option<UserRole> {
    CURRENT_RBAC_SCOPE
        .try_with(|scope| {
            scope
                .matches(tenant_id, actor_id)
                .then(|| scope.role.clone())
        })
        .ok()
        .flatten()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[tokio::test]
    async fn request_scope_is_bound_to_exact_tenant_and_actor() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let scope = RbacRequestScope::new(
            tenant_id,
            actor_id,
            vec![Permission::USERS_READ],
            UserRole::Admin,
        );

        with_rbac_request_scope(Some(scope), async move {
            assert_eq!(
                permissions_for(&tenant_id, &actor_id),
                Some(vec![Permission::USERS_READ])
            );
            assert_eq!(role_for(&tenant_id, &actor_id), Some(UserRole::Admin));
            assert!(permissions_for(&tenant_id, &Uuid::new_v4()).is_none());
            assert!(permissions_for(&Uuid::new_v4(), &actor_id).is_none());
        })
        .await;
    }

    #[tokio::test]
    async fn request_scope_does_not_leak_after_future_completes() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        with_rbac_request_scope(
            Some(RbacRequestScope::new(
                tenant_id,
                actor_id,
                vec![Permission::USERS_LIST],
                UserRole::Manager,
            )),
            async {
                assert!(permissions_for(&tenant_id, &actor_id).is_some());
            },
        )
        .await;

        assert!(permissions_for(&tenant_id, &actor_id).is_none());
    }

    #[test]
    fn snapshots_canonicalize_permission_order() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let first = RbacRequestScope::new(
            tenant_id,
            actor_id,
            vec![Permission::USERS_LIST, Permission::USERS_READ],
            UserRole::Admin,
        );
        let second = RbacRequestScope::new(
            tenant_id,
            actor_id,
            vec![Permission::USERS_READ, Permission::USERS_LIST],
            UserRole::Admin,
        );

        assert_eq!(first, second);
    }

    #[test]
    fn snapshots_detect_permission_or_role_drift() {
        let tenant_id = Uuid::new_v4();
        let actor_id = Uuid::new_v4();
        let baseline = RbacRequestScope::new(
            tenant_id,
            actor_id,
            vec![Permission::USERS_READ],
            UserRole::Admin,
        );
        let changed_permissions = RbacRequestScope::new(
            tenant_id,
            actor_id,
            vec![Permission::USERS_LIST],
            UserRole::Admin,
        );
        let changed_role = RbacRequestScope::new(
            tenant_id,
            actor_id,
            vec![Permission::USERS_READ],
            UserRole::Manager,
        );

        assert_ne!(baseline, changed_permissions);
        assert_ne!(baseline, changed_role);
    }
}