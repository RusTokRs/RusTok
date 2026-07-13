use axum::{
    body::{to_bytes, Body},
    extract::State,
    http::{Method, Request, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use rustok_mcp::ApplyMcpModuleScaffoldDraftRequest;

use crate::services::mcp_scaffold_workspace::authorize_mcp_scaffold_workspace;
use crate::services::server_runtime_context::ServerRuntimeContext;

const MAX_SCAFFOLD_APPLY_BODY_BYTES: usize = 32 * 1024;

/// Canonicalize the REST scaffold workspace before the handler reaches the
/// filesystem-writing service. Other MCP transports use the same authorizer at
/// their port/runtime boundaries.
pub async fn authorize_workspace(
    State(_ctx): State<ServerRuntimeContext>,
    request: Request<Body>,
    next: Next,
) -> Response {
    if request.method() != Method::POST || !is_scaffold_apply_path(request.uri().path()) {
        return next.run(request).await;
    }

    let (parts, body) = request.into_parts();
    let bytes = match to_bytes(body, MAX_SCAFFOLD_APPLY_BODY_BYTES).await {
        Ok(bytes) => bytes,
        Err(_) => return invalid_request("MCP scaffold apply body is invalid or too large"),
    };
    let mut input = match serde_json::from_slice::<ApplyMcpModuleScaffoldDraftRequest>(&bytes) {
        Ok(input) => input,
        Err(_) => return invalid_request("MCP scaffold apply body must be valid JSON"),
    };
    input.workspace_root = match authorize_mcp_scaffold_workspace(&input.workspace_root) {
        Ok(root) => root,
        Err(error) => return error.into_response(),
    };
    let body = match serde_json::to_vec(&serde_json::json!({
        "workspace_root": input.workspace_root,
        "confirm": input.confirm,
    })) {
        Ok(body) => body,
        Err(_) => return invalid_request("Failed to normalize MCP scaffold apply request"),
    };

    next.run(Request::from_parts(parts, Body::from(body))).await
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
    use super::is_scaffold_apply_path;

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
        assert!(!is_scaffold_apply_path("/api/mcp/runtime/tools/call"));
    }
}