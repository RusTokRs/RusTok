use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use uuid::Uuid;

use super::connection::TestResult;

pub async fn running_job_id(db: &DatabaseConnection) -> TestResult<Uuid> {
    Ok(db
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT job_id FROM index_jobs WHERE kind = 'reconcile' AND state = 'running'"
                .to_owned(),
        ))
        .await?
        .ok_or("running reconciliation job was not persisted")?
        .try_get("", "job_id")?)
}
