use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{Method, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use rustok_mcp::{
    ApplyMcpModuleScaffoldDraftRequest, ApplyModuleScaffoldRequest, McpRemoteToolCallRequest,
    TOOL_ALLOY_APPLY_MODULE_SCAFFOLD,
};

use crate::services::mcp_scaffold_workspace::authorize_mcp_scaffold_workspace;
use crate::services::server_runtime_context::ServerRuntimeContext;

const MAX_SCAFFOLD_APPLY_BODY_BYTES: usize = 64 * 1024;

/// Canonicalize every HTTP-exposed MCP scaffold workspace before the handler
/// reaches the filesystem-writing service. GraphQL uses a guarded management
/// port; remote JSON/SSE and direct REST apply are normalized here.
pub async fn authorize_workspace(
    State(_ctx): State<ServerRuntimeContext>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if request.method() != Method::POST {
        return next.run(request).await;
    }

    let path = request.uri().path();
    let mode = if is_scaffold_apply_path(path) {
        RequestMode::DirectApply
    } else if is_remote_tool_path(path) {
        RequestMode::RemoteTool
    } else {
        return next.run(request).await;
    };

    let (parts, body) = request.into_parts();
    let bytes = match to_bytes(body, MAX_SCAFFOLD_APPLY_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => return invalid_request("MCP scaffold apply body is invalid or too large"),
    };
    let body = match mode {
        RequestMode::DirectApply => normalize_direct_apply(&bytes),
        RequestMode::RemoteTool => normalize_remote_tool(&bytes),
    };
    let body = match body {
        Ok(body) => body,
        Err(response) => return response,
    };

    next.run(Request::from_parts(parts, Body::from(body))).await
}

#[derive(Clone, Copy)]
enum RequestMode {
    DirectApply,
    RemoteTool,
}

fn normalize_direct_apply(bytes: &[u8]) -> Result<Vec<u8>, Response> {
    let mut input = serde_json::from_slice::<ApplyMcpModuleScaffoldDraftRequest>(bytes)
        .map_err(|_| invalid_request("MCP scaffold apply body must be valid JSON"))?;
    input.workspace_root = authorize_mcp_scaffold_workspace(&input.workspace_root)
        .map_err(IntoResponse::into_response)?;

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
            .map_err(IntoResponse::into_response)?;
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

fn is_scaffold_apply_path(path: &str) -> bool {
    let segments = path
        .trim_matches('/')
        .split('/')
        .collect::<Vec<_>>();
    segments.len() == 5
        && segments[0] == "api"
        && segments[1] == "mcp"
        && segments[2] == "scaffold-drafts"
        && uuid::Uuid::parse_str(segments[3]).is_ok()
        && segments[4] == "apply"
}

fn is_remote_tool_path(path: &str) -> bool {
    matches!(
        path,
        "/api/mcp/runtime/tools/call" | "/api/mcp/runtime/tools/stream"
    )
}

fn invalid_request(message: &str) -> Response {
    (
        StatusCode::BAD_REQUEST,
        Json(serde_json::json!({
            "error": "invalid_request",
            "message": message,
        })),
    )
        .into_response()
}

#[cfg(test)]
mod tests {
    use super::{is_remote_tool_path, is_scaffold_apply_path};

    #[test]
    fn matches_only_scaffold_apply_routes() {
        let id = uuid::Uuid::new_v4();
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
}