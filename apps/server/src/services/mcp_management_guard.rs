use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::Permission;
use rustok_mcp::{
    ApplyMcpScaffoldDraftCommand, CreateMcpClientCommand, McpAuditEventRecord,
    McpClientDetailsRecord, McpClientRecord, McpManagementContext, McpManagementMutationError,
    McpManagementPort, McpPolicyRecord, McpScaffoldDraftRecord, McpTokenSecretResult,
    RotateMcpTokenCommand, StageMcpScaffoldDraftCommand, UpdateMcpPolicyCommand,
};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use super::mcp_management_authority::{
    McpManagementAuthorityError, McpManagementAuthorityService,
};
use super::mcp_scaffold_workspace::authorize_mcp_scaffold_workspace;
use super::rbac_request_scope::permissions_for;

pub struct GuardedMcpManagementProvider {
    db: DatabaseConnection,
    inner: Arc<dyn McpManagementPort>,
}

impl GuardedMcpManagementProvider {
    pub fn new(db: DatabaseConnection, inner: Arc<dyn McpManagementPort>) -> Self {
        Self { db, inner }
    }

    fn manager_permissions(
        &self,
        context: &McpManagementContext,
    ) -> Result<Vec<Permission>, McpManagementMutationError> {
        permissions_for(&context.tenant_id, &context.actor_id).ok_or_else(|| {
            McpManagementMutationError::Validation(
                "MCP management requires a request-bound effective permission snapshot"
                    .to_string(),
            )
        })
    }
}

fn map_authority_error(error: McpManagementAuthorityError) -> McpManagementMutationError {
    match error {
        McpManagementAuthorityError::Invalid(message)
        | McpManagementAuthorityError::Forbidden(message) => {
            McpManagementMutationError::Validation(message)
        }
        McpManagementAuthorityError::NotFound(message) => {
            McpManagementMutationError::NotFound(message)
        }
        McpManagementAuthorityError::Internal(message) => {
            McpManagementMutationError::Internal(message)
        }
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
        let manager_permissions = self.manager_permissions(context)?;
        McpManagementAuthorityService::validate_create_client(
            &self.db,
            context.tenant_id,
            &manager_permissions,
            command.actor_type,
            command.delegated_user_id,
            &command.granted_permissions,
        )
        .await
        .map_err(map_authority_error)?;
        self.inner.create_client(context, command).await
    }

    async fn rotate_token(
        &self,
        context: &McpManagementContext,
        command: RotateMcpTokenCommand,
    ) -> Result<McpTokenSecretResult, McpManagementMutationError> {
        let manager_permissions = self.manager_permissions(context)?;
        McpManagementAuthorityService::validate_token_rotation(
            &self.db,
            context.tenant_id,
            &manager_permissions,
            command.client_id,
        )
        .await
        .map_err(map_authority_error)?;
        self.inner.rotate_token(context, command).await
    }

    async fn update_policy(
        &self,
        context: &McpManagementContext,
        command: UpdateMcpPolicyCommand,
    ) -> Result<McpPolicyRecord, McpManagementMutationError> {
        let manager_permissions = self.manager_permissions(context)?;
        McpManagementAuthorityService::validate_policy_update(
            &self.db,
            context.tenant_id,
            &manager_permissions,
            command.client_id,
            &command.granted_permissions,
        )
        .await
        .map_err(map_authority_error)?;
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
