use crate::error::{Error, Result, http_error};
use axum::response::Response;
use axum::{
    extract::{Path, Query},
    routing::get,
};
use rustok_api::{Permission, has_effective_permission};
pub use rustok_auth::{UserItem, UsersListParams, UsersResponse};
use rustok_web::json_response;
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};
use uuid::Uuid;

use crate::extractors::{auth::CurrentUser, tenant::CurrentTenant};
use crate::models::users::{self, Column as UserColumn};
use crate::services::server_runtime_context::ServerRuntimeContext;

fn map_user(m: users::Model) -> UserItem {
    UserItem {
        id: m.id,
        email: m.email,
        name: m.name,
        status: m.status.to_string(),
        created_at: m.created_at.into(),
    }
}

pub fn normalize_page_params(page: Option<u64>, page_size: Option<u64>) -> (u64, u64) {
    let page = page.unwrap_or(1).max(1);
    let page_size = page_size.unwrap_or(20).clamp(1, 100);
    (page, page_size)
}

#[utoipa::path(get, path = "/api/users", tag = "users", security(("bearer_auth" = [])),
    params(UsersListParams),
    responses(
        (status = 200, description = "List of users", body = UsersResponse),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ))]
async fn list_users(
    axum::extract::State(ctx): axum::extract::State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Query(params): Query<UsersListParams>,
) -> Result<Response> {
    if !has_effective_permission(&current.permissions, &Permission::USERS_LIST) {
        return Err(forbidden_error("Permission denied: users:list required"));
    }

    let (page, page_size) = normalize_page_params(params.page, params.page_size);

    tracing::debug!(
        tenant_id = %tenant.id,
        user_id = %current.user.id,
        page = page,
        page_size = page_size,
        has_search = params.search.is_some(),
        has_status = params.status.is_some(),
        "Handling users list request"
    );

    let mut query = users::Entity::find()
        .filter(UserColumn::TenantId.eq(tenant.id))
        .order_by_asc(UserColumn::CreatedAt);

    if let Some(search) = params.search.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        let pattern = format!("%{}%", search);
        query = query.filter(
            sea_orm::Condition::any()
                .add(UserColumn::Email.like(&pattern))
                .add(UserColumn::Name.like(&pattern)),
        );
    }

    if let Some(status) = params.status.as_deref().map(str::trim).filter(|s| !s.is_empty()) {
        query = query.filter(UserColumn::Status.eq(status));
    }

    let paginator = query.paginate(ctx.db(), page_size);
    let total = paginator.num_items().await?;
    let rows = paginator.fetch_page(page - 1).await?;

    Ok(json_response(UsersResponse {
        users: rows.into_iter().map(map_user).collect(),
        total,
        page,
        page_size,
    }))
}

#[utoipa::path(get, path = "/api/users/{id}", tag = "users", security(("bearer_auth" = [])),
    params(("id" = Uuid, Path, description = "User ID")),
    responses(
        (status = 200, description = "User details", body = UserItem),
        (status = 404, description = "User not found"),
        (status = 401, description = "Unauthorized"),
        (status = 403, description = "Forbidden")
    ))]
async fn get_user(
    axum::extract::State(ctx): axum::extract::State<ServerRuntimeContext>,
    CurrentTenant(tenant): CurrentTenant,
    current: CurrentUser,
    Path(user_id): Path<Uuid>,
) -> Result<Response> {
    tracing::debug!(
        tenant_id = %tenant.id,
        current_user_id = %current.user.id,
        target_user_id = %user_id,
        "Handling get user request"
    );

    if !has_effective_permission(&current.permissions, &Permission::USERS_READ) {
        return Err(forbidden_error("Permission denied: users:read required"));
    }

    let user = users::Entity::find_by_id(user_id)
        .filter(UserColumn::TenantId.eq(tenant.id))
        .one(ctx.db())
        .await?
        .ok_or(Error::NotFound)?;

    Ok(json_response(map_user(user)))
}

pub fn router() -> crate::routes::ServerRouter {
    axum::Router::new()
        .route("/api/users", get(list_users))
        .route("/api/users/", get(list_users))
        .route("/api/users/{id}", get(get_user))
}

fn forbidden_error(description: impl Into<String>) -> Error {
    http_error(rustok_web::HttpError::forbidden("forbidden", description))
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_core::generate_id;

    #[test]
    fn test_normalize_page_params_defaults() {
        let (page, page_size) = normalize_page_params(None, None);
        assert_eq!(page, 1);
        assert_eq!(page_size, 20);
    }

    #[test]
    fn test_normalize_page_params_bounds() {
        let (page, page_size) = normalize_page_params(Some(0), Some(500));
        assert_eq!(page, 1);
        assert_eq!(page_size, 100);

        let (page, page_size) = normalize_page_params(Some(5), Some(0));
        assert_eq!(page, 5);
        assert_eq!(page_size, 1);

        let (page, page_size) = normalize_page_params(Some(3), Some(42));
        assert_eq!(page, 3);
        assert_eq!(page_size, 42);
    }

    #[test]
    fn test_map_user_projection() {
        let user_id = generate_id();
        let tenant_id = generate_id();
        let model = users::Model::default_service_user(user_id, tenant_id);

        let item = map_user(model.clone());
        assert_eq!(item.id, user_id);
        assert_eq!(item.email, model.email);
        assert_eq!(item.name, model.name);
        assert_eq!(item.status, "active");
    }
}
