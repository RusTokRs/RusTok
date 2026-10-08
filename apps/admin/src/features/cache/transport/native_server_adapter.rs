use leptos::prelude::*;

#[cfg(feature = "ssr")]
use crate::features::cache::model::CacheHealthPayload;
use crate::features::cache::model::CacheHealthResponse;
#[cfg(feature = "ssr")]
use crate::features::cache::model::PlatformSettingsPayload;
use crate::features::cache::model::PlatformSettingsResponse;

#[cfg(feature = "ssr")]
fn server_error(message: impl Into<String>) -> ServerFnError {
    ServerFnError::ServerError(message.into())
}

#[cfg(feature = "ssr")]
fn require_cache_admin_tenant_scope(
    auth_tenant_id: uuid::Uuid,
    resolved_tenant_id: uuid::Uuid,
) -> Result<(), ServerFnError> {
    if auth_tenant_id == resolved_tenant_id {
        return Ok(());
    }

    tracing::warn!(
        auth_tenant_id = %auth_tenant_id,
        resolved_tenant_id = %resolved_tenant_id,
        code = "cache.admin_tenant_scope_mismatch",
        boundary = "cache_admin_native_transport",
        "cache admin permissions cannot cross the resolved tenant boundary"
    );
    Err(ServerFnError::new("Cache admin access is denied"))
}

#[server(prefix = "/api/fn", endpoint = "admin/cache-health")]
pub(super) async fn cache_health_native() -> Result<CacheHealthResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_cache::CacheService;

        let runtime = expect_context::<rustok_api::HostRuntimeContext>();
        let payload = if let Some(cache) = runtime.shared_get::<CacheService>() {
            let report = cache.health().await;
            CacheHealthPayload {
                redis_configured: report.redis_configured,
                redis_healthy: report.redis_healthy,
                redis_error: report.redis_error,
                backend: if report.redis_configured {
                    "redis".to_string()
                } else {
                    "in-memory".to_string()
                },
            }
        } else {
            CacheHealthPayload {
                redis_configured: false,
                redis_healthy: false,
                redis_error: None,
                backend: "none".to_string(),
            }
        };

        Ok(CacheHealthResponse {
            cache_health: payload,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/cache-health requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/cache-settings")]
pub(super) async fn cache_settings_native() -> Result<PlatformSettingsResponse, ServerFnError> {
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
        require_cache_admin_tenant_scope(auth.tenant_id, tenant.id)?;
        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ) {
            return Err(ServerFnError::new("settings:read required"));
        }

        let runtime = expect_context::<rustok_api::HostRuntimeContext>();
        let statement = match runtime.db().get_database_backend() {
            DbBackend::Sqlite => Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT settings FROM platform_settings WHERE tenant_id = ?1 AND category = ?2 LIMIT 1",
                vec![tenant.id.into(), "cache".into()],
            ),
            _ => Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT settings FROM platform_settings WHERE tenant_id = $1 AND category = $2 LIMIT 1",
                vec![tenant.id.into(), "cache".into()],
            ),
        };
        let settings = match runtime
            .db()
            .query_one_raw(statement)
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
                let cache_cfg = root
                    .get("rustok")
                    .and_then(|value| value.get("cache"))
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                let redis_url = cache_cfg
                    .get("redis_url")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                let mode = if !redis_url.is_empty() {
                    "redis"
                } else {
                    "in-memory"
                };
                serde_json::json!({
                    "mode": mode,
                    "redis_url": redis_url,
                    "redis_host": "127.0.0.1",
                    "redis_port": 6379,
                    "redis_password": "",
                    "redis_db": 0,
                })
                .to_string()
            }
        };
        Ok(PlatformSettingsResponse {
            platform_settings: PlatformSettingsPayload { settings },
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/cache-settings requires the `ssr` feature",
        ))
    }
}
