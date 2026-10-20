//! Operator-visible bounded migration/diagnostics, through the selected Product owner.
use super::super::{
    CommerceHttpRuntime,
    common::ensure_permissions,
    products::{
        admin_product_command_context, admin_product_command_idempotency_key,
        ensure_admin_product_scope,
    },
};
use axum::{
    Json,
    extract::{Query, State},
    http::HeaderMap,
};
use rustok_api::{
    AuthContext, Permission, PortError, PortErrorKind, RequestContext, TenantContext,
};
use rustok_web::{HttpError, HttpResult};
use serde::Deserialize;
use uuid::Uuid;

fn ensure_scope(
    tenant: &TenantContext,
    auth: &AuthContext,
    request: &RequestContext,
) -> HttpResult<()> {
    // One Product scope rule for commands, reads and maintenance. Keep the
    // existing maintenance-specific envelope while sharing identity admission.
    ensure_admin_product_scope(tenant, auth, request).map_err(|_| {
        HttpError::forbidden(
            "product_media_tenant_mismatch",
            "Authenticated Product context does not match the request",
        )
    })
}
fn default_limit() -> u64 {
    100
}
#[derive(Debug, Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct AuditQuery {
    pub after_image_id: Option<Uuid>,
    pub after_hold_id: Option<Uuid>,
    #[serde(default = "default_limit")]
    pub limit: u64,
}
#[derive(Debug, Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct BackfillInput {
    pub after_image_id: Option<Uuid>,
    #[serde(default = "default_limit")]
    pub limit: u64,
}
fn map_error(error: PortError) -> HttpError {
    match error.kind {
        PortErrorKind::Forbidden => HttpError::forbidden(
            "product_media_maintenance_forbidden",
            "Permission denied for Product Media maintenance",
        ),
        PortErrorKind::Validation => HttpError::bad_request(
            "product_media_maintenance_invalid",
            "Invalid Product Media maintenance request",
        ),
        _ => HttpError::new(
            axum::http::StatusCode::SERVICE_UNAVAILABLE,
            "product_media_maintenance_unavailable",
            "Product Media maintenance is unavailable",
        ),
    }
}
fn maintenance(
    runtime: &CommerceHttpRuntime,
) -> HttpResult<std::sync::Arc<dyn rustok_product::ProductMediaMaintenancePort>> {
    runtime
        .product_catalog_command_runtime
        .media_maintenance_port()
        .ok_or_else(|| {
            HttpError::new(
                axum::http::StatusCode::SERVICE_UNAVAILABLE,
                "product_media_maintenance_unavailable",
                "Product Media maintenance is unavailable",
            )
        })
}
#[utoipa::path(get, path = "/admin/products/media-references/audit", tag = "admin", params(AuditQuery),
    responses((status = 200, body = rustok_product::ProductMediaAuditPage), (status = 400, description = "Invalid maintenance request"), (status = 401, description = "Authentication required"), (status = 403, description = "Forbidden"), (status = 503, description = "Owner unavailable")))]
pub async fn audit(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request: RequestContext,
    Query(query): Query<AuditQuery>,
) -> HttpResult<Json<rustok_product::ProductMediaAuditPage>> {
    ensure_scope(&tenant, &auth, &request)?;
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_READ],
        "Permission denied: products:read required",
    )?;
    let context = admin_product_command_context(
        tenant.id,
        &auth,
        &request,
        None,
        "audit_product_media_references",
        Uuid::new_v4().to_string(),
    );
    maintenance(&runtime)?
        .audit(
            context,
            query.after_image_id,
            query.after_hold_id,
            query.limit,
        )
        .await
        .map(Json)
        .map_err(map_error)
}
#[utoipa::path(post, path = "/admin/products/media-references/backfill", tag = "admin",
    params(("Idempotency-Key" = String, Header, description = "Caller-owned operator run key")), request_body = BackfillInput,
    responses((status = 200, body = rustok_product::ProductMediaBackfillPage), (status = 400, description = "Invalid maintenance request"), (status = 401, description = "Authentication required"), (status = 403, description = "Forbidden"), (status = 503, description = "Owner unavailable")))]
pub async fn backfill(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request: RequestContext,
    headers: HeaderMap,
    Json(input): Json<BackfillInput>,
) -> HttpResult<Json<rustok_product::ProductMediaBackfillPage>> {
    ensure_scope(&tenant, &auth, &request)?;
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_MANAGE],
        "Permission denied: products:manage required",
    )?;
    let context = admin_product_command_context(
        tenant.id,
        &auth,
        &request,
        None,
        "backfill_product_media_references",
        admin_product_command_idempotency_key(&headers)?,
    );
    maintenance(&runtime)?
        .backfill(context, input.after_image_id, input.limit)
        .await
        .map(Json)
        .map_err(map_error)
}

#[derive(Debug, Deserialize, utoipa::ToSchema, utoipa::IntoParams)]
pub struct ReleasesQuery {
    pub after_image_id: Option<Uuid>,
    #[serde(default = "default_limit")]
    pub limit: u64,
}
#[utoipa::path(get, path = "/admin/products/media-references/releases", tag = "admin", params(ReleasesQuery),
    responses((status = 200, body = rustok_product::ProductMediaReleasePage), (status = 400, description = "Invalid maintenance request"), (status = 401, description = "Authentication required"), (status = 403, description = "Forbidden"), (status = 503, description = "Owner unavailable")))]
pub async fn releases(
    State(runtime): State<CommerceHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request: RequestContext,
    Query(query): Query<ReleasesQuery>,
) -> HttpResult<Json<rustok_product::ProductMediaReleasePage>> {
    ensure_scope(&tenant, &auth, &request)?;
    ensure_permissions(
        &auth,
        &[Permission::PRODUCTS_READ],
        "Permission denied: products:read required",
    )?;
    let context = admin_product_command_context(
        tenant.id,
        &auth,
        &request,
        None,
        "list_product_media_releases",
        Uuid::new_v4().to_string(),
    );
    maintenance(&runtime)?
        .releases(context, query.after_image_id, query.limit)
        .await
        .map(Json)
        .map_err(map_error)
}
