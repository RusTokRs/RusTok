//! Platform-owned HTTP transport for admitted artifact bindings.

use axum::{
    extract::{Path, State},
    http::{header::CONTENT_TYPE, HeaderMap, Method, StatusCode},
    response::Response,
    routing::{any, post},
    Json,
};
use rustok_modules::{
    artifact_binding_request_digest, dispatch_artifact_command_binding,
    dispatch_artifact_http_binding, find_artifact_command_binding, find_artifact_http_binding,
    ArtifactBindingExecutionContext, ArtifactBindingIdempotencyClaim,
    ArtifactBindingIdempotencyError, ArtifactBindingIdempotencyRequest, ArtifactInstallationTarget,
    InstalledModuleArtifact, ModuleBindingIdempotency, ModuleControlPlane, ModuleDispatchError,
    ModuleHttpMethod, ModuleRuntimeBinding, SharedArtifactBindingExecutor,
};
use rustok_rbac::SeaOrmArtifactPermissionAuthorizer;
use rustok_web::json_response;
use uuid::Uuid;

use crate::{
    error::{http_error, Error, Result},
    extractors::{auth::CurrentUser, tenant::CurrentTenant},
    services::server_runtime_context::ServerRuntimeContext,
};

const IDEMPOTENCY_KEY_HEADER: &str = "idempotency-key";

enum ArtifactOperation {
    Http {
        method: ModuleHttpMethod,
        path: String,
        body: serde_json::Value,
    },
    Command {
        binding_id: String,
        input: serde_json::Value,
    },
}

impl ArtifactOperation {
    fn request_digest(&self) -> Result<String> {
        let envelope = match self {
            Self::Http { method, path, body } => serde_json::json!({
                "kind": "http",
                "method": method,
                "path": path,
                "body": body,
            }),
            Self::Command { binding_id, input } => serde_json::json!({
                "kind": "command",
                "binding_id": binding_id,
                "input": input,
            }),
        };
        artifact_binding_request_digest(&envelope).map_err(map_idempotency_error)
    }
}

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
    let installation = resolve_installation(&ctx, installation_id, tenant.id).await?;
    let binding = find_artifact_http_binding(&installation.descriptor.bindings, method, path)
        .ok_or(Error::NotFound)?;
    dispatch_operation(
        &ctx,
        tenant.id,
        current.user.id,
        &headers,
        &installation,
        binding,
        ArtifactOperation::Http {
            method,
            path: path.to_string(),
            body,
        },
    )
    .await
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
    let installation = resolve_installation(&ctx, installation_id, tenant.id).await?;
    let binding = find_artifact_command_binding(&installation.descriptor.bindings, &binding_id)
        .ok_or(Error::NotFound)?;
    dispatch_operation(
        &ctx,
        tenant.id,
        current.user.id,
        &headers,
        &installation,
        binding,
        ArtifactOperation::Command { binding_id, input },
    )
    .await
}

async fn resolve_installation(
    ctx: &ServerRuntimeContext,
    installation_id: Uuid,
    tenant_id: Uuid,
) -> Result<InstalledModuleArtifact> {
    ModuleControlPlane::new(ctx.db_clone())
        .installation()
        .resolve_routed_installation(installation_id, tenant_id)
        .await
        .map_err(|_| Error::NotFound)
}

async fn dispatch_operation(
    ctx: &ServerRuntimeContext,
    tenant_id: Uuid,
    actor_id: Uuid,
    headers: &HeaderMap,
    installation: &InstalledModuleArtifact,
    binding: &ModuleRuntimeBinding,
    operation: ArtifactOperation,
) -> Result<Response> {
    authorize_binding(ctx, tenant_id, actor_id, &installation, binding).await?;
    let executor = ctx
        .shared_get::<SharedArtifactBindingExecutor>()
        .ok_or_else(|| Error::Message("artifact binding runtime is not initialized".to_string()))?;
    let request = idempotency_request(
        tenant_id,
        actor_id,
        installation.installation_id,
        binding,
        header_idempotency_key(headers)?,
        operation.request_digest()?,
    )?;
    let context = ArtifactBindingExecutionContext {
        actor_id: Some(actor_id.to_string()),
        trace_id: None,
    };
    let output = match request {
        Some(request) => {
            let store = ModuleControlPlane::new(ctx.db_clone()).artifact_binding_idempotency();
            match store.claim(&request).await.map_err(map_idempotency_error)? {
                ArtifactBindingIdempotencyClaim::Replay { response } => response,
                ArtifactBindingIdempotencyClaim::InProgress => {
                    return Err(http_error(rustok_web::HttpError::new(
                        StatusCode::CONFLICT,
                        "artifact_binding_in_progress",
                        "An identical artifact binding request is still executing",
                    )));
                }
                ArtifactBindingIdempotencyClaim::Execute { operation_id } => {
                    let result = execute_operation(
                        executor.as_ref(),
                        &installation,
                        tenant_id,
                        operation,
                        context,
                    )
                    .await;
                    match result {
                        Ok(output) => {
                            store
                                .complete(&request, operation_id, &output)
                                .await
                                .map_err(map_idempotency_error)?;
                            output
                        }
                        Err(error) => {
                            let _ = store.abandon(&request, operation_id).await;
                            return Err(error);
                        }
                    }
                }
            }
        }
        None => {
            execute_operation(
                executor.as_ref(),
                &installation,
                tenant_id,
                operation,
                context,
            )
            .await?
        }
    };
    Ok(json_response(output))
}

async fn authorize_binding(
    ctx: &ServerRuntimeContext,
    tenant_id: Uuid,
    actor_id: Uuid,
    installation: &InstalledModuleArtifact,
    binding: &ModuleRuntimeBinding,
) -> Result<()> {
    let allowed = SeaOrmArtifactPermissionAuthorizer::new(ctx.db_clone())
        .is_authorized(
            tenant_id,
            actor_id,
            installation.installation_id,
            &binding.permission,
        )
        .await
        .map_err(|error| {
            tracing::error!(%error, "artifact binding RBAC authorization failed");
            Error::InternalServerError
        })?;
    if allowed {
        Ok(())
    } else {
        Err(http_error(rustok_web::HttpError::forbidden(
            "forbidden",
            "Permission denied for artifact binding",
        )))
    }
}

async fn execute_operation(
    executor: &dyn rustok_modules::ArtifactBindingExecutor,
    installation: &InstalledModuleArtifact,
    tenant_id: Uuid,
    operation: ArtifactOperation,
    context: ArtifactBindingExecutionContext,
) -> Result<serde_json::Value> {
    match operation {
        ArtifactOperation::Http { method, path, body } => dispatch_artifact_http_binding(
            executor,
            &installation.release,
            &installation.descriptor.bindings,
            ArtifactInstallationTarget::ExactInstallation {
                installation_id: installation.installation_id,
            },
            tenant_id,
            method,
            &path,
            body,
            context,
        )
        .await
        .map_err(map_dispatch_error),
        ArtifactOperation::Command { binding_id, input } => dispatch_artifact_command_binding(
            executor,
            &installation.release,
            &installation.descriptor.bindings,
            ArtifactInstallationTarget::ExactInstallation {
                installation_id: installation.installation_id,
            },
            tenant_id,
            &binding_id,
            input,
            context,
        )
        .await
        .map_err(map_dispatch_error),
    }
}

fn idempotency_request(
    tenant_id: Uuid,
    actor_id: Uuid,
    installation_id: Uuid,
    binding: &ModuleRuntimeBinding,
    key: Option<String>,
    request_digest: String,
) -> Result<Option<ArtifactBindingIdempotencyRequest>> {
    let key = match (binding.idempotency.clone(), key) {
        (ModuleBindingIdempotency::None, _) => return Ok(None),
        (ModuleBindingIdempotency::Required, None) => {
            return Err(Error::BadRequest(
                "Idempotency-Key header is required for this artifact binding".to_string(),
            ));
        }
        (ModuleBindingIdempotency::Required, Some(key))
        | (ModuleBindingIdempotency::BestEffort, Some(key)) => key,
        (ModuleBindingIdempotency::BestEffort, None) => return Ok(None),
    };
    Ok(Some(ArtifactBindingIdempotencyRequest {
        tenant_id,
        actor_id,
        installation_id,
        binding_id: binding.id.clone(),
        idempotency_key: key,
        request_digest,
    }))
}

fn header_idempotency_key(headers: &HeaderMap) -> Result<Option<String>> {
    headers
        .get(IDEMPOTENCY_KEY_HEADER)
        .map(|value| {
            value
                .to_str()
                .map(ToString::to_string)
                .map_err(|_| Error::BadRequest("Idempotency-Key header is invalid".to_string()))
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

fn map_dispatch_error(error: ModuleDispatchError) -> Error {
    match error {
        ModuleDispatchError::ArtifactHttpRouteUnavailable { .. }
        | ModuleDispatchError::ArtifactCommandUnavailable { .. } => Error::NotFound,
        ModuleDispatchError::ArtifactHttpRequestTooLarge { .. } => {
            http_error(rustok_web::HttpError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "artifact_http_request_too_large",
                "Artifact HTTP request exceeds the declared body limit",
            ))
        }
        _ => {
            tracing::error!(%error, "artifact binding dispatch failed");
            Error::InternalServerError
        }
    }
}

fn map_idempotency_error(error: ArtifactBindingIdempotencyError) -> Error {
    let message = error.to_string();
    match error {
        ArtifactBindingIdempotencyError::InvalidRequest => {
            Error::BadRequest("Artifact binding idempotency request is invalid".to_string())
        }
        ArtifactBindingIdempotencyError::Conflict => http_error(rustok_web::HttpError::new(
            StatusCode::CONFLICT,
            "artifact_binding_idempotency_conflict",
            "Idempotency-Key was reused for a different artifact binding request",
        )),
        ArtifactBindingIdempotencyError::InvalidStoredResponse
        | ArtifactBindingIdempotencyError::Storage(_) => {
            tracing::error!(%message, "artifact binding idempotency operation failed");
            Error::InternalServerError
        }
    }
}

pub fn router() -> crate::routes::ServerRouter {
    axum::Router::new()
        .route(
            "/api/artifacts/{installation_id}/commands/{binding_id}",
            post(dispatch_command),
        )
        .route(
            "/api/artifacts/{installation_id}/{*path}",
            any(dispatch_http),
        )
}
