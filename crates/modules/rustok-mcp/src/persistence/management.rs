use async_trait::async_trait;
use chrono::{DateTime, Utc};
use rustok_api::{Permission, has_effective_permission};

use super::authority::Authority;
use super::{McpPersistence, McpPersistenceError};

use crate::{
    ApplyMcpScaffoldDraftCommand, CreateMcpClientCommand, McpAuditEventRecord,
    McpClientDetailsRecord, McpClientRecord, McpManagementContext, McpManagementMutationError,
    McpManagementPort, McpPolicyRecord, McpScaffoldDraftRecord, McpTokenRecord,
    McpTokenSecretResult, RotateMcpTokenCommand, StageMcpScaffoldDraftCommand,
    UpdateMcpPolicyCommand,
};

use super::service::{
    ApplyMcpScaffoldDraftInput, CreateMcpClientInput, McpAuditFilters, McpManagementService,
    RotateMcpTokenInput, StageMcpScaffoldDraftInput, UpdateMcpPolicyInput,
};

impl McpPersistence {
    fn manager_permissions(
        &self,
        context: &McpManagementContext,
    ) -> Result<Vec<Permission>, McpManagementMutationError> {
        let permissions = match context.request_actor_id {
            Some(actor_id) => self
                .host
                .request_permissions(context.tenant_id, actor_id)
                .ok_or_else(|| {
                    McpManagementMutationError::Validation(
                        "MCP management requires a request-bound effective permission snapshot"
                            .to_string(),
                    )
                })?,
            None => context.permissions.clone(),
        };
        ensure_permission(context, &permissions, Permission::MCP_MANAGE)?;
        Ok(permissions)
    }
}

fn ensure_permission(
    context: &McpManagementContext,
    permissions: &[Permission],
    required: Permission,
) -> Result<(), McpManagementMutationError> {
    if context.tenant_id.is_nil() || context.actor_id.is_nil() {
        return Err(McpManagementMutationError::Forbidden(
            "MCP management requires a tenant-bound principal".to_string(),
        ));
    }
    if !has_effective_permission(permissions, &required) {
        return Err(McpManagementMutationError::Forbidden(format!(
            "{required} permission is required"
        )));
    }
    Ok(())
}

#[async_trait]
impl McpManagementPort for McpPersistence {
    async fn list_clients(
        &self,
        context: &McpManagementContext,
        limit: Option<u64>,
    ) -> Result<Vec<McpClientRecord>, McpManagementMutationError> {
        ensure_permission(context, &context.permissions, Permission::MCP_READ)?;
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
        ensure_permission(context, &context.permissions, Permission::MCP_READ)?;
        let details =
            McpManagementService::get_client_details(&self.db, context.tenant_id, client_id)
                .await
                .map_err(mutation_error)?;
        details
            .map(|details| {
                Ok(McpClientDetailsRecord {
                    client: client_record(&details.client),
                    policy: details.policy.as_ref().map(policy_record).transpose()?,
                    tokens: details.tokens.iter().map(token_record).collect(),
                    effective_access_context: details.effective_access_context,
                })
            })
            .transpose()
    }

    async fn list_audit_events(
        &self,
        context: &McpManagementContext,
        client_id: Option<uuid::Uuid>,
        outcome: Option<String>,
        limit: Option<u64>,
    ) -> Result<Vec<McpAuditEventRecord>, McpManagementMutationError> {
        ensure_permission(context, &context.permissions, Permission::MCP_READ)?;
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
        ensure_permission(context, &context.permissions, Permission::MCP_MANAGE)?;
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
        ensure_permission(context, &context.permissions, Permission::MCP_MANAGE)?;
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
        let permissions = self.manager_permissions(context)?;
        Authority::validate_create_client(
            self.host.as_ref(),
            context.tenant_id,
            &permissions,
            command.actor_type,
            command.delegated_user_id,
            &command.granted_permissions,
        )
        .await?;
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
                created_by: context.human_actor_id(),
                audit_actor_id: Some(context.actor_id.to_string()),
                audit_actor_type: Some(context.actor_type.as_str().to_string()),
            },
        )
        .await
        .map_err(mutation_error)?;

        Ok(McpTokenSecretResult {
            client: client_record(&result.client),
            policy: Some(policy_record(&result.policy)?),
            token: token_record(&result.token),
            plaintext_token: result.plaintext_token,
        })
    }

    async fn rotate_token(
        &self,
        context: &McpManagementContext,
        command: RotateMcpTokenCommand,
    ) -> Result<McpTokenSecretResult, McpManagementMutationError> {
        let permissions = self.manager_permissions(context)?;
        let delegated = Authority::token_rotation_delegation(
            &self.db,
            self.host.as_ref(),
            context.tenant_id,
            command.client_id,
        )
        .await?;
        let result = McpManagementService::rotate_token(
            &self.db,
            context.tenant_id,
            command.client_id,
            RotateMcpTokenInput {
                token_name: command.token_name,
                expires_at: parse_optional_datetime(command.expires_at)?,
                metadata: command.metadata,
                created_by: context.human_actor_id(),
                audit_actor_id: Some(context.actor_id.to_string()),
                audit_actor_type: Some(context.actor_type.as_str().to_string()),
                revoke_existing_tokens: command.revoke_existing_tokens,
            },
            &permissions,
            delegated.as_deref(),
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
        let permissions = self.manager_permissions(context)?;
        Authority::validate_policy_update(
            &self.db,
            self.host.as_ref(),
            context.tenant_id,
            &permissions,
            command.client_id,
            &command.granted_permissions,
        )
        .await?;
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
                updated_by: context.human_actor_id(),
                audit_actor_id: Some(context.actor_id.to_string()),
                audit_actor_type: Some(context.actor_type.as_str().to_string()),
            },
        )
        .await
        .map_err(mutation_error)?;

        policy_record(&policy)
    }

    async fn revoke_token(
        &self,
        context: &McpManagementContext,
        token_id: uuid::Uuid,
        reason: Option<String>,
    ) -> Result<(), McpManagementMutationError> {
        ensure_permission(context, &context.permissions, Permission::MCP_MANAGE)?;
        McpManagementService::revoke_token(
            &self.db,
            context.tenant_id,
            token_id,
            context.human_actor_id(),
            Some(context.actor_id.to_string()),
            Some(context.actor_type.as_str().to_string()),
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
        ensure_permission(context, &context.permissions, Permission::MCP_MANAGE)?;
        McpManagementService::deactivate_client(
            &self.db,
            context.tenant_id,
            client_id,
            context.human_actor_id(),
            Some(context.actor_id.to_string()),
            Some(context.actor_type.as_str().to_string()),
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
        ensure_permission(context, &context.permissions, Permission::MCP_MANAGE)?;
        McpManagementService::stage_scaffold_draft(
            &self.db,
            context.tenant_id,
            StageMcpScaffoldDraftInput {
                client_id: command.client_id,
                request: command.request,
                created_by: context.human_actor_id(),
                actor_id: Some(context.actor_id.to_string()),
                actor_type: Some(context.actor_type.as_str().to_string()),
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
        ensure_permission(context, &context.permissions, Permission::MCP_MANAGE)?;
        McpManagementService::apply_scaffold_draft(
            &self.db,
            self.host.as_ref(),
            context.tenant_id,
            command.draft_id,
            ApplyMcpScaffoldDraftInput {
                workspace_root: command.workspace_root,
                confirm: command.confirm,
                expected_client_id: None,
                applied_by: context.human_actor_id(),
                actor_id: Some(context.actor_id.to_string()),
                actor_type: Some(context.actor_type.as_str().to_string()),
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

fn client_record(client: &super::models::mcp_clients::Model) -> McpClientRecord {
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

fn policy_record(
    policy: &super::models::mcp_policies::Model,
) -> Result<McpPolicyRecord, McpManagementMutationError> {
    let decoded = (|| {
        Ok::<_, String>((
            policy.allowed_tools_list()?,
            policy.denied_tools_list()?,
            policy.granted_permissions_list()?,
            policy.granted_scopes_list()?,
        ))
    })()
    .map_err(|error| {
        tracing::error!(%error, client_id = %policy.client_id, "Persisted MCP policy is invalid");
        mutation_error(McpPersistenceError::InternalServerError)
    })?;
    Ok(McpPolicyRecord {
        id: policy.id,
        client_id: policy.client_id,
        allowed_tools: decoded.0,
        denied_tools: decoded.1,
        granted_permissions: decoded.2,
        granted_scopes: decoded.3,
        metadata: policy.metadata.clone(),
        updated_by: policy.updated_by,
        created_at: policy.created_at.into(),
        updated_at: policy.updated_at.into(),
    })
}

fn token_record(token: &super::models::mcp_tokens::Model) -> McpTokenRecord {
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

fn audit_record(event: &super::models::mcp_audit_logs::Model) -> McpAuditEventRecord {
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
    draft: super::models::mcp_scaffold_drafts::Model,
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

fn mutation_error(error: McpPersistenceError) -> McpManagementMutationError {
    match error {
        McpPersistenceError::Authority(error) => error.into(),
        McpPersistenceError::Forbidden(message) => McpManagementMutationError::Forbidden(message),
        McpPersistenceError::BadRequest(message) => McpManagementMutationError::Validation(message),
        McpPersistenceError::NotFound => {
            McpManagementMutationError::NotFound("MCP management resource not found".to_string())
        }
        McpPersistenceError::InternalServerError => {
            McpManagementMutationError::Internal("MCP management operation failed".to_string())
        }
        McpPersistenceError::Message(message) => McpManagementMutationError::Internal(message),
        McpPersistenceError::Unauthorized(message) => {
            McpManagementMutationError::Forbidden(message)
        }
    }
}

#[cfg(test)]
mod provider_error_tests {
    use super::McpPersistenceError as Error;
    use super::mutation_error;
    use crate::McpManagementMutationError;

    #[test]
    fn maps_forbidden_errors_to_permission_denied_variant() {
        assert!(matches!(
            mutation_error(Error::Forbidden("denied".to_string())),
            McpManagementMutationError::Forbidden(message) if message == "denied"
        ));
    }

    #[test]
    fn maps_user_errors_without_exposing_internal_details() {
        assert!(matches!(
            mutation_error(Error::BadRequest("bad input".to_string())),
            McpManagementMutationError::Validation(message) if message == "bad input"
        ));
        assert!(matches!(
            mutation_error(Error::NotFound),
            McpManagementMutationError::NotFound(message) if message == "MCP management resource not found"
        ));
        assert!(matches!(
            mutation_error(Error::InternalServerError),
            McpManagementMutationError::Internal(message) if message == "MCP management operation failed"
        ));
    }
}
