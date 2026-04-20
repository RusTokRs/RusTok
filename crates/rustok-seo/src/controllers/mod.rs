use axum::{
    extract::{Path, Query, State},
    http::{header::CONTENT_TYPE, StatusCode},
    response::{IntoResponse, Response},
    Json,
};
use loco_rs::{app::AppContext, controller::Routes, Error, Result};
use rustok_api::{loco::transactional_event_bus_from_context, RequestContext, TenantContext};
use serde::Deserialize;

use crate::{SeoError, SeoPageContext, SeoService};

#[derive(Debug, Deserialize)]
pub struct SeoPageContextQuery {
    pub route: String,
}

pub async fn page_context_json(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    request: RequestContext,
    Query(query): Query<SeoPageContextQuery>,
) -> Result<Json<SeoPageContext>> {
    let service = SeoService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
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
    let service = SeoService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
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
    let service = SeoService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
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
    let service = SeoService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
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

    Routes::new().add("/page-context", get(page_context_json))
}

fn map_seo_http_error(error: SeoError) -> Error {
    match error {
        SeoError::Validation(message) => Error::BadRequest(message),
        SeoError::NotFound => Error::NotFound,
        SeoError::PermissionDenied => Error::Unauthorized("Permission denied".to_string()),
        SeoError::Database(error) => {
            tracing::warn!(error = %error, "SEO HTTP handler failed");
            let _ = StatusCode::INTERNAL_SERVER_ERROR;
            Error::Message(error.to_string())
        }
    }
}
