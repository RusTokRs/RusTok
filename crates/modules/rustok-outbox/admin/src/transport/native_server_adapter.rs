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
    if auth_tenant_id == resolved_tenant_id
        && !auth_tenant_id.is_nil()
        && !resolved_tenant_id.is_nil()
    {
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
        if auth.user_id.is_nil() {
            return Err(ServerFnError::new(
                "authenticated user is required for outbox inspection",
            ));
        }
        let tenant = leptos_axum::extract::<OptionalTenant>()
            .await
            .map_err(ServerFnError::new)?
            .0
            .ok_or_else(|| {
                ServerFnError::new("tenant context is required for outbox inspection")
            })?;
        if tenant.id.is_nil() {
            return Err(ServerFnError::new(
                "tenant context is required for outbox inspection",
            ));
        }
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
            counters: {
                let counters = query_counters(&db, backend, tenant.id)
                    .await
                    .map_err(ServerFnError::new)?;
                vec![
                    OutboxCounterSnapshot {
                        key: "pending".to_string(),
                        label: "Pending events".to_string(),
                        value: counters.pending,
                    },
                    OutboxCounterSnapshot {
                        key: "dispatched".to_string(),
                        label: "Dispatched events".to_string(),
                        value: counters.dispatched,
                    },
                    OutboxCounterSnapshot {
                        key: "failed".to_string(),
                        label: "Failed events".to_string(),
                        value: counters.failed,
                    },
                    OutboxCounterSnapshot {
                        key: "retries".to_string(),
                        label: "Max retry count".to_string(),
                        value: counters.max_retry_count,
                    },
                ]
            },
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

/// Tenant-scoped operational counters, read in one aggregate pass.
///
/// Previously each card ran its own tenant-filtered scan of `sys_events`,
/// so opening the page cost four passes over the table's tenant slice and
/// a read error was silently rendered as "0 events". Errors now propagate.
#[cfg(feature = "ssr")]
#[derive(Debug, Default, PartialEq, Eq)]
struct OutboxCounters {
    pending: u64,
    dispatched: u64,
    failed: u64,
    max_retry_count: u64,
}

#[cfg(feature = "ssr")]
async fn query_counters(
    db: &sea_orm::DatabaseConnection,
    backend: sea_orm::DbBackend,
    tenant_id: uuid::Uuid,
) -> Result<OutboxCounters, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, Statement};

    let rows = db
        .query_all_raw(Statement::from_sql_and_values(
            backend,
            tenant_scoped_counters_sql(backend),
            [tenant_id.to_string().into()],
        ))
        .await?;
    let mut counters = OutboxCounters::default();
    for row in rows {
        let status: String = row.try_get("", "status")?;
        let count: i64 = row.try_get("", "value")?;
        let max_retry: i64 = row.try_get("", "max_retry")?;
        match status.as_str() {
            "pending" => counters.pending = count as u64,
            "dispatched" => counters.dispatched = count as u64,
            "failed" => counters.failed = count as u64,
            // Unknown statuses cannot be produced by the relay; ignore
            // them rather than inventing a card for them.
            _ => {}
        }
        counters.max_retry_count = counters.max_retry_count.max(max_retry as u64);
    }
    Ok(counters)
}

/// Tenant predicate for operator reads: the denormalised `tenant_id`
/// column is authoritative (and indexed); the payload extraction remains
/// only as a fallback for rows written before the column was backfilled.
#[cfg(feature = "ssr")]
fn tenant_scoped_counters_sql(backend: sea_orm::DbBackend) -> &'static str {
    match backend {
        sea_orm::DbBackend::Sqlite => {
            "SELECT status, COUNT(*) AS value, COALESCE(MAX(retry_count), 0) AS max_retry FROM sys_events WHERE tenant_id = ?1 OR (tenant_id IS NULL AND (CASE WHEN json_type(payload, '$.tenant_id') IS NOT NULL THEN json_extract(payload, '$.tenant_id') = ?1 ELSE json_extract(payload, '$.event.tenant_id') = ?1 END)) GROUP BY status"
        }
        _ => {
            "SELECT status, COUNT(*) AS value, COALESCE(MAX(retry_count), 0)::bigint AS max_retry FROM sys_events WHERE tenant_id = $1::uuid OR (tenant_id IS NULL AND (CASE WHEN payload->'tenant_id' IS NOT NULL THEN payload->>'tenant_id' = $1 ELSE payload->'event'->>'tenant_id' = $1 END)) GROUP BY status"
        }
    }
}

#[cfg(all(test, feature = "ssr"))]
mod tests {
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};

    use super::{OutboxCounters, query_counters, tenant_scoped_counters_sql};

    #[test]
    fn operational_queries_prefer_the_tenant_column_over_payload_fallback() {
        for backend in [DbBackend::Postgres, DbBackend::Sqlite] {
            let sql = tenant_scoped_counters_sql(backend);
            assert!(sql.contains("tenant_id ="));
            assert!(sql.contains("tenant_id IS NULL"));
            assert!(sql.contains("GROUP BY status"));
        }
    }

    #[tokio::test]
    async fn operational_counts_do_not_cross_canonical_tenant_boundary() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("SQLite database");
        db.execute_unprepared(
            "CREATE TABLE sys_events (id TEXT PRIMARY KEY, tenant_id TEXT, status TEXT NOT NULL, payload JSON NOT NULL, retry_count INTEGER NOT NULL)",
        )
        .await
        .expect("create minimal outbox table");

        let tenant_a = uuid::Uuid::new_v4();
        let tenant_b = uuid::Uuid::new_v4();
        // (tenant_id column, payload, retry_count): the column wins when
        // populated; the payload fallback covers un-backfilled rows.
        let rows = [
            (
                Some(tenant_a),
                serde_json::json!({
                    "tenant_id": tenant_a,
                    "event": {"tenant_id": tenant_b}
                }),
                9_i64,
            ),
            (
                None,
                serde_json::json!({"event": {"tenant_id": tenant_b}}),
                3_i64,
            ),
            (
                Some(tenant_b),
                serde_json::json!({"tenant_id": tenant_b}),
                5_i64,
            ),
        ];

        for (tenant_id, payload, retry_count) in rows {
            db.execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO sys_events (id, tenant_id, status, payload, retry_count) VALUES (?1, ?2, 'pending', ?3, ?4)",
                [
                    uuid::Uuid::new_v4().to_string().into(),
                    tenant_id.map(|id| id.to_string()).into(),
                    payload.to_string().into(),
                    retry_count.into(),
                ],
            ))
            .await
            .expect("insert outbox event");
        }

        assert_eq!(
            query_counters(&db, DbBackend::Sqlite, tenant_a)
                .await
                .expect("tenant A counters"),
            OutboxCounters {
                pending: 1,
                dispatched: 0,
                failed: 0,
                max_retry_count: 9,
            }
        );
        assert_eq!(
            query_counters(&db, DbBackend::Sqlite, tenant_b)
                .await
                .expect("tenant B counters"),
            OutboxCounters {
                pending: 2,
                dispatched: 0,
                failed: 0,
                max_retry_count: 5,
            }
        );
    }
}
