use std::str::FromStr;

use crate::{McpActorType, McpManagementAuthorityError};
use rustok_api::{Permission, has_effective_permission};
use sea_orm::{ColumnTrait, DatabaseConnection, DatabaseTransaction, EntityTrait, QueryFilter};
use uuid::Uuid;

use super::models::{mcp_clients, mcp_policies};
use super::{McpDelegatedPrincipal, McpHostServices};

/// Seal an internal failure: the detail is logged server-side and the
/// client only receives a generic internal-error payload.
fn internal_error<E: std::fmt::Display>(
    context: &'static str,
    error: E,
) -> McpManagementAuthorityError {
    tracing::error!(%error, context, "MCP management authority check failed");
    McpManagementAuthorityError::Internal("Internal error".to_string())
}

pub struct Authority;

impl Authority {
    pub async fn validate_create_client(
        host: &dyn McpHostServices,
        tenant_id: Uuid,
        manager_permissions: &[Permission],
        actor_type: McpActorType,
        delegated_user_id: Option<Uuid>,
        granted_permissions: &[String],
    ) -> Result<(), McpManagementAuthorityError> {
        let delegated =
            delegated_permissions(host, tenant_id, actor_type, delegated_user_id).await?;
        validate_all_authorities(
            granted_permissions,
            manager_permissions,
            delegated.as_deref(),
        )
    }

    pub async fn validate_policy_update(
        db: &DatabaseConnection,
        host: &dyn McpHostServices,
        tenant_id: Uuid,
        manager_permissions: &[Permission],
        client_id: Uuid,
        granted_permissions: &[String],
    ) -> Result<(), McpManagementAuthorityError> {
        let client = require_client(db, tenant_id, client_id).await?;
        let delegated = delegated_permissions(
            host,
            tenant_id,
            client.actor_type(),
            client.delegated_user_id,
        )
        .await?;
        validate_all_authorities(
            granted_permissions,
            manager_permissions,
            delegated.as_deref(),
        )
    }

    /// Revalidate persisted delegated permissions when a token is used. A
    /// policy was constrained at creation/update time, but the delegated user
    /// may since have been disabled or had permissions removed.
    pub async fn validate_runtime_policy(
        host: &dyn McpHostServices,
        tenant_id: Uuid,
        client: &mcp_clients::Model,
        policy: Option<&mcp_policies::Model>,
    ) -> Result<(), McpManagementAuthorityError> {
        // INVARIANT: In-memory verification ensuring client and policy match the active tenant.
        if client.tenant_id != tenant_id
            || policy.is_some_and(|policy| {
                policy.tenant_id != tenant_id || policy.client_id != client.id
            })
        {
            return Err(McpManagementAuthorityError::Forbidden(
                "MCP policy does not match its tenant-bound client".to_string(),
            ));
        }

        let delegated = delegated_permissions(
            host,
            tenant_id,
            client.actor_type(),
            client.delegated_user_id,
        )
        .await?;
        if let (Some(policy), Some(delegated)) = (policy, delegated) {
            let granted_permissions = policy.granted_permissions_list().map_err(|error| {
                internal_error("decode persisted MCP granted permissions", error)
            })?;
            validate_grants(
                &granted_permissions,
                &delegated,
                "current delegated MCP user",
            )?;
        }
        Ok(())
    }

    pub async fn token_rotation_delegation(
        db: &DatabaseConnection,
        host: &dyn McpHostServices,
        tenant_id: Uuid,
        client_id: Uuid,
    ) -> Result<Option<Vec<Permission>>, McpManagementAuthorityError> {
        let client = require_client(db, tenant_id, client_id).await?;
        delegated_permissions(
            host,
            tenant_id,
            client.actor_type(),
            client.delegated_user_id,
        )
        .await
    }

    pub async fn validate_token_rotation(
        txn: &DatabaseTransaction,
        client: &mcp_clients::Model,
        manager_permissions: &[Permission],
        delegated_permissions: Option<&[Permission]>,
    ) -> Result<(), McpManagementAuthorityError> {
        let policy = mcp_policies::Entity::find_by_client_scoped(txn, client.tenant_id, client.id)
            .await
            .map_err(|error| internal_error("load MCP policy", error))?;
        if let Some(policy) = policy {
            let granted_permissions = policy.granted_permissions_list().map_err(|error| {
                internal_error("decode persisted MCP granted permissions", error)
            })?;
            validate_all_authorities(
                &granted_permissions,
                manager_permissions,
                delegated_permissions,
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
        .map_err(|error| internal_error("load MCP client", error))?
        .ok_or_else(|| McpManagementAuthorityError::NotFound("MCP client".to_string()))
}

async fn delegated_permissions(
    host: &dyn McpHostServices,
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

    match host
        .delegated_principal(tenant_id, user_id)
        .await
        .map_err(|error| internal_error("resolve delegated MCP principal", error))?
    {
        McpDelegatedPrincipal::Missing => Err(McpManagementAuthorityError::Invalid(
            "delegated MCP user does not exist in the current tenant".to_string(),
        )),
        McpDelegatedPrincipal::Inactive => Err(McpManagementAuthorityError::Invalid(
            "delegated MCP user must be active".to_string(),
        )),
        McpDelegatedPrincipal::Active(permissions) => Ok(Some(permissions)),
    }
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
    use super::validate_grants;
    use crate::McpManagementAuthorityError;
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

    #[test]
    fn runtime_revalidation_rejects_policy_grants_after_delegated_authority_is_removed() {
        let persisted_grants = vec![Permission::MODULES_MANAGE.to_string()];
        validate_grants(
            &persisted_grants,
            &[Permission::MODULES_MANAGE],
            "current delegated MCP user",
        )
        .expect("current delegated authority admits the stored policy");

        let error = validate_grants(
            &persisted_grants,
            &[Permission::MODULES_READ],
            "current delegated MCP user",
        )
        .expect_err("a later permission downgrade must invalidate the stored grant");
        assert!(matches!(error, McpManagementAuthorityError::Forbidden(_)));
    }
}
