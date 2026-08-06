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
    AuditPageArtifactsInput, PageArtifactIntegrityAuditResult, PageService, PagesError,
};

const PAGE_ARTIFACT_INTEGRITY_AUDIT_INVALID_INPUT: &str =
    "PAGE_ARTIFACT_INTEGRITY_AUDIT_INVALID_INPUT";
const PAGE_ARTIFACT_INTEGRITY_AUDIT_FAILED: &str = "PAGE_ARTIFACT_INTEGRITY_AUDIT_FAILED";

#[derive(Clone)]
pub(crate) struct PagesArtifactAuditHttpRuntime {
    db: DatabaseConnection,
    event_bus: TransactionalEventBus,
}

impl PagesArtifactAuditHttpRuntime {
    fn from_host(runtime: &HostRuntimeContext) -> anyhow::Result<Self> {
        let event_bus = runtime
            .shared_get::<TransactionalEventBus>()
            .context("Pages artifact audit HTTP route requires TransactionalEventBus")?;
        Ok(Self {
            db: runtime.db_clone(),
            event_bus,
        })
    }
}

#[utoipa::path(
    post,
    path = "/api/admin/pages/{id}/artifacts/audit",
    tag = "pages",
    params(("id" = Uuid, Path, description = "Page ID")),
    request_body = AuditPageArtifactsInput,
    responses(
        (status = 200, description = "Bounded immutable artifact integrity audit", body = PageArtifactIntegrityAuditResult),
        (status = 400, description = "Invalid audit input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Tenant mismatch or tenant-wide pages:manage required"),
        (status = 404, description = "Page not found"),
        (status = 500, description = "Audit could not be completed")
    )
)]
pub(crate) async fn audit_page_artifacts(
    State(runtime): State<PagesArtifactAuditHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<AuditPageArtifactsInput>,
) -> HttpResult<Json<PageArtifactIntegrityAuditResult>> {
    ensure_current_tenant(&tenant, &auth)?;
    ensure_manage_permission(&auth)?;
    PageService::new(runtime.db, runtime.event_bus)
        .audit_immutable_artifact_integrity(tenant.id, page_security(&auth), id, input)
        .await
        .map(Json)
        .map_err(map_artifact_audit_error)
}

pub(super) fn router(runtime: &HostRuntimeContext) -> anyhow::Result<axum::Router> {
    Ok(axum::Router::new()
        .route(
            "/api/admin/pages/{id}/artifacts/audit",
            axum::routing::post(audit_page_artifacts),
        )
        .with_state(PagesArtifactAuditHttpRuntime::from_host(runtime)?))
}

fn ensure_current_tenant(tenant: &TenantContext, auth: &AuthContext) -> HttpResult<()> {
    if auth.tenant_id == tenant.id {
        Ok(())
    } else {
        Err(HttpError::forbidden(
            "PAGES_PERMISSION_DENIED",
            "Pages artifact audits must use the current tenant",
        ))
    }
}

fn ensure_manage_permission(auth: &AuthContext) -> HttpResult<()> {
    let permission = Permission::new(Resource::Pages, Action::Manage);
    if has_any_effective_permission(&auth.permissions, &[permission]) {
        Ok(())
    } else {
        Err(HttpError::forbidden(
            "PAGES_PERMISSION_DENIED",
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

fn map_artifact_audit_error(error: PagesError) -> HttpError {
    match error {
        PagesError::PageNotFound(_) => {
            HttpError::not_found("PAGE_NOT_FOUND", "Page not found")
        }
        PagesError::Forbidden(_) => {
            HttpError::forbidden("PAGES_PERMISSION_DENIED", "Permission denied")
        }
        PagesError::Validation(_) => HttpError::new(
            StatusCode::BAD_REQUEST,
            PAGE_ARTIFACT_INTEGRITY_AUDIT_INVALID_INPUT,
            "Invalid immutable artifact audit input",
        ),
        _ => HttpError::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            PAGE_ARTIFACT_INTEGRITY_AUDIT_FAILED,
            "Immutable artifact audit failed",
        ),
    }
}
