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
    pub mutation_count: i64,
    pub applied_count: i64,
    pub duplicate_count: i64,
    pub stale_count: i64,
    pub last_error_code: Option<String>,
    pub last_error_details: Option<JsonValue>,
    pub lease_released: bool,
    pub completed: bool,
}

#[derive(Debug)]
pub struct EntityEvidence {
    pub source_version: i64,
    pub is_deleted: bool,
    pub payload: Option<JsonValue>,
}

pub async fn read_job(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    state: &str,
) -> TestResult<JobEvidence> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT job_id, state, attempt_count::bigint AS attempt_count_value, (cursor->>'completed_passes')::bigint AS completed_passes, (cursor->>'pages_processed')::bigint AS pages_processed, cursor->'source_cursor' AS source_cursor, (cursor->>'mutation_count')::bigint AS mutation_count, (cursor->>'applied_count')::bigint AS applied_count, (cursor->>'duplicate_count')::bigint AS duplicate_count, (cursor->>'stale_count')::bigint AS stale_count, last_error_code, last_error_details, (lease_owner IS NULL AND lease_expires_at IS NULL) AS lease_released, (completed_at IS NOT NULL) AS completed FROM index_jobs WHERE tenant_id = $1 AND kind = 'reconcile' AND state = $2 ORDER BY created_at DESC LIMIT 1",
            vec![tenant_id.into(), state.to_owned().into()],
        ))
        .await?
        .ok_or_else(|| io::Error::other(format!("reconciliation {state} job row is missing")))?;
    Ok(JobEvidence {
        job_id: row.try_get("", "job_id")?,
        state: row.try_get("", "state")?,
        attempt_count: row.try_get("", "attempt_count_value")?,
        completed_passes: row.try_get("", "completed_passes")?,
        pages_processed: row.try_get("", "pages_processed")?,
        source_cursor: row.try_get("", "source_cursor")?,
        mutation_count: row.try_get("", "mutation_count")?,
        applied_count: row.try_get("", "applied_count")?,
        duplicate_count: row.try_get("", "duplicate_count")?,
        stale_count: row.try_get("", "stale_count")?,
        last_error_code: row.try_get("", "last_error_code")?,
        last_error_details: row.try_get("", "last_error_details")?,
        lease_released: row.try_get("", "lease_released")?,
        completed: row.try_get("", "completed")?,
    })
}

pub async fn read_entity(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    entity_id: Uuid,
) -> TestResult<EntityEvidence> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT source_version::bigint AS source_version_value, is_deleted, payload FROM index_entities WHERE tenant_id = $1 AND entity_id = $2 LIMIT 1",
            vec![tenant_id.into(), entity_id.into()],
        ))
        .await?
        .ok_or_else(|| io::Error::other("reconciliation entity row is missing"))?;
    Ok(EntityEvidence {
        source_version: row.try_get("", "source_version_value")?,
        is_deleted: row.try_get("", "is_deleted")?,
        payload: row.try_get("", "payload")?,
    })
}

pub async fn assert_inbox_applied(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    event_id: Uuid,
) -> TestResult<()> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*)::bigint AS value FROM index_inbox WHERE tenant_id = $1 AND delivery_id = $2 AND state = 'applied' AND completed_at IS NOT NULL",
            vec![tenant_id.into(), event_id.to_string().into()],
        ))
        .await?
        .ok_or_else(|| io::Error::other("inbox evidence query returned no row"))?;
    let count: i64 = row.try_get("", "value")?;
    if count != 1 {
        return Err(io::Error::other(format!(
            "expected one terminal applied inbox row for event {event_id}, found {count}"
        ))
        .into());
    }
    Ok(())
}

pub async fn count(db: &DatabaseConnection, table: &str) -> TestResult<i64> {
    let sql = match table {
        "index_entities" => "SELECT COUNT(*)::bigint AS value FROM index_entities",
        "index_inbox" => "SELECT COUNT(*)::bigint AS value FROM index_inbox",
        "index_jobs" => "SELECT COUNT(*)::bigint AS value FROM index_jobs WHERE kind = 'reconcile'",
        "index_links" => "SELECT COUNT(*)::bigint AS value FROM index_links",
        _ => panic!("unsupported fixture table"),
    };
    Ok(db
        .query_one(Statement::from_string(DbBackend::Postgres, sql.to_owned()))
        .await?
        .ok_or_else(|| io::Error::other("count query returned no row"))?
        .try_get("", "value")?)
}
