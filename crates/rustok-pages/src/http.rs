use anyhow::Context;
use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use rustok_api::{
    Action, AuthContext, HostRuntimeContext, Permission, Resource, TenantContext,
    has_any_effective_permission,
};
use rustok_outbox::TransactionalEventBus;
use rustok_web::{HttpError, HttpResult};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use crate::error::BUILDER_FEATURE_DISABLED_ERROR_CODE;
use crate::{
    PAGE_BUILDER_PUBLISH_RUNTIME_MATERIALIZATION_MISMATCH,
    PAGE_BUILDER_PUBLISH_RUNTIME_REVIEW_INVALID, PAGE_BUILDER_PUBLISH_SANITIZE_FAILED,
    PAGE_PUBLISH_IDEMPOTENCY_CONFLICT, PAGE_PUBLISH_OPERATION_INTEGRITY, PageService, PagesError,
    PublishPageInput, PublishPageResult,
};

#[derive(Clone)]
pub struct PagesPublishHttpRuntime {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl PagesPublishHttpRuntime {
    fn from_host(runtime: &HostRuntimeContext) -> anyhow::Result<Self> {
        let event_bus = runtime
            .shared_get::<TransactionalEventBus>()
            .context("Pages atomic publish HTTP route requires TransactionalEventBus")?;
        Ok(Self {
            db: runtime.db_clone(),
            event_bus,
        })
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/pages/{id}/publish",
    tag = "pages",
    params(("id" = Uuid, Path, description = "Page ID")),
    request_body = PublishPageInput,
    responses(
        (status = 200, description = "Atomic reviewed page publish receipt", body = PublishPageResult),
        (status = 400, description = "Invalid review, sanitizer rejection, or publish input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Page not found"),
        (status = 409, description = "Revision, materialization, or idempotency conflict"),
        (status = 500, description = "Publish receipt or persistence integrity failure")
    )
)]
pub async fn publish_page(
    State(runtime): State<PagesPublishHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<PublishPageInput>,
) -> HttpResult<Json<PublishPageResult>> {
    ensure_publish_permission(&auth)?;
    PageService::new(runtime.db, runtime.event_bus)
        .publish_reviewed(tenant.id, page_security(&auth), id, input)
        .await
        .map(Json)
        .map_err(map_publish_error)
}

pub fn axum_router(runtime: &HostRuntimeContext) -> anyhow::Result<axum::Router> {
    let publish_runtime = PagesPublishHttpRuntime::from_host(runtime)?;
    let publish_router = axum::Router::new()
        .route(
            "/api/admin/pages/{id}/publish",
            axum::routing::post(publish_page),
        )
        .with_state(publish_runtime);
    Ok(crate::controllers::axum_router(runtime)?.merge(publish_router))
}

fn page_security(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
}

fn ensure_publish_permission(auth: &AuthContext) -> HttpResult<()> {
    let permission = Permission::new(Resource::Pages, Action::Publish);
    if has_any_effective_permission(&auth.permissions, &[permission]) {
        Ok(())
    } else {
        Err(HttpError::forbidden(
            "PAGES_PERMISSION_DENIED",
            "Permission denied: pages:publish required",
        ))
    }
}

fn map_publish_error(error: PagesError) -> HttpError {
    let message = error.to_string();
    match error {
        PagesError::PageNotFound(_) => HttpError::not_found("PAGE_NOT_FOUND", message),
        PagesError::VersionConflict { .. } => HttpError::new(
            StatusCode::CONFLICT,
            "PAGE_METADATA_VERSION_CONFLICT",
            message,
        ),
        PagesError::PublishRuntimeReviewInvalid(_) => HttpError::new(
            StatusCode::BAD_REQUEST,
            PAGE_BUILDER_PUBLISH_RUNTIME_REVIEW_INVALID,
            message,
        ),
        PagesError::PublishSanitize(_) => HttpError::new(
            StatusCode::BAD_REQUEST,
            PAGE_BUILDER_PUBLISH_SANITIZE_FAILED,
            message,
        ),
        PagesError::PublishRuntimeMaterializationMismatch(_) => HttpError::new(
            StatusCode::CONFLICT,
            PAGE_BUILDER_PUBLISH_RUNTIME_MATERIALIZATION_MISMATCH,
            message,
        ),
        PagesError::PublishIdempotencyConflict(_) => HttpError::new(
            StatusCode::CONFLICT,
            PAGE_PUBLISH_IDEMPOTENCY_CONFLICT,
            message,
        ),
        PagesError::PublishOperationIntegrity(_) => HttpError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            PAGE_PUBLISH_OPERATION_INTEGRITY,
            message,
        ),
        PagesError::FeatureDisabled { .. } => HttpError::new(
            StatusCode::CONFLICT,
            BUILDER_FEATURE_DISABLED_ERROR_CODE,
            message,
        ),
        PagesError::Forbidden(_) => HttpError::forbidden("PAGES_PERMISSION_DENIED", message),
        PagesError::Database(_)
        | PagesError::Core(_)
        | PagesError::Tenant(_)
        | PagesError::ArtifactIntegrity(_) => HttpError::internal(message),
        PagesError::Rich(rich) => {
            let code = rich
                .error_code
                .clone()
                .unwrap_or_else(|| "PAGES_PUBLISH_FAILED".to_string());
            match rich.kind {
                rustok_core::error::ErrorKind::NotFound => HttpError::not_found(code, message),
                rustok_core::error::ErrorKind::Forbidden => HttpError::forbidden(code, message),
                rustok_core::error::ErrorKind::Conflict => {
                    HttpError::new(StatusCode::CONFLICT, code, message)
                }
                rustok_core::error::ErrorKind::Database
                | rustok_core::error::ErrorKind::Internal => HttpError::internal(message),
                _ => HttpError::bad_request(code, message),
            }
        }
        PagesError::Validation(_)
        | PagesError::DuplicateSlug { .. }
        | PagesError::CannotDeletePublished
        | PagesError::MenuNotFound(_)
        | PagesError::Content(_) => HttpError::bad_request("PAGES_PUBLISH_FAILED", message),
    }
}
