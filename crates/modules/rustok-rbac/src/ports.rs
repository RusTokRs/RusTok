use async_trait::async_trait;
use rustok_api::{PortActorKind, PortCallPolicy, PortContext, PortError, PortErrorKind};
use serde::{Deserialize, Serialize};
use std::fmt::Display;
use uuid::Uuid;

use crate::PermissionResolver;

/// Transport-neutral request for checking effective permission claims.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbacPermissionCheckRequest {
    pub permissions: Vec<String>,
    pub mode: RbacPermissionCheckMode,
}

/// Evaluation strategy for a permission check request.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RbacPermissionCheckMode {
    Any,
    All,
}

/// Transport-neutral response for permission check consumers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RbacPermissionCheckResponse {
    pub allowed: bool,
    pub matched_permissions: Vec<String>,
    pub missing_permissions: Vec<String>,
    pub reason: Option<String>,
}

/// Transport-neutral owner boundary for RBAC permission decisions.
#[async_trait]
pub trait RbacPermissionDecisionPort: Send + Sync {
    async fn check_permissions(
        &self,
        context: PortContext,
        request: RbacPermissionCheckRequest,
    ) -> Result<RbacPermissionCheckResponse, PortError>;
}

/// Owner-managed permission-decision provider backed by the authoritative
/// tenant relation resolver. Consumers never receive RBAC entities or stores.
pub struct RbacPermissionDecisionProvider<R> {
    resolver: R,
}

impl<R> RbacPermissionDecisionProvider<R> {
    pub fn new(resolver: R) -> Self {
        Self { resolver }
    }
}

#[async_trait]
impl<R> RbacPermissionDecisionPort for RbacPermissionDecisionProvider<R>
where
    R: PermissionResolver + Send + Sync,
    R::Error: Display + Send + Sync,
{
    async fn check_permissions(
        &self,
        context: PortContext,
        request: RbacPermissionCheckRequest,
    ) -> Result<RbacPermissionCheckResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        validate_permission_request(&request)?;
        let tenant_id = parse_tenant_id(&context)?;
        let user_id = parse_user_actor(&context)?;
        tracing::debug!(
            tenant_id = %tenant_id,
            user_id = %user_id,
            mode = ?request.mode,
            permissions_count = request.permissions.len(),
            "Checking permissions via RbacPermissionDecisionPort"
        );
        let resolution = self
            .resolver
            .resolve_permissions(&tenant_id, &user_id)
            .await
            .map_err(|error| {
                PortError::unavailable("rbac.permission_resolution", error.to_string())
            })?;
        let resolved_permissions = resolution
            .permissions
            .iter()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        let matched_permissions = request
            .permissions
            .iter()
            .filter(|permission| {
                if let Ok(parsed) = permission.parse::<rustok_api::Permission>() {
                    crate::has_effective_permission_in_set(&resolution.permissions, &parsed)
                } else {
                    resolved_permissions
                        .iter()
                        .any(|resolved| resolved == *permission)
                }
            })
            .cloned()
            .collect::<Vec<_>>();
        let missing_permissions = request
            .permissions
            .iter()
            .filter(|permission| {
                !matched_permissions
                    .iter()
                    .any(|matched| matched == *permission)
            })
            .cloned()
            .collect::<Vec<_>>();
        let allowed = match request.mode {
            RbacPermissionCheckMode::Any => !matched_permissions.is_empty(),
            RbacPermissionCheckMode::All => missing_permissions.is_empty(),
        };

        Ok(RbacPermissionCheckResponse {
            allowed,
            matched_permissions,
            missing_permissions,
            reason: (!allowed).then_some("permission_denied".to_string()),
        })
    }
}

fn parse_tenant_id(context: &PortContext) -> Result<Uuid, PortError> {
    let id = Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "rbac.invalid_tenant_id",
            "RBAC permission decision context must carry a UUID tenant_id",
        )
    })?;
    if id.is_nil() {
        return Err(PortError::validation(
            "rbac.nil_tenant_id",
            "RBAC permission decision context must carry a non-nil UUID tenant_id",
        ));
    }
    Ok(id)
}

fn parse_user_actor(context: &PortContext) -> Result<Uuid, PortError> {
    if context.actor.kind != PortActorKind::User {
        return Err(PortError::new(
            PortErrorKind::Forbidden,
            "rbac.user_actor_required",
            "RBAC permission decisions require an authenticated user actor",
            false,
        ));
    }

    let id = Uuid::parse_str(&context.actor.id).map_err(|_| {
        PortError::validation(
            "rbac.invalid_actor_id",
            "RBAC permission decision context must carry a UUID user actor",
        )
    })?;
    if id.is_nil() {
        return Err(PortError::validation(
            "rbac.nil_actor_id",
            "RBAC permission decision context must carry a non-nil UUID user actor",
        ));
    }
    Ok(id)
}

fn validate_permission_request(request: &RbacPermissionCheckRequest) -> Result<(), PortError> {
    if request.permissions.is_empty() {
        return Err(PortError::validation(
            "rbac.permissions_empty",
            "permission decision port requires at least one permission",
        ));
    }
    if request.permissions.len() > 100 {
        return Err(PortError::validation(
            "rbac.permissions_limit_exceeded",
            "permission decision port accepts at most 100 permissions per request",
        ));
    }
    if request
        .permissions
        .iter()
        .any(|permission| permission.trim().is_empty())
    {
        return Err(PortError::new(
            PortErrorKind::Validation,
            "rbac.permission_invalid",
            "permission decision port requires non-empty permission names",
            false,
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::PermissionResolution;
    use rustok_api::{Permission, PortActor, PortContext};

    struct MockResolver {
        permissions: Vec<Permission>,
    }

    #[async_trait]
    impl PermissionResolver for MockResolver {
        type Error = String;

        async fn resolve_permissions(
            &self,
            _tenant_id: &Uuid,
            _user_id: &Uuid,
        ) -> Result<PermissionResolution, Self::Error> {
            Ok(PermissionResolution {
                permissions: self.permissions.clone(),
                cache_hit: false,
            })
        }
    }

    #[tokio::test]
    async fn decision_port_respects_manage_wildcard() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let provider = RbacPermissionDecisionProvider::new(MockResolver {
            permissions: vec![Permission::USERS_MANAGE],
        });

        let context = PortContext::new(
            tenant_id.to_string(),
            PortActor::user(user_id.to_string()),
            "en",
            "corr-decision-manage",
        )
        .with_deadline(std::time::Duration::from_secs(5));

        let response = provider
            .check_permissions(
                context,
                RbacPermissionCheckRequest {
                    permissions: vec!["users:read".to_string(), "users:delete".to_string()],
                    mode: RbacPermissionCheckMode::All,
                },
            )
            .await
            .expect("check_permissions should succeed");

        assert!(response.allowed);
        assert_eq!(response.matched_permissions.len(), 2);
        assert!(response.missing_permissions.is_empty());
    }

    #[tokio::test]
    async fn decision_port_denies_missing_permission() {
        let tenant_id = Uuid::new_v4();
        let user_id = Uuid::new_v4();
        let provider = RbacPermissionDecisionProvider::new(MockResolver {
            permissions: vec![Permission::USERS_READ],
        });

        let context = PortContext::new(
            tenant_id.to_string(),
            PortActor::user(user_id.to_string()),
            "en",
            "corr-decision-denies",
        )
        .with_deadline(std::time::Duration::from_secs(5));

        let response = provider
            .check_permissions(
                context,
                RbacPermissionCheckRequest {
                    permissions: vec!["users:read".to_string(), "users:delete".to_string()],
                    mode: RbacPermissionCheckMode::All,
                },
            )
            .await
            .expect("check_permissions should succeed");

        assert!(!response.allowed);
        assert_eq!(response.matched_permissions, vec!["users:read".to_string()]);
        assert_eq!(
            response.missing_permissions,
            vec!["users:delete".to_string()]
        );
    }

    #[tokio::test]
    async fn decision_port_rejects_nil_tenant_and_actor() {
        let provider = RbacPermissionDecisionProvider::new(MockResolver {
            permissions: vec![Permission::USERS_READ],
        });

        let nil_tenant_context = PortContext::new(
            Uuid::nil().to_string(),
            PortActor::user(Uuid::new_v4().to_string()),
            "en",
            "corr-nil-tenant",
        )
        .with_deadline(std::time::Duration::from_secs(5));
        let err = provider
            .check_permissions(
                nil_tenant_context,
                RbacPermissionCheckRequest {
                    permissions: vec!["users:read".to_string()],
                    mode: RbacPermissionCheckMode::All,
                },
            )
            .await
            .expect_err("should reject nil tenant");
        assert_eq!(err.code, "rbac.nil_tenant_id");

        let nil_actor_context = PortContext::new(
            Uuid::new_v4().to_string(),
            PortActor::user(Uuid::nil().to_string()),
            "en",
            "corr-nil-actor",
        )
        .with_deadline(std::time::Duration::from_secs(5));
        let err = provider
            .check_permissions(
                nil_actor_context,
                RbacPermissionCheckRequest {
                    permissions: vec!["users:read".to_string()],
                    mode: RbacPermissionCheckMode::All,
                },
            )
            .await
            .expect_err("should reject nil actor");
        assert_eq!(err.code, "rbac.nil_actor_id");
    }

    #[tokio::test]
    async fn decision_port_rejects_oversized_permissions() {
        let provider = RbacPermissionDecisionProvider::new(MockResolver {
            permissions: vec![Permission::USERS_READ],
        });
        let context = PortContext::new(
            Uuid::new_v4().to_string(),
            PortActor::user(Uuid::new_v4().to_string()),
            "en",
            "corr-oversized",
        )
        .with_deadline(std::time::Duration::from_secs(5));
        let oversized = (0..101).map(|i| format!("perm:{i}")).collect();
        let err = provider
            .check_permissions(
                context,
                RbacPermissionCheckRequest {
                    permissions: oversized,
                    mode: RbacPermissionCheckMode::All,
                },
            )
            .await
            .expect_err("should reject >100 permissions");
        assert_eq!(err.code, "rbac.permissions_limit_exceeded");
    }
}
