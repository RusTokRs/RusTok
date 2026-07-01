use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;

use rustok_mcp::{
    ApplyMcpScaffoldDraftCommand, CreateMcpClientCommand, McpClientMutationRecord,
    McpManagementMutationContext, McpManagementMutationError, McpManagementMutationPort,
    McpPolicyMutationRecord, McpScaffoldDraftMutationRecord, McpTokenSecretResult,
    RotateMcpTokenCommand, StageMcpScaffoldDraftCommand, UpdateMcpPolicyCommand,
};

use super::mcp_management::{
    ApplyMcpScaffoldDraftInput, CreateMcpClientInput, McpManagementService, RotateMcpTokenInput,
    StageMcpScaffoldDraftInput, UpdateMcpPolicyInput,
};

pub struct ServerMcpManagementMutationProvider {
    db: DatabaseConnection,
}

impl ServerMcpManagementMutationProvider {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }
}

#[async_trait]
impl McpManagementMutationPort for ServerMcpManagementMutationProvider {
    async fn create_client(
        &self,
        context: &McpManagementMutationContext,
        command: CreateMcpClientCommand,
    ) -> Result<McpTokenSecretResult, McpManagementMutationError> {
        let result = McpManagementService::create_client(
            &self.db,
            context.tenant_id,
            CreateMcpClientInput {
                slug: command.slug,
                display_name: command.display_name,
                description: command.description,
                actor_type: command.actor_type,
                delegated_user_id: None,
                token_name: command.token_name,
                token_expires_at: parse_optional_datetime(command.token_expires_at)?,
                allowed_tools: command.allowed_tools,
                denied_tools: command.denied_tools,
                granted_permissions: command.granted_permissions,
                granted_scopes: command.granted_scopes,
                metadata: serde_json::json!({}),
                created_by: Some(context.actor_id),
            },
        )
        .await
        .map_err(mutation_error)?;

        Ok(McpTokenSecretResult {
            client: client_record(&result.client),
            token_id: result.token.id,
            token_name: result.token.token_name,
            token_preview: result.token.token_preview,
            plaintext_token: result.plaintext_token,
        })
    }

    async fn rotate_token(
        &self,
        context: &McpManagementMutationContext,
        command: RotateMcpTokenCommand,
    ) -> Result<McpTokenSecretResult, McpManagementMutationError> {
        let result = McpManagementService::rotate_token(
            &self.db,
            context.tenant_id,
            command.client_id,
            RotateMcpTokenInput {
                token_name: command.token_name,
                expires_at: parse_optional_datetime(command.expires_at)?,
                metadata: serde_json::json!({}),
                created_by: Some(context.actor_id),
                revoke_existing_tokens: command.revoke_existing_tokens,
            },
        )
        .await
        .map_err(mutation_error)?;

        Ok(McpTokenSecretResult {
            client: client_record(&result.client),
            token_id: result.token.id,
            token_name: result.token.token_name,
            token_preview: result.token.token_preview,
            plaintext_token: result.plaintext_token,
        })
    }

    async fn update_policy(
        &self,
        context: &McpManagementMutationContext,
        command: UpdateMcpPolicyCommand,
    ) -> Result<McpPolicyMutationRecord, McpManagementMutationError> {
        let policy = McpManagementService::update_policy(
            &self.db,
            context.tenant_id,
            command.client_id,
            UpdateMcpPolicyInput {
                allowed_tools: command.allowed_tools,
                denied_tools: command.denied_tools,
                granted_permissions: command.granted_permissions,
                granted_scopes: command.granted_scopes,
                metadata: serde_json::json!({}),
                updated_by: Some(context.actor_id),
            },
        )
        .await
        .map_err(mutation_error)?;

        Ok(McpPolicyMutationRecord {
            client_id: policy.client_id,
            allowed_tools: policy.allowed_tools_list(),
            denied_tools: policy.denied_tools_list(),
            granted_permissions: policy.granted_permissions_list(),
            granted_scopes: policy.granted_scopes_list(),
        })
    }

    async fn revoke_token(
        &self,
        context: &McpManagementMutationContext,
        token_id: uuid::Uuid,
        reason: Option<String>,
    ) -> Result<(), McpManagementMutationError> {
        McpManagementService::revoke_token(
            &self.db,
            context.tenant_id,
            token_id,
            Some(context.actor_id),
            reason,
        )
        .await
        .map(|_| ())
        .map_err(mutation_error)
    }

    async fn deactivate_client(
        &self,
        context: &McpManagementMutationContext,
        client_id: uuid::Uuid,
        reason: Option<String>,
    ) -> Result<(), McpManagementMutationError> {
        McpManagementService::deactivate_client(
            &self.db,
            context.tenant_id,
            client_id,
            Some(context.actor_id),
            reason,
        )
        .await
        .map(|_| ())
        .map_err(mutation_error)
    }

    async fn stage_scaffold_draft(
        &self,
        context: &McpManagementMutationContext,
        command: StageMcpScaffoldDraftCommand,
    ) -> Result<McpScaffoldDraftMutationRecord, McpManagementMutationError> {
        McpManagementService::stage_scaffold_draft(
            &self.db,
            context.tenant_id,
            StageMcpScaffoldDraftInput {
                client_id: command.client_id,
                request: command.request,
                created_by: Some(context.actor_id),
            },
        )
        .await
        .map(scaffold_draft_record)
        .map_err(mutation_error)
    }

    async fn apply_scaffold_draft(
        &self,
        context: &McpManagementMutationContext,
        command: ApplyMcpScaffoldDraftCommand,
    ) -> Result<McpScaffoldDraftMutationRecord, McpManagementMutationError> {
        McpManagementService::apply_scaffold_draft(
            &self.db,
            context.tenant_id,
            command.draft_id,
            ApplyMcpScaffoldDraftInput {
                workspace_root: command.workspace_root,
                confirm: command.confirm,
                applied_by: Some(context.actor_id),
            },
        )
        .await
        .map(|(draft, _)| scaffold_draft_record(draft))
        .map_err(mutation_error)
    }
}

fn parse_optional_datetime(
    value: Option<String>,
) -> Result<Option<DateTime<Utc>>, McpManagementMutationError> {
    value
        .filter(|value| !value.trim().is_empty())
        .map(|value| {
            DateTime::parse_from_rfc3339(&value)
                .map(|value| value.with_timezone(&Utc))
                .map_err(|error| {
                    McpManagementMutationError::Validation(format!(
                        "invalid token expiry `{value}`: {error}"
                    ))
                })
        })
        .transpose()
}

fn client_record(client: &crate::models::mcp_clients::Model) -> McpClientMutationRecord {
    McpClientMutationRecord {
        id: client.id,
        slug: client.slug.clone(),
        display_name: client.display_name.clone(),
        actor_type: client.actor_type(),
        is_active: client.is_active(),
    }
}

fn scaffold_draft_record(
    draft: crate::models::mcp_scaffold_drafts::Model,
) -> McpScaffoldDraftMutationRecord {
    McpScaffoldDraftMutationRecord {
        id: draft.id,
        client_id: draft.client_id,
        slug: draft.slug,
        crate_name: draft.crate_name,
        status: draft.status,
        request_payload: draft.request_payload,
        preview_payload: draft.preview_payload,
        workspace_root: draft.workspace_root,
        applied_at: draft.applied_at.map(|value| value.to_rfc3339()),
        created_at: draft.created_at.to_rfc3339(),
        updated_at: draft.updated_at.to_rfc3339(),
    }
}

fn mutation_error(error: impl std::fmt::Display) -> McpManagementMutationError {
    McpManagementMutationError::Internal(error.to_string())
}
