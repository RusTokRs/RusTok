use std::str::FromStr;

use rustok_api::{Permission, has_effective_permission};
use rustok_mcp::McpActorType;
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::models::{mcp_clients, mcp_policies, users};

use super::rbac_service::RbacService;

#[derive(Debug, thiserror::Error)]
pub enum McpManagementAuthorityError {
    #[error("invalid MCP management authority request: {0}")]
    Invalid(String),
    #[error("MCP management authority denied: {0}")]
    Forbidden(String),
    #[error("MCP management resource not found: {0}")]
    NotFound(String),
    #[error("MCP management authority validation failed: {0}")]
    Internal(String),
}

pub struct McpManagementAuthorityService;

impl McpManagementAuthorityService {
    pub async fn validate_create_client(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        manager_permissions: &[Permission],
        actor_type: McpActorType,
        delegated_user_id: Option<Uuid>,
        granted_permissions: &[String],
    ) -> Result<(), McpManagementAuthorityError> {
        let delegated = delegated_permissions(db, tenant_id, actor_type, delegated_user_id).await?;
        validate_all_authorities(
            granted_permissions,
            manager_permissions,
            delegated.as_deref(),
        )
    }

    pub async fn validate_policy_update(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        manager_permissions: &[Permission],
        client_id: Uuid,
        granted_permissions: &[String],
    ) -> Result<(), McpManagementAuthorityError> {
        let client = require_client(db, tenant_id, client_id).await?;
        let delegated =
            delegated_permissions(db, tenant_id, client.actor_type(), client.delegated_user_id)
                .await?;
        validate_all_authorities(
            granted_permissions,
            manager_permissions,
            delegated.as_deref(),
        )
    }

    pub async fn validate_token_rotation(
        db: &DatabaseConnection,
        tenant_id: Uuid,
        manager_permissions: &[Permission],
        client_id: Uuid,
    ) -> Result<(), McpManagementAuthorityError> {
        let client = require_client(db, tenant_id, client_id).await?;
        let delegated =
            delegated_permissions(db, tenant_id, client.actor_type(), client.delegated_user_id)
                .await?;
        let policy = mcp_policies::Entity::find_by_client(db, client.id)
            .await
            .map_err(|error| McpManagementAuthorityError::Internal(error.to_string()))?;

        if let Some(policy) = policy {
            if policy.tenant_id != tenant_id {
                return Err(McpManagementAuthorityError::Forbidden(
                    "MCP policy belongs to another tenant".to_string(),
                ));
            }
            validate_all_authorities(
                &policy.granted_permissions_list(),
                manager_permissions,
                delegated.as_deref(),
            )?;
        }
        Ok(())
    }
}

async fn require_client(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    client_id: Uuid,
) -> Result<mcp_clients::Model, McpManagementAuthorityError> {
    mcp_clients::Entity::find_by_id(client_id)
        .filter(mcp_clients::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(|error| McpManagementAuthorityError::Internal(error.to_string()))?
        .ok_or_else(|| McpManagementAuthorityError::NotFound("MCP client".to_string()))
}

async fn delegated_permissions(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    actor_type: McpActorType,
    delegated_user_id: Option<Uuid>,
) -> Result<Option<Vec<Permission>>, McpManagementAuthorityError> {
    if actor_type == McpActorType::HumanUser && delegated_user_id.is_none() {
        return Err(McpManagementAuthorityError::Invalid(
            "human_user MCP clients require delegated_user_id".to_string(),
        ));
    }
    let Some(user_id) = delegated_user_id else {
        return Ok(None);
    };

    let user = users::Entity::find_by_id(user_id)
        .filter(users::Column::TenantId.eq(tenant_id))
        .one(db)
        .await
        .map_err(|error| McpManagementAuthorityError::Internal(error.to_string()))?
        .ok_or_else(|| {
            McpManagementAuthorityError::Invalid(
                "delegated MCP user does not exist in the current tenant".to_string(),
            )
        })?;
    if !user.is_active() {
        return Err(McpManagementAuthorityError::Invalid(
            "delegated MCP user must be active".to_string(),
        ));
    }

    RbacService::get_user_permissions_authoritative(db, &tenant_id, &user_id)
        .await
        .map(Some)
        .map_err(|error| McpManagementAuthorityError::Internal(error.to_string()))
}

fn validate_all_authorities(
    requested: &[String],
    manager_permissions: &[Permission],
    delegated_permissions: Option<&[Permission]>,
) -> Result<(), McpManagementAuthorityError> {
    validate_grants(requested, manager_permissions, "current MCP manager")?;
    if let Some(delegated_permissions) = delegated_permissions {
        validate_grants(requested, delegated_permissions, "delegated MCP user")?;
    }
    Ok(())
}

fn validate_grants(
    requested: &[String],
    authority: &[Permission],
    principal: &str,
) -> Result<(), McpManagementAuthorityError> {
    for raw in requested {
        let permission = Permission::from_str(raw.trim()).map_err(|error| {
            McpManagementAuthorityError::Invalid(format!(
                "invalid MCP granted permission `{raw}`: {error}"
            ))
        })?;
        if !has_effective_permission(authority, &permission) {
            return Err(McpManagementAuthorityError::Forbidden(format!(
                "MCP permission `{permission}` exceeds {principal} authority"
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{McpManagementAuthorityError, validate_grants};
    use rustok_api::Permission;

    #[test]
    fn manager_authority_accepts_implied_read_but_rejects_escalation() {
        validate_grants(
            &[Permission::PAGES_READ.to_string()],
            &[Permission::PAGES_MANAGE],
            "manager",
        )
        .expect("manage authority should imply read");

        let error = validate_grants(
            &[Permission::PAGES_MANAGE.to_string()],
            &[Permission::PAGES_READ],
            "manager",
        )
        .expect_err("read authority must not grant manage");
        assert!(matches!(error, McpManagementAuthorityError::Forbidden(_)));
    }

    #[test]
    fn malformed_permission_is_rejected_before_persistence() {
        let error = validate_grants(&["not-a-permission".to_string()], &[], "manager")
            .expect_err("malformed permission must fail");
        assert!(matches!(error, McpManagementAuthorityError::Invalid(_)));
    }
}
