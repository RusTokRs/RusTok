use leptos::prelude::*;

use crate::features::dashboard::model::{
    ActivityItem, ActivityUser, DashboardStats, DashboardStatsResponse, RecentActivityResponse,
};

#[cfg(feature = "ssr")]
use chrono::Utc;
#[cfg(feature = "ssr")]
use sea_orm::{ConnectionTrait, DbBackend, Statement};

#[server(prefix = "/api/fn", endpoint = "admin/dashboard-stats")]
pub(super) async fn dashboard_stats_native() -> Result<DashboardStatsResponse, ServerFnError> {
    #[cfg(feature = "ssr")]
    {
        use chrono::Duration;

        let _auth = leptos_axum::extract::<rustok_api::AuthContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;
        let runtime = expect_context::<rustok_api::HostRuntimeContext>();
        let now = Utc::now();
        let current_period_start = now - Duration::days(30);
        let previous_period_start = current_period_start - Duration::days(30);
        let user_stats = load_period_count_snapshot(
            runtime.db(),
            "users",
            tenant.id,
            current_period_start,
            previous_period_start,
            None,
            None,
        )
        .await
        .map_err(|err| server_error(err.to_string()))?;
        let post_stats = load_period_count_snapshot(
            runtime.db(),
            "nodes",
            tenant.id,
            current_period_start,
            previous_period_start,
            Some(match runtime.db().get_database_backend() {
                DbBackend::Sqlite => " AND kind = ?4",
                _ => " AND kind = $4",
            }),
            Some("post"),
        )
        .await
        .map_err(|err| server_error(err.to_string()))?;
        let order_stats = load_order_stats_snapshot(
            runtime.db(),
            tenant.id,
            current_period_start,
            previous_period_start,
        )
        .await
        .map_err(|err| server_error(err.to_string()))?;
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
            .map_err(|err| server_error(err.to_string()))?;
        let tenant = leptos_axum::extract::<rustok_api::TenantContext>()
            .await
            .map_err(|err| server_error(err.to_string()))?;
        let runtime = expect_context::<rustok_api::HostRuntimeContext>();
        Ok(RecentActivityResponse {
            recent_activity: load_recent_activity(runtime.db(), tenant.id, limit.clamp(1, 50))
                .await
                .map_err(|err| server_error(err.to_string()))?,
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
#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct OrderStatsSnapshot {
    pub(super) total_orders: i64,
    pub(super) total_revenue: i64,
    pub(super) current_orders: i64,
    pub(super) previous_orders: i64,
    pub(super) current_revenue: i64,
    pub(super) previous_revenue: i64,
}

#[cfg(feature = "ssr")]
#[derive(Debug, Clone, Copy, Default)]
pub(super) struct PeriodCountSnapshot {
    pub(super) total_count: i64,
    pub(super) current_count: i64,
    pub(super) previous_count: i64,
}

#[cfg(feature = "ssr")]
pub(crate) fn server_error(message: impl Into<String>) -> ServerFnError {
    ServerFnError::ServerError(message.into())
}

#[cfg(feature = "ssr")]
pub(crate) fn calculate_percent_change(current: i64, previous: i64) -> f64 {
    if previous == 0 {
        if current == 0 { 0.0 } else { 100.0 }
    } else {
        ((current - previous) as f64 / previous as f64) * 100.0
    }
}

#[cfg(feature = "ssr")]
pub(crate) async fn load_period_count_snapshot(
    db: &sea_orm::DatabaseConnection,
    table: &str,
    tenant_id: uuid::Uuid,
    current_period_start: chrono::DateTime<Utc>,
    previous_period_start: chrono::DateTime<Utc>,
    extra_filter_sql: Option<&str>,
    extra_value: Option<&str>,
) -> std::result::Result<PeriodCountSnapshot, sea_orm::DbErr> {
    let backend = db.get_database_backend();
    let filter_sql = extra_filter_sql.unwrap_or("");

    let statement = match backend {
        DbBackend::Sqlite => {
            let sql = format!(
                r#"
                SELECT
                    CAST(COUNT(*) AS INTEGER) AS total_count,
                    CAST(COALESCE(SUM(CASE WHEN created_at >= ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS current_count,
                    CAST(COALESCE(SUM(CASE WHEN created_at >= ?3 AND created_at < ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS previous_count
                FROM {table}
                WHERE tenant_id = ?1{filter_sql}
                "#
            );

            let mut values = vec![
                tenant_id.into(),
                current_period_start.into(),
                previous_period_start.into(),
            ];
            if let Some(extra_value) = extra_value {
                values.push(extra_value.into());
            }

            Statement::from_sql_and_values(backend, sql, values)
        }
        _ => {
            let sql = format!(
                r#"
                SELECT
                    COUNT(*)::bigint AS total_count,
                    COALESCE(SUM(CASE WHEN created_at >= $2 THEN 1 ELSE 0 END), 0)::bigint AS current_count,
                    COALESCE(SUM(CASE WHEN created_at >= $3 AND created_at < $2 THEN 1 ELSE 0 END), 0)::bigint AS previous_count
                FROM {table}
                WHERE tenant_id = $1{filter_sql}
                "#
            );

            let mut values = vec![
                tenant_id.into(),
                current_period_start.into(),
                previous_period_start.into(),
            ];
            if let Some(extra_value) = extra_value {
                values.push(extra_value.into());
            }

            Statement::from_sql_and_values(backend, sql, values)
        }
    };

    let Some(row) = db.query_one(statement).await? else {
        return Ok(PeriodCountSnapshot::default());
    };

    Ok(PeriodCountSnapshot {
        total_count: row.try_get("", "total_count")?,
        current_count: row.try_get("", "current_count")?,
        previous_count: row.try_get("", "previous_count")?,
    })
}

#[cfg(feature = "ssr")]
pub(crate) async fn load_order_stats_snapshot(
    db: &sea_orm::DatabaseConnection,
    tenant_id: uuid::Uuid,
    current_period_start: chrono::DateTime<Utc>,
    previous_period_start: chrono::DateTime<Utc>,
) -> std::result::Result<OrderStatsSnapshot, sea_orm::DbErr> {
    let backend = db.get_database_backend();
    let tenant_id = tenant_id.to_string();

    let statement = match backend {
        DbBackend::Sqlite => Statement::from_sql_and_values(
            backend,
            r#"
            SELECT
                CAST(COUNT(*) AS INTEGER) AS total_orders,
                CAST(COALESCE(SUM(COALESCE(CAST(json_extract(payload, '$.event.data.total') AS INTEGER), 0)), 0) AS INTEGER) AS total_revenue,
                CAST(COALESCE(SUM(CASE WHEN created_at >= ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS current_orders,
                CAST(COALESCE(SUM(CASE WHEN created_at >= ?3 AND created_at < ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS previous_orders,
                CAST(COALESCE(SUM(CASE
                    WHEN created_at >= ?2 THEN COALESCE(CAST(json_extract(payload, '$.event.data.total') AS INTEGER), 0)
                    ELSE 0
                END), 0) AS INTEGER) AS current_revenue,
                CAST(COALESCE(SUM(CASE
                    WHEN created_at >= ?3 AND created_at < ?2 THEN COALESCE(CAST(json_extract(payload, '$.event.data.total') AS INTEGER), 0)
                    ELSE 0
                END), 0) AS INTEGER) AS previous_revenue
            FROM sys_events
            WHERE event_type = 'order.placed'
              AND (
                  json_extract(payload, '$.tenant_id') = ?1
                  OR json_extract(payload, '$.event.tenant_id') = ?1
              )
            "#,
            vec![
                tenant_id.into(),
                current_period_start.into(),
                previous_period_start.into(),
            ],
        ),
        _ => Statement::from_sql_and_values(
            backend,
            r#"
            SELECT
                COUNT(*)::bigint AS total_orders,
                COALESCE(SUM(COALESCE((payload->'event'->'data'->>'total')::bigint, 0)), 0)::bigint AS total_revenue,
                COALESCE(SUM(CASE WHEN created_at >= $2 THEN 1 ELSE 0 END), 0)::bigint AS current_orders,
                COALESCE(SUM(CASE WHEN created_at >= $3 AND created_at < $2 THEN 1 ELSE 0 END), 0)::bigint AS previous_orders,
                COALESCE(SUM(CASE
                    WHEN created_at >= $2 THEN COALESCE((payload->'event'->'data'->>'total')::bigint, 0)
                    ELSE 0
                END), 0)::bigint AS current_revenue,
                COALESCE(SUM(CASE
                    WHEN created_at >= $3 AND created_at < $2 THEN COALESCE((payload->'event'->'data'->>'total')::bigint, 0)
                    ELSE 0
                END), 0)::bigint AS previous_revenue
            FROM sys_events
            WHERE event_type = 'order.placed'
              AND (
                  payload->>'tenant_id' = $1
                  OR payload->'event'->>'tenant_id' = $1
              )
            "#,
            vec![
                tenant_id.into(),
                current_period_start.into(),
                previous_period_start.into(),
            ],
        ),
    };

    let Some(row) = db.query_one(statement).await? else {
        return Ok(OrderStatsSnapshot::default());
    };

    Ok(OrderStatsSnapshot {
        total_orders: row.try_get("", "total_orders")?,
        total_revenue: row.try_get("", "total_revenue")?,
        current_orders: row.try_get("", "current_orders")?,
        previous_orders: row.try_get("", "previous_orders")?,
        current_revenue: row.try_get("", "current_revenue")?,
        previous_revenue: row.try_get("", "previous_revenue")?,
    })
}

#[cfg(feature = "ssr")]
pub(crate) async fn load_recent_activity(
    db: &sea_orm::DatabaseConnection,
    tenant_id: uuid::Uuid,
    limit: i64,
) -> std::result::Result<Vec<ActivityItem>, sea_orm::DbErr> {
    let backend = db.get_database_backend();
    let statement = match backend {
        DbBackend::Sqlite => Statement::from_sql_and_values(
            backend,
            r#"
            SELECT id, email, name, created_at
            FROM users
            WHERE tenant_id = ?1
            ORDER BY created_at DESC
            LIMIT ?2
            "#,
            vec![tenant_id.into(), limit.into()],
        ),
        _ => Statement::from_sql_and_values(
            backend,
            r#"
            SELECT id, email, name, created_at
            FROM users
            WHERE tenant_id = $1
            ORDER BY created_at DESC
            LIMIT $2
            "#,
            vec![tenant_id.into(), limit.into()],
        ),
    };

    let rows = db.query_all(statement).await?;
    rows.into_iter()
        .map(|row| {
            let id: uuid::Uuid = row.try_get("", "id")?;
            let email: String = row.try_get("", "email")?;
            let name: Option<String> = row.try_get("", "name")?;
            let created_at: chrono::DateTime<chrono::FixedOffset> =
                row.try_get("", "created_at")?;

            Ok(ActivityItem {
                id: id.to_string(),
                r#type: "user.created".to_string(),
                description: format!("New user {email} joined"),
                timestamp: created_at.to_rfc3339(),
                user: Some(ActivityUser {
                    id: id.to_string(),
                    name,
                }),
            })
        })
        .collect()
}
