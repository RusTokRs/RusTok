//! Shared server adapter for an admitted artifact binding invocation.
//!
//! HTTP and GraphQL select their transport inputs independently, but both must
//! pass through the same effective-policy, dynamic-RBAC, idempotency, schema,
//! and audited sandbox boundary. This adapter exposes no route or descriptor
//! enumeration surface.

use axum::http::StatusCode;
use rustok_core::ModuleRegistry;
use rustok_modules::{
    ArtifactBindingExecutionContext, ArtifactBindingIdempotencyClaim,
    ArtifactBindingIdempotencyError, ArtifactBindingIdempotencyRequest, ArtifactInstallationTarget,
    InstalledModuleArtifact, ModuleBindingIdempotency, ModuleCommandContext, ModuleControlPlane,
    ModuleDispatchError, ModuleHttpMethod, ModuleRuntimeBinding, SharedArtifactBindingExecutor,
    artifact_binding_request_digest, dispatch_artifact_command_binding,
    dispatch_artifact_http_binding,
};
use uuid::Uuid;

use crate::{
    error::{Error, Result, http_error},
    services::{
        artifact_ui::require_artifact_permission, server_runtime_context::ServerRuntimeContext,
    },
};

/// One transport-neutral invocation shape selected only after the host has
/// resolved its admitted binding.
pub(crate) enum ArtifactBindingOperation {
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

impl ArtifactBindingOperation {
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

/// Executes an already-resolved binding through the sole admitted runtime path.
/// `idempotency_key` is transport-normalized rather than read from an HTTP
/// header, so GraphQL cannot bypass the durable binding receipt contract.
pub(crate) async fn dispatch_artifact_binding_operation(
    ctx: &ServerRuntimeContext,
    tenant_id: Uuid,
    actor_id: Uuid,
    installation: &InstalledModuleArtifact,
    binding: &ModuleRuntimeBinding,
    idempotency_key: Option<Uuid>,
    operation: ArtifactBindingOperation,
) -> Result<serde_json::Value> {
    let registry = ctx.shared_get::<ModuleRegistry>().ok_or_else(|| {
        Error::Message("artifact dispatch requires the initialized module registry".to_string())
    })?;
    let policy = crate::services::effective_module_policy::EffectiveModulePolicyService::resolve(
        ctx.db(),
        &registry,
        tenant_id,
    )
    .await
    .map_err(|error| {
        tracing::error!(%error, "artifact dispatch effective-policy resolution failed");
        Error::InternalServerError
    })?;
    if !policy.contains(&installation.release.slug) {
        return Err(http_error(rustok_web::HttpError::forbidden(
            "module_policy_denied",
            "The module is not enabled by the effective module policy",
        )));
    }
    authorize_binding(ctx, tenant_id, actor_id, installation, binding).await?;
    let executor = ctx
        .shared_get::<SharedArtifactBindingExecutor>()
        .ok_or_else(|| Error::Message("artifact binding runtime is not initialized".to_string()))?;
    let request = idempotency_request(
        tenant_id,
        actor_id,
        installation.installation_id,
        binding,
        idempotency_key,
        operation.request_digest()?,
    )?;
    let binding_execution_context = ArtifactBindingExecutionContext {
        actor_id: Some(actor_id.to_string()),
        trace_id: request
            .as_ref()
            .map(|request| request.context.trace_id.clone())
            .or_else(|| rustok_telemetry::current_trace_id()),
    };
    match request {
        Some(request) => {
            let store = ModuleControlPlane::new(ctx.db_clone()).artifact_binding_idempotency();
            match store.claim(&request).await.map_err(map_idempotency_error)? {
                ArtifactBindingIdempotencyClaim::Replay { response } => Ok(response),
                ArtifactBindingIdempotencyClaim::InProgress => {
                    Err(http_error(rustok_web::HttpError::new(
                        StatusCode::CONFLICT,
                        "artifact_binding_in_progress",
                        "An identical artifact binding request is still executing",
                    )))
                }
                ArtifactBindingIdempotencyClaim::Execute { operation_id } => {
                    let result = execute_operation(
                        executor.as_ref(),
                        installation,
                        tenant_id,
                        operation,
                        binding_execution_context,
                    )
                    .await;
                    match result {
                        Ok(output) => {
                            store
                                .complete(&request, operation_id, &output)
                                .await
                                .map_err(map_idempotency_error)?;
                            Ok(output)
                        }
                        Err(error) => {
                            let _ = store.abandon(&request, operation_id).await;
                            Err(error)
                        }
                    }
                }
            }
        }
        None => {
            execute_operation(
                executor.as_ref(),
                installation,
                tenant_id,
                operation,
                binding_execution_context,
            )
            .await
        }
    }
}

async fn authorize_binding(
    ctx: &ServerRuntimeContext,
    tenant_id: Uuid,
    actor_id: Uuid,
    installation: &InstalledModuleArtifact,
    binding: &ModuleRuntimeBinding,
) -> Result<()> {
    require_artifact_permission(ctx, tenant_id, actor_id, installation, &binding.permission).await
}

async fn execute_operation(
    executor: &dyn rustok_modules::ArtifactBindingExecutor,
    installation: &InstalledModuleArtifact,
    tenant_id: Uuid,
    operation: ArtifactBindingOperation,
    context: ArtifactBindingExecutionContext,
) -> Result<serde_json::Value> {
    match operation {
        ArtifactBindingOperation::Http { method, path, body } => dispatch_artifact_http_binding(
            executor,
            rustok_modules::ArtifactHttpBindingRequest {
                release: &installation.release,
                bindings: &installation.descriptor.bindings,
                target: ArtifactInstallationTarget::ExactInstallation {
                    installation_id: installation.installation_id,
                },
                tenant_id,
                method,
                path: &path,
                body,
                context,
            },
        )
        .await
        .map_err(map_dispatch_error),
        ArtifactBindingOperation::Command { binding_id, input } => {
            dispatch_artifact_command_binding(
                executor,
                rustok_modules::ArtifactCommandBindingRequest {
                    release: &installation.release,
                    bindings: &installation.descriptor.bindings,
                    target: ArtifactInstallationTarget::ExactInstallation {
                        installation_id: installation.installation_id,
                    },
                    tenant_id,
                    binding_id: &binding_id,
                    input,
                    context,
                },
            )
            .await
            .map_err(map_dispatch_error)
        }
    }
}

fn idempotency_request(
    tenant_id: Uuid,
    actor_id: Uuid,
    installation_id: Uuid,
    binding: &ModuleRuntimeBinding,
    key: Option<Uuid>,
    request_digest: String,
) -> Result<Option<ArtifactBindingIdempotencyRequest>> {
    let key = match (binding.idempotency.clone(), key) {
        (ModuleBindingIdempotency::None, _) => return Ok(None),
        (ModuleBindingIdempotency::Required, None) => {
            return Err(Error::BadRequest(
                "Idempotency key is required for this artifact binding".to_string(),
            ));
        }
        (ModuleBindingIdempotency::Required, Some(key))
        | (ModuleBindingIdempotency::BestEffort, Some(key)) => key,
        (ModuleBindingIdempotency::BestEffort, None) => return Ok(None),
    };
    Ok(Some(ArtifactBindingIdempotencyRequest {
        context: artifact_binding_command_context(tenant_id, actor_id, key),
        installation_id,
        binding_id: binding.id.clone(),
        request_digest,
    }))
}

fn artifact_binding_command_context(
    tenant_id: Uuid,
    actor_id: Uuid,
    idempotency_key: Uuid,
) -> ModuleCommandContext {
    let trace_id = rustok_telemetry::current_trace_id()
        .filter(|trace_id| !trace_id.trim().is_empty())
        .unwrap_or_else(|| format!("artifact-binding:{idempotency_key}"));
    ModuleCommandContext {
        actor_id,
        tenant_id: Some(tenant_id),
        trace_id,
        correlation_id: idempotency_key,
        idempotency_key,
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
            "Idempotency key was reused for a different artifact binding request",
        )),
        ArtifactBindingIdempotencyError::InvalidStoredResponse
        | ArtifactBindingIdempotencyError::Storage(_) => {
            tracing::error!(%message, "artifact binding idempotency operation failed");
            Error::InternalServerError
        }
    }
}
