use crate::{evaluate_all_permissions, evaluate_any_permission, evaluate_single_permission};
use async_trait::async_trait;
use rustok_api::Permission;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionResolution {
    pub permissions: Vec<Permission>,
    pub cache_hit: bool,
}

/// Read-only owner contract for resolving effective tenant permissions.
///
/// Role and permission mutations intentionally do not belong to this trait.
/// Existing-user writes must use transaction-owned or committed RBAC mutation
/// entry points so continuity locks, durable invalidation generations and
/// cross-replica cache recovery cannot be bypassed.
#[async_trait]
pub trait PermissionResolver {
    type Error;

    async fn resolve_permissions(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
    ) -> Result<PermissionResolution, Self::Error>;

    async fn has_permission(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        required_permission: &Permission,
    ) -> Result<bool, Self::Error> {
        let resolved = self.resolve_permissions(tenant_id, user_id).await?;
        Ok(evaluate_single_permission(&resolved.permissions, required_permission).allowed)
    }

    async fn has_any_permission(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        required_permissions: &[Permission],
    ) -> Result<bool, Self::Error> {
        let resolved = self.resolve_permissions(tenant_id, user_id).await?;
        Ok(evaluate_any_permission(&resolved.permissions, required_permissions).allowed)
    }

    async fn has_all_permissions(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        required_permissions: &[Permission],
    ) -> Result<bool, Self::Error> {
        let resolved = self.resolve_permissions(tenant_id, user_id).await?;
        Ok(evaluate_all_permissions(&resolved.permissions, required_permissions).allowed)
    }
}

#[cfg(test)]
mod tests {
    use super::{PermissionResolution, PermissionResolver};
    use async_trait::async_trait;
    use rustok_api::Permission;

    struct StubResolver {
        permissions: Vec<Permission>,
    }

    #[async_trait]
    impl PermissionResolver for StubResolver {
        type Error = String;

        async fn resolve_permissions(
            &self,
            _tenant_id: &uuid::Uuid,
            _user_id: &uuid::Uuid,
        ) -> Result<PermissionResolution, Self::Error> {
            Ok(PermissionResolution {
                permissions: self.permissions.clone(),
                cache_hit: true,
            })
        }
    }

    #[test]
    fn resolution_keeps_permissions_payload() {
        let resolved = PermissionResolution {
            permissions: vec![Permission::USERS_READ],
            cache_hit: true,
        };

        assert_eq!(resolved.permissions, vec![Permission::USERS_READ]);
        assert!(resolved.cache_hit);
    }

    #[tokio::test]
    async fn default_has_permission_uses_resolved_permissions() {
        let tenant_id = uuid::Uuid::new_v4();
        let user_id = uuid::Uuid::new_v4();
        let resolver = StubResolver {
            permissions: vec![Permission::USERS_READ],
        };

        let allowed = resolver
            .has_permission(&tenant_id, &user_id, &Permission::USERS_READ)
            .await
            .unwrap();

        assert!(allowed);
    }

    #[tokio::test]
    async fn default_has_all_permissions_respects_missing_permissions() {
        let tenant_id = uuid::Uuid::new_v4();
        let user_id = uuid::Uuid::new_v4();
        let resolver = StubResolver {
            permissions: vec![Permission::USERS_READ],
        };

        let allowed = resolver
            .has_all_permissions(
                &tenant_id,
                &user_id,
                &[Permission::USERS_READ, Permission::USERS_UPDATE],
            )
            .await
            .unwrap();

        assert!(!allowed);
    }
}
