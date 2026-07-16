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
                resolved_permissions
                    .iter()
                    .any(|resolved| resolved == *permission)
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
    Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "rbac.invalid_tenant_id",
            "RBAC permission decision context must carry a UUID tenant_id",
        )
    })
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

    Uuid::parse_str(&context.actor.id).map_err(|_| {
        PortError::validation(
            "rbac.invalid_actor_id",
            "RBAC permission decision context must carry a UUID user actor",
        )
    })
}

fn validate_permission_request(request: &RbacPermissionCheckRequest) -> Result<(), PortError> {
    if request.permissions.is_empty() {
        return Err(PortError::validation(
            "rbac.permissions_empty",
            "permission decision port requires at least one permission",
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
