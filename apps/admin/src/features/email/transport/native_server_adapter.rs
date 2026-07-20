use leptos::prelude::*;

#[cfg(feature = "ssr")]
use crate::features::email::model::PlatformSettingsPayload;
use crate::features::email::model::PlatformSettingsResponse;

#[cfg(feature = "ssr")]
fn server_error(message: impl Into<String>) -> ServerFnError {
    ServerFnError::ServerError(message.into())
}

#[server(prefix = "/api/fn", endpoint = "admin/email-settings")]
pub(super) async fn email_settings_native() -> Result<PlatformSettingsResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{AuthContext, TenantContext, has_effective_permission};
        use rustok_api::{HostSettingsSnapshot, Permission};
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
        let settings = match runtime
            .db()
            .query_one(statement)
            .await
            .map_err(|err| server_error(err.to_string()))?
        {
            Some(row) => row
                .try_get::<Value>("", "settings")
                .map(|value| value.to_string())
                .or_else(|_| row.try_get::<String>("", "settings"))
                .map_err(|err| server_error(err.to_string()))?,
            None => {
                let root = runtime
                    .shared_get::<HostSettingsSnapshot>()
                    .map(|snapshot| snapshot.value().clone())
                    .unwrap_or_else(|| serde_json::json!({}));
                let email = root
                    .get("rustok")
                    .and_then(|value| value.get("email"))
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                serde_json::json!({
                    "smtp_host": email.pointer("/smtp/host").and_then(|value| value.as_str()).unwrap_or("localhost"),
                    "smtp_port": email.pointer("/smtp/port").and_then(|value| value.as_u64()).unwrap_or(1025),
                    "smtp_username": email.pointer("/smtp/username").and_then(|value| value.as_str()).unwrap_or(""),
                    "from_address": email.get("from").and_then(|value| value.as_str()).unwrap_or("no-reply@rustok.local"),
                }).to_string()
            }
        };
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
