use std::io;

use rustok_core::MigrationSource;
use rustok_index::IndexModule;
use sea_orm::{ConnectionTrait, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

use super::{
    connection::{DATABASE_ENV, TestResult, connect, database_url, scoped_connection},
    schema::persist_schema,
};

pub struct TestDatabase {
    control: DatabaseConnection,
    database_url: String,
    schema_name: String,
    pub tenant_id: Uuid,
}

impl TestDatabase {
    pub async fn setup(case: &str) -> TestResult<Option<Self>> {
        let Some(database_url) = database_url() else {
            eprintln!(
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping reconciliation stored-job admission harness"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let schema_name = format!(
            "rustok_idx_recon_stored_job_{}_{}",
            case,
            Uuid::new_v4().simple()
        );
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let tenant_id = Uuid::new_v4();
        let db = scoped_connection(&database_url, &schema_name).await?;
        db.execute_unprepared("CREATE TABLE tenants (id UUID NOT NULL PRIMARY KEY)")
            .await?;
        db.execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "INSERT INTO tenants (id) VALUES ($1)",
            vec![tenant_id.into()],
        ))
        .await?;

        let manager = SchemaManager::new(&db);
        for migration in IndexModule.migrations() {
            migration.up(&manager).await?;
        }
        persist_schema(&db, tenant_id).await?;

        Ok(Some(Self {
            control,
            database_url,
            schema_name,
            tenant_id,
        }))
    }

    pub async fn connection(&self) -> TestResult<DatabaseConnection> {
        scoped_connection(&self.database_url, &self.schema_name).await
    }

    pub async fn corrupt_request_pass_count(&self, job_id: Uuid) -> TestResult<()> {
        self.update_pending_job(
            job_id,
            "UPDATE index_jobs SET request = jsonb_set(request, '{pass_count}', '2'::jsonb, false), updated_at = CURRENT_TIMESTAMP WHERE tenant_id = $1 AND job_id = $2 AND kind = 'reconcile' AND state = 'pending'",
        )
        .await
    }

    pub async fn restore_request_pass_count(&self, job_id: Uuid) -> TestResult<()> {
        self.update_pending_job(
            job_id,
            "UPDATE index_jobs SET request = jsonb_set(request, '{pass_count}', '1'::jsonb, false), updated_at = CURRENT_TIMESTAMP WHERE tenant_id = $1 AND job_id = $2 AND kind = 'reconcile' AND state = 'pending'",
        )
        .await
    }

    pub async fn corrupt_cursor_contract(&self, job_id: Uuid) -> TestResult<()> {
        self.update_pending_job(
            job_id,
            r#"UPDATE index_jobs SET cursor = jsonb_set(cursor, '{contract}', '"index_reconciliation_cursor_corrupt"'::jsonb, false), updated_at = CURRENT_TIMESTAMP WHERE tenant_id = $1 AND job_id = $2 AND kind = 'reconcile' AND state = 'pending'"#,
        )
        .await
    }

    pub async fn restore_cursor_contract(&self, job_id: Uuid) -> TestResult<()> {
        self.update_pending_job(
            job_id,
            r#"UPDATE index_jobs SET cursor = jsonb_set(cursor, '{contract}', '"index_reconciliation_cursor_v1"'::jsonb, false), updated_at = CURRENT_TIMESTAMP WHERE tenant_id = $1 AND job_id = $2 AND kind = 'reconcile' AND state = 'pending'"#,
        )
        .await
    }

    async fn update_pending_job(&self, job_id: Uuid, sql: &str) -> TestResult<()> {
        let result = self
            .connection()
            .await?
            .execute(Statement::from_sql_and_values(
                DbBackend::Postgres,
                sql.to_owned(),
                vec![self.tenant_id.into(), job_id.into()],
            ))
            .await?;
        if result.rows_affected() != 1 {
            return Err(io::Error::other("expected exactly one pending reconciliation job update").into());
        }
        Ok(())
    }

    pub async fn cleanup(self) -> TestResult<()> {
        self.control
            .execute_unprepared(&format!(
                r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
                self.schema_name
            ))
            .await?;
        Ok(())
    }
}
