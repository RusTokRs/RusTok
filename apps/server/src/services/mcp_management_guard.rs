use std::str::FromStr;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{has_effective_permission, Permission};
use rustok_mcp::{
    ApplyMcpScaffoldDraftCommand, CreateMcpClientCommand, McpActorType, McpAuditEventRecord,
    McpClientDetailsRecord, McpClientRecord, McpManagementContext, McpManagementMutationError,
    McpManagementPort, McpPolicyRecord, McpScaffoldDraftRecord, McpTokenSecretResult,
    RotateMcpTokenCommand, StageMcpScaffoldDraftCommand, UpdateMcpPolicyCommand,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::models::{mcp_clients, mcp_policies, users};

use super::mcp_scaffold_workspace::authorize_mcp_scaffold_workspace;
use super::rbac_service::RbacService;

pub struct GuardedMcpManagementProvider {
    db: DatabaseConnection,
    inner: Arc<dyn McpManagementPort>,
}

impl GuardedMcpManagementProvider {
    pub fn new(db: DatabaseConnection, inner: Arc<dyn McpManagementPort>) -> Self {
        Self { db, inner }
    }

    async fn authoritative_permissions(
        &self,
        context: &McpManagementContext,
        user_id: Uuid,
        principal: &'static str,
    ) -> Result<Vec<Permission>, McpManagementMutationError> {
        let user = users::Entity::find_by_id(user_id)
            .filter(users::Column::TenantId.eq(context.tenant_id))
            .one(&self.db)
            .await
            .map_err(|error| McpManagementMutationError::Internal(error.to_string()))?
            .ok_or_else(|| {
                McpManagementMutationError::Validation(format!(
                    "{principal} does not exist in the current tenant"
                ))
            })?;

        if !user.is_active() {
            return Err(McpManagementMutationError::Validation(format!(
                "{principal} must be active"
            )));
        }

        RbacService::get_user_permissions_authoritative(
            &self.db,
            &context.tenant_id,
            &user_id,
        )
        .await
        .map_err(|error| McpManagementMutationError::Internal(error.to_string()))
    }

    async fn manager_permissions(
        &self,
        context: &McpManagementContext,
    ) -> Result<Vec<Permission>, McpManagementMutationError> {
        self.authoritative_permissions(context, context.actor_id, "MCP manager")
            .await
    }

    async fn delegated_permissions(
        &self,
        context: &McpManagementContext,
        actor_type: McpActorType,
        delegated_user_id: Option<Uuid>,
    ) -> Result<Option<Vec<Permission>>, McpManagementMutationError> {
        if actor_type == McpActorType::HumanUser && delegated_user_id.is_none() {
            return Err(McpManagementMutationError::Validation(
                "human_user MCP clients require delegated_user_id".to_string(),
            ));
        }

        let Some(delegated_user_id) = delegated_user_id else {
            return Ok(None);
        };

        self.authoritative_permissions(context, delegated_user_id, "delegated MCP user")
            .await
            .map(Some)
    }

    async fn require_client(
        &self,
        context: &McpManagementContext,
        client_id: Uuid,
    ) -> Result<mcp_clients::Model, McpManagementMutationError> {
        mcp_clients::Entity::find_by_id(client_id)
            .filter(mcp_clients::Column::TenantId.eq(context.tenant_id))
            .one(&self.db)
            .await
            .map_err(|error| McpManagementMutationError::Internal(error.to_string()))?
            .ok_or_else(|| McpManagementMutationError::NotFound("mcp client".to_string()))
    }

    fn validate_policy_grants(
        &self,
        requested: &[String],
        authority: &[Permission],
        principal: &str,
    ) -> Result<(), McpManagementMutationError> {
        for raw in requested {
            let permission = Permission::from_str(raw.trim()).map_err(|error| {
                McpManagementMutationError::Validation(format!(
                    "invalid MCP granted permission `{raw}`: {error}"
                ))
            })?;
            if !has_effective_permission(authority, &permission) {
                return Err(McpManagementMutationError::Validation(format!(
                    "MCP permission `{permission}` exceeds {principal} authority"
                )));
            }
        }

        Ok(())
    }

    fn validate_all_authorities(
        &self,
        requested: &[String],
        manager_permissions: &[Permission],
        delegated_permissions: Option<&[Permission]>,
    ) -> Result<(), McpManagementMutationError> {
        self.validate_policy_grants(requested, manager_permissions, "MCP manager")?;
        if let Some(delegated_permissions) = delegated_permissions {
            self.validate_policy_grants(
                requested,
                delegated_permissions,
                "delegated MCP user",
            )?;
        }
        Ok(())
    }

    async fn validate_existing_client_policy(
        &self,
        context: &McpManagementContext,
        client: &mcp_clients::Model,
    ) -> Result<(), McpManagementMutationError> {
        let manager_permissions = self.manager_permissions(context).await?;
        let delegated_permissions = self
            .delegated_permissions(context, client.actor_type(), client.delegated_user_id)
            .await?;
        let policy = mcp_policies::Entity::find_by_client(&self.db, client.id)
            .await
            .map_err(|error| McpManagementMutationError::Internal(error.to_string()))?;

        if let Some(policy) = policy {
            self.validate_all_authorities(
                &policy.granted_permissions_list(),
                &manager_permissions,
                delegated_permissions.as_deref(),
            )?;
        }
        Ok(())
    }
}

#[async_trait]
impl McpManagementPort for GuardedMcpManagementProvider {
    async fn list_clients(
        &self,
        context: &McpManagementContext,
        limit: Option<u64>,
    ) -> Result<Vec<McpClientRecord>, McpManagementMutationError> {
        self.inner.list_clients(context, limit).await
    }

    async fn get_client(
        &self,
        context: &McpManagementContext,
        client_id: Uuid,
    ) -> Result<Option<McpClientDetailsRecord>, McpManagementMutationError> {
        self.inner.get_client(context, client_id).await
    }

    async fn list_audit_events(
        &self,
        context: &McpManagementContext,
        client_id: Option<Uuid>,
        outcome: Option<String>,
        limit: Option<u64>,
    ) -> Result<Vec<McpAuditEventRecord>, McpManagementMutationError> {
        self.inner
            .list_audit_events(context, client_id, outcome, limit)
            .await
    }

    async fn list_scaffold_drafts(
        &self,
        context: &McpManagementContext,
        limit: Option<u64>,
    ) -> Result<Vec<McpScaffoldDraftRecord>, McpManagementMutationError> {
        self.inner.list_scaffold_drafts(context, limit).await
    }

    async fn get_scaffold_draft(
        &self,
        context: &McpManagementContext,
        draft_id: Uuid,
    ) -> Result<Option<McpScaffoldDraftRecord>, McpManagementMutationError> {
        self.inner.get_scaffold_draft(context, draft_id).await
    }

    async fn create_client(
        &self,
        context: &McpManagementContext,
        command: CreateMcpClientCommand,
    ) -> Result<McpTokenSecretResult, McpManagementMutationError> {
        let manager_permissions = self.manager_permissions(context).await?;
        let delegated_permissions = self
            .delegated_permissions(context, command.actor_type, command.delegated_user_id)
            .await?;
        self.validate_all_authorities(
            &command.granted_permissions,
            &manager_permissions,
            delegated_permissions.as_deref(),
        )?;
        self.inner.create_client(context, command).await
    }

    async fn rotate_token(
        &self,
        context: &McpManagementContext,
        command: RotateMcpTokenCommand,
    ) -> Result<McpTokenSecretResult, McpManagementMutationError> {
        let client = self.require_client(context, command.client_id).await?;
        self.validate_existing_client_policy(context, &client).await?;
        self.inner.rotate_token(context, command).await
    }

    async fn update_policy(
        &self,
        context: &McpManagementContext,
        command: UpdateMcpPolicyCommand,
    ) -> Result<McpPolicyRecord, McpManagementMutationError> {
        let client = self.require_client(context, command.client_id).await?;
        let manager_permissions = self.manager_permissions(context).await?;
        let delegated_permissions = self
            .delegated_permissions(context, client.actor_type(), client.delegated_user_id)
            .await?;
        self.validate_all_authorities(
            &command.granted_permissions,
            &manager_permissions,
            delegated_permissions.as_deref(),
        )?;
        self.inner.update_policy(context, command).await
    }

    async fn revoke_token(
        &self,
        context: &McpManagementContext,
        token_id: Uuid,
        reason: Option<String>,
    ) -> Result<(), McpManagementMutationError> {
        self.inner.revoke_token(context, token_id, reason).await
    }

    async fn deactivate_client(
        &self,
        context: &McpManagementContext,
        client_id: Uuid,
        reason: Option<String>,
    ) -> Result<(), McpManagementMutationError> {
        self.inner.deactivate_client(context, client_id, reason).await
    }

    async fn stage_scaffold_draft(
        &self,
        context: &McpManagementContext,
        command: StageMcpScaffoldDraftCommand,
    ) -> Result<McpScaffoldDraftRecord, McpManagementMutationError> {
        self.inner.stage_scaffold_draft(context, command).await
    }

    async fn apply_scaffold_draft(
        &self,
        context: &McpManagementContext,
        mut command: ApplyMcpScaffoldDraftCommand,
    ) -> Result<McpScaffoldDraftRecord, McpManagementMutationError> {
        command.workspace_root = authorize_mcp_scaffold_workspace(&command.workspace_root)
            .map_err(|error| McpManagementMutationError::Validation(error.to_string()))?;
        self.inner.apply_scaffold_draft(context, command).await
    }
}
