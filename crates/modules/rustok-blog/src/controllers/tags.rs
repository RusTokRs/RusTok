use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rustok_api::Permission;
use rustok_api::{AuthContext, RequestContext, TenantContext};
use rustok_web::{HttpError, HttpResult};
use serde::Serialize;
use utoipa::ToSchema;
use uuid::Uuid;

use super::{BlogHttpRuntime, ensure_blog_module_enabled, posts::ensure_blog_permission};
use crate::dto::{CreateTagInput, ListTagsFilter, TagListItem, TagResponse, UpdateTagInput};
use crate::TagService;

fn security_context(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
}

#[derive(Debug, Serialize, ToSchema)]
pub struct TagListResponse {
    pub items: Vec<TagListItem>,
    pub total: u64,
}

#[utoipa::path(
    get,
    path = "/api/blog/tags",
    tag = "blog",
    params(ListTagsFilter),
    responses(
        (status = 200, description = "List of Blog tags", body = TagListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn list_tags(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Query(mut filter): Query<ListTagsFilter>,
) -> HttpResult<Json<TagListResponse>> {
    ensure_blog_module_enabled(&runtime, tenant.id).await?;
    ensure_blog_permission(
        &tenant,
        &auth,
        &[Permission::TAGS_LIST],
        "Permission denied: tags:list required",
    )?;
    filter.locale = filter.locale.or(Some(request_context.locale.clone()));

    let (items, total) = TagService::new(runtime.db_clone())
        .list_tags(tenant.id, security_context(&auth), filter)
        .await
        .map_err(crate::error::public::to_http_error)?;
    Ok(Json(TagListResponse { items, total }))
}

#[utoipa::path(
    get,
    path = "/api/blog/tags/{id}",
    tag = "blog",
    params(
        ("id" = Uuid, Path, description = "Tag ID"),
        ("locale" = Option<String>, Query, description = "Requested locale")
    ),
    responses(
        (status = 200, description = "Blog tag", body = TagResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Tag not found")
    )
)]
pub async fn get_tag(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> HttpResult<Json<TagResponse>> {
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tag_id",
            "Tag ID must not be nil",
        ));
    }
    ensure_blog_module_enabled(&runtime, tenant.id).await?;
    ensure_blog_permission(
        &tenant,
        &auth,
        &[Permission::TAGS_READ],
        "Permission denied: tags:read required",
    )?;
    let locale = params
        .get("locale")
        .map(String::as_str)
        .unwrap_or(request_context.locale.as_str());

    let tag = TagService::new(runtime.db_clone())
        .get_tag(tenant.id, security_context(&auth), id, locale)
        .await
        .map_err(crate::error::public::to_http_error)?;
    Ok(Json(tag))
}

#[utoipa::path(
    post,
    path = "/api/blog/tags",
    tag = "blog",
    request_body = CreateTagInput,
    responses(
        (status = 201, description = "Tag created", body = Uuid),
        (status = 400, description = "Invalid input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn create_tag(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<CreateTagInput>,
) -> HttpResult<(StatusCode, Json<Uuid>)> {
    ensure_blog_module_enabled(&runtime, tenant.id).await?;
    ensure_blog_permission(
        &tenant,
        &auth,
        &[Permission::TAGS_CREATE],
        "Permission denied: tags:create required",
    )?;

    let tag_id = TagService::new(runtime.db_clone())
        .create_tag(tenant.id, security_context(&auth), input)
        .await
        .map_err(crate::error::public::to_http_error)?;
    Ok((StatusCode::CREATED, Json(tag_id)))
}

#[utoipa::path(
    put,
    path = "/api/blog/tags/{id}",
    tag = "blog",
    params(("id" = Uuid, Path, description = "Tag ID")),
    request_body = UpdateTagInput,
    responses(
        (status = 200, description = "Tag updated", body = TagResponse),
        (status = 400, description = "Invalid input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Tag not found")
    )
)]
pub async fn update_tag(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateTagInput>,
) -> HttpResult<Json<TagResponse>> {
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tag_id",
            "Tag ID must not be nil",
        ));
    }
    ensure_blog_module_enabled(&runtime, tenant.id).await?;
    ensure_blog_permission(
        &tenant,
        &auth,
        &[Permission::TAGS_UPDATE],
        "Permission denied: tags:update required",
    )?;

    let tag = TagService::new(runtime.db_clone())
        .update_tag(tenant.id, id, security_context(&auth), input)
        .await
        .map_err(crate::error::public::to_http_error)?;
    Ok(Json(tag))
}

#[utoipa::path(
    delete,
    path = "/api/blog/tags/{id}",
    tag = "blog",
    params(("id" = Uuid, Path, description = "Tag ID")),
    responses(
        (status = 204, description = "Tag deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Tag not found")
    )
)]
pub async fn delete_tag(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<StatusCode> {
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tag_id",
            "Tag ID must not be nil",
        ));
    }
    ensure_blog_module_enabled(&runtime, tenant.id).await?;
    ensure_blog_permission(
        &tenant,
        &auth,
        &[Permission::TAGS_DELETE],
        "Permission denied: tags:delete required",
    )?;

    TagService::new(runtime.db_clone())
        .delete_tag(tenant.id, id, security_context(&auth))
        .await
        .map_err(crate::error::public::to_http_error)?;
    Ok(StatusCode::NO_CONTENT)
}
