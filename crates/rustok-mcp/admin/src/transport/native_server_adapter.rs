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

#[server(prefix = "/api/fn", endpoint = "mcp/scaffold-drafts")]
pub async fn mcp_scaffold_drafts_native() -> Result<Vec<McpScaffoldDraftPayload>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (db, auth) = mcp_native_context().await?;
        ensure_mcp_manage(&auth)?;
        list_scaffold_drafts(&db, auth.tenant_id).await
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "mcp/scaffold-drafts requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "mcp/audit-events")]
pub async fn mcp_audit_events_native() -> Result<Vec<McpAuditEventPayload>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (db, auth) = mcp_native_context().await?;
        ensure_mcp_read(&auth)?;
        list_audit_events(&db, auth.tenant_id).await
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "mcp/audit-events requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "mcp/clients")]
pub async fn mcp_clients_native() -> Result<Vec<McpClientPayload>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (db, auth) = mcp_native_context().await?;
        ensure_mcp_read(&auth)?;
        list_clients(&db, auth.tenant_id).await
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new("mcp/clients requires the `ssr` feature"))
    }
}

#[server(prefix = "/api/fn", endpoint = "mcp/client-details")]
pub async fn mcp_client_details_native(
    client_id: String,
) -> Result<Option<McpClientDetailsPayload>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let client_id = uuid::Uuid::parse_str(&client_id).map_err(ServerFnError::new)?;
        let (db, auth) = mcp_native_context().await?;
        ensure_mcp_read(&auth)?;
        get_client_details(&db, auth.tenant_id, client_id).await
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = client_id;
        Err(ServerFnError::new(
            "mcp/client-details requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "mcp/create-client")]
pub async fn mcp_create_client_native(
    input: CreateMcpClientPayload,
) -> Result<McpTokenSecretPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (context, runtime) = mcp_mutation_context().await?;
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

#[server(prefix = "/api/fn", endpoint = "mcp/rotate-token")]
pub async fn mcp_rotate_token_native(
    input: RotateMcpTokenPayload,
) -> Result<McpTokenSecretPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (context, runtime) = mcp_mutation_context().await?;
        let result = runtime
            .port()
            .rotate_token(
                &context,
                rustok_mcp::RotateMcpTokenCommand {
                    client_id: uuid::Uuid::parse_str(&input.client_id)
                        .map_err(ServerFnError::new)?,
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

#[server(prefix = "/api/fn", endpoint = "mcp/update-policy")]
pub async fn mcp_update_policy_native(input: UpdateMcpPolicyPayload) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (context, runtime) = mcp_mutation_context().await?;
        runtime
            .port()
            .update_policy(
                &context,
                rustok_mcp::UpdateMcpPolicyCommand {
                    client_id: uuid::Uuid::parse_str(&input.client_id)
                        .map_err(ServerFnError::new)?,
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

#[server(prefix = "/api/fn", endpoint = "mcp/revoke-token")]
pub async fn mcp_revoke_token_native(
    token_id: String,
    reason: String,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (context, runtime) = mcp_mutation_context().await?;
        runtime
            .port()
            .revoke_token(
                &context,
                uuid::Uuid::parse_str(&token_id).map_err(ServerFnError::new)?,
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

#[server(prefix = "/api/fn", endpoint = "mcp/deactivate-client")]
pub async fn mcp_deactivate_client_native(
    client_id: String,
    reason: String,
) -> Result<(), ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (context, runtime) = mcp_mutation_context().await?;
        runtime
            .port()
            .deactivate_client(
                &context,
                uuid::Uuid::parse_str(&client_id).map_err(ServerFnError::new)?,
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

#[server(prefix = "/api/fn", endpoint = "mcp/stage-scaffold-draft")]
pub async fn mcp_stage_scaffold_draft_native(
    input: StageMcpScaffoldDraftPayload,
) -> Result<McpScaffoldDraftPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (context, runtime) = mcp_mutation_context().await?;
        let client_id = input
            .client_id
            .map(|value| uuid::Uuid::parse_str(&value).map_err(ServerFnError::new))
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

#[server(prefix = "/api/fn", endpoint = "mcp/apply-scaffold-draft")]
pub async fn mcp_apply_scaffold_draft_native(
    input: ApplyMcpScaffoldDraftPayload,
) -> Result<McpScaffoldDraftPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (context, runtime) = mcp_mutation_context().await?;
        runtime
            .port()
            .apply_scaffold_draft(
                &context,
                rustok_mcp::ApplyMcpScaffoldDraftCommand {
                    draft_id: uuid::Uuid::parse_str(&input.draft_id).map_err(ServerFnError::new)?,
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
async fn mcp_native_context()
-> Result<(sea_orm::DatabaseConnection, rustok_api::AuthContext), ServerFnError> {
    let auth = leptos_axum::extract::<rustok_api::AuthContext>()
        .await
        .map_err(ServerFnError::new)?;
    let runtime_ctx = leptos::prelude::expect_context::<rustok_api::HostRuntimeContext>();
    Ok((runtime_ctx.db_clone(), auth))
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
async fn mcp_mutation_context() -> Result<
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
    ensure_mcp_manage(&auth)?;
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
        rustok_mcp::McpManagementContext {
            actor_id: auth.user_id,
            tenant_id: auth.tenant_id,
        },
        runtime,
    ))
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
async fn list_audit_events(
    db: &sea_orm::DatabaseConnection,
    tenant_id: uuid::Uuid,
) -> Result<Vec<McpAuditEventPayload>, ServerFnError> {
    use sea_orm::{ConnectionTrait, Statement};

    let backend = db.get_database_backend();
    let rows = db
        .query_all(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT id, client_id, actor_id, actor_type, action, outcome, tool_name, reason, correlation_id, created_at FROM mcp_audit_logs WHERE tenant_id = {} ORDER BY created_at DESC LIMIT {}",
                placeholder(backend, 1),
                placeholder(backend, 2)
            ),
            vec![tenant_id.into(), 30_i64.into()],
        ))
        .await
        .map_err(server_error)?;

    rows.into_iter().map(map_audit_row).collect()
}

#[cfg(feature = "ssr")]
async fn list_clients(
    db: &sea_orm::DatabaseConnection,
    tenant_id: uuid::Uuid,
) -> Result<Vec<McpClientPayload>, ServerFnError> {
    use sea_orm::{ConnectionTrait, Statement};

    let backend = db.get_database_backend();
    let rows = db
        .query_all(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT id, slug, display_name, description, actor_type, is_active, last_used_at, created_at FROM mcp_clients WHERE tenant_id = {} ORDER BY created_at DESC LIMIT {}",
                placeholder(backend, 1),
                placeholder(backend, 2)
            ),
            vec![tenant_id.into(), 50_i64.into()],
        ))
        .await
        .map_err(server_error)?;

    rows.into_iter().map(map_client_row).collect()
}

#[cfg(feature = "ssr")]
async fn get_client_details(
    db: &sea_orm::DatabaseConnection,
    tenant_id: uuid::Uuid,
    client_id: uuid::Uuid,
) -> Result<Option<McpClientDetailsPayload>, ServerFnError> {
    use sea_orm::{ConnectionTrait, Statement};

    let backend = db.get_database_backend();
    let client = db
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT id, slug, display_name, description, actor_type, is_active, last_used_at, created_at FROM mcp_clients WHERE id = {} AND tenant_id = {} LIMIT 1",
                placeholder(backend, 1),
                placeholder(backend, 2)
            ),
            vec![client_id.into(), tenant_id.into()],
        ))
        .await
        .map_err(server_error)?;
    let Some(client) = client else {
        return Ok(None);
    };

    let policy = db
        .query_one(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT allowed_tools, denied_tools, granted_permissions, granted_scopes, updated_at FROM mcp_policies WHERE client_id = {} AND tenant_id = {} LIMIT 1",
                placeholder(backend, 1),
                placeholder(backend, 2)
            ),
            vec![client_id.into(), tenant_id.into()],
        ))
        .await
        .map_err(server_error)?
        .map(map_policy_row)
        .transpose()?;

    let tokens = db
        .query_all(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT id, token_name, token_preview, last_used_at, expires_at, revoked_at, created_at FROM mcp_tokens WHERE client_id = {} AND tenant_id = {} ORDER BY created_at DESC",
                placeholder(backend, 1),
                placeholder(backend, 2)
            ),
            vec![client_id.into(), tenant_id.into()],
        ))
        .await
        .map_err(server_error)?
        .into_iter()
        .map(map_token_row)
        .collect::<Result<Vec<_>, _>>()?;

    Ok(Some(McpClientDetailsPayload {
        client: map_client_row(client)?,
        policy,
        tokens,
    }))
}

#[cfg(feature = "ssr")]
async fn list_scaffold_drafts(
    db: &sea_orm::DatabaseConnection,
    tenant_id: uuid::Uuid,
) -> Result<Vec<McpScaffoldDraftPayload>, ServerFnError> {
    use sea_orm::{ConnectionTrait, Statement};

    let backend = db.get_database_backend();
    let rows = db
        .query_all(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT id, client_id, slug, crate_name, status, request_payload, preview_payload, workspace_root, applied_at, created_at, updated_at FROM mcp_scaffold_drafts WHERE tenant_id = {} ORDER BY created_at DESC LIMIT {}",
                placeholder(backend, 1),
                placeholder(backend, 2)
            ),
            vec![tenant_id.into(), 20_i64.into()],
        ))
        .await
        .map_err(server_error)?;

    rows.into_iter().map(map_draft_row).collect()
}

#[cfg(feature = "ssr")]
fn map_draft_row(row: sea_orm::QueryResult) -> Result<McpScaffoldDraftPayload, ServerFnError> {
    let request_json = row
        .try_get::<serde_json::Value>("", "request_payload")
        .map_err(server_error)?;
    let preview_json = row
        .try_get::<serde_json::Value>("", "preview_payload")
        .map_err(server_error)?;
    Ok(McpScaffoldDraftPayload {
        id: row
            .try_get::<uuid::Uuid>("", "id")
            .map_err(server_error)?
            .to_string(),
        client_id: row
            .try_get::<Option<uuid::Uuid>>("", "client_id")
            .map_err(server_error)?
            .map(|value| value.to_string()),
        slug: row.try_get("", "slug").map_err(server_error)?,
        crate_name: row.try_get("", "crate_name").map_err(server_error)?,
        status: status_for_ui(&row.try_get::<String>("", "status").map_err(server_error)?),
        request_json: serde_json::to_string(&request_json).map_err(server_error)?,
        preview_json: serde_json::to_string(&preview_json).map_err(server_error)?,
        workspace_root: row.try_get("", "workspace_root").map_err(server_error)?,
        applied_at: row
            .try_get::<Option<chrono::DateTime<chrono::FixedOffset>>>("", "applied_at")
            .map_err(server_error)?
            .map(|value| value.to_rfc3339()),
        created_at: row
            .try_get::<chrono::DateTime<chrono::FixedOffset>>("", "created_at")
            .map_err(server_error)?
            .to_rfc3339(),
        updated_at: row
            .try_get::<chrono::DateTime<chrono::FixedOffset>>("", "updated_at")
            .map_err(server_error)?
            .to_rfc3339(),
    })
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
fn map_audit_row(row: sea_orm::QueryResult) -> Result<McpAuditEventPayload, ServerFnError> {
    Ok(McpAuditEventPayload {
        id: row
            .try_get::<uuid::Uuid>("", "id")
            .map_err(server_error)?
            .to_string(),
        client_id: row
            .try_get::<Option<uuid::Uuid>>("", "client_id")
            .map_err(server_error)?
            .map(|value| value.to_string()),
        actor_id: row.try_get("", "actor_id").map_err(server_error)?,
        actor_type: row.try_get("", "actor_type").map_err(server_error)?,
        action: row.try_get("", "action").map_err(server_error)?,
        outcome: row.try_get("", "outcome").map_err(server_error)?,
        tool_name: row.try_get("", "tool_name").map_err(server_error)?,
        reason: row.try_get("", "reason").map_err(server_error)?,
        correlation_id: row.try_get("", "correlation_id").map_err(server_error)?,
        created_at: row
            .try_get::<chrono::DateTime<chrono::FixedOffset>>("", "created_at")
            .map_err(server_error)?
            .to_rfc3339(),
    })
}

#[cfg(feature = "ssr")]
fn map_client_row(row: sea_orm::QueryResult) -> Result<McpClientPayload, ServerFnError> {
    Ok(McpClientPayload {
        id: row
            .try_get::<uuid::Uuid>("", "id")
            .map_err(server_error)?
            .to_string(),
        slug: row.try_get("", "slug").map_err(server_error)?,
        display_name: row.try_get("", "display_name").map_err(server_error)?,
        description: row.try_get("", "description").map_err(server_error)?,
        actor_type: row.try_get("", "actor_type").map_err(server_error)?,
        is_active: row.try_get("", "is_active").map_err(server_error)?,
        last_used_at: optional_timestamp(&row, "last_used_at")?,
        created_at: timestamp(&row, "created_at")?,
    })
}

#[cfg(feature = "ssr")]
fn map_policy_row(row: sea_orm::QueryResult) -> Result<McpPolicyPayload, ServerFnError> {
    Ok(McpPolicyPayload {
        allowed_tools: json_string_list(&row, "allowed_tools")?,
        denied_tools: json_string_list(&row, "denied_tools")?,
        granted_permissions: json_string_list(&row, "granted_permissions")?,
        granted_scopes: json_string_list(&row, "granted_scopes")?,
        updated_at: timestamp(&row, "updated_at")?,
    })
}

#[cfg(feature = "ssr")]
fn map_token_row(row: sea_orm::QueryResult) -> Result<McpTokenPayload, ServerFnError> {
    let expires_at = row
        .try_get::<Option<chrono::DateTime<chrono::FixedOffset>>>("", "expires_at")
        .map_err(server_error)?;
    let revoked_at = row
        .try_get::<Option<chrono::DateTime<chrono::FixedOffset>>>("", "revoked_at")
        .map_err(server_error)?;
    let is_active = revoked_at.is_none()
        && expires_at
            .as_ref()
            .is_none_or(|expires_at| expires_at.timestamp() > chrono::Utc::now().timestamp());

    Ok(McpTokenPayload {
        id: row
            .try_get::<uuid::Uuid>("", "id")
            .map_err(server_error)?
            .to_string(),
        token_name: row.try_get("", "token_name").map_err(server_error)?,
        token_preview: row.try_get("", "token_preview").map_err(server_error)?,
        is_active,
        last_used_at: optional_timestamp(&row, "last_used_at")?,
        expires_at: expires_at.map(|value| value.to_rfc3339()),
        created_at: timestamp(&row, "created_at")?,
    })
}

#[cfg(feature = "ssr")]
fn json_string_list(
    row: &sea_orm::QueryResult,
    column: &str,
) -> Result<Vec<String>, ServerFnError> {
    let value = row
        .try_get::<serde_json::Value>("", column)
        .map_err(server_error)?;
    serde_json::from_value(value).map_err(server_error)
}

#[cfg(feature = "ssr")]
fn timestamp(row: &sea_orm::QueryResult, column: &str) -> Result<String, ServerFnError> {
    row.try_get::<chrono::DateTime<chrono::FixedOffset>>("", column)
        .map(|value| value.to_rfc3339())
        .map_err(server_error)
}

#[cfg(feature = "ssr")]
fn optional_timestamp(
    row: &sea_orm::QueryResult,
    column: &str,
) -> Result<Option<String>, ServerFnError> {
    row.try_get::<Option<chrono::DateTime<chrono::FixedOffset>>>("", column)
        .map(|value| value.map(|value| value.to_rfc3339()))
        .map_err(server_error)
}

#[cfg(feature = "ssr")]
fn status_for_ui(value: &str) -> String {
    match value {
        "applied" => "APPLIED".to_string(),
        _ => "STAGED".to_string(),
    }
}

#[cfg(feature = "ssr")]
fn placeholder(backend: sea_orm::DbBackend, index: usize) -> String {
    match backend {
        sea_orm::DbBackend::Sqlite => format!("?{index}"),
        _ => format!("${index}"),
    }
}

#[cfg(feature = "ssr")]
fn server_error(error: impl std::fmt::Display) -> ServerFnError {
    ServerFnError::new(error.to_string())
}
