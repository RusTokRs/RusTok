use crate::entities::oauth_app::model::{AppType, OAuthApp};
use crate::shared::api::{request, ApiError};
use leptos::prelude::*;
#[cfg(feature = "ssr")]
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use serde::{Deserialize, Serialize};
#[cfg(feature = "ssr")]
use serde_json::Value;
use uuid::Uuid;

pub const OAUTH_APPS_QUERY: &str = r#"
query OAuthApps($limit: Int) {
  oauthApps(limit: $limit) {
    id
    name
    slug
    description
    iconUrl
    appType
    clientId
    redirectUris
    scopes
    grantTypes
    manifestRef
    autoCreated
    managedByManifest
    isActive
    canEdit
    canRotateSecret
    canRevoke
    activeTokenCount
    lastUsedAt
    createdAt
  }
}
"#;

pub const CREATE_OAUTH_APP_MUTATION: &str = r#"
mutation CreateOAuthApp($input: CreateOAuthAppInput!) {
  createOAuthApp(input: $input) {
    app {
      id
      name
      slug
      description
      iconUrl
      appType
      clientId
      redirectUris
      scopes
      grantTypes
      manifestRef
      autoCreated
      managedByManifest
      isActive
      canEdit
      canRotateSecret
      canRevoke
      activeTokenCount
      lastUsedAt
      createdAt
    }
    clientSecret
  }
}
"#;

pub const UPDATE_OAUTH_APP_MUTATION: &str = r#"
mutation UpdateOAuthApp($id: UUID!, $input: UpdateOAuthAppInput!) {
  updateOAuthApp(id: $id, input: $input) {
    id
    name
    slug
    description
    iconUrl
    appType
    clientId
    redirectUris
    scopes
    grantTypes
    manifestRef
    autoCreated
    managedByManifest
    isActive
    canEdit
    canRotateSecret
    canRevoke
    activeTokenCount
    lastUsedAt
    createdAt
  }
}
"#;

pub const ROTATE_OAUTH_APP_SECRET_MUTATION: &str = r#"
mutation RotateOAuthAppSecret($id: UUID!) {
  rotateOAuthAppSecret(id: $id) {
    app {
      id
      name
      slug
      description
      iconUrl
      appType
      clientId
      redirectUris
      scopes
      grantTypes
      manifestRef
      autoCreated
      managedByManifest
      isActive
      canEdit
      canRotateSecret
      canRevoke
      activeTokenCount
      lastUsedAt
      createdAt
    }
    clientSecret
  }
}
"#;

pub const REVOKE_OAUTH_APP_MUTATION: &str = r#"
mutation RevokeOAuthApp($id: UUID!) {
  revokeOAuthApp(id: $id) {
    id
  }
}
"#;

#[derive(Clone, Debug, Default, Serialize)]
pub struct OAuthAppsVariables {
    pub limit: Option<i64>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct OAuthAppsResponse {
    #[serde(rename = "oauthApps")]
    pub oauth_apps: Vec<OAuthApp>,
}

#[derive(Clone, Debug, Serialize)]
pub struct CreateOAuthAppVariables {
    pub input: CreateOAuthAppInput,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateOAuthAppInput {
    pub name: String,
    pub slug: String,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub app_type: AppType,
    pub redirect_uris: Option<Vec<String>>,
    pub scopes: Vec<String>,
    pub grant_types: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct CreateOAuthAppResponse {
    #[serde(rename = "createOAuthApp")]
    pub create_oauth_app: CreateOAuthAppResult,
}

#[derive(Clone, Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateOAuthAppResult {
    pub app: OAuthApp,
    pub client_secret: String,
}

#[derive(Clone, Debug, Serialize)]
pub struct UpdateOAuthAppVariables {
    pub id: Uuid,
    pub input: UpdateOAuthAppInput,
}

#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateOAuthAppInput {
    pub name: String,
    pub description: Option<String>,
    pub icon_url: Option<String>,
    pub redirect_uris: Vec<String>,
    pub scopes: Vec<String>,
    pub grant_types: Vec<String>,
}

#[derive(Clone, Debug, Deserialize)]
pub struct UpdateOAuthAppResponse {
    #[serde(rename = "updateOAuthApp")]
    pub update_oauth_app: OAuthApp,
}

#[derive(Clone, Debug, Serialize)]
pub struct OAuthAppIdVariables {
    pub id: Uuid,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RotateOAuthAppSecretResponse {
    #[serde(rename = "rotateOAuthAppSecret")]
    pub rotate_oauth_app_secret: CreateOAuthAppResult,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RevokeOAuthAppResponse {
    #[serde(rename = "revokeOAuthApp")]
    pub _revoke_oauth_app: RevokeOAuthAppPayload,
}

#[derive(Clone, Debug, Deserialize)]
pub struct RevokeOAuthAppPayload {
    pub id: Uuid,
}

#[cfg(feature = "ssr")]
fn server_error(message: impl Into<String>) -> ServerFnError {
    ServerFnError::ServerError(message.into())
}

async fn list_oauth_apps_graphql(
    token: Option<String>,
    tenant: Option<String>,
) -> Result<Vec<OAuthApp>, ApiError> {
    let response = request::<OAuthAppsVariables, OAuthAppsResponse>(
        OAUTH_APPS_QUERY,
        OAuthAppsVariables { limit: Some(100) },
        token,
        tenant,
    )
    .await?;

    Ok(response.oauth_apps)
}

async fn list_oauth_apps_server(limit: i64) -> Result<Vec<OAuthApp>, ServerFnError> {
    list_oauth_apps_native(limit).await
}

pub async fn list_oauth_apps(
    token: Option<String>,
    tenant: Option<String>,
) -> Result<Vec<OAuthApp>, String> {
    match list_oauth_apps_server(100).await {
        Ok(apps) => Ok(apps),
        Err(server_err) => list_oauth_apps_graphql(token, tenant)
            .await
            .map_err(|graphql_err| {
                format!(
                    "native path failed: {}; graphql path failed: {}",
                    server_err, graphql_err
                )
            }),
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
async fn list_oauth_apps_native(limit: i64) -> Result<Vec<OAuthApp>, ServerFnError> {
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
