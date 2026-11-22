use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement, Value as SqlValue};

use super::source_job_lease::{lease_clock, placeholder_prefix};

#[derive(Debug)]
pub(super) enum FailureCountError {
    LeaseLost,
    Storage,
}

/// Read the current budget without holding a lock across retry policy evaluation.
/// The transition revalidates the lease after locking. Concurrent readers may see the same
/// count, but at most one can increment it: both writes require and leave `running`.
pub(super) async fn next_failure_count(
    db: &DatabaseConnection,
    backend: DbBackend,
    kind: &'static str,
    lease: Vec<SqlValue>,
) -> Result<u32, FailureCountError> {
    let prefix = placeholder_prefix(backend);
    let clock = lease_clock(backend);
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "SELECT CAST(failure_count AS BIGINT) AS failure_count FROM index_jobs \
                 WHERE tenant_id = {prefix}1 AND job_id = {prefix}2 AND kind = '{kind}' \
                 AND state = 'running' AND lease_owner = {prefix}3 AND attempt_count = {prefix}4 \
                 AND lease_expires_at > {clock} AND cancel_requested = FALSE"
            ),
            lease,
        ))
        .await
        .map_err(|_| FailureCountError::Storage)?
        .ok_or(FailureCountError::LeaseLost)?;
    let count: i64 = row
        .try_get("", "failure_count")
        .map_err(|_| FailureCountError::Storage)?;
    // The persisted counter is signed INT4.
    i32::try_from(count)
        .ok()
        .and_then(|count| count.checked_add(1))
        .and_then(|count| u32::try_from(count).ok())
        .filter(|count| *count > 0)
        .ok_or(FailureCountError::Storage)
}
