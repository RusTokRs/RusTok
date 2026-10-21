use chrono::{DateTime, Utc};
use sea_orm::{ConnectionTrait, DbBackend, Statement};
use uuid::Uuid;

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ContentCountSnapshot {
    pub total_count: i64,
    pub current_count: i64,
    pub previous_count: i64,
}

pub async fn load_post_stats_snapshot<C>(
    db: &C,
    tenant_id: Uuid,
    current_period_start: DateTime<Utc>,
    previous_period_start: DateTime<Utc>,
) -> Result<ContentCountSnapshot, sea_orm::DbErr>
where
    C: ConnectionTrait,
{
    if tenant_id.is_nil() {
        return Err(sea_orm::DbErr::Custom("tenant_id must not be nil".to_string()));
    }
    if current_period_start < previous_period_start {
        return Err(sea_orm::DbErr::Custom(
            "current_period_start must be greater than or equal to previous_period_start".to_string(),
        ));
    }

    let backend = db.get_database_backend();
    let statement = match backend {
        DbBackend::Sqlite => Statement::from_sql_and_values(
            backend,
            r#"
            SELECT
                CAST(COUNT(*) AS INTEGER) AS total_count,
                CAST(COALESCE(SUM(CASE WHEN created_at >= ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS current_count,
                CAST(COALESCE(SUM(CASE WHEN created_at >= ?3 AND created_at < ?2 THEN 1 ELSE 0 END), 0) AS INTEGER) AS previous_count
            FROM nodes
            WHERE tenant_id = ?1
              AND kind = ?4
            "#,
            vec![
                tenant_id.into(),
                current_period_start.into(),
                previous_period_start.into(),
                "post".into(),
            ],
        ),
        _ => Statement::from_sql_and_values(
            backend,
            r#"
            SELECT
                COUNT(*)::bigint AS total_count,
                COALESCE(SUM(CASE WHEN created_at >= $2 THEN 1 ELSE 0 END), 0)::bigint AS current_count,
                COALESCE(SUM(CASE WHEN created_at >= $3 AND created_at < $2 THEN 1 ELSE 0 END), 0)::bigint AS previous_count
            FROM nodes
            WHERE tenant_id = $1
              AND kind = $4
            "#,
            vec![
                tenant_id.into(),
                current_period_start.into(),
                previous_period_start.into(),
                "post".into(),
            ],
        ),
    };

    let Some(row) = db.query_one_raw(statement).await? else {
        return Ok(ContentCountSnapshot::default());
    };

    Ok(ContentCountSnapshot {
        total_count: row.try_get("", "total_count")?,
        current_count: row.try_get("", "current_count")?,
        previous_count: row.try_get("", "previous_count")?,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm::Database;

    #[tokio::test]
    async fn rejects_nil_tenant_id() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let now = Utc::now();
        let err = load_post_stats_snapshot(&db, Uuid::nil(), now, now)
            .await
            .unwrap_err();
        assert!(err.to_string().contains("tenant_id must not be nil"));
    }

    #[tokio::test]
    async fn rejects_temporal_skew() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        let now = Utc::now();
        let past = now - chrono::Duration::days(1);
        let err = load_post_stats_snapshot(&db, Uuid::new_v4(), past, now)
            .await
            .unwrap_err();
        assert!(err.to_string().contains(
            "current_period_start must be greater than or equal to previous_period_start"
        ));
    }
}
