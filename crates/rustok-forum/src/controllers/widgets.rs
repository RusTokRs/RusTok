use axum::{Json, extract::State, http::StatusCode};
use rustok_api::Permission;
use rustok_api::{AuthContext, has_any_effective_permission};
use rustok_web::{HttpError, HttpResult};

use crate::{
    ForumWidgetCatalogResponse, ForumWidgetContractService, ForumWidgetPropsValidationResponse,
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
    _runtime: State<crate::controllers::ForumHttpRuntime>,
    auth: AuthContext,
) -> HttpResult<Json<ForumWidgetCatalogResponse>> {
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
    _runtime: State<crate::controllers::ForumHttpRuntime>,
    auth: AuthContext,
    Json(input): Json<ValidateForumWidgetPropsInput>,
) -> HttpResult<(StatusCode, Json<ForumWidgetPropsValidationResponse>)> {
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
) -> HttpResult<()> {
    if !has_any_effective_permission(&auth.permissions, permissions) {
        return Err(HttpError::forbidden(
            "forum_permission_denied",
            message.to_string(),
        ));
    }

    Ok(())
}
