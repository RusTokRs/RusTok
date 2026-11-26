use chrono::{DateTime, Utc};
use sea_orm::entity::prelude::DateTimeWithTimeZone;
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, DbBackend, DbErr, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Statement,
};
use uuid::Uuid;

use crate::models::_entities::users::{Column as UsersColumn, Entity as UsersEntity};

#[derive(Debug, Clone, Copy, Default)]
pub struct DashboardUserStatsSnapshot {
    pub total_count: i64,
    pub current_count: i64,
    pub previous_count: i64,
}

#[derive(Debug, Clone)]
pub struct DashboardActivityUser {
    pub id: Uuid,
    pub email: String,
    pub name: Option<String>,
    pub created_at: DateTimeWithTimeZone,
}

pub async fn load_user_stats_snapshot(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    current_period_start: DateTime<Utc>,
    previous_period_start: DateTime<Utc>,
) -> Result<DashboardUserStatsSnapshot, DbErr> {
    let backend = db.get_database_backend();

    let statement = match backend {
        DbBackend::Sqlite => {
            let sql = r#"
                SELECT
                    CAST(COUNT(*) AS INTEGER) AS total_count,
                    CAST(COALESCE(SUM(CASE WHEN created_at >= ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS current_count,
                    CAST(COALESCE(SUM(CASE WHEN created_at >= ?3 AND created_at < ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS previous_count
                FROM users
                WHERE tenant_id = ?1
                "#;

            let values = vec![
                tenant_id.into(),
                current_period_start.into(),
                previous_period_start.into(),
            ];

            Statement::from_sql_and_values(backend, sql, values)
        }
        _ => {
            let sql = r#"
                SELECT
                    COUNT(*)::bigint AS total_count,
                    COALESCE(SUM(CASE WHEN created_at >= $2 THEN 1 ELSE 0 END), 0)::bigint AS current_count,
                    COALESCE(SUM(CASE WHEN created_at >= $3 AND created_at < $2 THEN 1 ELSE 0 END), 0)::bigint AS previous_count
                FROM users
                WHERE tenant_id = $1
                "#;

            let values = vec![
                tenant_id.into(),
                current_period_start.into(),
                previous_period_start.into(),
            ];

            Statement::from_sql_and_values(backend, sql, values)
        }
    };

    let Some(row) = db.query_one_raw(statement).await? else {
        return Ok(DashboardUserStatsSnapshot::default());
    };

    Ok(DashboardUserStatsSnapshot {
        total_count: row.try_get("", "total_count")?,
        current_count: row.try_get("", "current_count")?,
        previous_count: row.try_get("", "previous_count")?,
    })
}

pub async fn load_recent_user_activity(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    limit: u64,
) -> Result<Vec<DashboardActivityUser>, DbErr> {
    let users = recent_user_activity_query(tenant_id, limit).all(db).await?;

    Ok(users
        .into_iter()
        .map(|user| DashboardActivityUser {
            id: user.id,
            email: user.email,
            name: user.name,
            created_at: user.created_at,
        })
        .collect())
}

fn recent_user_activity_query(tenant_id: Uuid, limit: u64) -> sea_orm::Select<UsersEntity> {
    UsersEntity::find()
        .filter(UsersColumn::TenantId.eq(tenant_id))
        .order_by_desc(UsersColumn::CreatedAt)
        .order_by_desc(UsersColumn::Id)
        .limit(limit)
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::QueryTrait;

    #[test]
    fn recent_activity_uses_a_unique_tie_breaker_on_both_backends() {
        for backend in [DbBackend::Sqlite, DbBackend::Postgres] {
            let sql = recent_user_activity_query(Uuid::new_v4(), 8)
                .build(backend)
                .to_string();
            let order = sql.split("ORDER BY").nth(1).expect("activity ordering");
            assert!(
                order.contains("\"created_at\" DESC"),
                "{sql}"
            );
            assert!(
                order.contains("\"id\" DESC"),
                "{sql}"
            );
            assert!(order.find("created_at") < order.find("\"id\""), "{sql}");
        }
    }
}
