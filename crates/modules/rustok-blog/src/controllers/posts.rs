use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rustok_api::Permission;
use rustok_api::{AuthContext, RequestContext, TenantContext, has_any_effective_permission};
use rustok_telemetry::metrics;
use rustok_web::{HttpError, HttpResult};
use std::{collections::HashMap, time::Instant};
use uuid::Uuid;

use super::{BlogHttpRuntime, ensure_blog_module_enabled};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;

use crate::dto::BulkOperationResult;
use crate::state_machine::BlogPostStatus;
use crate::{
    ArchivePostInput, CreatePostInput, PostListQuery, PostResponse, PostService, UpdatePostInput,
};

fn security_context(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
}

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
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Query(mut query): Query<PostListQuery>,
) -> HttpResult<Json<crate::PostListResponse>> {
    tracing::debug!(
        tenant_id = %tenant.id,
        user_id = ?auth.user_id,
        "Handling list_posts HTTP request"
    );
    ensure_blog_module_enabled(&runtime, tenant.id).await?;
    ensure_blog_permission(
        &tenant,
        &auth,
        &[Permission::BLOG_POSTS_LIST],
        "Permission denied: blog_posts:list required",
    )?;

    query.locale = query.locale.or(Some(request_context.locale.clone()));
    let requested_limit = query.per_page.map(u64::from);
    let effective_limit = query.per_page() as u64;
    let service = PostService::new(runtime.db_clone(), runtime.event_bus());
    let list_started_at = Instant::now();
    let result = service
        .list_posts_with_locale_fallback(
            tenant.id,
            security_context(&auth),
            query,
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(crate::error::public::to_http_error)?;
    metrics::record_read_path_query(
        "http",
        "blog.list_posts",
        "service_list",
        list_started_at.elapsed().as_secs_f64(),
        result.items.len() as u64,
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
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Query(params): Query<HashMap<String, String>>,
) -> HttpResult<Json<PostResponse>> {
    tracing::debug!(
        tenant_id = %tenant.id,
        post_id = %id,
        "Handling get_post HTTP request"
    );
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_post_id",
            "Post ID must not be nil",
        ));
    }
    ensure_blog_module_enabled(&runtime, tenant.id).await?;
    ensure_blog_permission(
        &tenant,
        &auth,
        &[Permission::BLOG_POSTS_READ],
        "Permission denied: blog_posts:read required",
    )?;

    let locale = params
        .get("locale")
        .map(String::as_str)
        .unwrap_or(request_context.locale.as_str());
    let service = PostService::new(runtime.db_clone(), runtime.event_bus());
    let post = service
        .get_post_with_locale_fallback(
            tenant.id,
            security_context(&auth),
            id,
            locale,
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(crate::error::public::to_http_error)?;
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
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<CreatePostInput>,
) -> HttpResult<(StatusCode, Json<Uuid>)> {
    tracing::debug!(
        tenant_id = %tenant.id,
        user_id = ?auth.user_id,
        "Handling create_post HTTP request"
    );
    ensure_blog_module_enabled(&runtime, tenant.id).await?;
    ensure_blog_permission(
        &tenant,
        &auth,
        &[Permission::BLOG_POSTS_CREATE],
        "Permission denied: blog_posts:create required",
    )?;

    let service = PostService::new(runtime.db_clone(), runtime.event_bus());
    let post_id = service
        .create_post(tenant.id, security_context(&auth), input)
        .await
        .map_err(crate::error::public::to_http_error)?;
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
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdatePostInput>,
) -> HttpResult<()> {
    tracing::debug!(
        tenant_id = %tenant.id,
        post_id = %id,
        "Handling update_post HTTP request"
    );
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_post_id",
            "Post ID must not be nil",
        ));
    }
    ensure_blog_module_enabled(&runtime, tenant.id).await?;
    ensure_blog_permission(
        &tenant,
        &auth,
        &[Permission::BLOG_POSTS_UPDATE],
        "Permission denied: blog_posts:update required",
    )?;

    let service = PostService::new(runtime.db_clone(), runtime.event_bus());
    service
        .update_post(tenant.id, id, security_context(&auth), input)
        .await
        .map_err(crate::error::public::to_http_error)?;
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
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<StatusCode> {
    tracing::debug!(
        tenant_id = %tenant.id,
        post_id = %id,
        "Handling delete_post HTTP request"
    );
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_post_id",
            "Post ID must not be nil",
        ));
    }
    ensure_blog_module_enabled(&runtime, tenant.id).await?;
    ensure_blog_permission(
        &tenant,
        &auth,
        &[Permission::BLOG_POSTS_DELETE],
        "Permission denied: blog_posts:delete required",
    )?;

    let service = PostService::new(runtime.db_clone(), runtime.event_bus());
    service
        .delete_post(tenant.id, id, security_context(&auth))
        .await
        .map_err(crate::error::public::to_http_error)?;
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
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<()> {
    tracing::debug!(
        tenant_id = %tenant.id,
        post_id = %id,
        "Handling publish_post HTTP request"
    );
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_post_id",
            "Post ID must not be nil",
        ));
    }
    ensure_blog_module_enabled(&runtime, tenant.id).await?;
    ensure_blog_permission(
        &tenant,
        &auth,
        &[Permission::BLOG_POSTS_PUBLISH],
        "Permission denied: blog_posts:publish required",
    )?;

    let service = PostService::new(runtime.db_clone(), runtime.event_bus());
    service
        .publish_post(tenant.id, id, security_context(&auth))
        .await
        .map_err(crate::error::public::to_http_error)?;
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
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<()> {
    tracing::debug!(
        tenant_id = %tenant.id,
        post_id = %id,
        "Handling unpublish_post HTTP request"
    );
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_post_id",
            "Post ID must not be nil",
        ));
    }
    ensure_blog_module_enabled(&runtime, tenant.id).await?;
    ensure_blog_permission(
        &tenant,
        &auth,
        &[Permission::BLOG_POSTS_PUBLISH],
        "Permission denied: blog_posts:publish required",
    )?;

    let service = PostService::new(runtime.db_clone(), runtime.event_bus());
    service
        .unpublish_post(tenant.id, id, security_context(&auth))
        .await
        .map_err(crate::error::public::to_http_error)?;
    Ok(())
}

pub(super) fn ensure_blog_permission(
    tenant: &TenantContext,
    auth: &AuthContext,
    permissions: &[Permission],
    message: &str,
) -> HttpResult<()> {
    if tenant.id.is_nil() || auth.tenant_id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_tenant_id",
            "Tenant ID must not be nil",
        ));
    }
    if auth.tenant_id != tenant.id {
        return Err(HttpError::forbidden(
            "blog_tenant_mismatch",
            "Authenticated principal is not bound to the current tenant",
        ));
    }
    if !has_any_effective_permission(&auth.permissions, permissions) {
        return Err(HttpError::forbidden("blog_permission_denied", message));
    }

    Ok(())
}

#[utoipa::path(
    post,
    path = "/api/blog/posts/{id}/archive",
    tag = "blog",
    params(("id" = Uuid, Path, description = "Post ID")),
    request_body = ArchivePostInput,
    responses(
        (status = 200, description = "Post archived"),
        (status = 400, description = "Invalid lifecycle transition"),
        (status = 404, description = "Post not found"),
        (status = 409, description = "Concurrent modification"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn archive_post(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<ArchivePostInput>,
) -> HttpResult<()> {
    tracing::debug!(
        tenant_id = %tenant.id,
        post_id = %id,
        "Handling archive_post HTTP request"
    );
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_post_id",
            "Post ID must not be nil",
        ));
    }
    ensure_blog_module_enabled(&runtime, tenant.id).await?;
    ensure_blog_permission(
        &tenant,
        &auth,
        &[Permission::BLOG_POSTS_PUBLISH],
        "Permission denied: blog_posts:publish required",
    )?;

    PostService::new(runtime.db_clone(), runtime.event_bus())
        .archive_post(tenant.id, id, security_context(&auth), input.reason)
        .await
        .map_err(crate::error::public::to_http_error)
}

#[utoipa::path(
    post,
    path = "/api/blog/posts/{id}/restore",
    tag = "blog",
    params(("id" = Uuid, Path, description = "Post ID")),
    responses(
        (status = 200, description = "Post restored to draft"),
        (status = 400, description = "Invalid lifecycle transition"),
        (status = 404, description = "Post not found"),
        (status = 409, description = "Concurrent modification"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn restore_post(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<()> {
    tracing::debug!(
        tenant_id = %tenant.id,
        post_id = %id,
        "Handling restore_post HTTP request"
    );
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_post_id",
            "Post ID must not be nil",
        ));
    }
    ensure_blog_module_enabled(&runtime, tenant.id).await?;
    ensure_blog_permission(
        &tenant,
        &auth,
        &[Permission::BLOG_POSTS_PUBLISH],
        "Permission denied: blog_posts:publish required",
    )?;

    PostService::new(runtime.db_clone(), runtime.event_bus())
        .restore_post(tenant.id, id, security_context(&auth))
        .await
        .map_err(crate::error::public::to_http_error)
}

/// Get a post by preview token (unauthenticated)
///
/// The bearer token travels in the `x-blog-preview-token` header, never in
/// the URL: URLs are written to proxy/access logs, browser history and
/// `Referer` headers, all of which would leak a live preview credential.
#[utoipa::path(
    get,
    path = "/api/blog/preview",
    tag = "blog",
    params(
        ("x-blog-preview-token" = String, Header, description = "Preview token"),
        ("locale" = Option<String>, Query, description = "Requested locale")
    ),
    responses(
        (status = 200, description = "Post preview", body = PostResponse),
        (status = 404, description = "Token invalid, expired, or post not found")
    )
)]
pub async fn preview_post(
    State(runtime): State<BlogHttpRuntime>,
    headers: axum::http::HeaderMap,
    Query(params): Query<PreviewQueryParams>,
) -> HttpResult<Json<PostResponse>> {
    let token = headers
        .get("x-blog-preview-token")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_string();
    tracing::debug!(
        token_len = token.len(),
        "Handling preview_post HTTP request"
    );
    // The token is the authorization of this read, so the request carries no tenant
    // context. The token's own tenant is resolved inside the service, which also
    // enforces the Blog module gate for that tenant and serves drafts only.
    let locale = params
        .locale
        .unwrap_or_else(|| rustok_api::PLATFORM_FALLBACK_LOCALE.to_string());
    let service =
        crate::services::PreviewTokenService::new(runtime.db_clone(), runtime.event_bus());
    let post = service
        .get_post_by_token(&token, &locale, Some(rustok_api::PLATFORM_FALLBACK_LOCALE))
        .await
        .map_err(crate::error::public::to_http_error)?;

    match post {
        Some(post) => Ok(Json(post)),
        None => Err(HttpError::new(
            StatusCode::NOT_FOUND,
            "PREVIEW_NOT_FOUND",
            "Preview token is invalid, expired, or post not found",
        )),
    }
}

#[derive(serde::Deserialize, utoipa::IntoParams)]
pub struct PreviewQueryParams {
    pub locale: Option<String>,
}

/// Pin a blog post to the top of public listings
#[utoipa::path(
    post,
    path = "/api/blog/posts/{id}/pin",
    tag = "blog",
    params(
        ("id" = Uuid, Path, description = "Post ID")
    ),
    responses(
        (status = 200, description = "Post pinned"),
        (status = 404, description = "Post not found"),
        (status = 400, description = "Only published posts can be pinned"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn pin_post(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<()> {
    tracing::debug!(
        tenant_id = %tenant.id,
        post_id = %id,
        "Handling pin_post HTTP request"
    );
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_post_id",
            "Post ID must not be nil",
        ));
    }
    ensure_blog_module_enabled(&runtime, tenant.id).await?;
    ensure_blog_permission(
        &tenant,
        &auth,
        &[Permission::BLOG_POSTS_UPDATE],
        "Permission denied: blog_posts:update required",
    )?;

    let service = PostService::new(runtime.db_clone(), runtime.event_bus());
    service
        .pin_post(tenant.id, id, security_context(&auth))
        .await
        .map_err(crate::error::public::to_http_error)?;
    Ok(())
}

/// Unpin a blog post from the top of public listings
#[utoipa::path(
    post,
    path = "/api/blog/posts/{id}/unpin",
    tag = "blog",
    params(
        ("id" = Uuid, Path, description = "Post ID")
    ),
    responses(
        (status = 200, description = "Post unpinned"),
        (status = 404, description = "Post not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn unpin_post(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
) -> HttpResult<()> {
    tracing::debug!(
        tenant_id = %tenant.id,
        post_id = %id,
        "Handling unpin_post HTTP request"
    );
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_post_id",
            "Post ID must not be nil",
        ));
    }
    ensure_blog_module_enabled(&runtime, tenant.id).await?;
    ensure_blog_permission(
        &tenant,
        &auth,
        &[Permission::BLOG_POSTS_UPDATE],
        "Permission denied: blog_posts:update required",
    )?;

    let service = PostService::new(runtime.db_clone(), runtime.event_bus());
    service
        .unpin_post(tenant.id, id, security_context(&auth))
        .await
        .map_err(crate::error::public::to_http_error)?;
    Ok(())
}

// ─── Bulk operations ────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, ToSchema)]
pub struct BulkPostsInput {
    pub post_ids: Vec<Uuid>,
}

async fn bulk_transition(
    runtime: &BlogHttpRuntime,
    tenant: &TenantContext,
    auth: &AuthContext,
    post_ids: Vec<Uuid>,
    target: BlogPostStatus,
) -> HttpResult<Json<BulkOperationResult>> {
    ensure_blog_module_enabled(runtime, tenant.id).await?;
    ensure_blog_permission(
        tenant,
        auth,
        &[Permission::BLOG_POSTS_PUBLISH],
        "Permission denied: blog_posts:publish required",
    )?;
    let result = PostService::new(runtime.db_clone(), runtime.event_bus())
        .bulk_transition_posts(tenant.id, &post_ids, target, security_context(auth))
        .await
        .map_err(crate::error::public::to_http_error)?;
    Ok(Json(result))
}

#[utoipa::path(
    post,
    path = "/api/blog/posts/bulk/publish",
    tag = "blog",
    request_body = BulkPostsInput,
    responses(
        (status = 200, description = "Bulk operation result", body = BulkOperationResult),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn bulk_publish_posts(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<BulkPostsInput>,
) -> HttpResult<Json<BulkOperationResult>> {
    bulk_transition(
        &runtime,
        &tenant,
        &auth,
        input.post_ids,
        BlogPostStatus::Published,
    )
    .await
}

#[utoipa::path(
    post,
    path = "/api/blog/posts/bulk/unpublish",
    tag = "blog",
    request_body = BulkPostsInput,
    responses(
        (status = 200, description = "Bulk operation result", body = BulkOperationResult),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn bulk_unpublish_posts(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<BulkPostsInput>,
) -> HttpResult<Json<BulkOperationResult>> {
    ensure_blog_module_enabled(&runtime, tenant.id).await?;
    ensure_blog_permission(
        &tenant,
        &auth,
        &[Permission::BLOG_POSTS_PUBLISH],
        "Permission denied: blog_posts:publish required",
    )?;
    let result = PostService::new(runtime.db_clone(), runtime.event_bus())
        .bulk_unpublish_posts(tenant.id, &input.post_ids, security_context(&auth))
        .await
        .map_err(crate::error::public::to_http_error)?;
    Ok(Json(result))
}

#[utoipa::path(
    post,
    path = "/api/blog/posts/bulk/restore",
    tag = "blog",
    request_body = BulkPostsInput,
    responses(
        (status = 200, description = "Bulk operation result", body = BulkOperationResult),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn bulk_restore_posts(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<BulkPostsInput>,
) -> HttpResult<Json<BulkOperationResult>> {
    ensure_blog_module_enabled(&runtime, tenant.id).await?;
    ensure_blog_permission(
        &tenant,
        &auth,
        &[Permission::BLOG_POSTS_PUBLISH],
        "Permission denied: blog_posts:publish required",
    )?;
    let result = PostService::new(runtime.db_clone(), runtime.event_bus())
        .bulk_restore_posts(tenant.id, &input.post_ids, security_context(&auth))
        .await
        .map_err(crate::error::public::to_http_error)?;
    Ok(Json(result))
}

#[utoipa::path(
    post,
    path = "/api/blog/posts/bulk/archive",
    tag = "blog",
    request_body = BulkPostsInput,
    responses(
        (status = 200, description = "Bulk operation result", body = BulkOperationResult),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn bulk_archive_posts(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<BulkPostsInput>,
) -> HttpResult<Json<BulkOperationResult>> {
    bulk_transition(
        &runtime,
        &tenant,
        &auth,
        input.post_ids,
        BlogPostStatus::Archived,
    )
    .await
}

#[utoipa::path(
    post,
    path = "/api/blog/posts/bulk/delete",
    tag = "blog",
    request_body = BulkPostsInput,
    responses(
        (status = 200, description = "Bulk operation result", body = BulkOperationResult),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn bulk_delete_posts(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Json(input): Json<BulkPostsInput>,
) -> HttpResult<Json<BulkOperationResult>> {
    ensure_blog_module_enabled(&runtime, tenant.id).await?;
    ensure_blog_permission(
        &tenant,
        &auth,
        &[Permission::BLOG_POSTS_DELETE],
        "Permission denied: blog_posts:delete required",
    )?;
    let result = PostService::new(runtime.db_clone(), runtime.event_bus())
        .bulk_delete_posts(tenant.id, &input.post_ids, security_context(&auth))
        .await
        .map_err(crate::error::public::to_http_error)?;
    Ok(Json(result))
}

// ─── Preview tokens ─────────────────────────────────────────────────────────

#[derive(Debug, Deserialize, ToSchema)]
pub struct CreatePreviewTokenInput {
    pub ttl_hours: Option<i64>,
}

/// The create result is the only response that ever carries the plaintext
/// token, and it does so exactly once.
#[derive(Debug, Serialize, ToSchema)]
pub struct PreviewTokenCreatedResponse {
    pub id: Uuid,
    pub token: String,
    pub post_id: Uuid,
    pub expires_at: String,
    pub created_at: String,
}

/// A token listing entry: identity and expiry only, never a credential.
#[derive(Debug, Serialize, ToSchema)]
pub struct PreviewTokenSummaryResponse {
    pub id: Uuid,
    pub post_id: Uuid,
    pub expires_at: String,
    pub created_at: String,
}

fn preview_token_service(runtime: &BlogHttpRuntime) -> crate::services::PreviewTokenService {
    crate::services::PreviewTokenService::new(runtime.db_clone(), runtime.event_bus())
}

#[utoipa::path(
    post,
    path = "/api/blog/posts/{id}/preview-tokens",
    tag = "blog",
    params(("id" = Uuid, Path, description = "Post ID")),
    request_body = CreatePreviewTokenInput,
    responses(
        (status = 201, description = "Preview token created (plaintext shown once)", body = PreviewTokenCreatedResponse),
        (status = 400, description = "Invalid input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Post not found")
    )
)]
pub async fn create_preview_token(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(post_id): Path<Uuid>,
    Json(input): Json<CreatePreviewTokenInput>,
) -> HttpResult<(StatusCode, Json<PreviewTokenCreatedResponse>)> {
    if post_id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_post_id",
            "Post ID must not be nil",
        ));
    }
    ensure_blog_module_enabled(&runtime, tenant.id).await?;
    ensure_blog_permission(
        &tenant,
        &auth,
        &[Permission::BLOG_POSTS_UPDATE],
        "Permission denied: blog_posts:update required",
    )?;

    let token = preview_token_service(&runtime)
        .create_token(tenant.id, post_id, security_context(&auth), input.ttl_hours)
        .await
        .map_err(crate::error::public::to_http_error)?;
    let plaintext = token.token.ok_or_else(|| {
        crate::error::public::to_http_error(crate::BlogError::invariant(
            "preview token create result is missing its one-time plaintext",
        ))
    })?;
    Ok((
        StatusCode::CREATED,
        Json(PreviewTokenCreatedResponse {
            id: token.id,
            token: plaintext,
            post_id: token.post_id,
            expires_at: token.expires_at.to_rfc3339(),
            created_at: token.created_at.to_rfc3339(),
        }),
    ))
}

#[utoipa::path(
    get,
    path = "/api/blog/posts/{id}/preview-tokens",
    tag = "blog",
    params(("id" = Uuid, Path, description = "Post ID")),
    responses(
        (status = 200, description = "Preview tokens of the post (no secrets)", body = Vec<PreviewTokenSummaryResponse>),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Post not found")
    )
)]
pub async fn list_preview_tokens(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(post_id): Path<Uuid>,
) -> HttpResult<Json<Vec<PreviewTokenSummaryResponse>>> {
    if post_id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_post_id",
            "Post ID must not be nil",
        ));
    }
    ensure_blog_module_enabled(&runtime, tenant.id).await?;
    ensure_blog_permission(
        &tenant,
        &auth,
        &[Permission::BLOG_POSTS_UPDATE],
        "Permission denied: blog_posts:update required",
    )?;

    let tokens = preview_token_service(&runtime)
        .list_tokens(tenant.id, post_id, security_context(&auth))
        .await
        .map_err(crate::error::public::to_http_error)?;
    Ok(Json(
        tokens
            .into_iter()
            .map(|token| PreviewTokenSummaryResponse {
                id: token.id,
                post_id: token.post_id,
                expires_at: token.expires_at.to_rfc3339(),
                created_at: token.created_at.to_rfc3339(),
            })
            .collect(),
    ))
}

#[utoipa::path(
    delete,
    path = "/api/blog/preview-tokens/{id}",
    tag = "blog",
    params(("id" = Uuid, Path, description = "Preview token ID")),
    responses(
        (status = 204, description = "Preview token revoked"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Preview token not found")
    )
)]
pub async fn revoke_preview_token(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(token_id): Path<Uuid>,
) -> HttpResult<StatusCode> {
    if token_id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_token_id",
            "Token ID must not be nil",
        ));
    }
    ensure_blog_module_enabled(&runtime, tenant.id).await?;
    ensure_blog_permission(
        &tenant,
        &auth,
        &[Permission::BLOG_POSTS_UPDATE],
        "Permission denied: blog_posts:update required",
    )?;

    preview_token_service(&runtime)
        .revoke_token(tenant.id, token_id, security_context(&auth))
        .await
        .map_err(crate::error::public::to_http_error)?;
    Ok(StatusCode::NO_CONTENT)
}
