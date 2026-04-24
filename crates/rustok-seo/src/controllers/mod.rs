use axum::{
    extract::{Path, Query, State},
    http::{
        header::{CONTENT_DISPOSITION, CONTENT_TYPE},
        Response as HttpResponse, StatusCode,
    },
    response::{IntoResponse, Response},
    Json,
};
use loco_rs::{app::AppContext, controller::Routes, Error, Result};
use rustok_api::{
    has_any_effective_permission, loco::transactional_event_bus_from_context, AuthContext,
    RequestContext, TenantContext,
};
use rustok_core::{ModuleRuntimeExtensions, Permission};
use serde::Deserialize;
use uuid::Uuid;

use crate::{
    SeoError, SeoPageContext, SeoService, SeoTargetCapabilityKind, SeoTargetRegistryEntry,
};

#[derive(Debug, Deserialize)]
pub struct SeoPageContextQuery {
    pub route: String,
}

#[derive(Debug, Deserialize)]
pub struct SeoTargetsQuery {
    pub capability: Option<SeoTargetCapabilityKind>,
}

pub async fn page_context_json(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    request: RequestContext,
    Query(query): Query<SeoPageContextQuery>,
) -> Result<Json<SeoPageContext>> {
    let service = seo_service_from_app_ctx(&ctx)?;
    let context = service
        .resolve_page_context_for_channel(
            &tenant,
            request.locale.as_str(),
            query.route.as_str(),
            request.channel_slug.as_deref(),
        )
        .await
        .map_err(map_seo_http_error)?
        .ok_or(Error::NotFound)?;
    Ok(Json(context))
}

pub async fn robots_txt(State(ctx): State<AppContext>, tenant: TenantContext) -> Result<Response> {
    let service = seo_service_from_app_ctx(&ctx)?;
    let body = service
        .render_robots(&tenant)
        .await
        .map_err(map_seo_http_error)?;
    Ok(([(CONTENT_TYPE, "text/plain; charset=utf-8")], body).into_response())
}

pub async fn sitemap_index(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
) -> Result<Response> {
    let service = seo_service_from_app_ctx(&ctx)?;
    if !service
        .load_settings(tenant.id)
        .await
        .map_err(map_seo_http_error)?
        .sitemap_enabled
    {
        return Err(Error::NotFound);
    }

    let file = match service
        .latest_sitemap_index(tenant.id)
        .await
        .map_err(map_seo_http_error)?
    {
        Some(file) => file,
        None => {
            service
                .generate_sitemaps(&tenant)
                .await
                .map_err(map_seo_http_error)?;
            service
                .latest_sitemap_index(tenant.id)
                .await
                .map_err(map_seo_http_error)?
                .ok_or(Error::NotFound)?
        }
    };

    Ok((
        [(CONTENT_TYPE, "application/xml; charset=utf-8")],
        file.content,
    )
        .into_response())
}

pub async fn sitemap_file(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    Path(name): Path<String>,
) -> Result<Response> {
    let service = seo_service_from_app_ctx(&ctx)?;
    let file = service
        .sitemap_file(tenant.id, name.as_str())
        .await
        .map_err(map_seo_http_error)?
        .ok_or(Error::NotFound)?;

    Ok((
        [(CONTENT_TYPE, "application/xml; charset=utf-8")],
        file.content,
    )
        .into_response())
}

pub async fn bulk_artifact_download(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path((job_id, artifact_id)): Path<(Uuid, Uuid)>,
) -> Result<HttpResponse<axum::body::Body>> {
    ensure_seo_permission(&auth, &[Permission::SEO_MANAGE], "seo:manage required")?;

    let service = seo_service_from_app_ctx(&ctx)?;
    let artifact = service
        .bulk_artifact(tenant.id, job_id, artifact_id)
        .await
        .map_err(map_seo_http_error)?
        .ok_or(Error::NotFound)?;

    HttpResponse::builder()
        .status(StatusCode::OK)
        .header(CONTENT_TYPE, artifact.mime_type)
        .header(
            CONTENT_DISPOSITION,
            format!("attachment; filename=\"{}\"", artifact.file_name),
        )
        .body(axum::body::Body::from(artifact.content))
        .map_err(|err| Error::Message(format!("failed to build SEO bulk artifact response: {err}")))
}

pub async fn targets_json(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Query(query): Query<SeoTargetsQuery>,
) -> Result<Json<Vec<SeoTargetRegistryEntry>>> {
    let service = seo_service_from_app_ctx(&ctx)?;
    ensure_seo_module_enabled(&service, tenant.id).await?;
    ensure_seo_permission(&auth, &[Permission::SEO_MANAGE], "seo:manage required")?;
    Ok(Json(service.target_registry_entries(query.capability)))
}

pub fn routes() -> Routes {
    use axum::routing::get;

    Routes::new()
        .add("/robots.txt", get(robots_txt))
        .add("/sitemap.xml", get(sitemap_index))
        .add("/sitemaps/{name}", get(sitemap_file))
        .nest("/api/seo", api_routes())
}

fn api_routes() -> Routes {
    use axum::routing::get;

    Routes::new()
        .add("/page-context", get(page_context_json))
        .add("/targets", get(targets_json))
        .add(
            "/bulk/jobs/{job_id}/artifacts/{artifact_id}",
            get(bulk_artifact_download),
        )
}

fn ensure_seo_permission(
    auth: &AuthContext,
    permissions: &[Permission],
    message: &str,
) -> Result<()> {
    if !has_any_effective_permission(&auth.permissions, permissions) {
        return Err(Error::Unauthorized(message.to_string()));
    }
    Ok(())
}

async fn ensure_seo_module_enabled(service: &SeoService, tenant_id: Uuid) -> Result<()> {
    if service
        .is_enabled(tenant_id)
        .await
        .map_err(map_seo_http_error)?
    {
        Ok(())
    } else {
        Err(Error::NotFound)
    }
}

fn map_seo_http_error(error: SeoError) -> Error {
    match error {
        SeoError::Validation(message) => Error::BadRequest(message),
        SeoError::Configuration(message) => {
            tracing::warn!(message = %message, "SEO runtime wiring is incomplete");
            Error::Message(message)
        }
        SeoError::NotFound => Error::NotFound,
        SeoError::PermissionDenied => Error::Unauthorized("Permission denied".to_string()),
        SeoError::Database(error) => {
            tracing::warn!(error = %error, "SEO HTTP handler failed");
            let _ = StatusCode::INTERNAL_SERVER_ERROR;
            Error::Message(error.to_string())
        }
    }
}

fn seo_service_from_app_ctx(ctx: &AppContext) -> Result<SeoService> {
    let extensions = ctx
        .shared_store
        .get::<std::sync::Arc<ModuleRuntimeExtensions>>()
        .ok_or_else(|| {
            map_seo_http_error(SeoError::configuration(
                "SEO runtime extensions are not initialized; host bootstrap must insert ModuleRuntimeExtensions",
            ))
        })?;

    SeoService::from_runtime_extensions(
        ctx.db.clone(),
        transactional_event_bus_from_context(ctx),
        extensions.as_ref(),
    )
    .map_err(map_seo_http_error)
}
