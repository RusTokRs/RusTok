use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::core::{
    ApplyMcpScaffoldDraftPayload, CreateMcpClientPayload, McpAuditEventPayload,
    McpClientDetailsPayload, McpClientPayload, McpScaffoldDraftPayload, McpTokenSecretPayload,
    RotateMcpTokenPayload, StageMcpScaffoldDraftPayload, UpdateMcpPolicyPayload,
};
#[cfg(feature = "ssr")]
use crate::core::{McpPolicyPayload, McpTokenPayload};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    ServerFn(String),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ServerFn(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

pub async fn fetch_scaffold_drafts() -> Result<Vec<McpScaffoldDraftPayload>, ApiError> {
    mcp_scaffold_drafts_native().await.map_err(Into::into)
}

pub async fn fetch_audit_events() -> Result<Vec<McpAuditEventPayload>, ApiError> {
    mcp_audit_events_native().await.map_err(Into::into)
}

pub async fn fetch_clients() -> Result<Vec<McpClientPayload>, ApiError> {
    mcp_clients_native().await.map_err(Into::into)
}

pub async fn fetch_client_details(
    client_id: String,
) -> Result<Option<McpClientDetailsPayload>, ApiError> {
    mcp_client_details_native(client_id)
        .await
        .map_err(Into::into)
}

pub async fn create_client(
    input: CreateMcpClientPayload,
) -> Result<McpTokenSecretPayload, ApiError> {
    mcp_create_client_native(input).await.map_err(Into::into)
}

pub async fn rotate_token(input: RotateMcpTokenPayload) -> Result<McpTokenSecretPayload, ApiError> {
    mcp_rotate_token_native(input).await.map_err(Into::into)
}

pub async fn update_policy(input: UpdateMcpPolicyPayload) -> Result<(), ApiError> {
    mcp_update_policy_native(input).await.map_err(Into::into)
}

pub async fn revoke_token(token_id: String, reason: String) -> Result<(), ApiError> {
    mcp_revoke_token_native(token_id, reason)
        .await
        .map_err(Into::into)
}

pub async fn deactivate_client(client_id: String, reason: String) -> Result<(), ApiError> {
    mcp_deactivate_client_native(client_id, reason)
        .await
        .map_err(Into::into)
}

pub async fn stage_scaffold_draft(
    input: StageMcpScaffoldDraftPayload,
) -> Result<McpScaffoldDraftPayload, ApiError> {
    mcp_stage_scaffold_draft_native(input)
        .await
        .map_err(Into::into)
}

pub async fn apply_scaffold_draft(
    input: ApplyMcpScaffoldDraftPayload,
) -> Result<McpScaffoldDraftPayload, ApiError> {
    mcp_apply_scaffold_draft_native(input)
        .await
        .map_err(Into::into)
}

#[server(
    prefix = "/api/fn",
    endpoint = "mcp/scaffold-drafts",
    client = rustok_auth_ui::ui::leptos::AuthorizedBrowserClient
)]
pub async fn mcp_scaffold_drafts_native() -> Result<Vec<McpScaffoldDraftPayload>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (context, runtime) = mcp_management_context(ensure_mcp_manage).await?;
        let drafts = runtime
            .port()
            .list_scaffold_drafts(&context, Some(20))
            .await
            .map_err(server_error)?;
        drafts.into_iter().map(scaffold_draft_payload).collect()
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "mcp/scaffold-drafts requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "mcp/audit-events",
    client = rustok_auth_ui::ui::leptos::AuthorizedBrowserClient
)]
pub async fn mcp_audit_events_native() -> Result<Vec<McpAuditEventPayload>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (context, runtime) = mcp_management_context(ensure_mcp_read).await?;
        let events = runtime
            .port()
            .list_audit_events(&context, None, None, Some(30))
            .await
            .map_err(server_error)?;
        Ok(events.into_iter().map(audit_event_payload).collect())
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "mcp/audit-events requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "mcp/clients",
    client = rustok_auth_ui::ui::leptos::AuthorizedBrowserClient
)]
pub async fn mcp_clients_native() -> Result<Vec<McpClientPayload>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (context, runtime) = mcp_management_context(ensure_mcp_read).await?;
        let clients = runtime
            .port()
            .list_clients(&context, Some(50))
            .await
            .map_err(server_error)?;
        Ok(clients.into_iter().map(client_payload).collect())
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new("mcp/clients requires the `ssr` feature"))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "mcp/client-details",
    client = rustok_auth_ui::ui::leptos::AuthorizedBrowserClient
)]
pub async fn mcp_client_details_native(
    client_id: String,
) -> Result<Option<McpClientDetailsPayload>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let client_id = parse_non_nil_uuid(&client_id, "client_id")?;
        let (context, runtime) = mcp_management_context(ensure_mcp_read).await?;
        let details = runtime
            .port()
            .get_client(&context, client_id)
            .await
            .map_err(server_error)?;
        Ok(details.map(client_details_payload))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = client_id;
        Err(ServerFnError::new(
            "mcp/client-details requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "mcp/create-client",
    client = rustok_auth_ui::ui::leptos::AuthorizedBrowserClient
)]
pub async fn mcp_create_client_native(
    input: CreateMcpClientPayload,
) -> Result<McpTokenSecretPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (context, runtime) = mcp_management_context(ensure_mcp_manage).await?;
        let result = runtime
            .port()
            .create_client(
                &context,
                rustok_mcp::CreateMcpClientCommand {
                    slug: input.slug,
                    display_name: input.display_name,
                    description: optional_string(input.description),
                    actor_type: parse_actor_type(&input.actor_type)?,
                    delegated_user_id: None,
                    token_name: optional_string(input.token_name),
                    token_expires_at: optional_string(input.token_expires_at),
                    allowed_tools: input.allowed_tools,
                    denied_tools: input.denied_tools,
                    granted_permissions: input.granted_permissions,
                    granted_scopes: input.granted_scopes,
                    metadata: serde_json::json!({}),
                },
            )
            .await
            .map_err(server_error)?;
        Ok(secret_payload(result))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = input;
        Err(ServerFnError::new(
            "mcp/create-client requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "mcp/rotate-token",
    client = rustok_auth_ui::ui::leptos::AuthorizedBrowserClient
)]
pub async fn mcp_rotate_token_native(
    input: RotateMcpTokenPayload,
) -> Result<McpTokenSecretPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (context, runtime) = mcp_management_context(ensure_mcp_manage).await?;
        let result = runtime
            .port()
            .rotate_token(
                &context,
                rustok_mcp::RotateMcpTokenCommand {
                    client_id: parse_non_nil_uuid(&input.client_id, "client_id")?,
                    token_name: optional_string(input.token_name),
                    expires_at: optional_string(input.expires_at),
                    revoke_existing_tokens: input.revoke_existing_tokens,
                    metadata: serde_json::json!({}),
                },
            )
            .await
            .map_err(server_error)?;
        Ok(secret_payload(result))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = input;
        Err(ServerFnError::new(
            "mcp/rotate-token requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "mcp/update-policy",
    client = rustok_auth_ui::ui::leptos::AuthorizedBrowserClient
)]
pub async fn mcp_update_policy_native(input: UpdateMcpPolicyPayload) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (context, runtime) = mcp_management_context(ensure_mcp_manage).await?;
        runtime
            .port()
            .update_policy(
                &context,
                rustok_mcp::UpdateMcpPolicyCommand {
                    client_id: parse_non_nil_uuid(&input.client_id, "client_id")?,
                    allowed_tools: input.allowed_tools,
                    denied_tools: input.denied_tools,
                    granted_permissions: input.granted_permissions,
                    granted_scopes: input.granted_scopes,
                    metadata: serde_json::json!({}),
                },
            )
            .await
            .map(|_| ())
            .map_err(server_error)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = input;
        Err(ServerFnError::new(
            "mcp/update-policy requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "mcp/revoke-token",
    client = rustok_auth_ui::ui::leptos::AuthorizedBrowserClient
)]
pub async fn mcp_revoke_token_native(
    token_id: String,
    reason: String,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (context, runtime) = mcp_management_context(ensure_mcp_manage).await?;
        runtime
            .port()
            .revoke_token(
                &context,
                parse_non_nil_uuid(&token_id, "token_id")?,
                optional_string(reason),
            )
            .await
            .map_err(server_error)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (token_id, reason);
        Err(ServerFnError::new(
            "mcp/revoke-token requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "mcp/deactivate-client",
    client = rustok_auth_ui::ui::leptos::AuthorizedBrowserClient
)]
pub async fn mcp_deactivate_client_native(
    client_id: String,
    reason: String,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (context, runtime) = mcp_management_context(ensure_mcp_manage).await?;
        runtime
            .port()
            .deactivate_client(
                &context,
                parse_non_nil_uuid(&client_id, "client_id")?,
                optional_string(reason),
            )
            .await
            .map_err(server_error)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (client_id, reason);
        Err(ServerFnError::new(
            "mcp/deactivate-client requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "mcp/stage-scaffold-draft",
    client = rustok_auth_ui::ui::leptos::AuthorizedBrowserClient
)]
pub async fn mcp_stage_scaffold_draft_native(
    input: StageMcpScaffoldDraftPayload,
) -> Result<McpScaffoldDraftPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (context, runtime) = mcp_management_context(ensure_mcp_manage).await?;
        let client_id = input
            .client_id
            .map(|value| parse_non_nil_uuid(&value, "client_id"))
            .transpose()?;
        runtime
            .port()
            .stage_scaffold_draft(
                &context,
                rustok_mcp::StageMcpScaffoldDraftCommand {
                    client_id,
                    request: rustok_mcp::ScaffoldModuleRequest {
                        slug: input.slug,
                        name: input.name,
                        description: input.description,
                        dependencies: input.dependencies,
                        with_graphql: input.with_graphql,
                        with_rest: input.with_rest,
                        write_files: false,
                    },
                },
            )
            .await
            .map_err(server_error)
            .and_then(scaffold_draft_payload)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = input;
        Err(ServerFnError::new(
            "mcp/stage-scaffold-draft requires the `ssr` feature",
        ))
    }
}

#[server(
    prefix = "/api/fn",
    endpoint = "mcp/apply-scaffold-draft",
    client = rustok_auth_ui::ui::leptos::AuthorizedBrowserClient
)]
pub async fn mcp_apply_scaffold_draft_native(
    input: ApplyMcpScaffoldDraftPayload,
) -> Result<McpScaffoldDraftPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (context, runtime) = mcp_management_context(ensure_mcp_manage).await?;
        runtime
            .port()
            .apply_scaffold_draft(
                &context,
                rustok_mcp::ApplyMcpScaffoldDraftCommand {
                    draft_id: parse_non_nil_uuid(&input.draft_id, "draft_id")?,
                    workspace_root: input.workspace_root,
                    confirm: input.confirm,
                },
            )
            .await
            .map_err(server_error)
            .and_then(scaffold_draft_payload)
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = input;
        Err(ServerFnError::new(
            "mcp/apply-scaffold-draft requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn ensure_mcp_manage(auth: &rustok_api::AuthContext) -> Result<(), ServerFnError> {
    if rustok_api::has_effective_permission(&auth.permissions, &rustok_api::Permission::MCP_MANAGE)
    {
        Ok(())
    } else {
        Err(ServerFnError::new("mcp:manage required"))
    }
}

#[cfg(feature = "ssr")]
fn ensure_mcp_read(auth: &rustok_api::AuthContext) -> Result<(), ServerFnError> {
    if rustok_api::has_effective_permission(&auth.permissions, &rustok_api::Permission::MCP_READ)
        || rustok_api::has_effective_permission(
            &auth.permissions,
            &rustok_api::Permission::MCP_MANAGE,
        )
    {
        Ok(())
    } else {
        Err(ServerFnError::new("mcp:read required"))
    }
}

#[cfg(feature = "ssr")]
async fn mcp_management_context(
    authorize: fn(&rustok_api::AuthContext) -> Result<(), ServerFnError>,
) -> Result<
    (
        rustok_mcp::McpManagementContext,
        rustok_mcp::McpManagementRuntime,
    ),
    ServerFnError,
> {
    use std::sync::Arc;

    let auth = leptos_axum::extract::<rustok_api::AuthContext>()
        .await
        .map_err(ServerFnError::new)?;
    if auth.tenant_id.is_nil() || auth.user_id.is_nil() {
        return Err(ServerFnError::new(
            "Unauthorized: tenant_id or user_id is nil",
        ));
    }
    authorize(&auth)?;
    let runtime_ctx = leptos::prelude::expect_context::<rustok_api::HostRuntimeContext>();
    let extensions = runtime_ctx
        .shared_get::<Arc<rustok_core::ModuleRuntimeExtensions>>()
        .ok_or_else(|| ServerFnError::new("ModuleRuntimeExtensions not initialized"))?;
    let runtime = extensions
        .get::<rustok_mcp::McpManagementRuntime>()
        .cloned()
        .ok_or_else(|| {
            ServerFnError::new(
                "McpManagementRuntime is not registered; initialize the server provider",
            )
        })?;
    Ok((
        rustok_mcp::McpManagementContext::from_auth_context(&auth),
        runtime,
    ))
}

#[cfg(feature = "ssr")]
fn parse_non_nil_uuid(value: &str, field_name: &str) -> Result<uuid::Uuid, ServerFnError> {
    let id = uuid::Uuid::parse_str(value.trim()).map_err(ServerFnError::new)?;
    if id.is_nil() {
        return Err(ServerFnError::new(format!(
            "{field_name} cannot be empty (nil UUID)"
        )));
    }
    Ok(id)
}

#[cfg(feature = "ssr")]
fn parse_actor_type(value: &str) -> Result<rustok_mcp::McpActorType, ServerFnError> {
    match value {
        "HUMAN_USER" | "human_user" => Ok(rustok_mcp::McpActorType::HumanUser),
        "SERVICE_CLIENT" | "service_client" => Ok(rustok_mcp::McpActorType::ServiceClient),
        "MODEL_AGENT" | "model_agent" => Ok(rustok_mcp::McpActorType::ModelAgent),
        _ => Err(ServerFnError::new(format!(
            "unsupported MCP actor type: {value}"
        ))),
    }
}

#[cfg(feature = "ssr")]
fn optional_string(value: String) -> Option<String> {
    let value = value.trim();
    (!value.is_empty()).then(|| value.to_string())
}

#[cfg(feature = "ssr")]
fn secret_payload(result: rustok_mcp::McpTokenSecretResult) -> McpTokenSecretPayload {
    McpTokenSecretPayload {
        client_id: result.client.id.to_string(),
        token_id: result.token.id.to_string(),
        token_name: result.token.token_name,
        token_preview: result.token.token_preview,
        plaintext_token: result.plaintext_token,
    }
}

#[cfg(feature = "ssr")]
fn scaffold_draft_payload(
    record: rustok_mcp::McpScaffoldDraftRecord,
) -> Result<McpScaffoldDraftPayload, ServerFnError> {
    Ok(McpScaffoldDraftPayload {
        id: record.id.to_string(),
        client_id: record.client_id.map(|value| value.to_string()),
        slug: record.slug,
        crate_name: record.crate_name,
        status: status_for_ui(&record.status),
        request_json: serde_json::to_string(&record.request_payload).map_err(server_error)?,
        preview_json: serde_json::to_string(&record.preview_payload).map_err(server_error)?,
        workspace_root: record.workspace_root,
        applied_at: record.applied_at.map(|value| value.to_rfc3339()),
        created_at: record.created_at.to_rfc3339(),
        updated_at: record.updated_at.to_rfc3339(),
    })
}

#[cfg(feature = "ssr")]
fn audit_event_payload(record: rustok_mcp::McpAuditEventRecord) -> McpAuditEventPayload {
    McpAuditEventPayload {
        id: record.id.to_string(),
        client_id: record.client_id.map(|value| value.to_string()),
        actor_id: record.actor_id,
        actor_type: record.actor_type,
        action: record.action,
        outcome: record.outcome,
        tool_name: record.tool_name,
        reason: record.reason,
        correlation_id: record.correlation_id,
        created_at: record.created_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn client_payload(record: rustok_mcp::McpClientRecord) -> McpClientPayload {
    use rustok_mcp::McpActorType;

    let actor_type = match record.actor_type {
        McpActorType::HumanUser => "human_user",
        McpActorType::ServiceClient => "service_client",
        McpActorType::ModelAgent => "model_agent",
    };
    McpClientPayload {
        id: record.id.to_string(),
        slug: record.slug,
        display_name: record.display_name,
        description: record.description,
        actor_type: actor_type.to_string(),
        is_active: record.is_active,
        last_used_at: record.last_used_at.map(|value| value.to_rfc3339()),
        created_at: record.created_at.to_rfc3339(),
    }
}

#[cfg(feature = "ssr")]
fn client_details_payload(record: rustok_mcp::McpClientDetailsRecord) -> McpClientDetailsPayload {
    McpClientDetailsPayload {
        client: client_payload(record.client),
        policy: record.policy.map(|policy| McpPolicyPayload {
            allowed_tools: policy.allowed_tools,
            denied_tools: policy.denied_tools,
            granted_permissions: policy.granted_permissions,
            granted_scopes: policy.granted_scopes,
            updated_at: policy.updated_at.to_rfc3339(),
        }),
        tokens: record
            .tokens
            .into_iter()
            .map(|token| McpTokenPayload {
                id: token.id.to_string(),
                token_name: token.token_name,
                token_preview: token.token_preview,
                is_active: token.is_active,
                last_used_at: token.last_used_at.map(|value| value.to_rfc3339()),
                expires_at: token.expires_at.map(|value| value.to_rfc3339()),
                created_at: token.created_at.to_rfc3339(),
            })
            .collect(),
    }
}

#[cfg(feature = "ssr")]
fn status_for_ui(value: &str) -> String {
    match value {
        "applied" => "APPLIED".to_string(),
        _ => "STAGED".to_string(),
    }
}

#[cfg(feature = "ssr")]
fn server_error(error: impl std::fmt::Display) -> ServerFnError {
    ServerFnError::new(error.to_string())
}
