use leptos::prelude::*;

#[cfg(feature = "ssr")]
use crate::entities::oauth_app::model::AppType;
use crate::entities::oauth_app::model::OAuthApp;

#[cfg(feature = "ssr")]
use sea_orm::{ConnectionTrait, DbBackend, Statement};
#[cfg(feature = "ssr")]
use serde_json::Value;

#[cfg(feature = "ssr")]
fn server_error(message: impl Into<String>) -> ServerFnError {
    ServerFnError::ServerError(message.into())
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
pub(super) async fn list_oauth_apps_native(limit: i64) -> Result<Vec<OAuthApp>, ServerFnError> {
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

        let runtime = expect_context::<rustok_api::HostRuntimeContext>();
        let backend = runtime.db().get_database_backend();
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

        runtime
            .db()
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
