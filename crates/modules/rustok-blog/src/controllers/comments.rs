use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
};
use rustok_api::Permission;
use rustok_api::{AuthContext, RequestContext, TenantContext};
use rustok_web::{HttpError, HttpResult};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};
use uuid::Uuid;

use super::{BlogHttpRuntime, ensure_blog_module_enabled, posts::ensure_blog_permission};
use crate::dto::{
    CommentListItem, CreateCommentInput, ListCommentsFilter, UpdateCommentInput,
};
use crate::{CommentResponse, ModerateCommentInput};

fn security_context(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
}

#[derive(Debug, Serialize, ToSchema)]
pub struct CommentListResponse {
    pub items: Vec<CommentListItem>,
    pub total: u64,
}

#[derive(Debug, Deserialize, IntoParams)]
pub struct DeleteCommentParams {
    pub command_id: Uuid,
}

#[utoipa::path(
    get,
    path = "/api/blog/posts/{id}/comments",
    tag = "blog",
    params(
        ("id" = Uuid, Path, description = "Post ID"),
        ListCommentsFilter
    ),
    responses(
        (status = 200, description = "Comments of the post", body = CommentListResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Post not found")
    )
)]
pub async fn list_comments(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(post_id): Path<Uuid>,
    Query(mut filter): Query<ListCommentsFilter>,
) -> HttpResult<Json<CommentListResponse>> {
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
        &[Permission::COMMENTS_LIST],
        "Permission denied: comments:list required",
    )?;
    filter.locale = filter.locale.or(Some(request_context.locale.clone()));

    let (items, total) = runtime
        .comment_service()
        .list_for_post_with_locale_fallback(
            tenant.id,
            security_context(&auth),
            post_id,
            filter,
            Some(tenant.default_locale.as_str()),
            request_context.channel_slug.as_deref(),
        )
        .await
        .map_err(crate::error::public::to_http_error)?;
    Ok(Json(CommentListResponse { items, total }))
}

#[utoipa::path(
    post,
    path = "/api/blog/posts/{id}/comments",
    tag = "blog",
    params(("id" = Uuid, Path, description = "Post ID")),
    request_body = CreateCommentInput,
    responses(
        (status = 201, description = "Comment created", body = CommentResponse),
        (status = 400, description = "Invalid input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Post not found")
    )
)]
pub async fn create_comment(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    _request_context: RequestContext,
    Path(post_id): Path<Uuid>,
    Json(input): Json<CreateCommentInput>,
) -> HttpResult<(StatusCode, Json<CommentResponse>)> {
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
        &[Permission::COMMENTS_CREATE],
        "Permission denied: comments:create required",
    )?;

    // This is the authenticated owner surface (like the other comment
    // handlers here), so creation follows the owner command: the target post
    // must exist in the tenant, without the public publication/channel
    // boundary that `create_public_comment` applies to storefront callers.
    let comment = runtime
        .comment_service()
        .create_comment(tenant.id, security_context(&auth), post_id, input)
        .await
        .map_err(crate::error::public::to_http_error)?;
    Ok((StatusCode::CREATED, Json(comment)))
}

#[utoipa::path(
    get,
    path = "/api/blog/comments/{id}",
    tag = "blog",
    params(
        ("id" = Uuid, Path, description = "Comment ID"),
        ("locale" = Option<String>, Query, description = "Requested locale")
    ),
    responses(
        (status = 200, description = "Comment", body = CommentResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Comment not found")
    )
)]
pub async fn get_comment(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Query(params): Query<std::collections::HashMap<String, String>>,
) -> HttpResult<Json<CommentResponse>> {
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_comment_id",
            "Comment ID must not be nil",
        ));
    }
    ensure_blog_module_enabled(&runtime, tenant.id).await?;
    ensure_blog_permission(
        &tenant,
        &auth,
        &[Permission::COMMENTS_READ],
        "Permission denied: comments:read required",
    )?;
    let locale = params
        .get("locale")
        .map(String::as_str)
        .unwrap_or(request_context.locale.as_str());

    let comment = runtime
        .comment_service()
        .get_comment_with_locale_fallback(
            tenant.id,
            security_context(&auth),
            id,
            locale,
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(crate::error::public::to_http_error)?;
    Ok(Json(comment))
}

#[utoipa::path(
    put,
    path = "/api/blog/comments/{id}",
    tag = "blog",
    params(("id" = Uuid, Path, description = "Comment ID")),
    request_body = UpdateCommentInput,
    responses(
        (status = 200, description = "Comment updated", body = CommentResponse),
        (status = 400, description = "Invalid input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Comment not found")
    )
)]
pub async fn update_comment(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Json(input): Json<UpdateCommentInput>,
) -> HttpResult<Json<CommentResponse>> {
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_comment_id",
            "Comment ID must not be nil",
        ));
    }
    ensure_blog_module_enabled(&runtime, tenant.id).await?;
    ensure_blog_permission(
        &tenant,
        &auth,
        &[Permission::COMMENTS_UPDATE],
        "Permission denied: comments:update required",
    )?;

    let comment = runtime
        .comment_service()
        .update_comment(tenant.id, id, security_context(&auth), input)
        .await
        .map_err(crate::error::public::to_http_error)?;
    Ok(Json(comment))
}

#[utoipa::path(
    delete,
    path = "/api/blog/comments/{id}",
    tag = "blog",
    params(
        ("id" = Uuid, Path, description = "Comment ID"),
        DeleteCommentParams
    ),
    responses(
        (status = 204, description = "Comment deleted"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Comment not found")
    )
)]
pub async fn delete_comment(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    Path(id): Path<Uuid>,
    Query(params): Query<DeleteCommentParams>,
) -> HttpResult<StatusCode> {
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_comment_id",
            "Comment ID must not be nil",
        ));
    }
    ensure_blog_module_enabled(&runtime, tenant.id).await?;
    ensure_blog_permission(
        &tenant,
        &auth,
        &[Permission::COMMENTS_DELETE],
        "Permission denied: comments:delete required",
    )?;

    runtime
        .comment_service()
        .delete_comment(tenant.id, id, security_context(&auth), params.command_id)
        .await
        .map_err(crate::error::public::to_http_error)?;
    Ok(StatusCode::NO_CONTENT)
}

#[utoipa::path(
    post,
    path = "/api/blog/comments/{id}/moderate",
    tag = "blog",
    params(
        ("id" = Uuid, Path, description = "Comment ID")
    ),
    request_body = ModerateCommentInput,
    responses(
        (status = 200, description = "Comment moderated", body = CommentResponse),
        (status = 400, description = "Invalid input"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Comment not found")
    )
)]
pub async fn moderate_comment(
    State(runtime): State<BlogHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(mut input): Json<ModerateCommentInput>,
) -> HttpResult<Json<CommentResponse>> {
    tracing::debug!(
        tenant_id = %tenant.id,
        comment_id = %id,
        "Handling moderate_comment HTTP request"
    );
    if id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_comment_id",
            "Comment ID must not be nil",
        ));
    }
    if input.command_id.is_nil() {
        return Err(HttpError::bad_request(
            "invalid_command_id",
            "Command ID must not be nil",
        ));
    }
    ensure_blog_module_enabled(&runtime, tenant.id).await?;
    ensure_blog_permission(
        &tenant,
        &auth,
        &[Permission::BLOG_POSTS_MANAGE],
        "Permission denied: blog_posts:manage required",
    )?;

    let locale = input
        .locale
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToOwned::to_owned)
        .unwrap_or_else(|| request_context.locale.clone());
    input.locale = Some(locale);

    let service = runtime.comment_service();
    let comment = service
        .moderate_comment(
            tenant.id,
            id,
            rustok_core::security_context_from_access_token(
                auth.user_id,
                &auth.grant_type,
                &auth.permissions,
            ),
            input,
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(crate::error::public::to_http_error)?;

    Ok(Json(comment))
}
