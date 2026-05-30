use axum::{
    extract::State,
    http::StatusCode,
    Json,
};
use loco_rs::{app::AppContext, Error, Result};
use rustok_api::{has_any_effective_permission, AuthContext};
use rustok_core::Permission;

use crate::{
    ForumWidgetContractService, ForumWidgetCatalogResponse, ForumWidgetPropsValidationResponse,
    ValidateForumWidgetPropsInput,
};

#[utoipa::path(
    get,
    path = "/api/forum/widgets/catalog",
    tag = "forum",
    responses(
        (status = 200, description = "Forum widget contract catalog", body = ForumWidgetCatalogResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn get_widget_catalog(
    _ctx: State<AppContext>,
    auth: AuthContext,
) -> Result<Json<ForumWidgetCatalogResponse>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_TOPICS_READ],
        "Permission denied: forum_topics:read required",
    )?;

    Ok(Json(ForumWidgetContractService::catalog()))
}

#[utoipa::path(
    post,
    path = "/api/forum/widgets/validate",
    tag = "forum",
    request_body = ValidateForumWidgetPropsInput,
    responses(
        (status = 200, description = "Widget props valid", body = ForumWidgetPropsValidationResponse),
        (status = 422, description = "Widget props invalid", body = ForumWidgetPropsValidationResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    )
)]
pub async fn validate_widget_props(
    _ctx: State<AppContext>,
    auth: AuthContext,
    Json(input): Json<ValidateForumWidgetPropsInput>,
) -> Result<(StatusCode, Json<ForumWidgetPropsValidationResponse>)> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_TOPICS_READ],
        "Permission denied: forum_topics:read required",
    )?;

    let response = ForumWidgetContractService::validate_props(input);
    let status = if response.valid {
        StatusCode::OK
    } else {
        StatusCode::UNPROCESSABLE_ENTITY
    };

    Ok((status, Json(response)))
}

fn ensure_forum_permission(
    auth: &AuthContext,
    permissions: &[Permission],
    message: &str,
) -> Result<()> {
    if !has_any_effective_permission(&auth.permissions, permissions) {
        return Err(Error::Unauthorized(message.to_string()));
    }

    Ok(())
}
