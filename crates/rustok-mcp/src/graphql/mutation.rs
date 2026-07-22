use async_graphql::{Context, FieldError, Object, Result};
use rustok_api::{Permission, graphql::GraphQLError};
use uuid::Uuid;

use crate::{
    ApplyMcpScaffoldDraftCommand, CreateMcpClientCommand, RotateMcpTokenCommand,
    ScaffoldModuleRequest, StageMcpScaffoldDraftCommand, UpdateMcpPolicyCommand,
};

use super::{
    ApplyMcpModuleScaffoldDraftInput, CreateMcpClientInput, CreateMcpClientResultGql,
    McpModuleScaffoldDraftGql, McpPolicyGql, RotateMcpTokenInput, RotateMcpTokenResultGql,
    StageMcpModuleScaffoldDraftInput, UpdateMcpPolicyInput, ensure_permission, management_context,
    map_error, parse_metadata, require_auth_context, runtime,
};

#[derive(Default)]
pub struct McpMutation;

#[Object]
impl McpMutation {
    async fn create_mcp_client(
        &self,
        ctx: &Context<'_>,
        input: CreateMcpClientInput,
    ) -> Result<CreateMcpClientResultGql> {
        let auth = managed_auth(ctx)?;
        let result = runtime(ctx)?
            .port()
            .create_client(
                &management_context(auth),
                CreateMcpClientCommand {
                    slug: input.slug,
                    display_name: input.display_name,
                    description: input.description,
                    actor_type: input.actor_type.to_runtime(),
                    delegated_user_id: input.delegated_user_id,
                    token_name: input.token_name,
                    token_expires_at: input.token_expires_at.map(|value| value.to_rfc3339()),
                    allowed_tools: input.allowed_tools,
                    denied_tools: input.denied_tools,
                    granted_permissions: input.granted_permissions,
                    granted_scopes: input.granted_scopes,
                    metadata: parse_metadata(input.metadata)?,
                },
            )
            .await
            .map_err(map_error)?;
        let policy = result.policy.ok_or_else(|| {
            <FieldError as GraphQLError>::internal_error(
                "MCP create client result is missing policy",
            )
        })?;
        Ok(CreateMcpClientResultGql {
            client: result.client.into(),
            policy: policy.into(),
            token: result.token.into(),
            plaintext_token: result.plaintext_token,
        })
    }

    async fn rotate_mcp_client_token(
        &self,
        ctx: &Context<'_>,
        client_id: Uuid,
        input: RotateMcpTokenInput,
    ) -> Result<RotateMcpTokenResultGql> {
        let auth = managed_auth(ctx)?;
        let result = runtime(ctx)?
            .port()
            .rotate_token(
                &management_context(auth),
                RotateMcpTokenCommand {
                    client_id,
                    token_name: input.token_name,
                    expires_at: input.expires_at.map(|value| value.to_rfc3339()),
                    revoke_existing_tokens: input.revoke_existing_tokens.unwrap_or(true),
                    metadata: parse_metadata(input.metadata)?,
                },
            )
            .await
            .map_err(map_error)?;
        Ok(RotateMcpTokenResultGql {
            client: result.client.into(),
            token: result.token.into(),
            plaintext_token: result.plaintext_token,
        })
    }

    async fn update_mcp_client_policy(
        &self,
        ctx: &Context<'_>,
        client_id: Uuid,
        input: UpdateMcpPolicyInput,
    ) -> Result<McpPolicyGql> {
        let auth = managed_auth(ctx)?;
        runtime(ctx)?
            .port()
            .update_policy(
                &management_context(auth),
                UpdateMcpPolicyCommand {
                    client_id,
                    allowed_tools: input.allowed_tools,
                    denied_tools: input.denied_tools,
                    granted_permissions: input.granted_permissions,
                    granted_scopes: input.granted_scopes,
                    metadata: parse_metadata(input.metadata)?,
                },
            )
            .await
            .map(Into::into)
            .map_err(map_error)
    }

    async fn revoke_mcp_token(
        &self,
        ctx: &Context<'_>,
        token_id: Uuid,
        reason: Option<String>,
    ) -> Result<bool> {
        let auth = managed_auth(ctx)?;
        runtime(ctx)?
            .port()
            .revoke_token(&management_context(auth), token_id, reason)
            .await
            .map_err(map_error)?;
        Ok(true)
    }

    async fn deactivate_mcp_client(
        &self,
        ctx: &Context<'_>,
        client_id: Uuid,
        reason: Option<String>,
    ) -> Result<bool> {
        let auth = managed_auth(ctx)?;
        runtime(ctx)?
            .port()
            .deactivate_client(&management_context(auth), client_id, reason)
            .await
            .map_err(map_error)?;
        Ok(true)
    }

    async fn stage_mcp_module_scaffold_draft(
        &self,
        ctx: &Context<'_>,
        input: StageMcpModuleScaffoldDraftInput,
    ) -> Result<McpModuleScaffoldDraftGql> {
        let auth = managed_auth(ctx)?;
        runtime(ctx)?
            .port()
            .stage_scaffold_draft(
                &management_context(auth),
                StageMcpScaffoldDraftCommand {
                    client_id: input.client_id,
                    request: ScaffoldModuleRequest {
                        slug: input.slug,
                        name: input.name,
                        description: input.description,
                        dependencies: input.dependencies,
                        with_graphql: input.with_graphql.unwrap_or(true),
                        with_rest: input.with_rest.unwrap_or(true),
                        write_files: false,
                    },
                },
            )
            .await
            .map_err(map_error)?
            .try_into()
    }

    async fn apply_mcp_module_scaffold_draft(
        &self,
        ctx: &Context<'_>,
        draft_id: Uuid,
        input: ApplyMcpModuleScaffoldDraftInput,
    ) -> Result<McpModuleScaffoldDraftGql> {
        let auth = managed_auth(ctx)?;
        runtime(ctx)?
            .port()
            .apply_scaffold_draft(
                &management_context(auth),
                ApplyMcpScaffoldDraftCommand {
                    draft_id,
                    workspace_root: input.workspace_root,
                    confirm: input.confirm,
                },
            )
            .await
            .map_err(map_error)?
            .try_into()
    }
}

fn managed_auth<'a>(ctx: &'a Context<'a>) -> Result<&'a rustok_api::AuthContext> {
    let auth = require_auth_context(ctx)?;
    ensure_permission(
        auth,
        Permission::MCP_MANAGE,
        "Permission denied: mcp:manage required",
    )?;
    Ok(auth)
}
