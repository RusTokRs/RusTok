use axum::{Json, extract::State, http::StatusCode};
use rustok_api::{AuthContext, Permission, RequestContext, TenantContext};
use rustok_web::HttpResult;

use super::authorization::{ensure_blog_permission, security_context};
use super::{BlogHttpRuntime, ensure_blog_module_enabled};
use crate::{
    BlogWidgetCatalog, BlogWidgetInput, BlogWidgetPreview, BlogWidgetService, BlogWidgetValidation,
};

async fn authorize(
    runtime: &BlogHttpRuntime,
    tenant: &TenantContext,
    auth: &AuthContext,
) -> HttpResult<()> {
    ensure_blog_permission(
        tenant,
        auth,
        &[Permission::BLOG_POSTS_READ],
        "Permission denied: blog_posts:read required",
    )?;
    if auth.user_id.is_nil() {
        return Err(rustok_web::HttpError::unauthorized(
            "AUTHENTICATION_REQUIRED",
            "Authentication required",
        ));
    }
    ensure_blog_module_enabled(runtime, tenant.id).await
}

#[utoipa::path(get, path = "/api/blog/widgets/catalog", tag = "blog",
    responses((status = 200, description = "Blog widget schemas", body = BlogWidgetCatalog)))]
pub async fn catalog(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
) -> HttpResult<Json<BlogWidgetCatalog>> {
    authorize(&runtime, &tenant, &auth).await?;
    Ok(Json(BlogWidgetService::catalog()))
}

#[utoipa::path(post, path = "/api/blog/widgets/validate", tag = "blog", request_body = BlogWidgetInput,
    responses((status = 200, description = "Valid normalized props", body = BlogWidgetValidation), (status = 422, description = "Invalid props", body = BlogWidgetValidation)))]
pub async fn validate(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<BlogWidgetInput>,
) -> HttpResult<(StatusCode, Json<BlogWidgetValidation>)> {
    authorize(&runtime, &tenant, &auth).await?;
    let response = BlogWidgetService::validate(input);
    let status = if response.valid {
        StatusCode::OK
    } else {
        StatusCode::UNPROCESSABLE_ENTITY
    };
    Ok((status, Json(response)))
}

#[utoipa::path(post, path = "/api/blog/widgets/preview", tag = "blog", request_body = BlogWidgetInput,
    responses((status = 200, description = "Owner preview data", body = BlogWidgetPreview), (status = 422, description = "Invalid props", body = BlogWidgetPreview)))]
pub async fn preview(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request: RequestContext,
    Json(input): Json<BlogWidgetInput>,
) -> HttpResult<(StatusCode, Json<BlogWidgetPreview>)> {
    authorize(&runtime, &tenant, &auth).await?;
    ensure_request_context(tenant.id, auth.user_id, &request)?;
    let response = BlogWidgetService::new(runtime.db_clone(), runtime.event_bus())
        .preview(
            tenant.id,
            security_context(&auth),
            &request.locale,
            Some(&tenant.default_locale),
            request.channel_slug.as_deref(),
            input,
        )
        .await
        .map_err(crate::error::public::to_http_error)?;
    let status = if response.valid {
        StatusCode::OK
    } else {
        StatusCode::UNPROCESSABLE_ENTITY
    };
    Ok((status, Json(response)))
}

fn ensure_request_context(
    tenant: uuid::Uuid,
    user: uuid::Uuid,
    request: &RequestContext,
) -> HttpResult<()> {
    if request.tenant_id != tenant || request.user_id != Some(user) {
        return Err(rustok_web::HttpError::forbidden(
            "blog_widget_context_mismatch",
            "Widget preview request context does not match the authenticated principal",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use uuid::Uuid;

    #[test]
    fn preview_context_must_match_both_tenant_and_principal() {
        let tenant = Uuid::new_v4();
        let user = Uuid::new_v4();
        let mut request = RequestContext {
            tenant_id: tenant,
            user_id: Some(user),
            channel_id: None,
            channel_slug: None,
            channel_resolution_source: None,
            locale: "en".into(),
            correlation_id: "widget-context-test".into(),
        };
        assert!(ensure_request_context(tenant, user, &request).is_ok());
        request.tenant_id = Uuid::new_v4();
        assert_eq!(
            ensure_request_context(tenant, user, &request)
                .unwrap_err()
                .status,
            StatusCode::FORBIDDEN
        );
        request.tenant_id = tenant;
        request.user_id = Some(Uuid::new_v4());
        assert_eq!(
            ensure_request_context(tenant, user, &request)
                .unwrap_err()
                .status,
            StatusCode::FORBIDDEN
        );
        request.user_id = None;
        assert!(ensure_request_context(tenant, user, &request).is_err());
    }
}
