//! Enforce exactly one active HTTP installer job across all server replicas.
//!
//! The application-level precheck is not a concurrency primitive: independent
//! replicas may both read "no running job" before either inserts. A partial
//! unique index makes the running claim atomic in the database. Existing
//! duplicate running jobs intentionally cause migration failure until an
//! operator reconciles their durable receipts; never discard one automatically.

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

const CREATE_INDEX: &str = "CREATE UNIQUE INDEX IF NOT EXISTS ux_install_http_jobs_one_running ON install_http_jobs(status) WHERE status = 'running'";
const DROP_INDEX: &str = "DROP INDEX IF EXISTS ux_install_http_jobs_one_running";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres | DatabaseBackend::Sqlite => {
                manager.get_connection().execute_unprepared(CREATE_INDEX).await?;
                Ok(())
            }
            backend => Err(DbErr::Custom(format!(
                "single-running installer job constraint does not support database backend {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres | DatabaseBackend::Sqlite => {
                manager.get_connection().execute_unprepared(DROP_INDEX).await?;
                Ok(())
            }
            backend => Err(DbErr::Custom(format!(
                "single-running installer job constraint does not support database backend {backend:?}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{ConnectionTrait, Database};

    #[tokio::test]
    async fn a_second_running_job_is_rejected_while_finished_history_is_unbounded() {
        let db = Database::connect("sqlite::memory:").await.expect("sqlite database");
        let manager = SchemaManager::new(&db);
        super::super::m20260928_000001_create_install_http_jobs::Migration
            .up(&manager)
            .await
            .expect("create installer job schema");
        Migration.up(&manager).await.expect("install running guard");

        for (id, status, should_succeed) in [
            ("00000000-0000-0000-0000-000000000001", "running", true),
            ("00000000-0000-0000-0000-000000000002", "running", false),
            ("00000000-0000-0000-0000-000000000003", "failed", true),
            ("00000000-0000-0000-0000-000000000004", "failed", true),
        ] {
            let sql = format!(
                "INSERT INTO install_http_jobs(id, status, submitted_at, started_at, updated_at) VALUES ('{id}', '{status}', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
            );
            assert_eq!(
                db.execute_unprepared(&sql).await.is_ok(),
                should_succeed,
                "unexpected running-claim outcome for {id}",
            );
        }

        db.execute_unprepared(
            "UPDATE install_http_jobs SET status = 'succeeded', finished_at = CURRENT_TIMESTAMP WHERE status = 'running'",
        )
        .await
        .expect("release the previous running claim");
        db.execute_unprepared(
            "INSERT INTO install_http_jobs(id, status, submitted_at, started_at, updated_at) VALUES ('00000000-0000-0000-0000-000000000005', 'running', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)",
        )
        .await
        .expect("new running claim must succeed after prior job finished");
    }
}
