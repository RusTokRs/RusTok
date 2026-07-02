use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Default)]
pub struct OrderStatsSnapshot {
    pub total_orders: i64,
    pub total_revenue: i64,
    pub current_orders: i64,
    pub previous_orders: i64,
    pub current_revenue: i64,
    pub previous_revenue: i64,
}

pub async fn load_order_stats_snapshot<C>(
    db: &C,
    tenant_id: Uuid,
    current_period_start: DateTime<Utc>,
    previous_period_start: DateTime<Utc>,
) -> Result<OrderStatsSnapshot, sea_orm::DbErr>
where
    C: ConnectionTrait,
{
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
