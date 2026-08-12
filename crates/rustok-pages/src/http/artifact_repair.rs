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

use crate::{
    ActivateRebuiltPageArtifactTransportResult, PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT,
    PAGE_ARTIFACT_BINDING_REPLACEMENT_IDEMPOTENCY_CONFLICT,
    PAGE_ARTIFACT_BINDING_REPLACEMENT_OPERATION_INTEGRITY,
    PAGE_ARTIFACT_BINDING_REPLACEMENT_TARGET_INVALID, PAGE_ARTIFACT_REBUILD_IDEMPOTENCY_CONFLICT,
    PAGE_ARTIFACT_REBUILD_OPERATION_INTEGRITY, PAGE_ARTIFACT_REBUILD_SOURCE_INVALID, PageService,
    PagesError, RebuildPageArtifactInput, RebuildPageArtifactTransportResult,
    ReplacePageArtifactBindingInput,
};

const PAGES_PERMISSION_DENIED: &str = "PAGES_PERMISSION_DENIED";
const PAGE_ARTIFACT_REPAIR_INVALID_INPUT: &str = "PAGE_ARTIFACT_REPAIR_INVALID_INPUT";
const PAGE_ARTIFACT_REBUILD_RUNTIME_REVIEW_REJECTED: &str =
    "PAGE_ARTIFACT_REBUILD_RUNTIME_REVIEW_REJECTED";
const PAGE_ARTIFACT_REBUILD_REPRODUCTION_MISMATCH: &str =
    "PAGE_ARTIFACT_REBUILD_REPRODUCTION_MISMATCH";
const PAGE_ARTIFACT_REBUILD_FAILED: &str = "PAGE_ARTIFACT_REBUILD_FAILED";
const PAGE_ARTIFACT_BINDING_REPLACEMENT_VERSION_CONFLICT: &str =
    "PAGE_ARTIFACT_BINDING_REPLACEMENT_VERSION_CONFLICT";
const PAGE_ARTIFACT_BINDING_REPLACEMENT_FAILED: &str = "PAGE_ARTIFACT_BINDING_REPLACEMENT_FAILED";

#[derive(Clone)]
pub struct PagesArtifactRepairHttpRuntime {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl PagesArtifactRepairHttpRuntime {
    fn from_host(runtime: &HostRuntimeContext) -> anyhow::Result<Self> {
        let event_bus = runtime
            .shared_get::<TransactionalEventBus>()
            .context("Pages artifact repair HTTP routes require TransactionalEventBus")?;
        Ok(Self {
            db: runtime.db_clone(),
            event_bus,
        })
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/pages/{id}/artifacts/rebuild",
    tag = "pages",
    params(("id" = Uuid, Path, description = "Page ID")),
    request_body = RebuildPageArtifactInput,
    responses(
        (status = 200, description = "Bounded append-only immutable artifact rebuild receipt", body = RebuildPageArtifactTransportResult),
        (status = 400, description = "Invalid rebuild input or reviewed runtime"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Tenant mismatch or tenant-wide pages:manage required"),
        (status = 404, description = "Page not found"),
        (status = 409, description = "Rebuild source, reproduction, or idempotency conflict"),
        (status = 500, description = "Rebuild receipt or persistence integrity failure")
    )
)]
pub async fn rebuild_page_artifact(
    State(runtime): State<PagesArtifactRepairHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<RebuildPageArtifactInput>,
) -> HttpResult<Json<RebuildPageArtifactTransportResult>> {
    ensure_current_tenant(&tenant, &auth)?;
    ensure_manage_permission(&auth)?;
    let result = PageService::new(runtime.db, runtime.event_bus)
        .rebuild_immutable_artifact(tenant.id, page_security(&auth), id, input)
        .await
        .map_err(map_rebuild_error)?;
    Ok(Json(result.into()))
}

#[utoipa::path(
    post,
    path = "/api/admin/pages/{id}/artifacts/activate",
    tag = "pages",
    params(("id" = Uuid, Path, description = "Page ID")),
    request_body = ReplacePageArtifactBindingInput,
    responses(
        (status = 200, description = "Bounded rebuilt artifact activation receipt", body = ActivateRebuiltPageArtifactTransportResult),
        (status = 400, description = "Invalid activation input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Tenant mismatch or tenant-wide pages:manage required"),
        (status = 404, description = "Page not found"),
        (status = 409, description = "Version, current binding, target, reuse, or idempotency conflict"),
        (status = 500, description = "Activation receipt or persistence integrity failure")
    )
)]
pub async fn activate_rebuilt_page_artifact(
    State(runtime): State<PagesArtifactRepairHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<ReplacePageArtifactBindingInput>,
) -> HttpResult<Json<ActivateRebuiltPageArtifactTransportResult>> {
    ensure_current_tenant(&tenant, &auth)?;
    ensure_manage_permission(&auth)?;
    let result = PageService::new(runtime.db, runtime.event_bus)
        .replace_rebuilt_artifact_binding(tenant.id, page_security(&auth), id, input)
        .await
        .map_err(map_activation_error)?;
    Ok(Json(result.into()))
}

pub(super) fn router(runtime: &HostRuntimeContext) -> anyhow::Result<axum::Router> {
    Ok(axum::Router::new()
        .route(
            "/api/admin/pages/{id}/artifacts/rebuild",
            axum::routing::post(rebuild_page_artifact),
        )
        .route(
            "/api/admin/pages/{id}/artifacts/activate",
            axum::routing::post(activate_rebuilt_page_artifact),
        )
        .with_state(PagesArtifactRepairHttpRuntime::from_host(runtime)?))
}

fn ensure_current_tenant(tenant: &TenantContext, auth: &AuthContext) -> HttpResult<()> {
    if auth.tenant_id == tenant.id {
        Ok(())
    } else {
        Err(HttpError::forbidden(
            PAGES_PERMISSION_DENIED,
            "Pages artifact repair routes must use the current tenant",
        ))
    }
}

fn ensure_manage_permission(auth: &AuthContext) -> HttpResult<()> {
    let permission = Permission::new(Resource::Pages, Action::Manage);
    if has_any_effective_permission(&auth.permissions, &[permission]) {
        Ok(())
    } else {
        Err(HttpError::forbidden(
            PAGES_PERMISSION_DENIED,
            "Permission denied: pages:manage required",
        ))
    }
}

fn page_security(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
}

fn map_rebuild_error(error: PagesError) -> HttpError {
    match error {
        PagesError::PageNotFound(_) => HttpError::not_found("PAGE_NOT_FOUND", "Page not found"),
        PagesError::Forbidden(_) => {
            HttpError::forbidden(PAGES_PERMISSION_DENIED, "Permission denied")
        }
        PagesError::Validation(_) => HttpError::new(
            StatusCode::BAD_REQUEST,
            PAGE_ARTIFACT_REPAIR_INVALID_INPUT,
            "Invalid immutable artifact rebuild input",
        ),
        PagesError::PublishRuntimeReviewInvalid(_) => HttpError::new(
            StatusCode::BAD_REQUEST,
            PAGE_ARTIFACT_REBUILD_RUNTIME_REVIEW_REJECTED,
            "Reviewed runtime does not match retained artifact provenance",
        ),
        PagesError::PublishIdempotencyConflict(_) => HttpError::new(
            StatusCode::CONFLICT,
            PAGE_ARTIFACT_REBUILD_IDEMPOTENCY_CONFLICT,
            "Artifact rebuild idempotency conflict",
        ),
        PagesError::PublishOperationIntegrity(detail)
            if detail.starts_with(PAGE_ARTIFACT_REBUILD_SOURCE_INVALID) =>
        {
            HttpError::new(
                StatusCode::CONFLICT,
                PAGE_ARTIFACT_REBUILD_SOURCE_INVALID,
                "Immutable artifact rebuild source is unavailable or invalid",
            )
        }
        PagesError::PublishOperationIntegrity(detail)
            if detail.starts_with(PAGE_ARTIFACT_REBUILD_OPERATION_INTEGRITY) =>
        {
            HttpError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                PAGE_ARTIFACT_REBUILD_OPERATION_INTEGRITY,
                "Stored artifact rebuild receipt failed integrity validation",
            )
        }
        PagesError::ArtifactIntegrity(_) => HttpError::new(
            StatusCode::CONFLICT,
            PAGE_ARTIFACT_REBUILD_REPRODUCTION_MISMATCH,
            "Immutable artifact could not be reproduced exactly",
        ),
        _ => HttpError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            PAGE_ARTIFACT_REBUILD_FAILED,
            "Immutable artifact rebuild failed",
        ),
    }
}

fn map_activation_error(error: PagesError) -> HttpError {
    match error {
        PagesError::PageNotFound(_) => HttpError::not_found("PAGE_NOT_FOUND", "Page not found"),
        PagesError::Forbidden(_) => {
            HttpError::forbidden(PAGES_PERMISSION_DENIED, "Permission denied")
        }
        PagesError::Validation(_) => HttpError::new(
            StatusCode::BAD_REQUEST,
            PAGE_ARTIFACT_REPAIR_INVALID_INPUT,
            "Invalid rebuilt artifact activation input",
        ),
        PagesError::VersionConflict { .. } => HttpError::new(
            StatusCode::CONFLICT,
            PAGE_ARTIFACT_BINDING_REPLACEMENT_VERSION_CONFLICT,
            "Page changed before rebuilt artifact activation",
        ),
        PagesError::RollbackIdempotencyConflict(_) => HttpError::new(
            StatusCode::CONFLICT,
            PAGE_ARTIFACT_BINDING_REPLACEMENT_IDEMPOTENCY_CONFLICT,
            "Artifact activation idempotency conflict",
        ),
        PagesError::RollbackTargetUnavailable(detail)
            if detail.starts_with(PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT) =>
        {
            HttpError::new(
                StatusCode::CONFLICT,
                PAGE_ARTIFACT_BINDING_REPLACEMENT_CURRENT_CONFLICT,
                "Current artifact binding no longer matches the activation request",
            )
        }
        PagesError::RollbackTargetUnavailable(detail)
            if detail.starts_with(PAGE_ARTIFACT_BINDING_REPLACEMENT_TARGET_INVALID) =>
        {
            HttpError::new(
                StatusCode::CONFLICT,
                PAGE_ARTIFACT_BINDING_REPLACEMENT_TARGET_INVALID,
                "Rebuilt artifact activation target is unavailable or invalid",
            )
        }
        PagesError::RollbackOperationIntegrity(detail)
            if detail.starts_with(PAGE_ARTIFACT_BINDING_REPLACEMENT_OPERATION_INTEGRITY) =>
        {
            HttpError::new(
                StatusCode::INTERNAL_SERVER_ERROR,
                PAGE_ARTIFACT_BINDING_REPLACEMENT_OPERATION_INTEGRITY,
                "Stored artifact activation receipt failed integrity validation",
            )
        }
        PagesError::ArtifactIntegrity(_) => HttpError::new(
            StatusCode::CONFLICT,
            PAGE_ARTIFACT_BINDING_REPLACEMENT_TARGET_INVALID,
            "Rebuilt artifact activation target is unavailable or invalid",
        ),
        _ => HttpError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            PAGE_ARTIFACT_BINDING_REPLACEMENT_FAILED,
            "Rebuilt artifact activation failed",
        ),
    }
}
