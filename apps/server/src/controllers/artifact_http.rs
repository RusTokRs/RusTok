//! Platform-owned HTTP transport for admitted artifact bindings.

use axum::{
    Json,
    extract::{Extension, Path, State},
    http::{HeaderMap, Method, StatusCode, header::CONTENT_TYPE},
    response::Response,
    routing::{any, get, post},
};
use rustok_api::request::ResolvedRequestLocale;
use rustok_modules::{ModuleHttpMethod, find_artifact_command_binding, find_artifact_http_binding};
use rustok_web::json_response;
use uuid::Uuid;

use crate::{
    error::{Error, Result, http_error},
    extractors::{auth::CurrentUser, tenant::CurrentTenant},
    services::{
        artifact_binding::{ArtifactBindingOperation, dispatch_artifact_binding_operation},
        artifact_ui::{
            execute_artifact_ui_action, list_authorized_artifact_ui_action_audit,
            list_authorized_artifact_ui_contributions, resolve_artifact_installation,
        },
        server_runtime_context::ServerRuntimeContext,
    },
};

const IDEMPOTENCY_KEY_HEADER: &str = "idempotency-key";

async fn dispatch_http(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path((installation_id, wildcard_path)): Path<(Uuid, String)>,
    method: Method,
    headers: HeaderMap,
    Json(body): Json<serde_json::Value>,
) -> Result<Response> {
    ensure_json_content_type(&headers)?;
    let method = module_http_method(&method).ok_or(Error::NotFound)?;
    let path = wildcard_path.trim_matches('/');
    if path.is_empty() {
        return Err(Error::NotFound);
    }
    let installation = resolve_artifact_installation(&ctx, installation_id, tenant.id).await?;
    let binding = find_artifact_http_binding(&installation.descriptor.bindings, method, path)
        .ok_or(Error::NotFound)?;
    let output = dispatch_artifact_binding_operation(
        &ctx,
        tenant.id,
        current.user.id,
        &installation,
        binding,
        header_idempotency_key(&headers)?,
        ArtifactBindingOperation::Http {
            method,
            path: path.to_string(),
            body,
        },
    )
    .await?;
    Ok(json_response(output))
}

async fn dispatch_command(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path((installation_id, binding_id)): Path<(Uuid, String)>,
    headers: HeaderMap,
    Json(input): Json<serde_json::Value>,
) -> Result<Response> {
    ensure_json_content_type(&headers)?;
    let installation = resolve_artifact_installation(&ctx, installation_id, tenant.id).await?;
    let binding = find_artifact_command_binding(&installation.descriptor.bindings, &binding_id)
        .ok_or(Error::NotFound)?;
    let output = dispatch_artifact_binding_operation(
        &ctx,
        tenant.id,
        current.user.id,
        &installation,
        binding,
        header_idempotency_key(&headers)?,
        ArtifactBindingOperation::Command { binding_id, input },
    )
    .await?;
    Ok(json_response(output))
}

/// Executes one host-rendered declarative action or form. The caller names the
/// contribution, never an arbitrary artifact binding; the contribution must
/// still resolve to the exact admitted command contract before RBAC,
/// idempotency, schema validation, and audited sandbox dispatch occur.
async fn dispatch_ui_action(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path((installation_id, contribution_id)): Path<(Uuid, String)>,
    headers: HeaderMap,
    Json(input): Json<serde_json::Value>,
) -> Result<Response> {
    ensure_json_content_type(&headers)?;
    let output = execute_artifact_ui_action(
        &ctx,
        tenant.id,
        current.user.id,
        installation_id,
        &contribution_id,
        input,
        header_idempotency_key(&headers)?,
    )
    .await?;
    Ok(json_response(output))
}

/// Returns declarative UI metadata that the host may render for its resolved
/// effective locale. The caller cannot supply a locale or receive a fallback:
/// unavailable exact-locale contributions are omitted fail-closed.
async fn list_ui_contributions(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Extension(locale): Extension<ResolvedRequestLocale>,
    Path(installation_id): Path<Uuid>,
) -> Result<Response> {
    let contributions = list_authorized_artifact_ui_contributions(
        &ctx,
        tenant.id,
        current.user.id,
        installation_id,
        &locale.effective_locale,
    )
    .await?;
    Ok(json_response(contributions))
}

/// Lists redacted audit evidence for one host-rendered action or form. The
/// contribution resolves to its exact admitted binding before the same dynamic
/// RBAC permission that authorizes execution is checked.
async fn list_ui_action_audit(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path((installation_id, contribution_id)): Path<(Uuid, String)>,
) -> Result<Response> {
    let evidence = list_authorized_artifact_ui_action_audit(
        &ctx,
        tenant.id,
        current.user.id,
        installation_id,
        &contribution_id,
    )
    .await?;
    Ok(json_response(evidence))
}

fn header_idempotency_key(headers: &HeaderMap) -> Result<Option<Uuid>> {
    headers
        .get(IDEMPOTENCY_KEY_HEADER)
        .map(|value| {
            value
                .to_str()
                .map_err(|_| Error::BadRequest("Idempotency-Key header is invalid".to_string()))?
                .trim()
                .parse::<Uuid>()
                .map_err(|_| Error::BadRequest("Idempotency-Key header must be a UUID".to_string()))
                .and_then(|key| {
                    if key.is_nil() {
                        Err(Error::BadRequest(
                            "Idempotency-Key header must not be the nil UUID".to_string(),
                        ))
                    } else {
                        Ok(key)
                    }
                })
        })
        .transpose()
}

fn ensure_json_content_type(headers: &HeaderMap) -> Result<()> {
    let content_type = headers
        .get(CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.split(';').next())
        .map(str::trim);
    if content_type == Some("application/json") {
        return Ok(());
    }
    Err(http_error(rustok_web::HttpError::new(
        StatusCode::UNSUPPORTED_MEDIA_TYPE,
        "unsupported_media_type",
        "Artifact binding requests require application/json",
    )))
}

fn module_http_method(method: &Method) -> Option<ModuleHttpMethod> {
    match method.as_str() {
        "GET" => Some(ModuleHttpMethod::Get),
        "POST" => Some(ModuleHttpMethod::Post),
        "PUT" => Some(ModuleHttpMethod::Put),
        "PATCH" => Some(ModuleHttpMethod::Patch),
        "DELETE" => Some(ModuleHttpMethod::Delete),
        _ => None,
    }
}

pub fn router() -> crate::routes::ServerRouter {
    axum::Router::new()
        .route(
            "/api/artifacts/{installation_id}/commands/{binding_id}",
            post(dispatch_command),
        )
        .route(
            "/api/artifacts/{installation_id}/ui/contributions",
            get(list_ui_contributions),
        )
        .route(
            "/api/artifacts/{installation_id}/ui/contributions/{contribution_id}/execute",
            post(dispatch_ui_action),
        )
        .route(
            "/api/artifacts/{installation_id}/ui/contributions/{contribution_id}/audit",
            get(list_ui_action_audit),
        )
        .route(
            "/api/artifacts/{installation_id}/{*path}",
            any(dispatch_http),
        )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn artifact_binding_idempotency_header_requires_a_non_nil_uuid() {
        let key = Uuid::new_v4();
        let mut headers = HeaderMap::new();
        assert_eq!(header_idempotency_key(&headers).expect("absent key"), None);

        headers.insert(
            IDEMPOTENCY_KEY_HEADER,
            key.to_string().parse().expect("valid header value"),
        );
        assert_eq!(
            header_idempotency_key(&headers).expect("valid UUID key"),
            Some(key)
        );

        headers.insert(
            IDEMPOTENCY_KEY_HEADER,
            "not-a-uuid".parse().expect("valid text header value"),
        );
        assert!(matches!(
            header_idempotency_key(&headers),
            Err(Error::BadRequest(_))
        ));

        headers.insert(
            IDEMPOTENCY_KEY_HEADER,
            Uuid::nil()
                .to_string()
                .parse()
                .expect("valid nil UUID header value"),
        );
        assert!(matches!(
            header_idempotency_key(&headers),
            Err(Error::BadRequest(_))
        ));
    }
}
