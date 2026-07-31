use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use uuid::Uuid;

use super::connection::TestResult;

#[derive(Debug)]
pub struct JobEvidence {
    pub job_id: Uuid,
    pub state: String,
    pub attempt_count: i64,
    pub completed_passes: i64,
    pub pages_processed: i64,
}

pub async fn job_evidence(db: &DatabaseConnection) -> TestResult<JobEvidence> {
    let row = db
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT job_id, state, attempt_count::bigint AS attempt_count_value, (cursor->>'completed_passes')::bigint AS completed_passes, (cursor->>'pages_processed')::bigint AS pages_processed FROM index_jobs WHERE kind = 'reconcile'".to_owned(),
        ))
        .await?
        .ok_or("reconciliation job evidence is missing")?;
    Ok(JobEvidence {
        job_id: row.try_get("", "job_id")?,
        state: row.try_get("", "state")?,
        attempt_count: row.try_get("", "attempt_count_value")?,
        completed_passes: row.try_get("", "completed_passes")?,
        pages_processed: row.try_get("", "pages_processed")?,
    })
}

pub async fn count(db: &DatabaseConnection, table: &str) -> TestResult<i64> {
    let sql = format!("SELECT COUNT(*)::bigint AS value FROM {table}");
    Ok(db
        .query_one(Statement::from_string(DbBackend::Postgres, sql))
        .await?
        .ok_or("count query returned no row")?
        .try_get("", "value")?)
}
