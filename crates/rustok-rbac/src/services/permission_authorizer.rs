use crate::{AuthzEngine, PermissionResolver, denied_reason_for_denial, missing_permissions};
use rustok_api::Permission;

use super::{permission_check::PermissionCheck, policy_evaluator::evaluate_policy_permissions};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AuthorizationDecision {
    pub engine: AuthzEngine,
    pub allowed: bool,
    pub missing_permissions: Vec<Permission>,
    pub denied_reason: Option<(crate::DeniedReasonKind, String)>,
    pub resolved_permissions: Vec<Permission>,
    pub permissions_count: usize,
    pub cache_hit: bool,
}

pub async fn authorize_permission<R: PermissionResolver>(
    resolver: &R,
    tenant_id: &uuid::Uuid,
    user_id: &uuid::Uuid,
    required_permission: &Permission,
) -> Result<AuthorizationDecision, R::Error> {
    let resolved = resolver.resolve_permissions(tenant_id, user_id).await?;
    let engine = AuthzEngine::Policy;
    let (allowed, missing_permissions, denied_reason) = decision_from_policy_check(
        &resolved.permissions,
        tenant_id,
        PermissionCheck::Single(required_permission),
    )
    .await;

    Ok(AuthorizationDecision {
        engine,
        allowed,
        missing_permissions,
        denied_reason,
        resolved_permissions: resolved.permissions.clone(),
        permissions_count: resolved.permissions.len(),
        cache_hit: resolved.cache_hit,
    })
}

pub async fn authorize_any_permission<R: PermissionResolver>(
    resolver: &R,
    tenant_id: &uuid::Uuid,
    user_id: &uuid::Uuid,
    required_permissions: &[Permission],
) -> Result<AuthorizationDecision, R::Error> {
    let resolved = resolver.resolve_permissions(tenant_id, user_id).await?;
    let engine = AuthzEngine::Policy;
    let (allowed, missing_permissions, denied_reason) = decision_from_policy_check(
        &resolved.permissions,
        tenant_id,
        PermissionCheck::Any(required_permissions),
    )
    .await;

    Ok(AuthorizationDecision {
        engine,
        allowed,
        missing_permissions,
        denied_reason,
        resolved_permissions: resolved.permissions.clone(),
        permissions_count: resolved.permissions.len(),
        cache_hit: resolved.cache_hit,
    })
}

pub async fn authorize_all_permissions<R: PermissionResolver>(
    resolver: &R,
    tenant_id: &uuid::Uuid,
    user_id: &uuid::Uuid,
    required_permissions: &[Permission],
) -> Result<AuthorizationDecision, R::Error> {
    let resolved = resolver.resolve_permissions(tenant_id, user_id).await?;
    let engine = AuthzEngine::Policy;
    let (allowed, missing_permissions, denied_reason) = decision_from_policy_check(
        &resolved.permissions,
        tenant_id,
        PermissionCheck::All(required_permissions),
    )
    .await;

    Ok(AuthorizationDecision {
        engine,
        allowed,
        missing_permissions,
        denied_reason,
        resolved_permissions: resolved.permissions.clone(),
        permissions_count: resolved.permissions.len(),
        cache_hit: resolved.cache_hit,
    })
}

async fn decision_from_policy_check(
    resolved_permissions: &[Permission],
    tenant_id: &uuid::Uuid,
    check: PermissionCheck<'_>,
) -> (
    bool,
    Vec<Permission>,
    Option<(crate::DeniedReasonKind, String)>,
) {
    let required_permissions = required_permissions_for_check(check);
    let allowed = evaluate_policy_permissions(tenant_id, resolved_permissions, check).await;
    let missing_permissions = if allowed {
        Vec::new()
    } else {
        missing_permissions(resolved_permissions, &required_permissions)
    };
    let denied_reason = if allowed {
        None
    } else {
        Some(denied_reason_for_denial(
            resolved_permissions,
            &missing_permissions,
        ))
    };

    (allowed, missing_permissions, denied_reason)
}

fn required_permissions_for_check(check: PermissionCheck<'_>) -> Vec<Permission> {
    match check {
        PermissionCheck::Single(permission) => vec![*permission],
        PermissionCheck::Any(permissions) | PermissionCheck::All(permissions) => {
            permissions.to_vec()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{authorize_all_permissions, authorize_any_permission, authorize_permission};
    use crate::{AuthzEngine, PermissionResolution, PermissionResolver};
    use async_trait::async_trait;
    use rustok_api::Permission;
    use rustok_core::UserRole;

    struct StubResolver {
        permissions: Vec<Permission>,
        cache_hit: bool,
        fail_resolve: bool,
    }

    #[async_trait]
    impl PermissionResolver for StubResolver {
        type Error = String;

        async fn resolve_permissions(
            &self,
            _tenant_id: &uuid::Uuid,
            _user_id: &uuid::Uuid,
        ) -> Result<PermissionResolution, Self::Error> {
            if self.fail_resolve {
                return Err("resolve failed".to_string());
            }
            Ok(PermissionResolution {
                permissions: self.permissions.clone(),
                cache_hit: self.cache_hit,
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

    #[tokio::test]
    async fn authorize_permission_reports_denied_reason() {
        let resolver = StubResolver {
            permissions: vec![],
            cache_hit: true,
            fail_resolve: false,
        };

        let decision = authorize_permission(
            &resolver,
            &uuid::Uuid::new_v4(),
            &uuid::Uuid::new_v4(),
            &Permission::USERS_READ,
        )
        .await
        .unwrap();

        assert_eq!(decision.engine, AuthzEngine::Policy);
        assert!(!decision.allowed);
        assert_eq!(decision.permissions_count, 0);
        assert!(decision.denied_reason.is_some());
        assert!(decision.resolved_permissions.is_empty());
        assert!(decision.cache_hit);
    }

    #[tokio::test]
    async fn authorize_any_permission_respects_manage_wildcard() {
        let resolver = StubResolver {
            permissions: vec![Permission::USERS_MANAGE],
            cache_hit: false,
            fail_resolve: false,
        };

        let decision = authorize_any_permission(
            &resolver,
            &uuid::Uuid::new_v4(),
            &uuid::Uuid::new_v4(),
            &[Permission::USERS_READ, Permission::USERS_UPDATE],
        )
        .await
        .unwrap();

        assert_eq!(decision.engine, AuthzEngine::Policy);
        assert!(decision.allowed);
        assert!(decision.missing_permissions.is_empty());
        assert_eq!(
            decision.resolved_permissions,
            vec![Permission::USERS_MANAGE]
        );
        assert!(!decision.cache_hit);
    }

    #[tokio::test]
    async fn authorize_all_permissions_returns_missing_permissions() {
        let resolver = StubResolver {
            permissions: vec![Permission::USERS_READ],
            cache_hit: false,
            fail_resolve: false,
        };

        let decision = authorize_all_permissions(
            &resolver,
            &uuid::Uuid::new_v4(),
            &uuid::Uuid::new_v4(),
            &[Permission::USERS_READ, Permission::USERS_UPDATE],
        )
        .await
        .unwrap();

        assert_eq!(decision.engine, AuthzEngine::Policy);
        assert!(!decision.allowed);
        assert_eq!(decision.missing_permissions, vec![Permission::USERS_UPDATE]);
        assert_eq!(decision.resolved_permissions, vec![Permission::USERS_READ]);
    }

    #[tokio::test]
    async fn authorize_all_permissions_allows_empty_requirements() {
        let resolver = StubResolver {
            permissions: vec![Permission::USERS_READ],
            cache_hit: true,
            fail_resolve: false,
        };

        let decision =
            authorize_all_permissions(&resolver, &uuid::Uuid::new_v4(), &uuid::Uuid::new_v4(), &[])
                .await
                .unwrap();

        assert_eq!(decision.engine, AuthzEngine::Policy);
        assert!(decision.allowed);
        assert!(decision.missing_permissions.is_empty());
        assert!(decision.denied_reason.is_none());
        assert_eq!(decision.resolved_permissions, vec![Permission::USERS_READ]);
        assert_eq!(decision.permissions_count, 1);
        assert!(decision.cache_hit);
    }

    #[tokio::test]
    async fn authorize_any_permission_propagates_resolver_error() {
        let resolver = StubResolver {
            permissions: vec![Permission::USERS_READ],
            cache_hit: false,
            fail_resolve: true,
        };

        let result = authorize_any_permission(
            &resolver,
            &uuid::Uuid::new_v4(),
            &uuid::Uuid::new_v4(),
            &[Permission::USERS_READ],
        )
        .await;

        assert_eq!(result, Err("resolve failed".to_string()));
    }

    #[tokio::test]
    async fn authorize_permission_uses_policy_engine() {
        let resolver = StubResolver {
            permissions: vec![Permission::USERS_MANAGE],
            cache_hit: false,
            fail_resolve: false,
        };

        let decision = authorize_permission(
            &resolver,
            &uuid::Uuid::new_v4(),
            &uuid::Uuid::new_v4(),
            &Permission::USERS_UPDATE,
        )
        .await
        .unwrap();

        assert_eq!(decision.engine, AuthzEngine::Policy);
        assert!(decision.allowed);
        assert!(decision.denied_reason.is_none());
    }

    #[tokio::test]
    async fn authorize_all_permissions_reports_policy_missing_permissions() {
        let resolver = StubResolver {
            permissions: vec![Permission::USERS_READ],
            cache_hit: false,
            fail_resolve: false,
        };

        let decision = authorize_all_permissions(
            &resolver,
            &uuid::Uuid::new_v4(),
            &uuid::Uuid::new_v4(),
            &[Permission::USERS_READ, Permission::USERS_UPDATE],
        )
        .await
        .unwrap();

        assert_eq!(decision.engine, AuthzEngine::Policy);
        assert!(!decision.allowed);
        assert_eq!(decision.missing_permissions, vec![Permission::USERS_UPDATE]);
        assert_eq!(
            decision.denied_reason,
            Some((
                crate::DeniedReasonKind::MissingPermissions,
                "missing_permissions:users:update".to_string(),
            ))
        );
    }
}
