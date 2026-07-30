use leptos::prelude::*;

#[cfg(feature = "ssr")]
use crate::features::email::model::PlatformSettingsPayload;
use crate::features::email::model::PlatformSettingsResponse;

#[cfg(feature = "ssr")]
fn server_error(message: impl Into<String>) -> ServerFnError {
    ServerFnError::ServerError(message.into())
}

#[cfg(feature = "ssr")]
fn public_email_settings(value: &serde_json::Value) -> serde_json::Value {
    let smtp = value.get("smtp");
    let smtp_host = value
        .get("smtp_host")
        .and_then(serde_json::Value::as_str)
        .or_else(|| smtp.and_then(|smtp| smtp.get("host")).and_then(serde_json::Value::as_str))
        .unwrap_or("localhost");
    let smtp_port = value
        .get("smtp_port")
        .and_then(serde_json::Value::as_u64)
        .or_else(|| smtp.and_then(|smtp| smtp.get("port")).and_then(serde_json::Value::as_u64))
        .unwrap_or(1025);
    let smtp_username = value
        .get("smtp_username")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            smtp
                .and_then(|smtp| smtp.get("username"))
                .and_then(serde_json::Value::as_str)
        })
        .unwrap_or("");
    let from_address = value
        .get("from_address")
        .and_then(serde_json::Value::as_str)
        .or_else(|| value.get("from").and_then(serde_json::Value::as_str))
        .unwrap_or("no-reply@rustok.local");
    let password_configured = value
        .get("smtp_password")
        .and_then(serde_json::Value::as_str)
        .or_else(|| {
            smtp
                .and_then(|smtp| smtp.get("password"))
                .and_then(serde_json::Value::as_str)
        })
        .is_some_and(|secret| !secret.is_empty());

    serde_json::json!({
        "enabled": value.get("enabled").and_then(serde_json::Value::as_bool).unwrap_or(false),
        "provider": value.get("provider").and_then(serde_json::Value::as_str).unwrap_or("smtp"),
        "smtp_host": smtp_host,
        "smtp_port": smtp_port,
        "smtp_username": smtp_username,
        "from_address": from_address,
        "reset_base_url": value.get("reset_base_url").and_then(serde_json::Value::as_str).unwrap_or("/reset-password"),
        "password_configured": password_configured,
    })
}

#[server(prefix = "/api/fn", endpoint = "admin/email-settings")]
pub(super) async fn email_settings_native() -> Result<PlatformSettingsResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{AuthContext, HostSettingsSnapshot, Permission, TenantContext};
        use rustok_api::has_effective_permission;
        use sea_orm::{ConnectionTrait, DbBackend, Statement};
        use serde_json::Value;

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;
        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ) {
            return Err(ServerFnError::new("settings:read required"));
        }

        let runtime = expect_context::<rustok_api::HostRuntimeContext>();
        let statement = match runtime.db().get_database_backend() {
            DbBackend::Sqlite => Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT settings FROM platform_settings WHERE tenant_id = ?1 AND category = ?2 LIMIT 1",
                vec![tenant.id.into(), "email".into()],
            ),
            _ => Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT settings FROM platform_settings WHERE tenant_id = $1 AND category = $2 LIMIT 1",
                vec![tenant.id.into(), "email".into()],
            ),
        };
        let raw = match runtime
            .db()
            .query_one(statement)
            .await
            .map_err(|err| server_error(err.to_string()))?
        {
            Some(row) => row
                .try_get::<Value>("", "settings")
                .or_else(|_| {
                    row.try_get::<String>("", "settings")
                        .and_then(|raw| serde_json::from_str(&raw).map_err(sea_orm::DbErr::Json))
                })
                .map_err(|err| server_error(err.to_string()))?,
            None => {
                let root = runtime
                    .shared_get::<HostSettingsSnapshot>()
                    .map(|snapshot| snapshot.value().clone())
                    .unwrap_or_else(|| serde_json::json!({}));
                root.get("rustok")
                    .and_then(|value| value.get("email"))
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}))
            }
        };
        let settings = public_email_settings(&raw).to_string();

        Ok(PlatformSettingsResponse {
            platform_settings: PlatformSettingsPayload { settings },
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/email-settings requires the `ssr` feature",
        ))
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use super::*;

    #[test]
    fn public_projection_does_not_echo_historical_smtp_password() {
        let projection = public_email_settings(&serde_json::json!({
            "smtp": {
                "host": "smtp.example.test",
                "port": 587,
                "username": "mailer",
                "password": "top-secret"
            },
            "from": "no-reply@example.test"
        }));

        let encoded = projection.to_string();
        assert!(!encoded.contains("top-secret"));
        assert_eq!(projection["password_configured"], true);
    }
}
