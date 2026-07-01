use async_trait::async_trait;
use chrono::{DateTime, Utc};
use sea_orm::DatabaseConnection;

use rustok_mcp::{
    ApplyMcpScaffoldDraftCommand, CreateMcpClientCommand, McpAuditEventRecord,
    McpClientDetailsRecord, McpClientRecord, McpManagementContext, McpManagementMutationError,
    McpManagementPort, McpPolicyRecord, McpScaffoldDraftRecord, McpTokenRecord,
    McpTokenSecretResult, RotateMcpTokenCommand, StageMcpScaffoldDraftCommand,
    UpdateMcpPolicyCommand,
};

use super::mcp_management::{
    ApplyMcpScaffoldDraftInput, CreateMcpClientInput, McpAuditFilters, McpManagementService,
    RotateMcpTokenInput, StageMcpScaffoldDraftInput, UpdateMcpPolicyInput,
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
impl McpManagementPort for ServerMcpManagementMutationProvider {
    async fn list_clients(
        &self,
        context: &McpManagementContext,
        limit: Option<u64>,
    ) -> Result<Vec<McpClientRecord>, McpManagementMutationError> {
        McpManagementService::list_clients(&self.db, context.tenant_id, limit)
            .await
            .map(|clients| clients.iter().map(client_record).collect())
            .map_err(mutation_error)
    }

    async fn get_client(
        &self,
        context: &McpManagementContext,
        client_id: uuid::Uuid,
    ) -> Result<Option<McpClientDetailsRecord>, McpManagementMutationError> {
        McpManagementService::get_client_details(&self.db, context.tenant_id, client_id)
            .await
            .map(|details| {
                details.map(|details| McpClientDetailsRecord {
                    client: client_record(&details.client),
                    policy: details.policy.as_ref().map(policy_record),
                    tokens: details.tokens.iter().map(token_record).collect(),
                    effective_access_context: details.effective_access_context,
                })
            })
            .map_err(mutation_error)
    }

    async fn list_audit_events(
        &self,
        context: &McpManagementContext,
        client_id: Option<uuid::Uuid>,
        outcome: Option<String>,
        limit: Option<u64>,
    ) -> Result<Vec<McpAuditEventRecord>, McpManagementMutationError> {
        McpManagementService::list_audit_events(
            &self.db,
            context.tenant_id,
            McpAuditFilters {
                client_id,
                outcome,
                limit,
            },
        )
        .await
        .map(|events| events.iter().map(audit_record).collect())
        .map_err(mutation_error)
    }

    async fn list_scaffold_drafts(
        &self,
        context: &McpManagementContext,
        limit: Option<u64>,
    ) -> Result<Vec<McpScaffoldDraftRecord>, McpManagementMutationError> {
        McpManagementService::list_scaffold_drafts(&self.db, context.tenant_id, limit)
            .await
            .map(|drafts| drafts.into_iter().map(scaffold_draft_record).collect())
            .map_err(mutation_error)
    }

    async fn get_scaffold_draft(
        &self,
        context: &McpManagementContext,
        draft_id: uuid::Uuid,
    ) -> Result<Option<McpScaffoldDraftRecord>, McpManagementMutationError> {
        McpManagementService::get_scaffold_draft(&self.db, context.tenant_id, draft_id)
            .await
            .map(|draft| draft.map(scaffold_draft_record))
            .map_err(mutation_error)
    }

    async fn create_client(
        &self,
        context: &McpManagementContext,
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
                delegated_user_id: command.delegated_user_id,
                token_name: command.token_name,
                token_expires_at: parse_optional_datetime(command.token_expires_at)?,
                allowed_tools: command.allowed_tools,
                denied_tools: command.denied_tools,
                granted_permissions: command.granted_permissions,
                granted_scopes: command.granted_scopes,
                metadata: command.metadata,
                created_by: Some(context.actor_id),
            },
        )
        .await
        .map_err(mutation_error)?;

        Ok(McpTokenSecretResult {
            client: client_record(&result.client),
            policy: Some(policy_record(&result.policy)),
            token: token_record(&result.token),
            plaintext_token: result.plaintext_token,
        })
    }

    async fn rotate_token(
        &self,
        context: &McpManagementContext,
        command: RotateMcpTokenCommand,
    ) -> Result<McpTokenSecretResult, McpManagementMutationError> {
        let result = McpManagementService::rotate_token(
            &self.db,
            context.tenant_id,
            command.client_id,
            RotateMcpTokenInput {
                token_name: command.token_name,
                expires_at: parse_optional_datetime(command.expires_at)?,
                metadata: command.metadata,
                created_by: Some(context.actor_id),
                revoke_existing_tokens: command.revoke_existing_tokens,
            },
        )
        .await
        .map_err(mutation_error)?;

        Ok(McpTokenSecretResult {
            client: client_record(&result.client),
            policy: None,
            token: token_record(&result.token),
            plaintext_token: result.plaintext_token,
        })
    }

    async fn update_policy(
        &self,
        context: &McpManagementContext,
        command: UpdateMcpPolicyCommand,
    ) -> Result<McpPolicyRecord, McpManagementMutationError> {
        let policy = McpManagementService::update_policy(
            &self.db,
            context.tenant_id,
            command.client_id,
            UpdateMcpPolicyInput {
                allowed_tools: command.allowed_tools,
                denied_tools: command.denied_tools,
                granted_permissions: command.granted_permissions,
                granted_scopes: command.granted_scopes,
                metadata: command.metadata,
                updated_by: Some(context.actor_id),
            },
        )
        .await
        .map_err(mutation_error)?;

        Ok(policy_record(&policy))
    }

    async fn revoke_token(
        &self,
        context: &McpManagementContext,
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
        context: &McpManagementContext,
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
        context: &McpManagementContext,
        command: StageMcpScaffoldDraftCommand,
    ) -> Result<McpScaffoldDraftRecord, McpManagementMutationError> {
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
        context: &McpManagementContext,
        command: ApplyMcpScaffoldDraftCommand,
    ) -> Result<McpScaffoldDraftRecord, McpManagementMutationError> {
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

fn client_record(client: &crate::models::mcp_clients::Model) -> McpClientRecord {
    McpClientRecord {
        id: client.id,
        tenant_id: client.tenant_id,
        client_key: client.client_key,
        slug: client.slug.clone(),
        display_name: client.display_name.clone(),
        description: client.description.clone(),
        actor_type: client.actor_type(),
        delegated_user_id: client.delegated_user_id,
        is_active: client.is_active(),
        revoked_at: client.revoked_at.map(Into::into),
        last_used_at: client.last_used_at.map(Into::into),
        metadata: client.metadata.clone(),
        created_by: client.created_by,
        created_at: client.created_at.into(),
        updated_at: client.updated_at.into(),
    }
}

fn policy_record(policy: &crate::models::mcp_policies::Model) -> McpPolicyRecord {
    McpPolicyRecord {
        id: policy.id,
        client_id: policy.client_id,
        allowed_tools: policy.allowed_tools_list(),
        denied_tools: policy.denied_tools_list(),
        granted_permissions: policy.granted_permissions_list(),
        granted_scopes: policy.granted_scopes_list(),
        metadata: policy.metadata.clone(),
        updated_by: policy.updated_by,
        created_at: policy.created_at.into(),
        updated_at: policy.updated_at.into(),
    }
}

fn token_record(token: &crate::models::mcp_tokens::Model) -> McpTokenRecord {
    McpTokenRecord {
        id: token.id,
        client_id: token.client_id,
        token_name: token.token_name.clone(),
        token_preview: token.token_preview.clone(),
        is_active: token.is_active(),
        last_used_at: token.last_used_at.map(Into::into),
        expires_at: token.expires_at.map(Into::into),
        revoked_at: token.revoked_at.map(Into::into),
        metadata: token.metadata.clone(),
        created_at: token.created_at.into(),
    }
}

fn audit_record(event: &crate::models::mcp_audit_logs::Model) -> McpAuditEventRecord {
    McpAuditEventRecord {
        id: event.id,
        client_id: event.client_id,
        token_id: event.token_id,
        actor_id: event.actor_id.clone(),
        actor_type: event.actor_type.clone(),
        action: event.action.clone(),
        outcome: event.outcome.clone(),
        tool_name: event.tool_name.clone(),
        reason: event.reason.clone(),
        correlation_id: event.correlation_id.clone(),
        metadata: event.metadata.clone(),
        created_by: event.created_by,
        created_at: event.created_at.into(),
    }
}

fn scaffold_draft_record(
    draft: crate::models::mcp_scaffold_drafts::Model,
) -> McpScaffoldDraftRecord {
    McpScaffoldDraftRecord {
        id: draft.id,
        client_id: draft.client_id,
        slug: draft.slug,
        crate_name: draft.crate_name,
        status: draft.status,
        request_payload: draft.request_payload,
        preview_payload: draft.preview_payload,
        workspace_root: draft.workspace_root,
        applied_at: draft.applied_at.map(Into::into),
        created_by: draft.created_by,
        created_at: draft.created_at.into(),
        updated_at: draft.updated_at.into(),
    }
}

fn mutation_error(error: impl std::fmt::Display) -> McpManagementMutationError {
    McpManagementMutationError::Internal(error.to_string())
}
