use std::str::FromStr;

use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{request::Parts, Method, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use rustok_api::{
    has_effective_permission, AuthContextExtension, Permission, TenantContextExtension,
};
use rustok_mcp::{
    ApplyMcpModuleScaffoldDraftRequest, ApplyModuleScaffoldRequest, CreateMcpClientRequest,
    McpActorType, McpRemoteToolCallRequest, UpdateMcpPolicyRequest,
    TOOL_ALLOY_APPLY_MODULE_SCAFFOLD,
};
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter};
use uuid::Uuid;

use crate::models::{mcp_clients, mcp_policies, users};
use crate::services::mcp_scaffold_workspace::authorize_mcp_scaffold_workspace;
use crate::services::rbac_service::RbacService;
use crate::services::server_runtime_context::ServerRuntimeContext;

const MAX_MCP_MANAGEMENT_BODY_BYTES: usize = 64 * 1024;

/// Enforce the host-owned MCP filesystem and authority boundaries before a
/// transport reaches a direct service handler.
///
/// GraphQL and native mutations already use the guarded management port. The
/// legacy REST controller still calls `McpManagementService` directly, so its
/// create/update/rotate routes are validated here against the request-scoped
/// OAuth/RBAC snapshot and, when present, the delegated user's authoritative
/// tenant permissions.
pub async fn authorize_workspace(
    State(ctx): State<ServerRuntimeContext>,
    request: Request<Body>,
    next: Next,
) -> Response {
    let Some(mode) = request_mode(request.method(), request.uri().path()) else {
        return next.run(request).await;
    };

    let (parts, body) = request.into_parts();
    let bytes = match to_bytes(body, MAX_MCP_MANAGEMENT_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => return invalid_request("MCP request body is invalid or too large"),
    };

    let body = match process_request(&ctx, &parts, mode, &bytes).await {
        Ok(body) => body,
        Err(response) => return response,
    };

    next.run(Request::from_parts(parts, Body::from(body))).await
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RequestMode {
    DirectApply,
    RemoteTool,
    CreateClient,
    UpdatePolicy(Uuid),
    RotateToken(Uuid),
}

async fn process_request(
    ctx: &ServerRuntimeContext,
    parts: &Parts,
    mode: RequestMode,
    bytes: &[u8],
) -> Result<Vec<u8>, Response> {
    match mode {
        RequestMode::DirectApply => normalize_direct_apply(bytes),
        RequestMode::RemoteTool => normalize_remote_tool(bytes),
        RequestMode::CreateClient => {
            validate_create_client(ctx, parts, bytes).await?;
            Ok(bytes.to_vec())
        }
        RequestMode::UpdatePolicy(client_id) => {
            validate_update_policy(ctx, parts, client_id, bytes).await?;
            Ok(bytes.to_vec())
        }
        RequestMode::RotateToken(client_id) => {
            validate_rotate_token(ctx, parts, client_id).await?;
            Ok(bytes.to_vec())
        }
    }
}

async fn validate_create_client(
    ctx: &ServerRuntimeContext,
    parts: &Parts,
    bytes: &[u8],
) -> Result<(), Response> {
    let input = serde_json::from_slice::<CreateMcpClientRequest>(bytes)
        .map_err(|_| invalid_request("MCP create-client body must be valid JSON"))?;
    let actor_type = McpActorType::from_str(input.actor_type.trim())
        .map_err(|message| invalid_request(&message))?;
    let authority = management_authority(parts)?;
    let delegated = delegated_permissions(
        ctx,
        authority.tenant_id,
        actor_type,
        input.delegated_user_id,
    )
    .await?;

    validate_grants(
        &input.granted_permissions,
        authority.permissions,
        "current MCP manager",
    )?;
    if let Some(delegated) = delegated.as_deref() {
        validate_grants(
            &input.granted_permissions,
            delegated,
            "delegated MCP user",
        )?;
    }
    Ok(())
}

async fn validate_update_policy(
    ctx: &ServerRuntimeContext,
    parts: &Parts,
    client_id: Uuid,
    bytes: &[u8],
) -> Result<(), Response> {
    let input = serde_json::from_slice::<UpdateMcpPolicyRequest>(bytes)
        .map_err(|_| invalid_request("MCP policy body must be valid JSON"))?;
    let authority = management_authority(parts)?;
    let client = require_client(ctx, authority.tenant_id, client_id).await?;
    let delegated = delegated_permissions(
        ctx,
        authority.tenant_id,
        client.actor_type(),
        client.delegated_user_id,
    )
    .await?;

    validate_grants(
        &input.granted_permissions,
        authority.permissions,
        "current MCP manager",
    )?;
    if let Some(delegated) = delegated.as_deref() {
        validate_grants(
            &input.granted_permissions,
            delegated,
            "delegated MCP user",
        )?;
    }
    Ok(())
}

async fn validate_rotate_token(
    ctx: &ServerRuntimeContext,
    parts: &Parts,
    client_id: Uuid,
) -> Result<(), Response> {
    let authority = management_authority(parts)?;
    let client = require_client(ctx, authority.tenant_id, client_id).await?;
    let delegated = delegated_permissions(
        ctx,
        authority.tenant_id,
        client.actor_type(),
        client.delegated_user_id,
    )
    .await?;
    let policy = mcp_policies::Entity::find_by_client(ctx.db(), client.id)
        .await
        .map_err(|_| internal_error("Failed to load MCP policy"))?;

    if let Some(policy) = policy {
        if policy.tenant_id != authority.tenant_id {
            return Err(forbidden("MCP policy belongs to another tenant"));
        }
        let grants = policy.granted_permissions_list();
        validate_grants(&grants, authority.permissions, "current MCP manager")?;
        if let Some(delegated) = delegated.as_deref() {
            validate_grants(&grants, delegated, "delegated MCP user")?;
        }
    }
    Ok(())
}

struct ManagementAuthority<'a> {
    tenant_id: Uuid,
    permissions: &'a [Permission],
}

fn management_authority(parts: &Parts) -> Result<ManagementAuthority<'_>, Response> {
    let auth = parts
        .extensions
        .get::<AuthContextExtension>()
        .map(|extension| &extension.0)
        .ok_or_else(|| forbidden("MCP management requires authentication"))?;
    let tenant = parts
        .extensions
        .get::<TenantContextExtension>()
        .map(|extension| &extension.0)
        .ok_or_else(|| internal_error("Tenant context is unavailable"))?;

    if auth.tenant_id != tenant.id {
        return Err(forbidden("Authenticated principal belongs to another tenant"));
    }
    if !has_effective_permission(&auth.permissions, &Permission::MCP_MANAGE) {
        return Err(forbidden("mcp:manage permission is required"));
    }

    Ok(ManagementAuthority {
        tenant_id: tenant.id,
        permissions: &auth.permissions,
    })
}

async fn require_client(
    ctx: &ServerRuntimeContext,
    tenant_id: Uuid,
    client_id: Uuid,
) -> Result<mcp_clients::Model, Response> {
    mcp_clients::Entity::find_by_id(client_id)
        .filter(mcp_clients::Column::TenantId.eq(tenant_id))
        .one(ctx.db())
        .await
        .map_err(|_| internal_error("Failed to load MCP client"))?
        .ok_or_else(|| not_found("MCP client was not found"))
}

async fn delegated_permissions(
    ctx: &ServerRuntimeContext,
    tenant_id: Uuid,
    actor_type: McpActorType,
    delegated_user_id: Option<Uuid>,
) -> Result<Option<Vec<Permission>>, Response> {
    if actor_type == McpActorType::HumanUser && delegated_user_id.is_none() {
        return Err(invalid_request(
            "human_user MCP clients require delegated_user_id",
        ));
    }
    let Some(user_id) = delegated_user_id else {
        return Ok(None);
    };

    let user = users::Entity::find_by_id(user_id)
        .filter(users::Column::TenantId.eq(tenant_id))
        .one(ctx.db())
        .await
        .map_err(|_| internal_error("Failed to load delegated MCP user"))?
        .ok_or_else(|| invalid_request("Delegated MCP user does not exist in this tenant"))?;
    if !user.is_active() {
        return Err(invalid_request("Delegated MCP user must be active"));
    }

    RbacService::get_user_permissions_authoritative(ctx.db(), &tenant_id, &user_id)
        .await
        .map(Some)
        .map_err(|_| internal_error("Failed to resolve delegated MCP user permissions"))
}

fn validate_grants(
    requested: &[String],
    authority: &[Permission],
    principal: &str,
) -> Result<(), Response> {
    for raw in requested {
        let permission = Permission::from_str(raw.trim())
            .map_err(|_| invalid_request(&format!("Invalid MCP permission `{raw}`")))?;
        if !has_effective_permission(authority, &permission) {
            return Err(forbidden(&format!(
                "MCP permission `{permission}` exceeds {principal} authority"
            )));
        }
    }
    Ok(())
}

fn normalize_direct_apply(bytes: &[u8]) -> Result<Vec<u8>, Response> {
    let mut input = serde_json::from_slice::<ApplyMcpModuleScaffoldDraftRequest>(bytes)
        .map_err(|_| invalid_request("MCP scaffold apply body must be valid JSON"))?;
    input.workspace_root = authorize_mcp_scaffold_workspace(&input.workspace_root)
        .map_err(|error| error.into_response())?;

    serde_json::to_vec(&serde_json::json!({
        "workspace_root": input.workspace_root,
        "confirm": input.confirm,
    }))
    .map_err(|_| invalid_request("Failed to normalize MCP scaffold apply request"))
}

fn normalize_remote_tool(bytes: &[u8]) -> Result<Vec<u8>, Response> {
    let mut input = serde_json::from_slice::<McpRemoteToolCallRequest>(bytes)
        .map_err(|_| invalid_request("MCP remote tool body must be valid JSON"))?;

    if input.tool_name == TOOL_ALLOY_APPLY_MODULE_SCAFFOLD {
        let arguments = input
            .arguments
            .take()
            .ok_or_else(|| invalid_request("Scaffold apply tool arguments are required"))?;
        let mut apply = serde_json::from_value::<ApplyModuleScaffoldRequest>(arguments)
            .map_err(|_| invalid_request("Scaffold apply tool arguments are invalid"))?;
        apply.workspace_root = authorize_mcp_scaffold_workspace(&apply.workspace_root)
            .map_err(|error| error.into_response())?;
        input.arguments = Some(
            serde_json::to_value(apply)
                .map_err(|_| invalid_request("Failed to normalize scaffold apply arguments"))?,
        );
    }

    serde_json::to_vec(&serde_json::json!({
        "tool_name": input.tool_name,
        "arguments": input.arguments,
        "plaintext_token": input.plaintext_token,
        "correlation_id": input.correlation_id,
        "metadata": input.metadata,
    }))
    .map_err(|_| invalid_request("Failed to normalize MCP remote tool request"))
}

fn request_mode(method: &Method, path: &str) -> Option<RequestMode> {
    if method == Method::POST && path == "/api/mcp/clients" {
        return Some(RequestMode::CreateClient);
    }
    if method == Method::POST && is_scaffold_apply_path(path) {
        return Some(RequestMode::DirectApply);
    }
    if method == Method::POST && is_remote_tool_path(path) {
        return Some(RequestMode::RemoteTool);
    }

    let segments = path.trim_matches('/').split('/').collect::<Vec<_>>();
    if segments.len() != 5
        || segments[0] != "api"
        || segments[1] != "mcp"
        || segments[2] != "clients"
    {
        return None;
    }
    let client_id = Uuid::parse_str(segments[3]).ok()?;
    if method == Method::PUT && segments[4] == "policy" {
        Some(RequestMode::UpdatePolicy(client_id))
    } else if method == Method::POST && segments[4] == "rotate-token" {
        Some(RequestMode::RotateToken(client_id))
    } else {
        None
    }
}

fn is_scaffold_apply_path(path: &str) -> bool {
    let segments = path.trim_matches('/').split('/').collect::<Vec<_>>();
    segments.len() == 5
        && segments[0] == "api"
        && segments[1] == "mcp"
        && segments[2] == "scaffold-drafts"
        && Uuid::parse_str(segments[3]).is_ok()
        && segments[4] == "apply"
}

fn is_remote_tool_path(path: &str) -> bool {
    matches!(
        path,
        "/api/mcp/runtime/tools/call" | "/api/mcp/runtime/tools/stream"
    )
}

fn response(status: StatusCode, code: &str, message: &str) -> Response {
    (
        status,
        Json(serde_json::json!({
            "error": code,
            "message": message,
        })),
    )
        .into_response()
}

fn invalid_request(message: &str) -> Response {
    response(StatusCode::BAD_REQUEST, "invalid_request", message)
}

fn forbidden(message: &str) -> Response {
    response(StatusCode::FORBIDDEN, "access_denied", message)
}

fn not_found(message: &str) -> Response {
    response(StatusCode::NOT_FOUND, "not_found", message)
}

fn internal_error(message: &str) -> Response {
    response(StatusCode::INTERNAL_SERVER_ERROR, "internal_error", message)
}

#[cfg(test)]
mod tests {
    use super::{is_remote_tool_path, is_scaffold_apply_path, request_mode, RequestMode};
    use axum::http::Method;
    use uuid::Uuid;

    #[test]
    fn matches_only_scaffold_apply_routes() {
        let id = Uuid::new_v4();
        assert!(is_scaffold_apply_path(&format!(
            "/api/mcp/scaffold-drafts/{id}/apply"
        )));
        assert!(!is_scaffold_apply_path(&format!(
            "/api/mcp/scaffold-drafts/{id}"
        )));
        assert!(!is_scaffold_apply_path(
            "/api/mcp/scaffold-drafts/not-a-uuid/apply"
        ));
    }

    #[test]
    fn matches_remote_json_and_sse_tool_routes() {
        assert!(is_remote_tool_path("/api/mcp/runtime/tools/call"));
        assert!(is_remote_tool_path("/api/mcp/runtime/tools/stream"));
        assert!(!is_remote_tool_path("/api/mcp/runtime/bootstrap"));
    }

    #[test]
    fn matches_only_authority_sensitive_management_routes() {
        let id = Uuid::new_v4();
        assert_eq!(
            request_mode(&Method::POST, "/api/mcp/clients"),
            Some(RequestMode::CreateClient)
        );
        assert_eq!(
            request_mode(&Method::PUT, &format!("/api/mcp/clients/{id}/policy")),
            Some(RequestMode::UpdatePolicy(id))
        );
        assert_eq!(
            request_mode(
                &Method::POST,
                &format!("/api/mcp/clients/{id}/rotate-token")
            ),
            Some(RequestMode::RotateToken(id))
        );
        assert_eq!(
            request_mode(&Method::POST, &format!("/api/mcp/clients/{id}/deactivate")),
            None
        );
    }
}
