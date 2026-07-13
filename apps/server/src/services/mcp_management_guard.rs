use std::sync::Arc;

use async_trait::async_trait;
use rustok_mcp::{
    ApplyMcpScaffoldDraftCommand, CreateMcpClientCommand, McpAuditEventRecord,
    McpClientDetailsRecord, McpClientRecord, McpManagementContext, McpManagementMutationError,
    McpManagementPort, McpPolicyRecord, McpScaffoldDraftRecord, McpTokenSecretResult,
    RotateMcpTokenCommand, StageMcpScaffoldDraftCommand, UpdateMcpPolicyCommand,
};
use uuid::Uuid;

use super::mcp_scaffold_workspace::authorize_mcp_scaffold_workspace;

pub struct GuardedMcpManagementProvider {
    inner: Arc<dyn McpManagementPort>,
}

impl GuardedMcpManagementProvider {
    pub fn new(inner: Arc<dyn McpManagementPort>) -> Self {
        Self { inner }
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