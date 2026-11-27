//! Keep interrupted HTTP installer jobs inside the single-job admission fence.
//!
//! A stale job is quarantined, never reset to failed: the executor may have
//! crossed a durable target-database boundary before the host process stopped.

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

const CREATE_UNRESOLVED_INDEX: &str = "CREATE UNIQUE INDEX IF NOT EXISTS ux_install_http_jobs_one_unresolved ON install_http_jobs ((CASE WHEN status IN ('running', 'recovery_required') THEN 1 ELSE NULL END))";
const DROP_UNRESOLVED_INDEX: &str = "DROP INDEX IF EXISTS ux_install_http_jobs_one_unresolved";
const CREATE_RUNNING_INDEX: &str = "CREATE UNIQUE INDEX IF NOT EXISTS ux_install_http_jobs_one_running ON install_http_jobs(status) WHERE status = 'running'";
const DROP_RUNNING_INDEX: &str = "DROP INDEX IF EXISTS ux_install_http_jobs_one_running";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres | DatabaseBackend::Sqlite => {
                // Create the stricter guard first. If old recovery-required
                // duplicates exist, this fails while the previous running-only
                // guard remains installed and no durable history is discarded.
                manager
                    .get_connection()
                    .execute_unprepared(CREATE_UNRESOLVED_INDEX)
                    .await?;
                manager
                    .get_connection()
                    .execute_unprepared(DROP_RUNNING_INDEX)
                    .await?;
                Ok(())
            }
            backend => Err(DbErr::Custom(format!(
                "installer unresolved-job admission guard does not support database backend {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres | DatabaseBackend::Sqlite => {
                // The unresolved guard guarantees at most one running row, so
                // restoring the old guard is safe before dropping the new one.
                manager
                    .get_connection()
                    .execute_unprepared(CREATE_RUNNING_INDEX)
                    .await?;
                manager
                    .get_connection()
                    .execute_unprepared(DROP_UNRESOLVED_INDEX)
                    .await?;
                Ok(())
            }
            backend => Err(DbErr::Custom(format!(
                "installer unresolved-job admission guard does not support database backend {backend:?}"
            ))),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use sea_orm_migration::sea_orm::{ConnectionTrait, Database};

    async fn insert(db: &sea_orm::DatabaseConnection, id: u8, status: &str) -> Result<(), DbErr> {
        let id = format!("00000000-0000-0000-0000-{id:012}");
        db.execute_unprepared(&format!(
            "INSERT INTO install_http_jobs(id, status, submitted_at, started_at, updated_at) VALUES ('{id}', '{status}', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
        ))
        .await?;
        Ok(())
    }

    #[tokio::test]
    async fn migration_preserves_old_guard_if_unresolved_history_needs_reconciliation() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("sqlite database");
        let manager = SchemaManager::new(&db);
        super::super::m20260928_000001_create_install_http_jobs::Migration
            .up(&manager)
            .await
            .expect("create installer job schema");
        super::super::m20261028_000001_enforce_single_running_install_http_job::Migration
            .up(&manager)
            .await
            .expect("install old running guard");

        insert(&db, 1, "running").await.expect("running job");
        insert(&db, 2, "recovery_required")
            .await
            .expect("legacy recovery-required row");

        assert!(
            Migration.up(&manager).await.is_err(),
            "duplicate unresolved rows require explicit reconciliation"
        );
        assert!(
            db.execute_unprepared(
                "INSERT INTO install_http_jobs(id, status, submitted_at, started_at, updated_at) VALUES ('00000000-0000-0000-0000-000000000003', 'running', CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP)"
            )
            .await
            .is_err(),
            "the previous running-only guard must remain after failed migration"
        );

        db.execute_unprepared(
            "UPDATE install_http_jobs SET status = 'failed' WHERE status = 'recovery_required'",
        )
        .await
        .expect("operator reconciles legacy recovery row");
        Migration
            .up(&manager)
            .await
            .expect("install stricter unresolved guard");
    }

    #[tokio::test]
    async fn only_one_running_or_recovery_required_job_is_admitted() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("sqlite database");
        let manager = SchemaManager::new(&db);
        super::super::m20260928_000001_create_install_http_jobs::Migration
            .up(&manager)
            .await
            .expect("create installer job schema");
        super::super::m20261028_000001_enforce_single_running_install_http_job::Migration
            .up(&manager)
            .await
            .expect("install old running guard");
        Migration
            .up(&manager)
            .await
            .expect("install unresolved guard");

        insert(&db, 1, "recovery_required")
            .await
            .expect("first unresolved job");
        assert!(
            insert(&db, 2, "running").await.is_err(),
            "a recovery-required job must block a running job"
        );
        assert!(
            insert(&db, 3, "recovery_required").await.is_err(),
            "two recovery-required jobs must also conflict"
        );

        db.execute_unprepared(
            "UPDATE install_http_jobs SET status = 'failed' WHERE id = '00000000-0000-0000-0000-000000000001'",
        )
        .await
        .expect("operator reconciliation releases the admission fence");
        insert(&db, 2, "running")
            .await
            .expect("new running job after explicit reconciliation");
        insert(&db, 4, "failed")
            .await
            .expect("terminal history remains unbounded");
        insert(&db, 5, "succeeded")
            .await
            .expect("successful history remains unbounded");
    }
}
