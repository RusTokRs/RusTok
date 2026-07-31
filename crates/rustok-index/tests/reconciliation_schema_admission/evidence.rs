use std::io;

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use super::connection::TestResult;

#[derive(Debug)]
pub struct JobEvidence {
    pub job_id: Uuid,
    pub state: String,
    pub attempt_count: i64,
    pub completed_passes: i64,
    pub pages_processed: i64,
    pub source_cursor: JsonValue,
    pub lease_released: bool,
    pub completed: bool,
    pub cancel_requested: bool,
    pub last_error_code: Option<String>,
    pub last_error_details: Option<JsonValue>,
}

pub async fn read_job(db: &DatabaseConnection, tenant_id: Uuid) -> TestResult<JobEvidence> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT job_id, state, attempt_count::bigint AS attempt_count_value, (cursor->>'completed_passes')::bigint AS completed_passes, (cursor->>'pages_processed')::bigint AS pages_processed, cursor->'source_cursor' AS source_cursor, (lease_owner IS NULL AND lease_expires_at IS NULL) AS lease_released, (completed_at IS NOT NULL) AS completed, cancel_requested, last_error_code, last_error_details FROM index_jobs WHERE tenant_id = $1 AND kind = 'reconcile' ORDER BY created_at DESC LIMIT 1",
            vec![tenant_id.into()],
        ))
        .await?
        .ok_or_else(|| io::Error::other("reconciliation job row is missing"))?;
    Ok(JobEvidence {
        job_id: row.try_get("", "job_id")?,
        state: row.try_get("", "state")?,
        attempt_count: row.try_get("", "attempt_count_value")?,
        completed_passes: row.try_get("", "completed_passes")?,
        pages_processed: row.try_get("", "pages_processed")?,
        source_cursor: row.try_get("", "source_cursor")?,
        lease_released: row.try_get("", "lease_released")?,
        completed: row.try_get("", "completed")?,
        cancel_requested: row.try_get("", "cancel_requested")?,
        last_error_code: row.try_get("", "last_error_code")?,
        last_error_details: row.try_get("", "last_error_details")?,
    })
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
        .ok_or_else(|| io::Error::other("count query returned no row"))?
        .try_get("", "value")?)
}
