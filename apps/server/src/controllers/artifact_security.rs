use axum::{
    extract::{DefaultBodyLimit, Query, State},
    http::{HeaderValue, header::CACHE_CONTROL},
    response::Response,
    routing::{get, post},
    Json,
};
use rustok_api::{AuthContext, AuthPrincipalContext};
use rustok_modules::{ArtifactReleaseRef, ModuleArtifactSecurityError, ModuleControlPlane};
use rustok_web::json_response;
use serde::Deserialize;
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use crate::{
    error::{Error, Result},
    extractors::tenant::CurrentTenant,
    services::{
        artifact_security::{
            ArtifactSecurityAuthorizationError, ArtifactSecurityReceiptView,
            ArtifactSecuritySnapshotView, ServerArtifactSecurityAuthorizer,
            authorize_artifact_security_operator, build_module_artifact_security_command,
        },
        server_runtime_context::ServerRuntimeContext,
    },
};

const MAX_ARTIFACT_SECURITY_BODY_BYTES: usize = 8 * 1024;

#[derive(Debug, Deserialize, IntoParams, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactSecurityLookup {
    pub module_slug: String,
    pub version: String,
    pub payload_digest: String,
}

#[derive(Debug, Deserialize, ToSchema)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ArtifactSecurityCommandRequest {
    pub module_slug: String,
    pub version: String,
    pub payload_digest: String,
    pub expected_revision: String,
    pub policy_revision: String,
    pub reason_code: String,
    pub reason_detail: String,
    pub idempotency_key: Uuid,
}

#[derive(Clone, Copy)]
enum SecurityAction {
    Quarantine,
    ClearQuarantine,
    Revoke,
}

#[utoipa::path(
    get,
    path = "/api/modules/artifact-security",
    tag = "modules",
    security(("bearer_auth" = [])),
    params(ArtifactSecurityLookup),
    responses(
        (status = 200, description = "Current global artifact release security snapshot", body = ArtifactSecuritySnapshotView),
        (status = 400, description = "Invalid release identity"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Direct session user with modules:manage required"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn get_artifact_security_snapshot(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    auth: AuthContext,
    principal: AuthPrincipalContext,
    Query(query): Query<ArtifactSecurityLookup>,
) -> Result<Response> {
    authorize_artifact_security_operator(&ctx.db_clone(), &auth, principal, tenant.id)
        .await
        .map_err(map_authorization_error)?;

    let release = ArtifactReleaseRef {
        slug: query.module_slug,
        version: query.version,
        digest: query.payload_digest,
    };
    let snapshot = ModuleControlPlane::new(ctx.db_clone())
        .artifact_security_resolver()
        .resolve(&release)
        .await
        .map_err(map_artifact_security_error)?;

    Ok(private_json_response(ArtifactSecuritySnapshotView::from(
        snapshot,
    )))
}

#[utoipa::path(
    post,
    path = "/api/modules/artifact-security/quarantine",
    tag = "modules",
    security(("bearer_auth" = [])),
    request_body = ArtifactSecurityCommandRequest,
    responses(
        (status = 200, description = "Artifact release quarantined", body = ArtifactSecurityReceiptView),
        (status = 400, description = "Invalid command"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Direct session user with modules:manage required"),
        (status = 409, description = "Revision, transition or idempotency conflict"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn quarantine_artifact_release(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    auth: AuthContext,
    principal: AuthPrincipalContext,
    Json(input): Json<ArtifactSecurityCommandRequest>,
) -> Result<Response> {
    execute_security_command(ctx, tenant.id, auth, principal, input, SecurityAction::Quarantine)
        .await
}

#[utoipa::path(
    post,
    path = "/api/modules/artifact-security/clear-quarantine",
    tag = "modules",
    security(("bearer_auth" = [])),
    request_body = ArtifactSecurityCommandRequest,
    responses(
        (status = 200, description = "Artifact release quarantine cleared", body = ArtifactSecurityReceiptView),
        (status = 400, description = "Invalid command"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Direct session user with modules:manage required"),
        (status = 409, description = "Revision, transition or idempotency conflict"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn clear_artifact_release_quarantine(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    auth: AuthContext,
    principal: AuthPrincipalContext,
    Json(input): Json<ArtifactSecurityCommandRequest>,
) -> Result<Response> {
    execute_security_command(
        ctx,
        tenant.id,
        auth,
        principal,
        input,
        SecurityAction::ClearQuarantine,
    )
    .await
}

#[utoipa::path(
    post,
    path = "/api/modules/artifact-security/revoke",
    tag = "modules",
    security(("bearer_auth" = [])),
    request_body = ArtifactSecurityCommandRequest,
    responses(
        (status = 200, description = "Artifact release permanently revoked", body = ArtifactSecurityReceiptView),
        (status = 400, description = "Invalid command"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Direct session user with modules:manage required"),
        (status = 409, description = "Revision, transition or idempotency conflict"),
        (status = 500, description = "Internal server error")
    )
)]
pub async fn revoke_artifact_release(
    State(ctx): State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    auth: AuthContext,
    principal: AuthPrincipalContext,
    Json(input): Json<ArtifactSecurityCommandRequest>,
) -> Result<Response> {
    execute_security_command(ctx, tenant.id, auth, principal, input, SecurityAction::Revoke).await
}

async fn execute_security_command(
    ctx: ServerRuntimeContext,
    tenant_id: Uuid,
    auth: AuthContext,
    principal: AuthPrincipalContext,
    input: ArtifactSecurityCommandRequest,
    action: SecurityAction,
) -> Result<Response> {
    let expected_revision = input.expected_revision.parse::<u64>().map_err(|_| {
        Error::BadRequest(
            "expectedRevision must be an unsigned 64-bit decimal string".to_string(),
        )
    })?;
    let db = ctx.db_clone();
    let authorizer = ServerArtifactSecurityAuthorizer::new(db.clone(), &auth, principal, tenant_id)
        .map_err(map_authorization_error)?;
    let command = build_module_artifact_security_command(
        ArtifactReleaseRef {
            slug: input.module_slug,
            version: input.version,
            digest: input.payload_digest,
        },
        expected_revision,
        input.policy_revision,
        input.reason_code,
        input.reason_detail,
        auth.user_id,
        input.idempotency_key,
    );

    let service = ModuleControlPlane::new(db).artifact_security(authorizer);
    let receipt = match action {
        SecurityAction::Quarantine => service.quarantine(command).await,
        SecurityAction::ClearQuarantine => service.clear_quarantine(command).await,
        SecurityAction::Revoke => service.revoke(command).await,
    }
    .map_err(map_artifact_security_error)?;

    ctx.effective_policy_cache().clear();

    Ok(private_json_response(ArtifactSecurityReceiptView::from(
        receipt,
    )))
}

fn map_authorization_error(error: ArtifactSecurityAuthorizationError) -> Error {
    match error {
        ArtifactSecurityAuthorizationError::DirectSessionRequired => {
            Error::Forbidden("Direct authenticated user session required".to_string())
        }
        ArtifactSecurityAuthorizationError::ModulesManageRequired => {
            Error::Forbidden("Permission denied: modules:manage required".to_string())
        }
        ArtifactSecurityAuthorizationError::PermissionCheckFailed => {
            tracing::error!(
                error_code = "module_artifact_security_authorization_lookup_failed",
                "Artifact security authorization lookup failed"
            );
            Error::InternalServerError
        }
    }
}

fn map_artifact_security_error(error: ModuleArtifactSecurityError) -> Error {
    match error {
        ModuleArtifactSecurityError::InvalidCommand => {
            Error::BadRequest("Invalid artifact security command".to_string())
        }
        ModuleArtifactSecurityError::RevisionConflict { .. } => Error::Conflict(
            "Artifact security state changed; reload the snapshot before retrying".to_string(),
        ),
        ModuleArtifactSecurityError::AlreadyQuarantined => {
            Error::Conflict("Artifact release is already quarantined".to_string())
        }
        ModuleArtifactSecurityError::NotQuarantined => {
            Error::Conflict("Artifact release is not quarantined".to_string())
        }
        ModuleArtifactSecurityError::Revoked => {
            Error::Conflict("Artifact release is permanently revoked".to_string())
        }
        ModuleArtifactSecurityError::IdempotencyConflict => Error::Conflict(
            "Idempotency key conflicts with another artifact security command".to_string(),
        ),
        ModuleArtifactSecurityError::OperationInProgress => {
            Error::Conflict("Artifact security operation is still in progress".to_string())
        }
        ModuleArtifactSecurityError::RevisionOverflow => Error::InternalServerError,
        ModuleArtifactSecurityError::AuthorizationDenied(_) => {
            Error::Forbidden("Artifact security authorization denied".to_string())
        }
        ModuleArtifactSecurityError::Store(_) => {
            tracing::error!(
                error_code = "module_artifact_security_store_failed",
                "Artifact security owner operation failed"
            );
            Error::InternalServerError
        }
    }
}

fn private_json_response<T: serde::Serialize>(value: T) -> Response {
    let mut response = json_response(value);
    response.headers_mut().insert(
        CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response
}

pub fn router() -> crate::routes::ServerRouter {
    axum::Router::new()
        .route(
            "/api/modules/artifact-security",
            get(get_artifact_security_snapshot),
        )
        .route(
            "/api/modules/artifact-security/quarantine",
            post(quarantine_artifact_release),
        )
        .route(
            "/api/modules/artifact-security/clear-quarantine",
            post(clear_artifact_release_quarantine),
        )
        .route(
            "/api/modules/artifact-security/revoke",
            post(revoke_artifact_release),
        )
        .layer(DefaultBodyLimit::max(MAX_ARTIFACT_SECURITY_BODY_BYTES))
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::header::CACHE_CONTROL;

    #[test]
    fn release_security_responses_are_never_cacheable() {
        let response = private_json_response(serde_json::json!({"status":"clear"}));
        assert_eq!(
            response.headers().get(CACHE_CONTROL).and_then(|value| value.to_str().ok()),
            Some("private, no-store")
        );
    }

    #[test]
    fn owner_errors_preserve_conflict_and_validation_categories() {
        assert!(matches!(
            map_artifact_security_error(ModuleArtifactSecurityError::InvalidCommand),
            Error::BadRequest(_)
        ));
        assert!(matches!(
            map_artifact_security_error(ModuleArtifactSecurityError::RevisionConflict {
                expected: 1,
                current: 2,
            }),
            Error::Conflict(_)
        ));
        assert!(matches!(
            map_artifact_security_error(ModuleArtifactSecurityError::Revoked),
            Error::Conflict(_)
        ));
    }
}
