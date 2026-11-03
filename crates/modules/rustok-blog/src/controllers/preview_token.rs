use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
};
use rustok_api::{AuthContext, Permission, TenantContext};
use rustok_web::{HttpError, HttpResult};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use super::authorization::{ensure_blog_permission, security_context};
use super::{BlogHttpRuntime, ensure_blog_module_enabled};

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
