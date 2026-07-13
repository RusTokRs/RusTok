use std::sync::Arc;

use async_trait::async_trait;
use rustok_mcp::{
    ApplyMcpScaffoldDraftCommand, CreateMcpClientCommand, McpActorType, McpAuditEventRecord,
    McpClientDetailsRecord, McpClientRecord, McpManagementContext, McpManagementMutationError,
    McpManagementPort, McpPolicyRecord, McpScaffoldDraftRecord, McpTokenSecretResult,
    RotateMcpTokenCommand, StageMcpScaffoldDraftCommand, UpdateMcpPolicyCommand,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::models::users;

use super::mcp_scaffold_workspace::authorize_mcp_scaffold_workspace;

pub struct GuardedMcpManagementProvider {
    db: DatabaseConnection,
    inner: Arc<dyn McpManagementPort>,
}

impl GuardedMcpManagementProvider {
    pub fn new(db: DatabaseConnection, inner: Arc<dyn McpManagementPort>) -> Self {
        Self { db, inner }
    }

    async fn validate_delegated_identity(
        &self,
        context: &McpManagementContext,
        command: &CreateMcpClientCommand,
    ) -> Result<(), McpManagementMutationError> {
        if command.actor_type == McpActorType::HumanUser && command.delegated_user_id.is_none() {
            return Err(McpManagementMutationError::Validation(
                "human_user MCP clients require delegated_user_id".to_string(),
            ));
        }

        let Some(delegated_user_id) = command.delegated_user_id else {
            return Ok(());
        };
        let user = users::Entity::find_by_id(delegated_user_id)
            .filter(users::Column::TenantId.eq(context.tenant_id))
            .one(&self.db)
            .await
            .map_err(|error| McpManagementMutationError::Internal(error.to_string()))?
            .ok_or_else(|| {
                McpManagementMutationError::Validation(
                    "delegated MCP user does not exist in the current tenant".to_string(),
                )
            })?;

        if !user.is_active() {
            return Err(McpManagementMutationError::Validation(
                "delegated MCP user must be active".to_string(),
            ));
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
        self.validate_delegated_identity(context, &command).await?;
        self.inner.create_client(context, command).await
    }

    async fn rotate_token(
        &self,
        context: &McpManagementContext,
        command: RotateMcpTokenCommand,
    ) -> Result<McpTokenSecretResult, McpManagementMutationError> {
        self.inner.rotate_token(context, command).await
    }

    async fn update_policy(
        &self,
        context: &McpManagementContext,
        command: UpdateMcpPolicyCommand,
    ) -> Result<McpPolicyRecord, McpManagementMutationError> {
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