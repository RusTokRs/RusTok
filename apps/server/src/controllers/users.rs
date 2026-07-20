use crate::error::{Error, Result, http_error};
use axum::response::Response;
use axum::{
    extract::{Path, Query},
    routing::get,
};
use rustok_api::{Permission, has_effective_permission};
use rustok_auth::{UserItem, UsersListParams, UsersResponse};
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

    let page = params.page.unwrap_or(1).max(1);
    let page_size = params.page_size.unwrap_or(20).clamp(1, 100);

    let mut query = users::Entity::find()
        .filter(UserColumn::TenantId.eq(tenant.id))
        .order_by_asc(UserColumn::CreatedAt);

    if let Some(search) = &params.search {
        let pattern = format!("%{}%", search);
        query = query.filter(
            sea_orm::Condition::any()
                .add(UserColumn::Email.like(&pattern))
                .add(UserColumn::Name.like(&pattern)),
        );
    }

    if let Some(status) = &params.status {
        query = query.filter(UserColumn::Status.eq(status.as_str()));
    }

    let paginator = query.paginate(ctx.db(), page_size);
    let total = paginator.num_items().await.unwrap_or(0);
    let rows = paginator.fetch_page(page - 1).await.unwrap_or_default();

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
    if !has_effective_permission(&current.permissions, &Permission::USERS_READ) {
        return Err(forbidden_error("Permission denied: users:read required"));
    }

    let user = users::Entity::find_by_id(user_id)
        .filter(UserColumn::TenantId.eq(tenant.id))
        .one(ctx.db())
        .await
        .map_err(|e| Error::Message(e.to_string()))?
        .ok_or(Error::NotFound)?;

    Ok(json_response(map_user(user)))
}

pub fn router() -> crate::routes::ServerRouter {
    axum::Router::new()
        .route("/api/users/", get(list_users))
        .route("/api/users/{id}", get(get_user))
}

fn forbidden_error(description: impl Into<String>) -> Error {
    http_error(rustok_web::HttpError::forbidden("forbidden", description))
}
