use axum::{
    Json,
    extract::{Query, State},
};
use rustok_api::Permission;
use rustok_api::{AuthContext, RequestContext, TenantContext, has_any_effective_permission};
use rustok_telemetry::metrics;
use rustok_web::{HttpError, HttpResult};
use serde::Deserialize;
use std::time::Instant;
use utoipa::IntoParams;

use crate::{
    CategoryService, CategoryTreeQuery, CategoryTreeResponse, ForumError,
    MAX_FORUM_CATEGORY_TREE_NODES,
};

#[derive(Debug, Deserialize, IntoParams)]
pub struct CategoryTreeParams {
    pub locale: Option<String>,
    pub fallback_locale: Option<String>,
}

#[utoipa::path(
    get,
    path = "/api/forum/categories/tree",
    tag = "forum",
    params(CategoryTreeParams),
    responses(
        (status = 200, description = "Canonical nested category tree", body = CategoryTreeResponse),
        (status = 400, description = "Invalid locale or corrupted tree"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden"),
        (status = 404, description = "Category not found"),
        (status = 500, description = "Internal forum read failure")
    )
)]
pub async fn get_category_tree(
    State(runtime): State<crate::controllers::ForumHttpRuntime>,
    tenant: TenantContext,
    auth: AuthContext,
    request_context: RequestContext,
    Query(params): Query<CategoryTreeParams>,
) -> HttpResult<Json<CategoryTreeResponse>> {
    ensure_forum_permission(
        &auth,
        &[Permission::FORUM_CATEGORIES_LIST],
        "Permission denied: forum_categories:list required",
    )?;

    let query = CategoryTreeQuery {
        locale: Some(params.locale.unwrap_or(request_context.locale)),
        fallback_locale: Some(
            params
                .fallback_locale
                .unwrap_or_else(|| tenant.default_locale.clone()),
        ),
    };
    let started_at = Instant::now();
    let tree = CategoryService::new(runtime.db_clone())
        .tree(tenant.id, forum_security(&auth), query)
        .await
        .map_err(category_tree_error)?;

    metrics::record_read_path_query(
        "http",
        "forum.category_tree",
        "service_tree",
        started_at.elapsed().as_secs_f64(),
        tree.total_nodes as u64,
    );
    metrics::record_read_path_budget(
        "http",
        "forum.category_tree",
        None,
        MAX_FORUM_CATEGORY_TREE_NODES,
        tree.total_nodes as usize,
    );

    Ok(Json(tree))
}

fn category_tree_error(error: ForumError) -> HttpError {
    crate::controllers::map_forum_error(error)
}

fn forum_security(auth: &AuthContext) -> rustok_core::SecurityContext {
    rustok_core::security_context_from_access_token(
        auth.user_id,
        &auth.grant_type,
        &auth.permissions,
    )
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
