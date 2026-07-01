use crate::{evaluate_all_permissions, evaluate_any_permission, evaluate_single_permission};
use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::UserRole;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PermissionResolution {
    pub permissions: Vec<Permission>,
    pub cache_hit: bool,
}

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

    async fn assign_role_permissions(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        role: UserRole,
    ) -> Result<(), Self::Error>;

    async fn replace_user_role(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        role: UserRole,
    ) -> Result<(), Self::Error>;

    async fn remove_tenant_role_assignments(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
    ) -> Result<(), Self::Error>;

    async fn remove_user_role_assignment(
        &self,
        tenant_id: &uuid::Uuid,
        user_id: &uuid::Uuid,
        role: UserRole,
    ) -> Result<(), Self::Error>;
}

#[cfg(test)]
mod tests {
    use super::{PermissionResolution, PermissionResolver};
    use async_trait::async_trait;
    use rustok_api::Permission;
    use rustok_core::UserRole;

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

        async fn assign_role_permissions(
            &self,
            _tenant_id: &uuid::Uuid,
            _user_id: &uuid::Uuid,
            _role: UserRole,
        ) -> Result<(), Self::Error> {
            Ok(())
        }

        async fn replace_user_role(
            &self,
            _tenant_id: &uuid::Uuid,
            _user_id: &uuid::Uuid,
            _role: UserRole,
        ) -> Result<(), Self::Error> {
            Ok(())
        }

        async fn remove_tenant_role_assignments(
            &self,
            _tenant_id: &uuid::Uuid,
            _user_id: &uuid::Uuid,
        ) -> Result<(), Self::Error> {
            Ok(())
        }

        async fn remove_user_role_assignment(
            &self,
            _tenant_id: &uuid::Uuid,
            _user_id: &uuid::Uuid,
            _role: UserRole,
        ) -> Result<(), Self::Error> {
            Ok(())
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
