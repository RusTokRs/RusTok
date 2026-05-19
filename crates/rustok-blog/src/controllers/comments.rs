use axum::{
    extract::{Path, State},
    Json,
};
use loco_rs::{app::AppContext, Error, Result};
use rustok_api::{
    loco::transactional_event_bus_from_context, AuthContext, RequestContext, TenantContext,
};
use rustok_core::Permission;
use uuid::Uuid;

use super::posts::ensure_blog_permission;
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
    State(ctx): State<AppContext>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Path(id): Path<Uuid>,
    Json(mut input): Json<ModerateCommentInput>,
) -> Result<Json<CommentResponse>> {
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

    let service = CommentService::new(ctx.db.clone(), transactional_event_bus_from_context(&ctx));
    let comment = service
        .moderate_comment(
            tenant.id,
            id,
            auth.security_context(),
            input,
            Some(tenant.default_locale.as_str()),
        )
        .await
        .map_err(|err| Error::BadRequest(err.to_string()))?;

    Ok(Json(comment))
}
