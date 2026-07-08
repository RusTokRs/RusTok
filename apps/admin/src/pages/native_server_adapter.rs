use leptos::prelude::*;

#[cfg(feature = "ssr")]
use super::cache::CacheHealthPayload;
use super::cache::GraphqlCacheHealthResponse;
#[cfg(feature = "ssr")]
use super::dashboard::{
    calculate_percent_change, load_order_stats_snapshot, load_period_count_snapshot,
    load_recent_activity, server_error as dashboard_server_error,
};
use super::dashboard::{DashboardStatsResponse, RecentActivityResponse};
#[cfg(feature = "ssr")]
use super::dashboard::DashboardStats;
#[cfg(feature = "ssr")]
use super::email_settings::PlatformSettingsPayload as EmailPlatformSettingsPayload;
use super::email_settings::PlatformSettingsResponse as EmailPlatformSettingsResponse;
#[cfg(feature = "ssr")]
use super::events::{EventsStatus, PlatformSettingsPayload as EventsPlatformSettingsPayload};
use super::events::{
    EventsStatusResponse, PlatformSettingsResponse as EventsPlatformSettingsResponse,
};
use super::roles::GraphqlRolesResponse;
#[cfg(feature = "ssr")]
use super::roles::RoleInfo;

#[cfg(feature = "ssr")]
fn platform_settings_server_error(message: impl Into<String>) -> ServerFnError {
    ServerFnError::ServerError(message.into())
}

#[server(prefix = "/api/fn", endpoint = "admin/events-status")]
pub(super) async fn events_status_native() -> Result<EventsStatusResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use sea_orm::{ConnectionTrait, DbBackend, Statement};

        let app_ctx = expect_context::<AppContext>();
        let root = app_ctx
            .config
            .settings
            .clone()
            .unwrap_or_else(|| serde_json::json!({}));
        let events = root
            .get("rustok")
            .and_then(|value| value.get("events"))
            .cloned()
            .unwrap_or_else(|| serde_json::json!({}));

        let transport = events
            .get("transport")
            .and_then(|value| value.as_str())
            .unwrap_or("memory");
        let iggy_mode = events
            .pointer("/iggy/mode")
            .and_then(|value| value.as_str())
            .unwrap_or("embedded")
            .to_string();
        let configured_transport = match transport {
            "outbox" => "outbox".to_string(),
            "iggy" => {
                if iggy_mode.eq_ignore_ascii_case("remote") {
                    "iggy_external".to_string()
                } else {
                    "iggy_embedded".to_string()
                }
            }
            _ => "memory".to_string(),
        };

        let backend = app_ctx.db.get_database_backend();
        let outbox_statement = match backend {
            DbBackend::Sqlite => Statement::from_sql_and_values(
                DbBackend::Sqlite,
                r#"
                SELECT
                    COALESCE(SUM(CASE WHEN status = ?1 THEN 1 ELSE 0 END), 0) AS pending_events,
                    COALESCE(SUM(CASE WHEN status = ?2 THEN 1 ELSE 0 END), 0) AS dlq_events
                FROM sys_events
                "#,
                vec!["pending".into(), "failed".into()],
            ),
            _ => Statement::from_sql_and_values(
                DbBackend::Postgres,
                r#"
                SELECT
                    COALESCE(SUM(CASE WHEN status = $1 THEN 1 ELSE 0 END), 0) AS pending_events,
                    COALESCE(SUM(CASE WHEN status = $2 THEN 1 ELSE 0 END), 0) AS dlq_events
                FROM sys_events
                "#,
                vec!["pending".into(), "failed".into()],
            ),
        };

        let (pending_events, dlq_events) = match app_ctx.db.query_one(outbox_statement).await {
            Ok(Some(row)) => (
                row.try_get("", "pending_events").unwrap_or(0),
                row.try_get("", "dlq_events").unwrap_or(0),
            ),
            Ok(None) | Err(_) => (0, 0),
        };

        Ok(EventsStatusResponse {
            events_status: EventsStatus {
                configured_transport,
                iggy_mode,
                relay_interval_ms: events
                    .get("relay_interval_ms")
                    .and_then(|value| value.as_u64())
                    .unwrap_or(1_000),
                dlq_enabled: events
                    .pointer("/dlq/enabled")
                    .and_then(|value| value.as_bool())
                    .unwrap_or(true),
                max_attempts: events
                    .pointer("/relay_retry_policy/max_attempts")
                    .and_then(|value| value.as_i64())
                    .unwrap_or(5) as i32,
                pending_events,
                dlq_events,
                available_transports: vec![
                    "memory".to_string(),
                    "outbox".to_string(),
                    "iggy_embedded".to_string(),
                    "iggy_external".to_string(),
                ],
            },
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/events-status requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/event-settings")]
pub(super) async fn event_settings_native() -> Result<EventsPlatformSettingsResponse, ServerFnError>
{
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::Permission;
        use rustok_api::{has_effective_permission, AuthContext, TenantContext};
        use sea_orm::{ConnectionTrait, DbBackend, Statement};
        use serde_json::Value;

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(|err| platform_settings_server_error(err.to_string()))?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(|err| platform_settings_server_error(err.to_string()))?;

        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ) {
            return Err(ServerFnError::new("settings:read required"));
        }

        let app_ctx = expect_context::<AppContext>();
        let backend = app_ctx.db.get_database_backend();
        let statement = match backend {
            DbBackend::Sqlite => Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT settings FROM platform_settings WHERE tenant_id = ?1 AND category = ?2 LIMIT 1",
                vec![tenant.id.into(), "events".into()],
            ),
            _ => Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT settings FROM platform_settings WHERE tenant_id = $1 AND category = $2 LIMIT 1",
                vec![tenant.id.into(), "events".into()],
            ),
        };

        let settings = match app_ctx
            .db
            .query_one(statement)
            .await
            .map_err(|err| platform_settings_server_error(err.to_string()))?
        {
            Some(row) => row
                .try_get::<Value>("", "settings")
                .map(|value| value.to_string())
                .or_else(|_| row.try_get::<String>("", "settings"))
                .map_err(|err| platform_settings_server_error(err.to_string()))?,
            None => {
                let root = app_ctx
                    .config
                    .settings
                    .clone()
                    .unwrap_or_else(|| serde_json::json!({}));
                let events = root
                    .get("rustok")
                    .and_then(|value| value.get("events"))
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                let transport = events
                    .get("transport")
                    .and_then(|value| value.as_str())
                    .unwrap_or("memory");
                let mode = events
                    .pointer("/iggy/mode")
                    .and_then(|value| value.as_str())
                    .unwrap_or("embedded");

                serde_json::json!({
                    "transport": match transport {
                        "outbox" => "outbox",
                        "iggy" if mode.eq_ignore_ascii_case("remote") => "iggy_external",
                        "iggy" => "iggy_embedded",
                        _ => "memory",
                    },
                    "relay_interval_ms": events
                        .get("relay_interval_ms")
                        .and_then(|value| value.as_u64())
                        .unwrap_or(1_000),
                    "max_attempts": events
                        .pointer("/relay_retry_policy/max_attempts")
                        .and_then(|value| value.as_i64())
                        .unwrap_or(5),
                    "dlq_enabled": events
                        .pointer("/dlq/enabled")
                        .and_then(|value| value.as_bool())
                        .unwrap_or(true),
                    "iggy_addresses": events
                        .pointer("/iggy/remote/addresses")
                        .and_then(|value| value.as_array())
                        .map(|values| {
                            values
                                .iter()
                                .filter_map(|value| value.as_str())
                                .collect::<Vec<_>>()
                                .join(",")
                        })
                        .unwrap_or_else(|| "127.0.0.1:8090".to_string()),
                    "iggy_protocol": events
                        .pointer("/iggy/remote/protocol")
                        .and_then(|value| value.as_str())
                        .unwrap_or("tcp"),
                    "iggy_username": events
                        .pointer("/iggy/remote/username")
                        .and_then(|value| value.as_str())
                        .unwrap_or("iggy"),
                    "iggy_password": events
                        .pointer("/iggy/remote/password")
                        .and_then(|value| value.as_str())
                        .unwrap_or(""),
                    "iggy_tls": events
                        .pointer("/iggy/remote/tls_enabled")
                        .and_then(|value| value.as_bool())
                        .unwrap_or(false),
                    "iggy_stream": events
                        .pointer("/iggy/topology/stream_name")
                        .and_then(|value| value.as_str())
                        .unwrap_or("rustok"),
                    "iggy_partitions": events
                        .pointer("/iggy/topology/domain_partitions")
                        .and_then(|value| value.as_u64())
                        .unwrap_or(8),
                    "iggy_replication": events
                        .pointer("/iggy/topology/replication_factor")
                        .and_then(|value| value.as_u64())
                        .unwrap_or(1),
                })
                .to_string()
            }
        };

        Ok(EventsPlatformSettingsResponse {
            platform_settings: EventsPlatformSettingsPayload { settings },
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/event-settings requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/cache-health")]
pub(super) async fn cache_health_native() -> Result<GraphqlCacheHealthResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_cache::CacheService;

        let app_ctx = expect_context::<AppContext>();
        let payload = if let Some(cache) = app_ctx.shared_store.get::<CacheService>() {
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

        Ok(GraphqlCacheHealthResponse {
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

#[server(prefix = "/api/fn", endpoint = "admin/email-settings")]
pub(super) async fn email_settings_native() -> Result<EmailPlatformSettingsResponse, ServerFnError>
{
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::Permission;
        use rustok_api::{has_effective_permission, AuthContext, TenantContext};
        use sea_orm::{ConnectionTrait, DbBackend, Statement};
        use serde_json::Value;

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(|err| platform_settings_server_error(err.to_string()))?;
        let tenant = leptos_axum::extract::<TenantContext>()
            .await
            .map_err(|err| platform_settings_server_error(err.to_string()))?;

        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ) {
            return Err(ServerFnError::new("settings:read required"));
        }

        let app_ctx = expect_context::<AppContext>();
        let backend = app_ctx.db.get_database_backend();
        let statement = match backend {
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

        let settings = match app_ctx
            .db
            .query_one(statement)
            .await
            .map_err(|err| platform_settings_server_error(err.to_string()))?
        {
            Some(row) => row
                .try_get::<Value>("", "settings")
                .map(|value| value.to_string())
                .or_else(|_| row.try_get::<String>("", "settings"))
                .map_err(|err| platform_settings_server_error(err.to_string()))?,
            None => {
                let root = app_ctx
                    .config
                    .settings
                    .clone()
                    .unwrap_or_else(|| serde_json::json!({}));
                let email = root
                    .get("rustok")
                    .and_then(|value| value.get("email"))
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));

                serde_json::json!({
                    "smtp_host": email
                        .pointer("/smtp/host")
                        .and_then(|value| value.as_str())
                        .unwrap_or("localhost"),
                    "smtp_port": email
                        .pointer("/smtp/port")
                        .and_then(|value| value.as_u64())
                        .unwrap_or(1025),
                    "smtp_username": email
                        .pointer("/smtp/username")
                        .and_then(|value| value.as_str())
                        .unwrap_or(""),
                    "from_address": email
                        .get("from")
                        .and_then(|value| value.as_str())
                        .unwrap_or("no-reply@rustok.local"),
                })
                .to_string()
            }
        };

        Ok(EmailPlatformSettingsResponse {
            platform_settings: EmailPlatformSettingsPayload { settings },
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/email-settings requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/list-roles")]
pub(super) async fn list_roles_native() -> Result<GraphqlRolesResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos_axum::extract;
        use rustok_api::Permission;
        use rustok_api::{has_effective_permission, AuthContext};
        use rustok_core::{Rbac, UserRole};

        let auth = extract::<AuthContext>().await.map_err(ServerFnError::new)?;

        if !has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ) {
            return Err(ServerFnError::new("settings:read required to list roles"));
        }

        let roles = [
            UserRole::SuperAdmin,
            UserRole::Admin,
            UserRole::Manager,
            UserRole::Customer,
        ]
        .into_iter()
        .map(|role| {
            let mut permissions = Rbac::permissions_for_role(&role)
                .iter()
                .map(|permission| permission.to_string())
                .collect::<Vec<_>>();
            permissions.sort();

            let display_name = match role {
                UserRole::SuperAdmin => "Super Admin",
                UserRole::Admin => "Admin",
                UserRole::Manager => "Manager",
                UserRole::Customer => "Customer",
            };

            RoleInfo {
                slug: role.to_string(),
                display_name: display_name.to_string(),
                permissions,
            }
        })
        .collect();

        Ok(GraphqlRolesResponse { roles })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/list-roles requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/dashboard-stats")]
pub(super) async fn dashboard_stats_native() -> Result<DashboardStatsResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use chrono::{Duration, Utc};
        use sea_orm::{ConnectionTrait, DbBackend};

        let _auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(|err| dashboard_server_error(err.to_string()))?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(|err| dashboard_server_error(err.to_string()))?;
        let app_ctx = expect_context::<loco_rs::app::AppContext>();

        let now = Utc::now();
        let current_period_start = now - Duration::days(30);
        let previous_period_start = current_period_start - Duration::days(30);

        let user_stats = load_period_count_snapshot(
            &app_ctx.db,
            "users",
            tenant.id,
            current_period_start,
            previous_period_start,
            None,
            None,
        )
        .await
        .map_err(|err| dashboard_server_error(err.to_string()))?;

        let post_stats = load_period_count_snapshot(
            &app_ctx.db,
            "nodes",
            tenant.id,
            current_period_start,
            previous_period_start,
            Some(match app_ctx.db.get_database_backend() {
                DbBackend::Sqlite => " AND kind = ?4",
                _ => " AND kind = $4",
            }),
            Some("post"),
        )
        .await
        .map_err(|err| dashboard_server_error(err.to_string()))?;

        let order_stats = load_order_stats_snapshot(
            &app_ctx.db,
            tenant.id,
            current_period_start,
            previous_period_start,
        )
        .await
        .map_err(|err| dashboard_server_error(err.to_string()))?;

        Ok(DashboardStatsResponse {
            dashboard_stats: Some(DashboardStats {
                total_users: user_stats.total_count,
                total_posts: post_stats.total_count,
                total_orders: order_stats.total_orders,
                total_revenue: order_stats.total_revenue,
                users_change: calculate_percent_change(
                    user_stats.current_count,
                    user_stats.previous_count,
                ),
                posts_change: calculate_percent_change(
                    post_stats.current_count,
                    post_stats.previous_count,
                ),
                orders_change: calculate_percent_change(
                    order_stats.current_orders,
                    order_stats.previous_orders,
                ),
                revenue_change: calculate_percent_change(
                    order_stats.current_revenue,
                    order_stats.previous_revenue,
                ),
            }),
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "admin/dashboard-stats requires the `ssr` feature",
        ))
    }
}

#[server(prefix = "/api/fn", endpoint = "admin/recent-activity")]
pub(super) async fn recent_activity_native(
    limit: i64,
) -> Result<RecentActivityResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        let _auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(|err| dashboard_server_error(err.to_string()))?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(|err| dashboard_server_error(err.to_string()))?;
        let app_ctx = expect_context::<loco_rs::app::AppContext>();

        Ok(RecentActivityResponse {
            recent_activity: load_recent_activity(&app_ctx.db, tenant.id, limit.clamp(1, 50))
                .await
                .map_err(|err| dashboard_server_error(err.to_string()))?,
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        let _ = limit;
        Err(ServerFnError::new(
            "admin/recent-activity requires the `ssr` feature",
        ))
    }
}
