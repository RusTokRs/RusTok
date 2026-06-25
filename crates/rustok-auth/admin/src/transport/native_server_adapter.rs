use leptos::prelude::*;
use serde::{Deserialize, Serialize};

use crate::model::{GraphqlUserResponse, GraphqlUsersResponse, OAuthApp};

#[cfg(feature = "ssr")]
use crate::model::{
    AppType, GraphqlPageInfo, GraphqlUser, GraphqlUserEdge, GraphqlUsersConnection,
};
#[cfg(feature = "ssr")]
use sea_orm::{ConnectionTrait, DbBackend, Statement};
#[cfg(feature = "ssr")]
use serde_json::Value;
#[cfg(feature = "ssr")]
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub enum ApiError {
    ServerFn(String),
    Unauthorized,
    Http(String),
    Network,
    Graphql(String),
}

impl std::fmt::Display for ApiError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ServerFn(error) => write!(f, "{error}"),
            Self::Unauthorized => write!(f, "Unauthorized"),
            Self::Http(error) => write!(f, "HTTP error: {error}"),
            Self::Network => write!(f, "Network error"),
            Self::Graphql(error) => write!(f, "GraphQL error: {error}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

#[cfg(feature = "ssr")]
fn server_error(message: impl Into<String>) -> ServerFnError {
    ServerFnError::ServerError(message.into())
}

#[server(prefix = "/api/fn", endpoint = "admin/list-users")]
pub async fn list_users_native(
    page: i64,
    limit: i64,
    search: String,
    role: String,
    status: String,
) -> Result<GraphqlUsersResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use base64::{engine::general_purpose::STANDARD, Engine};
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{has_effective_permission, AuthContext, TenantContext};
        use rustok_core::Permission;

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;

        if !has_effective_permission(&auth.permissions, &Permission::USERS_LIST) {
            return Err(ServerFnError::new("users:list required"));
        }

        let app_ctx = expect_context::<AppContext>();
        let backend = app_ctx.db.get_database_backend();
        let page = page.max(1);
        let limit = limit.clamp(1, 100);
        let offset = (page - 1) * limit;
        let search = search.trim().to_ascii_lowercase();
        let role = role.trim().to_ascii_lowercase();
        let status = status.trim().to_ascii_lowercase();

        let placeholder = |index: usize| match backend {
            DbBackend::Sqlite => format!("?{index}"),
            _ => format!("${index}"),
        };

        let role_sql = r#"
COALESCE((
    SELECT r.slug
    FROM user_roles ur
    JOIN roles r ON r.id = ur.role_id
    WHERE ur.user_id = u.id
      AND r.tenant_id = u.tenant_id
    ORDER BY CASE r.slug
        WHEN 'super_admin' THEN 1
        WHEN 'admin' THEN 2
        WHEN 'manager' THEN 3
        WHEN 'customer' THEN 4
        ELSE 5
    END
    LIMIT 1
), 'customer')
"#;

        let mut values = vec![tenant.id.into()];
        let mut conditions = vec![format!("u.tenant_id = {}", placeholder(1))];
        let mut next_index = 2usize;

        if !role.is_empty() {
            conditions.push(format!("{role_sql} = {}", placeholder(next_index)));
            values.push(role.clone().into());
            next_index += 1;
        }

        if !status.is_empty() {
            conditions.push(format!(
                "LOWER(CAST(u.status AS TEXT)) = {}",
                placeholder(next_index)
            ));
            values.push(status.clone().into());
            next_index += 1;
        }

        if !search.is_empty() {
            let search_placeholder = placeholder(next_index);
            conditions.push(format!(
                "(LOWER(u.email) LIKE {search_placeholder} OR LOWER(COALESCE(u.name, '')) LIKE {search_placeholder})"
            ));
            values.push(format!("%{search}%").into());
            next_index += 1;
        }

        let where_sql = conditions.join(" AND ");

        let count_statement = Statement::from_sql_and_values(
            backend,
            format!(
                r#"
                SELECT CAST(COUNT(*) AS INTEGER) AS total_count
                FROM users u
                WHERE {where_sql}
                "#
            ),
            values.clone(),
        );

        let total_count = app_ctx
            .db
            .query_one(count_statement)
            .await
            .map_err(|err| server_error(err.to_string()))?
            .map(|row| row.try_get("", "total_count"))
            .transpose()
            .map_err(|err| server_error(err.to_string()))?
            .unwrap_or(0i64);

        let mut page_values = values;
        let limit_placeholder = placeholder(next_index);
        page_values.push(limit.into());
        next_index += 1;
        let offset_placeholder = placeholder(next_index);
        page_values.push(offset.into());

        let page_statement = Statement::from_sql_and_values(
            backend,
            format!(
                r#"
                SELECT
                    u.id,
                    u.email,
                    u.name,
                    {role_sql} AS role,
                    u.status,
                    u.created_at
                FROM users u
                WHERE {where_sql}
                ORDER BY u.created_at DESC
                LIMIT {limit_placeholder}
                OFFSET {offset_placeholder}
                "#
            ),
            page_statement_values(page_values),
        );

        let edges = app_ctx
            .db
            .query_all(page_statement)
            .await
            .map_err(|err| server_error(err.to_string()))?
            .into_iter()
            .enumerate()
            .map(|(index, row)| {
                Ok(GraphqlUserEdge {
                    node: GraphqlUser {
                        id: row
                            .try_get::<Uuid>("", "id")
                            .map(|value| value.to_string())
                            .map_err(|err| server_error(err.to_string()))?,
                        email: row
                            .try_get("", "email")
                            .map_err(|err| server_error(err.to_string()))?,
                        name: row
                            .try_get("", "name")
                            .map_err(|err| server_error(err.to_string()))?,
                        role: row
                            .try_get("", "role")
                            .map_err(|err| server_error(err.to_string()))?,
                        status: row
                            .try_get::<rustok_core::UserStatus>("", "status")
                            .map(|value| value.to_string())
                            .map_err(|err| server_error(err.to_string()))?,
                        created_at: row
                            .try_get::<chrono::DateTime<chrono::FixedOffset>>("", "created_at")
                            .map(|value| value.to_rfc3339())
                            .map_err(|err| server_error(err.to_string()))?,
                        tenant_name: None,
                    },
                    cursor: STANDARD.encode((offset + index as i64).to_string()),
                })
            })
            .collect::<Result<Vec<_>, ServerFnError>>()?;

        Ok(GraphqlUsersResponse {
            users: GraphqlUsersConnection {
                edges,
                page_info: GraphqlPageInfo { total_count },
            },
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (page, limit, search, role, status);
        Err(ServerFnError::new(
            "admin/list-users requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn page_statement_values(vals: Vec<sea_orm::Value>) -> Vec<sea_orm::Value> {
    vals
}

#[cfg(feature = "ssr")]
fn api_base_url() -> String {
    super::get_graphql_url()
        .trim_end_matches("/api/graphql")
        .trim_end_matches('/')
        .to_string()
}

#[cfg(feature = "ssr")]
async fn extract_http_error(response: reqwest::Response) -> String {
    let status = response.status();
    let text = response.text().await.unwrap_or_default();
    let trimmed = text.trim();

    if trimmed.is_empty() {
        return format!("request failed with status {status}");
    }

    if let Ok(payload) = serde_json::from_str::<serde_json::Value>(trimmed) {
        if let Some(message) = payload
            .get("message")
            .and_then(Value::as_str)
            .or_else(|| payload.get("error").and_then(Value::as_str))
            .map(str::trim)
            .filter(|value| !value.is_empty())
        {
            return message.to_string();
        }
    }

    trimmed.to_string()
}

#[server(prefix = "/api/fn", endpoint = "admin/user-details")]
pub async fn user_details_native(id: String) -> Result<GraphqlUserResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{has_effective_permission, AuthContext, TenantContext};
        use rustok_core::Permission;

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;

        if !has_effective_permission(&auth.permissions, &Permission::USERS_READ) {
            return Err(ServerFnError::new("users:read required"));
        }

        let user_id =
            Uuid::parse_str(&id).map_err(|err| server_error(format!("invalid user id: {err}")))?;
        let app_ctx = expect_context::<AppContext>();

        let statement = match app_ctx.db.get_database_backend() {
            DbBackend::Sqlite => Statement::from_sql_and_values(
                DbBackend::Sqlite,
                r#"
                SELECT
                    u.id,
                    u.email,
                    u.name,
                    COALESCE((
                        SELECT r.slug
                        FROM user_roles ur
                        JOIN roles r ON r.id = ur.role_id
                        WHERE ur.user_id = u.id
                          AND r.tenant_id = u.tenant_id
                        ORDER BY CASE r.slug
                            WHEN 'super_admin' THEN 1
                            WHEN 'admin' THEN 2
                            WHEN 'manager' THEN 3
                            WHEN 'customer' THEN 4
                            ELSE 5
                        END
                        LIMIT 1
                    ), 'customer') AS role,
                    u.status,
                    u.created_at,
                    t.name AS tenant_name
                FROM users u
                JOIN tenants t ON t.id = u.tenant_id
                WHERE u.id = ?1
                  AND u.tenant_id = ?2
                "#,
                vec![user_id.into(), tenant.id.into()],
            ),
            _ => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT
                    u.id,
                    u.email,
                    u.name,
                    COALESCE((
                        SELECT r.slug
                        FROM user_roles ur
                        JOIN roles r ON r.id = ur.role_id
                        WHERE ur.user_id = u.id
                          AND r.tenant_id = u.tenant_id
                        ORDER BY CASE r.slug
                            WHEN 'super_admin' THEN 1
                            WHEN 'admin' THEN 2
                            WHEN 'manager' THEN 3
                            WHEN 'customer' THEN 4
                            ELSE 5
                        END
                        LIMIT 1
                    ), 'customer') AS role,
                    u.status,
                    u.created_at,
                    t.name AS tenant_name
                FROM users u
                JOIN tenants t ON t.id = u.tenant_id
                WHERE u.id = $1
                  AND u.tenant_id = $2
                "#,
                vec![user_id.into(), tenant.id.into()],
            ),
        };

        let user = match app_ctx
            .db
            .query_one(statement)
            .await
            .map_err(|err| server_error(err.to_string()))?
        {
            Some(row) => Some(GraphqlUser {
                id: row
                    .try_get::<Uuid>("", "id")
                    .map(|value| value.to_string())
                    .map_err(|err| server_error(err.to_string()))?,
                email: row
                    .try_get("", "email")
                    .map_err(|err| server_error(err.to_string()))?,
                name: row
                    .try_get("", "name")
                    .map_err(|err| server_error(err.to_string()))?,
                role: row
                    .try_get("", "role")
                    .map_err(|err| server_error(err.to_string()))?,
                status: row
                    .try_get::<rustok_core::UserStatus>("", "status")
                    .map(|value| value.to_string())
                    .map_err(|err| server_error(err.to_string()))?,
                created_at: row
                    .try_get::<chrono::DateTime<chrono::FixedOffset>>("", "created_at")
                    .map(|value| value.to_rfc3339())
                    .map_err(|err| server_error(err.to_string()))?,
                tenant_name: row
                    .try_get("", "tenant_name")
                    .map_err(|err| server_error(err.to_string()))?,
            }),
            None => None,
        };

        Ok(GraphqlUserResponse { user })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = id;
        Err(ServerFnError::new(
            "admin/user-details requires the `ssr` feature",
        ))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct SuccessPayload {
    pub success: bool,
}

#[server(prefix = "/api/fn", endpoint = "admin/change-password")]
pub async fn change_password_native(
    token: String,
    tenant: String,
    current_password: String,
    new_password: String,
) -> Result<SuccessPayload, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};

        #[derive(Deserialize)]
        struct RestStatusResponse {
            #[allow(dead_code)]
            status: String,
        }

        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/api/auth/change-password", api_base_url()))
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .header(CONTENT_TYPE, "application/json")
            .header("X-Tenant-ID", tenant)
            .json(&serde_json::json!({
                "current_password": current_password,
                "new_password": new_password,
            }))
            .send()
            .await
            .map_err(ServerFnError::new)?;

        if !response.status().is_success() {
            return Err(ServerFnError::new(extract_http_error(response).await));
        }

        let _ = response
            .json::<RestStatusResponse>()
            .await
            .map_err(ServerFnError::new)?;

        Ok(SuccessPayload { success: true })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (token, tenant, current_password, new_password);
        Err(ServerFnError::new(
            "admin/change-password requires the `ssr` feature",
        ))
    }
}

#[derive(Clone, Debug, Deserialize, Serialize)]
pub struct ProfileUser {
    pub id: String,
    pub email: String,
    pub name: Option<String>,
    pub role: String,
}

#[server(prefix = "/api/fn", endpoint = "admin/update-profile")]
pub async fn update_profile_native(
    token: String,
    tenant: String,
    name: Option<String>,
) -> Result<ProfileUser, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};

        #[derive(Deserialize)]
        struct RestProfileUser {
            id: String,
            email: String,
            name: Option<String>,
            role: String,
        }

        let client = reqwest::Client::new();
        let response = client
            .post(format!("{}/api/auth/profile", api_base_url()))
            .header(AUTHORIZATION, format!("Bearer {token}"))
            .header(CONTENT_TYPE, "application/json")
            .header("X-Tenant-ID", tenant)
            .json(&serde_json::json!({ "name": name }))
            .send()
            .await
            .map_err(ServerFnError::new)?;

        if !response.status().is_success() {
            return Err(ServerFnError::new(extract_http_error(response).await));
        }

        let user = response
            .json::<RestProfileUser>()
            .await
            .map_err(ServerFnError::new)?;

        Ok(ProfileUser {
            id: user.id,
            email: user.email,
            name: user.name,
            role: user.role,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (token, tenant, name);
        Err(ServerFnError::new(
            "admin/update-profile requires the `ssr` feature",
        ))
    }
}

#[cfg(feature = "ssr")]
fn parse_json_list(row: &sea_orm::QueryResult, column: &str) -> Result<Vec<String>, ServerFnError> {
    let value = match row.try_get::<Value>("", column) {
        Ok(value) => value,
        Err(_) => {
            let raw: String = row
                .try_get("", column)
                .map_err(|err| server_error(err.to_string()))?;
            serde_json::from_str::<Value>(&raw).map_err(|err| server_error(err.to_string()))?
        }
    };

    serde_json::from_value::<Vec<String>>(value).map_err(|err| server_error(err.to_string()))
}

#[cfg(feature = "ssr")]
fn parse_app_type(value: &str) -> AppType {
    match value {
        "embedded" => AppType::Embedded,
        "first_party" => AppType::FirstParty,
        "mobile" => AppType::Mobile,
        "service" => AppType::Service,
        _ => AppType::ThirdParty,
    }
}

#[cfg(feature = "ssr")]
fn parse_created_at(
    row: &sea_orm::QueryResult,
    column: &str,
) -> Result<chrono::DateTime<chrono::Utc>, ServerFnError> {
    row.try_get::<chrono::DateTime<chrono::Utc>>("", column)
        .or_else(|_| {
            row.try_get::<chrono::DateTime<chrono::FixedOffset>>("", column)
                .map(|value| value.with_timezone(&chrono::Utc))
        })
        .map_err(|err| server_error(err.to_string()))
}

#[cfg(feature = "ssr")]
fn parse_optional_datetime(
    row: &sea_orm::QueryResult,
    column: &str,
) -> Result<Option<chrono::DateTime<chrono::Utc>>, ServerFnError> {
    row.try_get::<Option<chrono::DateTime<chrono::Utc>>>("", column)
        .or_else(|_| {
            row.try_get::<Option<chrono::DateTime<chrono::FixedOffset>>>("", column)
                .map(|value| value.map(|dt| dt.with_timezone(&chrono::Utc)))
        })
        .map_err(|err| server_error(err.to_string()))
}

#[server(prefix = "/api/fn", endpoint = "admin/list-oauth-apps")]
pub async fn list_oauth_apps_native(limit: i64) -> Result<Vec<OAuthApp>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{has_effective_permission, AuthContext, TenantContext};
        use rustok_core::Permission;

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;

        let has_admin_permission =
            has_effective_permission(&auth.permissions, &Permission::SETTINGS_MANAGE)
                || has_effective_permission(&auth.permissions, &Permission::USERS_MANAGE);
        if !has_admin_permission {
            return Err(ServerFnError::new(
                "settings:manage or users:manage required",
            ));
        }

        let app_ctx = expect_context::<AppContext>();
        let backend = app_ctx.db.get_database_backend();
        let limit = limit.clamp(1, 100);
        let statement = match backend {
            DbBackend::Sqlite => Statement::from_sql_and_values(
                DbBackend::Sqlite,
                r#"
                SELECT
                    oa.id,
                    oa.name,
                    oa.slug,
                    oa.description,
                    oa.icon_url,
                    oa.app_type,
                    oa.client_id,
                    oa.client_secret_hash,
                    oa.redirect_uris,
                    oa.scopes,
                    oa.grant_types,
                    oa.manifest_ref,
                    oa.auto_created,
                    oa.is_active,
                    oa.revoked_at,
                    oa.last_used_at,
                    oa.created_at,
                    COALESCE(tok.active_token_count, 0) AS active_token_count
                FROM oauth_apps oa
                LEFT JOIN (
                    SELECT app_id, COUNT(*) AS active_token_count
                    FROM oauth_tokens
                    WHERE revoked_at IS NULL
                    GROUP BY app_id
                ) tok ON tok.app_id = oa.id
                WHERE oa.tenant_id = ?1
                  AND oa.is_active = 1
                  AND oa.revoked_at IS NULL
                ORDER BY oa.created_at DESC
                LIMIT ?2
                "#,
                vec![tenant.id.into(), limit.into()],
            ),
            _ => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT
                    oa.id,
                    oa.name,
                    oa.slug,
                    oa.description,
                    oa.icon_url,
                    oa.app_type,
                    oa.client_id,
                    oa.client_secret_hash,
                    oa.redirect_uris,
                    oa.scopes,
                    oa.grant_types,
                    oa.manifest_ref,
                    oa.auto_created,
                    oa.is_active,
                    oa.revoked_at,
                    oa.last_used_at,
                    oa.created_at,
                    COALESCE(tok.active_token_count, 0) AS active_token_count
                FROM oauth_apps oa
                LEFT JOIN (
                    SELECT app_id, COUNT(*) AS active_token_count
                    FROM oauth_tokens
                    WHERE revoked_at IS NULL
                    GROUP BY app_id
                ) tok ON tok.app_id = oa.id
                WHERE oa.tenant_id = $1
                  AND oa.is_active = TRUE
                  AND oa.revoked_at IS NULL
                ORDER BY oa.created_at DESC
                LIMIT $2
                "#,
                vec![tenant.id.into(), limit.into()],
            ),
        };

        app_ctx
            .db
            .query_all(statement)
            .await
            .map_err(|err| server_error(err.to_string()))?
            .into_iter()
            .map(|row| {
                let app_type_value: String = row
                    .try_get("", "app_type")
                    .map_err(|err| server_error(err.to_string()))?;
                let auto_created: bool = row
                    .try_get("", "auto_created")
                    .map_err(|err| server_error(err.to_string()))?;
                let manifest_ref: Option<String> = row
                    .try_get("", "manifest_ref")
                    .map_err(|err| server_error(err.to_string()))?;
                let client_secret_hash: Option<String> = row
                    .try_get("", "client_secret_hash")
                    .map_err(|err| server_error(err.to_string()))?;
                let managed_by_manifest = auto_created && manifest_ref.is_some();
                let is_manual = !auto_created;

                Ok(OAuthApp {
                    id: row
                        .try_get("", "id")
                        .map_err(|err| server_error(err.to_string()))?,
                    name: row
                        .try_get("", "name")
                        .map_err(|err| server_error(err.to_string()))?,
                    slug: row
                        .try_get("", "slug")
                        .map_err(|err| server_error(err.to_string()))?,
                    description: row
                        .try_get("", "description")
                        .map_err(|err| server_error(err.to_string()))?,
                    icon_url: row
                        .try_get("", "icon_url")
                        .map_err(|err| server_error(err.to_string()))?,
                    app_type: parse_app_type(&app_type_value),
                    client_id: row
                        .try_get("", "client_id")
                        .map_err(|err| server_error(err.to_string()))?,
                    redirect_uris: parse_json_list(&row, "redirect_uris")?,
                    scopes: parse_json_list(&row, "scopes")?,
                    grant_types: parse_json_list(&row, "grant_types")?,
                    manifest_ref,
                    auto_created,
                    managed_by_manifest,
                    is_active: row
                        .try_get("", "is_active")
                        .map_err(|err| server_error(err.to_string()))?,
                    can_edit: is_manual
                        && matches!(
                            app_type_value.as_str(),
                            "third_party" | "mobile" | "service"
                        ),
                    can_rotate_secret: app_type_value != "embedded" && client_secret_hash.is_some(),
                    can_revoke: is_manual
                        && matches!(
                            app_type_value.as_str(),
                            "third_party" | "mobile" | "service"
                        ),
                    active_token_count: row
                        .try_get("", "active_token_count")
                        .map_err(|err| server_error(err.to_string()))?,
                    last_used_at: parse_optional_datetime(&row, "last_used_at")?,
                    created_at: parse_created_at(&row, "created_at")?,
                })
            })
            .collect()
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = limit;
        Err(ServerFnError::new(
            "admin/list-oauth-apps requires the `ssr` feature",
        ))
    }
}
