use std::io;

use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use serde_json::Value as JsonValue;
use uuid::Uuid;

use super::connection::TestResult;

#[derive(Debug)]
pub struct FailureEvidence {
    pub job_id: Uuid,
    pub state: String,
    pub attempt_count: i64,
    pub completed_passes: i64,
    pub pages_processed: i64,
    pub last_error_code: String,
    pub last_error_details: JsonValue,
    pub lease_released: bool,
    pub completed: bool,
}

pub async fn read_failure(
    db: &DatabaseConnection,
    tenant_id: Uuid,
) -> TestResult<FailureEvidence> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT job_id, state, attempt_count::bigint AS attempt_count_value, (cursor->>'completed_passes')::bigint AS completed_passes, (cursor->>'pages_processed')::bigint AS pages_processed, last_error_code, last_error_details, (lease_owner IS NULL AND lease_expires_at IS NULL) AS lease_released, (completed_at IS NOT NULL) AS completed FROM index_jobs WHERE tenant_id = $1 AND kind = 'reconcile'",
            vec![tenant_id.into()],
        ))
        .await?
        .ok_or_else(|| io::Error::other("reconciliation failure job row is missing"))?;
    Ok(FailureEvidence {
        job_id: row.try_get("", "job_id")?,
        state: row.try_get("", "state")?,
        attempt_count: row.try_get("", "attempt_count_value")?,
        completed_passes: row.try_get("", "completed_passes")?,
        pages_processed: row.try_get("", "pages_processed")?,
        last_error_code: row.try_get("", "last_error_code")?,
        last_error_details: row.try_get("", "last_error_details")?,
        lease_released: row.try_get("", "lease_released")?,
        completed: row.try_get("", "completed")?,
    })
}

pub async fn count(db: &DatabaseConnection, table: &str) -> TestResult<i64> {
    let sql = match table {
        "index_entities" => "SELECT COUNT(*)::bigint AS value FROM index_entities",
        "index_inbox" => "SELECT COUNT(*)::bigint AS value FROM index_inbox",
        "index_jobs" => {
            "SELECT COUNT(*)::bigint AS value FROM index_jobs WHERE kind = 'reconcile'"
        }
        _ => panic!("unsupported fixture table"),
    };
    Ok(db
        .query_one(Statement::from_string(DbBackend::Postgres, sql.to_owned()))
        .await?
        .ok_or_else(|| io::Error::other("count query returned no row"))?
        .try_get("", "value")?)
}
