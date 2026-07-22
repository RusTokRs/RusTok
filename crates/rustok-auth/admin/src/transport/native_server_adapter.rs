use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::model::{
    CreateOAuthAppInput, GraphqlUser, GraphqlUserResponse, GraphqlUsersResponse, OAuthApp,
    UpdateOAuthAppInput,
};

#[cfg(feature = "ssr")]
use crate::model::{AppType, GraphqlPageInfo, GraphqlUserEdge, GraphqlUsersConnection};
#[cfg(feature = "ssr")]
use rustok_api::HostRuntimeContext;
#[cfg(feature = "ssr")]
use sea_orm::{ConnectionTrait, DbBackend, Statement};
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

#[cfg(feature = "ssr")]
struct AuthAdminRuntime {
    db: sea_orm::DatabaseConnection,
    host: HostRuntimeContext,
}

#[cfg(feature = "ssr")]
impl AuthAdminRuntime {
    fn from_host(host: HostRuntimeContext) -> Self {
        Self {
            db: host.db_clone(),
            host,
        }
    }

    fn module_runtime_extensions(
        &self,
    ) -> Result<std::sync::Arc<rustok_core::ModuleRuntimeExtensions>, ServerFnError> {
        self.host
            .shared_get::<std::sync::Arc<rustok_core::ModuleRuntimeExtensions>>()
            .ok_or_else(|| server_error("ModuleRuntimeExtensions not initialized"))
    }
}

#[server(prefix = "/api/fn", endpoint = "auth/graphql")]
pub(super) async fn auth_graphql(
    request: super::ServerGraphqlRequest,
) -> Result<Value, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        super::execute_server_graphql(request)
            .await
            .map_err(|err| ServerFnError::ServerError(err.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = request;
        Err(ServerFnError::ServerError(
            "SSR feature not enabled".to_string(),
        ))
    }
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
        use base64::{Engine, engine::general_purpose::STANDARD};
        use leptos::prelude::expect_context;
        use rustok_api::Permission;
        use rustok_api::{AuthContext, TenantContext, has_effective_permission};

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;

        if !has_effective_permission(&auth.permissions, &Permission::USERS_LIST) {
            return Err(ServerFnError::new("users:list required"));
        }

        let app_ctx = AuthAdminRuntime::from_host(expect_context::<HostRuntimeContext>());
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
    super::api_base_url()
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
        use rustok_api::Permission;
        use rustok_api::{AuthContext, TenantContext, has_effective_permission};

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
        let app_ctx = AuthAdminRuntime::from_host(expect_context::<HostRuntimeContext>());

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

#[cfg(feature = "ssr")]
fn user_from_mutation_record(record: rustok_auth::UserMutationRecord) -> GraphqlUser {
    GraphqlUser {
        id: record.id.to_string(),
        email: record.email,
        name: record.name,
        role: record.role,
        status: record.status,
        created_at: record.created_at.to_rfc3339(),
        tenant_name: record.tenant_name,
    }
}

#[cfg(feature = "ssr")]
async fn user_mutation_context() -> Result<
    (
        rustok_auth::AuthAdminMutationContext,
        rustok_auth::UserAdminMutationRuntime,
    ),
    ServerFnError,
> {
    use leptos::prelude::expect_context;
    use rustok_api::AuthContext;

    let auth = leptos_axum::extract::<AuthContext>()
        .await
        .map_err(|error| server_error(error.to_string()))?;
    let request_context = leptos_axum::extract::<rustok_api::RequestContext>()
        .await
        .ok();
    let tenant_context = leptos_axum::extract::<rustok_api::TenantContext>()
        .await
        .ok();
    let locale = request_context
        .map(|request_context| request_context.locale)
        .or_else(|| tenant_context.map(|tenant_context| tenant_context.default_locale));
    let app_ctx = AuthAdminRuntime::from_host(expect_context::<HostRuntimeContext>());
    let extensions = app_ctx.module_runtime_extensions()?;
    let runtime = extensions
        .get::<rustok_auth::UserAdminMutationRuntime>()
        .cloned()
        .ok_or_else(|| {
            server_error(
                "UserAdminMutationRuntime is not registered; initialize shared host runtime providers",
            )
        })?;
    Ok((
        rustok_auth::AuthAdminMutationContext {
            actor_id: auth.user_id,
            tenant_id: auth.tenant_id,
            request_id: None,
            locale,
        },
        runtime,
    ))
}

#[server(prefix = "/api/fn", endpoint = "admin/create-user")]
pub async fn create_user_native(
    input: crate::model::CreateUserInput,
) -> Result<Option<GraphqlUser>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (context, runtime) = user_mutation_context().await?;
        runtime
            .port()
            .create_user(
                &context,
                rustok_auth::CreateUserCommand {
                    email: input.email,
                    password: input.password,
                    name: input.name,
                    role: input.role,
                    status: input.status,
                    custom_fields: None,
                },
            )
            .await
            .map(user_from_mutation_record)
            .map(Some)
            .map_err(|error| server_error(error.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = input;
        Err(ServerFnError::new(
            "admin/create-user requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/update-user")]
pub async fn update_user_native(
    id: String,
    input: crate::model::UpdateUserInput,
) -> Result<Option<GraphqlUser>, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let id = Uuid::parse_str(&id).map_err(|error| server_error(error.to_string()))?;
        let (context, runtime) = user_mutation_context().await?;
        runtime
            .port()
            .update_user(
                &context,
                rustok_auth::UpdateUserCommand {
                    id,
                    email: None,
                    password: None,
                    name: input.name,
                    role: Some(input.role),
                    status: Some(input.status),
                    custom_fields: None,
                },
            )
            .await
            .map(user_from_mutation_record)
            .map(Some)
            .map_err(|error| server_error(error.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (id, input);
        Err(ServerFnError::new(
            "admin/update-user requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/delete-user")]
pub async fn delete_user_native(id: String) -> Result<bool, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let id = Uuid::parse_str(&id).map_err(|error| server_error(error.to_string()))?;
        let (context, runtime) = user_mutation_context().await?;
        runtime
            .port()
            .delete_user(&context, id)
            .await
            .map(|()| true)
            .map_err(|error| server_error(error.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = id;
        Err(ServerFnError::new(
            "admin/delete-user requires the `ssr` feature",
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
        use rustok_api::Permission;
        use rustok_api::{AuthContext, TenantContext, has_effective_permission};

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

        let app_ctx = AuthAdminRuntime::from_host(expect_context::<HostRuntimeContext>());
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

#[cfg(feature = "ssr")]
fn oauth_app_from_mutation_record(record: rustok_auth::OAuthAppMutationRecord) -> OAuthApp {
    let app_type = parse_app_type(&record.app_type);
    let is_manual = !record.auto_created;
    let can_manage = is_manual
        && matches!(
            record.app_type.as_str(),
            "third_party" | "mobile" | "service"
        );
    OAuthApp {
        id: record.id,
        name: record.name,
        slug: record.slug,
        description: record.description,
        icon_url: record.icon_url,
        app_type,
        client_id: record.client_id,
        redirect_uris: record.redirect_uris,
        scopes: record.scopes,
        grant_types: record.grant_types,
        manifest_ref: record.manifest_ref.clone(),
        auto_created: record.auto_created,
        managed_by_manifest: record.auto_created && record.manifest_ref.is_some(),
        is_active: record.is_active,
        can_edit: can_manage,
        can_rotate_secret: record.app_type != "embedded",
        can_revoke: can_manage,
        active_token_count: record.active_token_count,
        last_used_at: record.last_used_at,
        created_at: record.created_at,
    }
}

#[cfg(feature = "ssr")]
async fn oauth_mutation_context() -> Result<
    (
        rustok_auth::AuthAdminMutationContext,
        rustok_auth::OAuthAdminRuntime,
    ),
    ServerFnError,
> {
    use leptos::prelude::expect_context;
    use rustok_api::AuthContext;

    let auth = leptos_axum::extract::<AuthContext>()
        .await
        .map_err(|error| server_error(error.to_string()))?;
    let app_ctx = AuthAdminRuntime::from_host(expect_context::<HostRuntimeContext>());
    let extensions = app_ctx.module_runtime_extensions()?;
    let runtime = extensions
        .get::<rustok_auth::OAuthAdminRuntime>()
        .cloned()
        .ok_or_else(|| {
            server_error(
                "OAuthAdminRuntime is not registered; initialize shared host runtime providers",
            )
        })?;

    Ok((
        rustok_auth::AuthAdminMutationContext {
            actor_id: auth.user_id,
            tenant_id: auth.tenant_id,
            request_id: None,
            locale: None,
        },
        runtime,
    ))
}

#[server(prefix = "/api/fn", endpoint = "admin/create-oauth-app")]
pub async fn create_oauth_app_native(
    input: CreateOAuthAppInput,
) -> Result<super::CreateOAuthAppResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (context, runtime) = oauth_mutation_context().await?;
        let result = runtime
            .port()
            .create_oauth_app(
                &context,
                rustok_auth::CreateOAuthAppCommand {
                    name: input.name,
                    slug: input.slug,
                    description: input.description,
                    icon_url: input.icon_url,
                    app_type: match input.app_type {
                        AppType::Embedded => "embedded",
                        AppType::FirstParty => "first_party",
                        AppType::Mobile => "mobile",
                        AppType::Service => "service",
                        AppType::ThirdParty => "third_party",
                    }
                    .to_string(),
                    redirect_uris: input.redirect_uris.unwrap_or_default(),
                    scopes: input.scopes,
                    grant_types: input.grant_types,
                    granted_permissions: input.granted_permissions,
                },
            )
            .await
            .map_err(|error| server_error(error.to_string()))?;
        Ok(super::CreateOAuthAppResult {
            app: oauth_app_from_mutation_record(result.app),
            client_secret: result.client_secret,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = input;
        Err(ServerFnError::new(
            "admin/create-oauth-app requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/update-oauth-app")]
pub async fn update_oauth_app_native(
    id: uuid::Uuid,
    input: UpdateOAuthAppInput,
) -> Result<OAuthApp, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (context, runtime) = oauth_mutation_context().await?;
        let result = runtime
            .port()
            .update_oauth_app(
                &context,
                rustok_auth::UpdateOAuthAppCommand {
                    id,
                    name: input.name,
                    description: input.description,
                    icon_url: input.icon_url,
                    redirect_uris: input.redirect_uris,
                    scopes: input.scopes,
                    grant_types: input.grant_types,
                    granted_permissions: input.granted_permissions,
                },
            )
            .await
            .map_err(|error| server_error(error.to_string()))?;
        Ok(oauth_app_from_mutation_record(result))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = (id, input);
        Err(ServerFnError::new(
            "admin/update-oauth-app requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/rotate-oauth-app-secret")]
pub async fn rotate_oauth_app_secret_native(
    id: uuid::Uuid,
) -> Result<super::CreateOAuthAppResult, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (context, runtime) = oauth_mutation_context().await?;
        let result = runtime
            .port()
            .rotate_oauth_app_secret(&context, id)
            .await
            .map_err(|error| server_error(error.to_string()))?;
        Ok(super::CreateOAuthAppResult {
            app: oauth_app_from_mutation_record(result.app),
            client_secret: result.client_secret,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = id;
        Err(ServerFnError::new(
            "admin/rotate-oauth-app-secret requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/revoke-oauth-app")]
pub async fn revoke_oauth_app_native(id: uuid::Uuid) -> Result<uuid::Uuid, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let (context, runtime) = oauth_mutation_context().await?;
        runtime
            .port()
            .revoke_oauth_app(&context, id)
            .await
            .map(|app| app.id)
            .map_err(|error| server_error(error.to_string()))
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = id;
        Err(ServerFnError::new(
            "admin/revoke-oauth-app requires the `ssr` feature",
        ))
    }
}
