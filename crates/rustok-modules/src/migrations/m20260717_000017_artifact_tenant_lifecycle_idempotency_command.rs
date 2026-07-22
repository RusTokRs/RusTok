use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Stores the original expected revision with tenant lifecycle intent so an
/// idempotency-key replay can prove it is the same immutable disable command.
/// Historical rows are backfilled from their current revision, which safely
/// rejects ambiguous old replays rather than accepting a mismatched command.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "ALTER TABLE module_artifact_tenant_lifecycle \
                 ADD COLUMN expected_revision BIGINT",
                "UPDATE module_artifact_tenant_lifecycle \
                 SET expected_revision = revision WHERE expected_revision IS NULL",
                "ALTER TABLE module_artifact_tenant_lifecycle \
                 ALTER COLUMN expected_revision SET NOT NULL",
                "ALTER TABLE module_artifact_tenant_lifecycle \
                 ADD CONSTRAINT module_artifact_tenant_lifecycle_expected_revision_positive \
                 CHECK (expected_revision > 0)",
            ],
            DbBackend::Sqlite => &[
                "ALTER TABLE module_artifact_tenant_lifecycle \
                 ADD COLUMN expected_revision INTEGER NOT NULL DEFAULT 1 \
                 CHECK (expected_revision > 0)",
                "UPDATE module_artifact_tenant_lifecycle \
                 SET expected_revision = revision",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact tenant lifecycle idempotency migration does not support database backend {backend:?}"
                )));
            }
        };
        for statement in statements {
            manager
                .get_connection()
                .execute(Statement::from_string(
                    manager.get_database_backend(),
                    (*statement).to_string(),
                ))
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DbBackend::Postgres => {
                for statement in [
                    "ALTER TABLE module_artifact_tenant_lifecycle \
                     DROP CONSTRAINT module_artifact_tenant_lifecycle_expected_revision_positive",
                    "ALTER TABLE module_artifact_tenant_lifecycle DROP COLUMN expected_revision",
                ] {
                    manager
                        .get_connection()
                        .execute_unprepared(statement)
                        .await?;
                }
            }
            DbBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        "ALTER TABLE module_artifact_tenant_lifecycle DROP COLUMN expected_revision",
                    )
                    .await?;
            }
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact tenant lifecycle idempotency migration does not support database backend {backend:?}"
                )));
            }
        }
        Ok(())
    }
}
