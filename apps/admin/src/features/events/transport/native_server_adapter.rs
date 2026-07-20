use leptos::prelude::*;

use crate::features::events::model::{
    EventsStatus, EventsStatusResponse, PlatformSettingsPayload, PlatformSettingsResponse,
};

#[cfg(feature = "ssr")]
fn server_error(message: impl Into<String>) -> ServerFnError {
    ServerFnError::ServerError(message.into())
}

#[server(prefix = "/api/fn", endpoint = "admin/events-status")]
pub(super) async fn events_status_native() -> Result<EventsStatusResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{AuthContext, HostSettingsSnapshot, Permission, has_effective_permission};
        use sea_orm::{ConnectionTrait, DbBackend, Statement};

        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;
        if !has_effective_permission(&auth.permissions, &Permission::LOGS_READ) {
            return Err(ServerFnError::new(
                "logs:read required to inspect event transport status",
            ));
        }

        let runtime = expect_context::<rustok_api::HostRuntimeContext>();
        let root = runtime
            .shared_get::<HostSettingsSnapshot>()
            .map(|snapshot| snapshot.value().clone())
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
            "outbox" => "outbox",
            "iggy" if iggy_mode.eq_ignore_ascii_case("remote") => "iggy_external",
            "iggy" => "iggy_embedded",
            _ => "memory",
        }
        .to_string();
        let statement = match runtime.db().get_database_backend() {
            DbBackend::Sqlite => Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT COALESCE(SUM(CASE WHEN status = ?1 THEN 1 ELSE 0 END), 0) AS pending_events, COALESCE(SUM(CASE WHEN status = ?2 THEN 1 ELSE 0 END), 0) AS dlq_events FROM sys_events",
                vec!["pending".into(), "failed".into()],
            ),
            _ => Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT COALESCE(SUM(CASE WHEN status = $1 THEN 1 ELSE 0 END), 0) AS pending_events, COALESCE(SUM(CASE WHEN status = $2 THEN 1 ELSE 0 END), 0) AS dlq_events FROM sys_events",
                vec!["pending".into(), "failed".into()],
            ),
        };
        let (pending_events, dlq_events) = match runtime.db().query_one(statement).await {
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
                    "memory".into(),
                    "outbox".into(),
                    "iggy_embedded".into(),
                    "iggy_external".into(),
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
pub(super) async fn event_settings_native() -> Result<PlatformSettingsResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::HostSettingsSnapshot;
        use rustok_api::{AuthContext, Permission, TenantContext, has_effective_permission};
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
                vec![tenant.id.into(), "events".into()],
            ),
            _ => Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT settings FROM platform_settings WHERE tenant_id = $1 AND category = $2 LIMIT 1",
                vec![tenant.id.into(), "events".into()],
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
                    "transport": match transport { "outbox" => "outbox", "iggy" if mode.eq_ignore_ascii_case("remote") => "iggy_external", "iggy" => "iggy_embedded", _ => "memory" },
                    "relay_interval_ms": events.get("relay_interval_ms").and_then(|value| value.as_u64()).unwrap_or(1_000),
                    "max_attempts": events.pointer("/relay_retry_policy/max_attempts").and_then(|value| value.as_i64()).unwrap_or(5),
                    "dlq_enabled": events.pointer("/dlq/enabled").and_then(|value| value.as_bool()).unwrap_or(true),
                    "iggy_addresses": events.pointer("/iggy/remote/addresses").and_then(|value| value.as_array()).map(|items| items.iter().filter_map(|item| item.as_str()).collect::<Vec<_>>().join(",")).unwrap_or_else(|| "127.0.0.1:8090".to_string()),
                    "iggy_protocol": events.pointer("/iggy/remote/protocol").and_then(|value| value.as_str()).unwrap_or("tcp"),
                    "iggy_username": events.pointer("/iggy/remote/username").and_then(|value| value.as_str()).unwrap_or("iggy"),
                    "iggy_password": events.pointer("/iggy/remote/password").and_then(|value| value.as_str()).unwrap_or(""),
                    "iggy_tls": events.pointer("/iggy/remote/tls_enabled").and_then(|value| value.as_bool()).unwrap_or(false),
                    "iggy_stream": events.pointer("/iggy/topology/stream_name").and_then(|value| value.as_str()).unwrap_or("rustok"),
                    "iggy_partitions": events.pointer("/iggy/topology/domain_partitions").and_then(|value| value.as_u64()).unwrap_or(8),
                    "iggy_replication": events.pointer("/iggy/topology/replication_factor").and_then(|value| value.as_u64()).unwrap_or(1),
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
            "admin/event-settings requires the `ssr` feature",
        ))
    }
}
