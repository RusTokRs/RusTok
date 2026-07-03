use crate::error::{Error, Result};
use axum::{
    extract::{Path, Query},
    routing::get,
};
use axum::{http::StatusCode, response::Response};
use loco_rs::controller::Routes;
use loco_rs::controller::{format, ErrorDetail};
use sea_orm::{ColumnTrait, EntityTrait, PaginatorTrait, QueryFilter, QueryOrder};
use serde::{Deserialize, Serialize};
use utoipa::ToSchema;
use uuid::Uuid;

use crate::extractors::{auth::CurrentUser, tenant::CurrentTenant};
use crate::models::users::{self, Column as UserColumn};
use crate::services::rbac_service::RbacService;
use crate::services::server_runtime_context::ServerRuntimeContext;

#[derive(Debug, Serialize, ToSchema)]
pub struct UserItem {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub status: String,
    pub created_at: chrono::DateTime<chrono::Utc>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct UsersResponse {
    pub users: Vec<UserItem>,
    pub total: u64,
    pub page: u64,
    pub page_size: u64,
}

#[derive(Debug, Deserialize, ToSchema, utoipa::IntoParams)]
pub struct UsersListParams {
    pub page: Option<u64>,
    pub page_size: Option<u64>,
    pub search: Option<String>,
    pub status: Option<String>,
}

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
    let can_list = RbacService::has_permission(
        ctx.db(),
        &tenant.id,
        &current.user.id,
        &rustok_api::Permission::USERS_LIST,
    )
    .await
    .map_err(|error| {
        tracing::error!(
            tenant_id = %tenant.id,
            user_id = %current.user.id,
            permission = %rustok_api::Permission::USERS_LIST,
            %error,
            "Failed to evaluate RBAC permission for list_users"
        );
        Error::InternalServerError
    })?;

    if !can_list {
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
        // Filter by status string value (e.g. "active", "inactive", "banned")
        query = query.filter(UserColumn::Status.eq(status.as_str()));
    }

    let paginator = query.paginate(ctx.db(), page_size);
    let total = paginator.num_items().await.unwrap_or(0);
    let rows = paginator.fetch_page(page - 1).await.unwrap_or_default();

    format::json(UsersResponse {
        users: rows.into_iter().map(map_user).collect(),
        total,
        page,
        page_size,
    })
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
    let can_read = RbacService::has_permission(
        ctx.db(),
        &tenant.id,
        &current.user.id,
        &rustok_api::Permission::USERS_READ,
    )
    .await
    .map_err(|error| {
        tracing::error!(
            tenant_id = %tenant.id,
            user_id = %current.user.id,
            permission = %rustok_api::Permission::USERS_READ,
            %error,
            "Failed to evaluate RBAC permission for get_user"
        );
        Error::InternalServerError
    })?;

    if !can_read {
        return Err(forbidden_error("Permission denied: users:read required"));
    }

    let user = users::Entity::find_by_id(user_id)
        .filter(UserColumn::TenantId.eq(tenant.id))
        .one(ctx.db())
        .await
        .map_err(|e| Error::Message(e.to_string()))?
        .ok_or(Error::NotFound)?;

    format::json(map_user(user))
}

pub fn routes() -> Routes {
    Routes::new()
        .prefix("api/users")
        .add("/", get(list_users))
        .add("/{id}", get(get_user))
}

fn forbidden_error(description: impl Into<String>) -> Error {
    let description = description.into();
    Error::CustomError(
        StatusCode::FORBIDDEN,
        ErrorDetail::new("forbidden", description.as_str()),
    )
}
