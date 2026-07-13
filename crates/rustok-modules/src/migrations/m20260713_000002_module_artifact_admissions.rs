use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Records the completed database half of an artifact admission. The CAS blob
/// is published before this transaction; a missing row therefore makes that
/// blob eligible for reconciler/retention processing rather than execution.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_admissions (\
                    stage_id UUID PRIMARY KEY,\
                    installation_id UUID NOT NULL UNIQUE REFERENCES module_artifact_installations(installation_id),\
                    payload_digest TEXT NOT NULL,\
                    media_type TEXT NOT NULL,\
                    size_bytes BIGINT NOT NULL CHECK (size_bytes >= 0),\
                    verification_evidence JSONB NOT NULL,\
                    status TEXT NOT NULL CHECK (status IN ('resolved', 'verifying', 'admitted', 'installed', 'active', 'failed', 'inactive', 'rolled_back')),\
                    revision BIGINT NOT NULL CHECK (revision > 0),\
                    committed_at TIMESTAMPTZ NOT NULL\
                )",
                "CREATE INDEX module_artifact_admissions_payload_digest_idx ON module_artifact_admissions (payload_digest)",
                "ALTER TABLE module_artifact_admissions ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_admissions_installation_scope ON module_artifact_admissions \
                    USING (EXISTS (SELECT 1 FROM module_artifact_installations installation \
                        WHERE installation.installation_id = module_artifact_admissions.installation_id \
                        AND (installation.scope_kind = 'platform' OR installation.tenant_id::text = current_setting('rustok.tenant_id', true))))",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_admissions (\
                    stage_id TEXT PRIMARY KEY NOT NULL,\
                    installation_id TEXT NOT NULL UNIQUE REFERENCES module_artifact_installations(installation_id),\
                    payload_digest TEXT NOT NULL,\
                    media_type TEXT NOT NULL,\
                    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),\
                    verification_evidence JSON NOT NULL,\
                    status TEXT NOT NULL CHECK (status IN ('resolved', 'verifying', 'admitted', 'installed', 'active', 'failed', 'inactive', 'rolled_back')),\
                    revision INTEGER NOT NULL CHECK (revision > 0),\
                    committed_at TEXT NOT NULL\
                )",
                "CREATE INDEX module_artifact_admissions_payload_digest_idx ON module_artifact_admissions (payload_digest)",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "module artifact admission migration does not support database backend {backend:?}"
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
            .execute_unprepared("DROP TABLE module_artifact_admissions")
            .await
            .map(|_| ())
    }
}
