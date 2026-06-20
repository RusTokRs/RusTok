use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use serde::{Deserialize, Serialize};

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

#[async_trait]
impl RbacPermissionDecisionPort for crate::RbacModule {
    async fn check_permissions(
        &self,
        context: PortContext,
        request: RbacPermissionCheckRequest,
    ) -> Result<RbacPermissionCheckResponse, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        validate_permission_request(&request)?;

        let claims = context.claims;
        let matched_permissions = request
            .permissions
            .iter()
            .filter(|permission| claims.iter().any(|claim| claim == *permission))
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
