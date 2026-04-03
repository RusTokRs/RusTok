use leptos::prelude::*;
use serde::{Deserialize, Serialize};
use std::fmt::{Display, Formatter};

use crate::model::OutboxAdminBootstrap;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub enum ApiError {
    ServerFn(String),
}

impl Display for ApiError {
    fn fmt(&self, f: &mut Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ServerFn(error) => write!(f, "{error}"),
        }
    }
}

impl std::error::Error for ApiError {}

impl From<ServerFnError> for ApiError {
    fn from(value: ServerFnError) -> Self {
        Self::ServerFn(value.to_string())
    }
}

pub async fn fetch_bootstrap() -> Result<OutboxAdminBootstrap, ApiError> {
    outbox_bootstrap_native().await.map_err(Into::into)
}

#[server(prefix = "/api/fn", endpoint = "outbox/bootstrap")]
async fn outbox_bootstrap_native() -> Result<OutboxAdminBootstrap, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use leptos::prelude::expect_context;
        use loco_rs::app::AppContext;
        use rustok_api::{AuthContext, OptionalTenant};
        use rustok_core::{HealthStatus, RusToKModule};

        let app_ctx = expect_context::<AppContext>();
        let _auth = leptos_axum::extract::<AuthContext>()
            .await
            .map_err(ServerFnError::new)?;
        let tenant = leptos_axum::extract::<OptionalTenant>()
            .await
            .ok()
            .and_then(|value| value.0);

        let db = app_ctx.db.clone();
        let backend = sea_orm::ConnectionTrait::get_database_backend(&db);

        let module = rustok_outbox::OutboxModule;
        Ok(OutboxAdminBootstrap {
            tenant_slug: tenant.map(|tenant| tenant.slug),
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
                    value: query_status_count(&db, backend, "pending")
                        .await
                        .map_err(ServerFnError::new)?,
                },
                OutboxCounterSnapshot {
                    key: "dispatched".to_string(),
                    label: "Dispatched events".to_string(),
                    value: query_status_count(&db, backend, "dispatched")
                        .await
                        .map_err(ServerFnError::new)?,
                },
                OutboxCounterSnapshot {
                    key: "failed".to_string(),
                    label: "Failed events".to_string(),
                    value: query_status_count(&db, backend, "failed")
                        .await
                        .map_err(ServerFnError::new)?,
                },
                OutboxCounterSnapshot {
                    key: "retries".to_string(),
                    label: "Max retry count".to_string(),
                    value: query_scalar_i64(
                        &db,
                        backend,
                        "SELECT COALESCE(MAX(retry_count), 0) AS value FROM sys_events",
                    )
                    .await
                    .map_err(ServerFnError::new)? as u64,
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
    status: &str,
) -> Result<u64, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, QueryResult, Statement};

    let row = db
        .query_one(Statement::from_sql_and_values(
            backend,
            "SELECT COUNT(*) AS value FROM sys_events WHERE status = $1",
            [status.into()],
        ))
        .await?;
    Ok(row
        .and_then(|row: QueryResult| row.try_get::<i64>("", "value").ok())
        .unwrap_or_default() as u64)
}

#[cfg(feature = "ssr")]
async fn query_scalar_i64(
    db: &sea_orm::DatabaseConnection,
    backend: sea_orm::DbBackend,
    sql: &str,
) -> Result<i64, sea_orm::DbErr> {
    use sea_orm::{ConnectionTrait, QueryResult, Statement};

    let row = db
        .query_one(Statement::from_string(backend, sql.to_string()))
        .await?;
    Ok(row
        .and_then(|row: QueryResult| row.try_get::<i64>("", "value").ok())
        .unwrap_or_default())
}
