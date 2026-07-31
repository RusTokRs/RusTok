use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use uuid::Uuid;

use super::connection::TestResult;

#[derive(Debug)]
pub struct DurableJob {
    pub job_id: Uuid,
    pub state: String,
    pub attempt_count: i64,
    pub completed_passes: i64,
    pub pages_processed: i64,
    pub lease_released: bool,
}

pub async fn read_job(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<DurableJob> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT job_id, state, attempt_count::bigint AS attempt_count_value, (cursor->>'completed_passes')::bigint AS completed_passes, (cursor->>'pages_processed')::bigint AS pages_processed, (lease_owner IS NULL) AS lease_released FROM index_jobs WHERE tenant_id = $1 AND kind = 'reconcile'",
            vec![tenant_id.into()],
        ))
        .await?
        .ok_or("reconciliation job row is missing")?;
    Ok(DurableJob {
        job_id: row.try_get("", "job_id")?,
        state: row.try_get("", "state")?,
        attempt_count: row.try_get("", "attempt_count_value")?,
        completed_passes: row.try_get("", "completed_passes")?,
        pages_processed: row.try_get("", "pages_processed")?,
        lease_released: row.try_get("", "lease_released")?,
    })
}

pub async fn expire_attempt_one(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    job_id: Uuid,
) -> TestResult<()> {
    let result = db
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE index_jobs SET lease_expires_at = CURRENT_TIMESTAMP - INTERVAL '1 second', updated_at = CURRENT_TIMESTAMP WHERE tenant_id = $1 AND job_id = $2 AND kind = 'reconcile' AND state = 'running' AND lease_owner = 'lease-worker-a' AND attempt_count = 1",
            vec![tenant_id.into(), job_id.into()],
        ))
        .await?;
    assert_eq!(result.rows_affected(), 1, "attempt one lease must expire exactly once");
    Ok(())
}

pub async fn count(db: &DatabaseConnection, table: &str) -> TestResult<i64> {
    let sql = match table {
        "index_entities" => "SELECT COUNT(*)::bigint AS value FROM index_entities",
        "index_inbox" => "SELECT COUNT(*)::bigint AS value FROM index_inbox",
        "index_jobs" => "SELECT COUNT(*)::bigint AS value FROM index_jobs WHERE kind = 'reconcile'",
        _ => panic!("unsupported fixture table"),
    };
    Ok(db
        .query_one(Statement::from_string(DbBackend::Postgres, sql.to_owned()))
        .await?
        .ok_or("count query returned no row")?
        .try_get("", "value")?)
}
