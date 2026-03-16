use axum::{
    extract::{Path, Query, State},
    http::StatusCode,
    Json,
};
use loco_rs::app::AppContext;
use loco_rs::Error;
use loco_rs::Result;
use rustok_blog::{CreatePostInput, PostListQuery, PostResponse, PostService, UpdatePostInput};
use rustok_telemetry::metrics;
use std::time::Instant;
use uuid::Uuid;

use crate::extractors::rbac::{
    RequireBlogPostsCreate, RequireBlogPostsDelete, RequireBlogPostsList, RequireBlogPostsPublish,
    RequireBlogPostsRead, RequireBlogPostsUpdate,
};
use crate::services::event_bus::transactional_event_bus_from_context;
use crate::{common::RequestContext, context::TenantContext};

/// List blog posts
#[utoipa::path(
    get,
    path = "/api/blog/posts",
    tag = "blog",
    params(PostListQuery),
    responses(
        (status = 200, description = "List of posts", body = rustok_blog::PostListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn list_posts(
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    RequireBlogPostsList(user): RequireBlogPostsList,
    request_context: RequestContext,
    Query(mut query): Query<PostListQuery>,
) -> Result<Json<rustok_blog::PostListResponse>> {
    query.locale = query.locale.or(Some(request_context.locale.clone()));
    let requested_limit = query.per_page.map(u64::from);
    let effective_limit = query.per_page() as u64;
    let service = PostService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let list_started_at = Instant::now();
    let result = service
        .list_posts_with_locale_fallback(
            tenant.id,
            user.security_context(),
            query,
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
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
    _user: RequireBlogPostsRead,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> Result<Json<PostResponse>> {
    let locale = params
        .get("locale")
        .map(String::as_str)
        .unwrap_or(request_context.locale.as_str());
    let service = PostService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let post = service
        .get_post_with_locale_fallback(tenant.id, id, locale, Some(tenant.default_locale.as_str()))
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
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
    RequireBlogPostsCreate(user): RequireBlogPostsCreate,
    Json(input): Json<CreatePostInput>,
) -> Result<(StatusCode, Json<Uuid>)> {
    let service = PostService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let post_id = service
        .create_post(tenant.id, user.security_context(), input)
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
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
    RequireBlogPostsUpdate(user): RequireBlogPostsUpdate,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdatePostInput>,
) -> Result<()> {
    let service = PostService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    service
        .update_post(tenant.id, id, user.security_context(), input)
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
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
    RequireBlogPostsDelete(user): RequireBlogPostsDelete,
    Path(id): Path<Uuid>,
) -> Result<StatusCode> {
    let service = PostService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    service
        .delete_post(tenant.id, id, user.security_context())
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
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
    RequireBlogPostsPublish(user): RequireBlogPostsPublish,
    Path(id): Path<Uuid>,
) -> Result<()> {
    let service = PostService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    service
        .publish_post(tenant.id, id, user.security_context())
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
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
    RequireBlogPostsPublish(user): RequireBlogPostsPublish,
    Path(id): Path<Uuid>,
) -> Result<()> {
    let service = PostService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    service
        .unpublish_post(tenant.id, id, user.security_context())
        .await
        .map_err(|e| Error::BadRequest(e.to_string()))?;
    Ok(())
}
