use leptos::prelude::*;

use crate::core::OutboxAdminBootstrap;
#[cfg(feature = "ssr")]
use crate::core::OutboxCounterSnapshot;

pub async fn fetch_bootstrap_native() -> Result<OutboxAdminBootstrap, ServerFnError> {
    outbox_bootstrap_native().await
}

#[cfg(feature = "ssr")]
fn require_outbox_admin_tenant_scope(
    auth_tenant_id: uuid::Uuid,
    resolved_tenant_id: uuid::Uuid,
) -> Result<(), ServerFnError> {
    if auth_tenant_id == resolved_tenant_id {
        return Ok(());
    }

    tracing::warn!(
        auth_tenant_id = %auth_tenant_id,
        resolved_tenant_id = %resolved_tenant_id,
        code = "outbox.admin_tenant_scope_mismatch",
        boundary = "outbox_admin_native_transport",
        "outbox admin permissions cannot cross the resolved tenant boundary"
    );
    Err(ServerFnError::new("Outbox admin access is denied"))
}

#[server(prefix = "/api/fn", endpoint = "outbox/bootstrap")]
async fn outbox_bootstrap_native() -> Result<OutboxAdminBootstrap, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use rustok_api::{
            AuthContext, HostRuntimeContext, OptionalTenant, Permission, has_effective_permission,
        };
        use rustok_core::{HealthStatus, RusToKModule};

        let runtime_ctx = expect_context::<HostRuntimeContext>();
        let auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<OptionalTenant>()
            .await
            .map_err(ServerFnError::new)?
            .0
            .ok_or_else(|| {
                ServerFnError::new("tenant context is required for outbox inspection")
            })?;
        require_outbox_admin_tenant_scope(auth.tenant_id, tenant.id)?;

        if !has_effective_permission(&auth.permissions, &Permission::LOGS_READ) {
            return Err(ServerFnError::new(
                "logs:read required to inspect outbox operational state",
            ));
        }

        let db = runtime_ctx.db_clone();
        let backend = sea_orm::ConnectionTrait::get_database_backend(&db);

        let module = rustok_outbox::OutboxModule;
        Ok(OutboxAdminBootstrap {
            tenant_slug: Some(tenant.slug),
            health: match module.health().await {
                HealthStatus::Healthy => "healthy",
                HealthStatus::Degraded => "degraded",
                HealthStatus::Unhealthy => "unhealthy",
            }
            .to_string(),
            counters: vec![
                OutboxCounterSnapshot {
                    key: "pending".to_string(),
                    label: "Pending events".to_string(),
                    value: query_status_count(&db, backend, tenant.id, "pending")
                        .await
                        .map_err(ServerFnError::new)?,
                },
                OutboxCounterSnapshot {
                    key: "dispatched".to_string(),
                    label: "Dispatched events".to_string(),
                    value: query_status_count(&db, backend, tenant.id, "dispatched")
                        .await
                        .map_err(ServerFnError::new)?,
                },
                OutboxCounterSnapshot {
                    key: "failed".to_string(),
                    label: "Failed events".to_string(),
                    value: query_status_count(&db, backend, tenant.id, "failed")
                        .await
                        .map_err(ServerFnError::new)?,
                },
                OutboxCounterSnapshot {
                    key: "retries".to_string(),
                    label: "Max retry count".to_string(),
                    value: query_max_retry_count(&db, backend, tenant.id)
                        .await
                        .map_err(ServerFnError::new)?,
                },
            ],
            relay_notes: vec![
                "Relay execution remains owned by apps/server runtime wiring.".to_string(),
                "This module-owned UI is read-only and does not replace transport controllers."
                    .to_string(),
            ],
        })
    }
    #[cfg(not(feature = "ssr"))]
    {
        Err(ServerFnError::new(
            "rustok-outbox-admin requires the `ssr` feature for native bootstrap",
        ))
    }
}

#[cfg(feature = "ssr")]
async fn query_status_count(
    db: &sea_orm::DatabaseConnection,
    backend: sea_orm::DbBackend,
    tenant_id: uuid::Uuid,
    status: &str,
) -> Result<u64, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, QueryResult, Statement};

    let sql = tenant_scoped_status_sql(backend);
    let row = db
        .query_one(Statement::from_sql_and_values(
            backend,
            sql,
            [status.into(), tenant_id.to_string().into()],
        ))
        .await?;
    Ok(row
        .and_then(|row: QueryResult| row.try_get::<i64>("", "value").ok())
        .unwrap_or_default() as u64)
}

#[cfg(feature = "ssr")]
async fn query_max_retry_count(
    db: &sea_orm::DatabaseConnection,
    backend: sea_orm::DbBackend,
    tenant_id: uuid::Uuid,
) -> Result<u64, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, QueryResult, Statement};

    let sql = tenant_scoped_max_retry_sql(backend);
    let row = db
        .query_one(Statement::from_sql_and_values(
            backend,
            sql,
            [tenant_id.to_string().into()],
        ))
        .await?;
    Ok(row
        .and_then(|row: QueryResult| row.try_get::<i64>("", "value").ok())
        .unwrap_or_default() as u64)
}

#[cfg(feature = "ssr")]
fn tenant_scoped_status_sql(backend: sea_orm::DbBackend) -> &'static str {
    match backend {
        sea_orm::DbBackend::Sqlite => {
            "SELECT COUNT(*) AS value FROM sys_events WHERE status = ?1 AND (json_extract(payload, '$.tenant_id') = ?2 OR json_extract(payload, '$.event.tenant_id') = ?2)"
        }
        _ => {
            "SELECT COUNT(*) AS value FROM sys_events WHERE status = $1 AND (payload->>'tenant_id' = $2 OR payload->'event'->>'tenant_id' = $2)"
        }
    }
}

#[cfg(feature = "ssr")]
fn tenant_scoped_max_retry_sql(backend: sea_orm::DbBackend) -> &'static str {
    match backend {
        sea_orm::DbBackend::Sqlite => {
            "SELECT COALESCE(MAX(retry_count), 0) AS value FROM sys_events WHERE json_extract(payload, '$.tenant_id') = ?1 OR json_extract(payload, '$.event.tenant_id') = ?1"
        }
        _ => {
            "SELECT COALESCE(MAX(retry_count), 0) AS value FROM sys_events WHERE payload->>'tenant_id' = $1 OR payload->'event'->>'tenant_id' = $1"
        }
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use sea_orm::DbBackend;

    use super::{tenant_scoped_max_retry_sql, tenant_scoped_status_sql};

    #[test]
    fn operational_queries_are_tenant_scoped_for_supported_backends() {
        for backend in [DbBackend::Postgres, DbBackend::Sqlite] {
            let status = tenant_scoped_status_sql(backend);
            let retries = tenant_scoped_max_retry_sql(backend);
            assert!(status.contains("tenant_id"));
            assert!(status.contains("event"));
            assert!(retries.contains("tenant_id"));
            assert!(retries.contains("event"));
        }
    }
}
