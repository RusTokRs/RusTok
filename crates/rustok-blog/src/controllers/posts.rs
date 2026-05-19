use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use loco_rs::{app::AppContext, Error, Result};
use rustok_api::{
    has_any_effective_permission, loco::transactional_event_bus_from_context, AuthContext,
    RequestContext, TenantContext,
};
use rustok_core::Permission;
use rustok_telemetry::metrics;
use std::{collections::HashMap, time::Instant};
use uuid::Uuid;

use crate::{CreatePostInput, PostListQuery, PostResponse, PostService, UpdatePostInput};

/// List blog posts
#[utoipa::path(
    get,
    path = "/api/blog/posts",
    tag = "blog",
    params(PostListQuery),
    responses(
        (status = 200, description = "List of posts", body = crate::PostListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn list_posts(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Query(mut query): Query<PostListQuery>,
) -> Result<Json<crate::PostListResponse>> {
    ensure_blog_permission(
        &auth,
        &[Permission::BLOG_POSTS_LIST],
        "Permission denied: blog_posts:list required",
    )?;

    query.locale = query.locale.or(Some(request_context.locale.clone()));
    let requested_limit = query.per_page.map(u64::from);
    let effective_limit = query.per_page() as u64;
    let service = PostService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let list_started_at = Instant::now();
    let result = service
        .list_posts_with_locale_fallback(
            tenant.id,
            auth.security_context(),
            query,
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    metrics::record_read_path_query(
        "http",
        "blog.list_posts",
        "service_list",
        list_started_at.elapsed().as_secs_f64(),
        result.total,
    );

    metrics::record_read_path_budget(
        "http",
        "blog.list_posts",
        requested_limit,
        effective_limit,
        result.items.len(),
    );

    Ok(Json(result))
}

/// Get a single blog post by ID
#[utoipa::path(
    get,
    path = "/api/blog/posts/{id}",
    tag = "blog",
    params(
        ("id" = Uuid, Path, description = "Post ID"),
        ("locale" = Option<String>, Query, description = "Requested locale")
    ),
    responses(
        (status = 200, description = "Post details", body = PostResponse),
        (status = 404, description = "Post not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn get_post(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Query(params): Query<HashMap<String, String>>,
) -> Result<Json<PostResponse>> {
    ensure_blog_permission(
        &auth,
        &[Permission::BLOG_POSTS_READ],
        "Permission denied: blog_posts:read required",
    )?;

    let locale = params
        .get("locale")
        .map(String::as_str)
        .unwrap_or(request_context.locale.as_str());
    let service = PostService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let post = service
        .get_post_with_locale_fallback(
            tenant.id,
            auth.security_context(),
            id,
            locale,
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    Ok(Json(post))
}

/// Create a new blog post
#[utoipa::path(
    post,
    path = "/api/blog/posts",
    tag = "blog",
    request_body = CreatePostInput,
    responses(
        (status = 201, description = "Post created", body = Uuid),
        (status = 400, description = "Invalid input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn create_post(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<CreatePostInput>,
) -> Result<(StatusCode, Json<Uuid>)> {
    ensure_blog_permission(
        &auth,
        &[Permission::BLOG_POSTS_CREATE],
        "Permission denied: blog_posts:create required",
    )?;

    let service = PostService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let post_id = service
        .create_post(tenant.id, auth.security_context(), input)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    Ok((StatusCode::CREATED, Json(post_id)))
}

/// Update an existing blog post
#[utoipa::path(
    put,
    path = "/api/blog/posts/{id}",
    tag = "blog",
    params(
        ("id" = Uuid, Path, description = "Post ID")
    ),
    request_body = UpdatePostInput,
    responses(
        (status = 200, description = "Post updated"),
        (status = 404, description = "Post not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn update_post(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdatePostInput>,
) -> Result<()> {
    ensure_blog_permission(
        &auth,
        &[Permission::BLOG_POSTS_UPDATE],
        "Permission denied: blog_posts:update required",
    )?;

    let service = PostService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    service
        .update_post(tenant.id, id, auth.security_context(), input)
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    Ok(())
}

/// Delete a blog post
#[utoipa::path(
    delete,
    path = "/api/blog/posts/{id}",
    tag = "blog",
    params(
        ("id" = Uuid, Path, description = "Post ID")
    ),
    responses(
        (status = 204, description = "Post deleted"),
        (status = 404, description = "Post not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn delete_post(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    ensure_blog_permission(
        &auth,
        &[Permission::BLOG_POSTS_DELETE],
        "Permission denied: blog_posts:delete required",
    )?;

    let service = PostService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    service
        .delete_post(tenant.id, id, auth.security_context())
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    Ok(StatusCode::NO_CONTENT)
}

/// Publish a blog post
#[utoipa::path(
    post,
    path = "/api/blog/posts/{id}/publish",
    tag = "blog",
    params(
        ("id" = Uuid, Path, description = "Post ID")
    ),
    responses(
        (status = 200, description = "Post published"),
        (status = 404, description = "Post not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn publish_post(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<()> {
    ensure_blog_permission(
        &auth,
        &[Permission::BLOG_POSTS_PUBLISH],
        "Permission denied: blog_posts:publish required",
    )?;

    let service = PostService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    service
        .publish_post(tenant.id, id, auth.security_context())
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    Ok(())
}

/// Unpublish a blog post
#[utoipa::path(
    post,
    path = "/api/blog/posts/{id}/unpublish",
    tag = "blog",
    params(
        ("id" = Uuid, Path, description = "Post ID")
    ),
    responses(
        (status = 200, description = "Post unpublished"),
        (status = 404, description = "Post not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn unpublish_post(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> Result<()> {
    ensure_blog_permission(
        &auth,
        &[Permission::BLOG_POSTS_PUBLISH],
        "Permission denied: blog_posts:publish required",
    )?;

    let service = PostService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    service
        .unpublish_post(tenant.id, id, auth.security_context())
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;
    Ok(())
}

pub(super) fn ensure_blog_permission(
    auth: &AuthContext,
    permissions: &[Permission],
    message: &str,
) -> Result<()> {
    if !has_any_effective_permission(&auth.permissions, permissions) {
        return Err(Error::Unauthorized(message.to_string()));
    }

    Ok(())
}
