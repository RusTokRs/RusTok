use std::str::FromStr;

use axum::{
    Json,
    body::{Body, to_bytes},
    extract::State,
    http::{Method, Request, StatusCode, request::Parts},
    middleware::Next,
    response::{IntoResponse, Response},
};
use rustok_api::{
    AuthContextExtension, Permission, TenantContextExtension, has_effective_permission,
};
use rustok_mcp::{
    ApplyMcpModuleScaffoldDraftRequest, ApplyModuleScaffoldRequest, CreateMcpClientRequest,
    McpActorType, McpRemoteToolCallRequest, TOOL_ALLOY_APPLY_MODULE_SCAFFOLD,
    UpdateMcpPolicyRequest,
};
use uuid::Uuid;

use crate::services::mcp_management_authority::{
    McpManagementAuthorityError, McpManagementAuthorityService,
};
use crate::services::mcp_scaffold_workspace::authorize_mcp_scaffold_workspace;
use crate::services::server_runtime_context::ServerRuntimeContext;

const MAX_MCP_MANAGEMENT_BODY_BYTES: usize = 64 * 1024;
type MiddlewareResult<T> = Result<T, Box<Response>>;

/// Enforce the host-owned MCP filesystem and management authority boundaries
/// before a transport reaches a direct service handler.
///
/// GraphQL/native mutations and legacy REST mutations share the same canonical
/// authority validator. This middleware owns only HTTP parsing, request-scoped
/// principal extraction, response mapping and workspace normalization.
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
        Err(response) => return *response,
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
) -> MiddlewareResult<Vec<u8>> {
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
) -> MiddlewareResult<()> {
    let input = serde_json::from_slice::<CreateMcpClientRequest>(bytes)
        .map_err(|_| Box::new(invalid_request("MCP create-client body must be valid JSON")))?;
    let actor_type = McpActorType::from_str(input.actor_type.trim())
        .map_err(|message| Box::new(invalid_request(&message)))?;
    let authority = management_authority(parts)?;

    McpManagementAuthorityService::validate_create_client(
        ctx.db(),
        authority.tenant_id,
        authority.permissions,
        actor_type,
        input.delegated_user_id,
        &input.granted_permissions,
    )
    .await
    .map_err(|error| Box::new(authority_error_response(error)))
}

async fn validate_update_policy(
    ctx: &ServerRuntimeContext,
    parts: &Parts,
    client_id: Uuid,
    bytes: &[u8],
) -> MiddlewareResult<()> {
    let input = serde_json::from_slice::<UpdateMcpPolicyRequest>(bytes)
        .map_err(|_| Box::new(invalid_request("MCP policy body must be valid JSON")))?;
    let authority = management_authority(parts)?;

    McpManagementAuthorityService::validate_policy_update(
        ctx.db(),
        authority.tenant_id,
        authority.permissions,
        client_id,
        &input.granted_permissions,
    )
    .await
    .map_err(|error| Box::new(authority_error_response(error)))
}

async fn validate_rotate_token(
    ctx: &ServerRuntimeContext,
    parts: &Parts,
    client_id: Uuid,
) -> MiddlewareResult<()> {
    let authority = management_authority(parts)?;
    McpManagementAuthorityService::validate_token_rotation(
        ctx.db(),
        authority.tenant_id,
        authority.permissions,
        client_id,
    )
    .await
    .map_err(|error| Box::new(authority_error_response(error)))
}

struct ManagementAuthority<'a> {
    tenant_id: Uuid,
    permissions: &'a [Permission],
}

fn management_authority(parts: &Parts) -> MiddlewareResult<ManagementAuthority<'_>> {
    let auth = parts
        .extensions
        .get::<AuthContextExtension>()
        .map(|extension| &extension.0)
        .ok_or_else(|| Box::new(forbidden("MCP management requires authentication")))?;
    let tenant = parts
        .extensions
        .get::<TenantContextExtension>()
        .map(|extension| &extension.0)
        .ok_or_else(|| Box::new(internal_error("Tenant context is unavailable")))?;

    if auth.tenant_id != tenant.id {
        return Err(Box::new(forbidden(
            "Authenticated principal belongs to another tenant",
        )));
    }
    if !has_effective_permission(&auth.permissions, &Permission::MCP_MANAGE) {
        return Err(Box::new(forbidden("mcp:manage permission is required")));
    }

    Ok(ManagementAuthority {
        tenant_id: tenant.id,
        permissions: &auth.permissions,
    })
}

fn authority_error_response(error: McpManagementAuthorityError) -> Response {
    match error {
        McpManagementAuthorityError::Invalid(message) => invalid_request(&message),
        McpManagementAuthorityError::Forbidden(message) => forbidden(&message),
        McpManagementAuthorityError::NotFound(message) => not_found(&message),
        McpManagementAuthorityError::Internal(message) => internal_error(&message),
    }
}

fn normalize_direct_apply(bytes: &[u8]) -> MiddlewareResult<Vec<u8>> {
    let mut input =
        serde_json::from_slice::<ApplyMcpModuleScaffoldDraftRequest>(bytes).map_err(|_| {
            Box::new(invalid_request(
                "MCP scaffold apply body must be valid JSON",
            ))
        })?;
    input.workspace_root = authorize_mcp_scaffold_workspace(&input.workspace_root)
        .map_err(|error| Box::new(error.into_response()))?;

    serde_json::to_vec(&serde_json::json!({
        "workspace_root": input.workspace_root,
        "confirm": input.confirm,
    }))
    .map_err(|_| {
        Box::new(invalid_request(
            "Failed to normalize MCP scaffold apply request",
        ))
    })
}

fn normalize_remote_tool(bytes: &[u8]) -> MiddlewareResult<Vec<u8>> {
    let mut input = serde_json::from_slice::<McpRemoteToolCallRequest>(bytes)
        .map_err(|_| Box::new(invalid_request("MCP remote tool body must be valid JSON")))?;

    if input.tool_name == TOOL_ALLOY_APPLY_MODULE_SCAFFOLD {
        let arguments = input.arguments.take().ok_or_else(|| {
            Box::new(invalid_request(
                "Scaffold apply tool arguments are required",
            ))
        })?;
        let mut apply = serde_json::from_value::<ApplyModuleScaffoldRequest>(arguments)
            .map_err(|_| Box::new(invalid_request("Scaffold apply tool arguments are invalid")))?;
        apply.workspace_root = authorize_mcp_scaffold_workspace(&apply.workspace_root)
            .map_err(|error| Box::new(error.into_response()))?;
        input.arguments = Some(serde_json::to_value(apply).map_err(|_| {
            Box::new(invalid_request(
                "Failed to normalize scaffold apply arguments",
            ))
        })?);
    }

    serde_json::to_vec(&serde_json::json!({
        "tool_name": input.tool_name,
        "arguments": input.arguments,
        "plaintext_token": input.plaintext_token,
        "correlation_id": input.correlation_id,
        "metadata": input.metadata,
    }))
    .map_err(|_| {
        Box::new(invalid_request(
            "Failed to normalize MCP remote tool request",
        ))
    })
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
    use super::{RequestMode, is_remote_tool_path, is_scaffold_apply_path, request_mode};
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
