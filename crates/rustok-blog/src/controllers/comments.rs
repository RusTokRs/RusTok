use axum::{
    Json,
    extract::{Path, State},
};
use rustok_api::Permission;
use rustok_api::{AuthContext, RequestContext, TenantContext};
use rustok_web::{HttpError, HttpResult};
use uuid::Uuid;

use super::{BlogHttpRuntime, posts::ensure_blog_permission};
use crate::{CommentResponse, CommentService, ModerateCommentInput};

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
    ensure_blog_permission(
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

    let service = CommentService::new(runtime.db_clone(), runtime.event_bus());
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
        .map_err(|err| HttpError::bad_request("blog_moderate_comment_failed", err.to_string()))?;

    Ok(Json(comment))
}
