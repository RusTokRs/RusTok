use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Materializes one immutable schedule slot per effective tenant binding. The
/// descriptor remains the schedule definition; this table is only durable
/// owner state for deduplication, lease, retry, cancellation, and evidence.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_schedule_deliveries (\
                    delivery_id UUID PRIMARY KEY, tenant_id UUID NOT NULL,\
                    installation_id UUID NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    binding_id TEXT NOT NULL CHECK (length(trim(binding_id)) BETWEEN 1 AND 128),\
                    schedule_digest TEXT NOT NULL CHECK (length(schedule_digest) = 71),\
                    scheduled_for TIMESTAMPTZ NOT NULL, attempt INTEGER NOT NULL DEFAULT 0 CHECK (attempt >= 0),\
                    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'succeeded', 'cancelled', 'dead_letter')),\
                    available_at TIMESTAMPTZ NOT NULL, claimed_by TEXT NULL CHECK (claimed_by IS NULL OR length(trim(claimed_by)) BETWEEN 1 AND 128), claimed_until TIMESTAMPTZ NULL,\
                    last_error_code TEXT NULL CHECK (last_error_code IS NULL OR length(trim(last_error_code)) BETWEEN 1 AND 96), completed_at TIMESTAMPTZ NULL, cancelled_at TIMESTAMPTZ NULL,\
                    dead_lettered_at TIMESTAMPTZ NULL, created_at TIMESTAMPTZ NOT NULL,\
                    UNIQUE (tenant_id, installation_id, binding_id, scheduled_for),\
                    CHECK ((status = 'running') = (claimed_by IS NOT NULL AND claimed_until IS NOT NULL)),\
                    CHECK ((status = 'succeeded') = (completed_at IS NOT NULL)),\
                    CHECK ((status = 'cancelled') = (cancelled_at IS NOT NULL)),\
                    CHECK ((status = 'dead_letter') = (dead_lettered_at IS NOT NULL))\
                )",
                "CREATE INDEX module_artifact_schedule_deliveries_claim_idx ON module_artifact_schedule_deliveries (tenant_id, status, available_at, delivery_id)",
                "ALTER TABLE module_artifact_schedule_deliveries ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_schedule_deliveries_scope ON module_artifact_schedule_deliveries USING (tenant_id::text = current_setting('rustok.tenant_id', true)) WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_schedule_deliveries (\
                    delivery_id TEXT PRIMARY KEY NOT NULL, tenant_id TEXT NOT NULL,\
                    installation_id TEXT NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    binding_id TEXT NOT NULL CHECK (length(trim(binding_id)) BETWEEN 1 AND 128),\
                    schedule_digest TEXT NOT NULL CHECK (length(schedule_digest) = 71),\
                    scheduled_for TEXT NOT NULL, attempt INTEGER NOT NULL DEFAULT 0 CHECK (attempt >= 0),\
                    status TEXT NOT NULL CHECK (status IN ('pending', 'running', 'succeeded', 'cancelled', 'dead_letter')),\
                    available_at TEXT NOT NULL, claimed_by TEXT NULL CHECK (claimed_by IS NULL OR length(trim(claimed_by)) BETWEEN 1 AND 128), claimed_until TEXT NULL,\
                    last_error_code TEXT NULL CHECK (last_error_code IS NULL OR length(trim(last_error_code)) BETWEEN 1 AND 96), completed_at TEXT NULL, cancelled_at TEXT NULL,\
                    dead_lettered_at TEXT NULL, created_at TEXT NOT NULL,\
                    UNIQUE (tenant_id, installation_id, binding_id, scheduled_for),\
                    CHECK ((status = 'running') = (claimed_by IS NOT NULL AND claimed_until IS NOT NULL)),\
                    CHECK ((status = 'succeeded') = (completed_at IS NOT NULL)),\
                    CHECK ((status = 'cancelled') = (cancelled_at IS NOT NULL)),\
                    CHECK ((status = 'dead_letter') = (dead_lettered_at IS NOT NULL))\
                )",
                "CREATE INDEX module_artifact_schedule_deliveries_claim_idx ON module_artifact_schedule_deliveries (tenant_id, status, available_at, delivery_id)",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact schedule-delivery migration does not support database backend {backend:?}"
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
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE module_artifact_schedule_deliveries")
            .await
            .map(|_| ())
    }
}
