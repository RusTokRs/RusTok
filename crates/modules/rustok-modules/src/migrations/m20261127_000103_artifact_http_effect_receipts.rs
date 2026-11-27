use sea_orm::ConnectionTrait;
use sea_orm_migration::prelude::*;

use crate::backend::SupportedBackend;

/// Durable, tenant-scoped idempotency receipts for mutating artifact HTTP
/// capability calls. An unresolved dispatch is never automatically replayed.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let statements: &[&str] = match backend {
            SupportedBackend::Postgres => &[
                "CREATE TABLE module_artifact_http_effect_receipts (
                    tenant_id UUID NOT NULL,
                    installation_id UUID NOT NULL,
                    execution_idempotency_key UUID NOT NULL,
                    request_idempotency_key UUID NOT NULL,
                    request_digest TEXT NOT NULL CHECK (length(request_digest) = 71 AND request_digest LIKE 'sha256:%'),
                    status TEXT NOT NULL CHECK (status IN ('dispatching', 'completed', 'ambiguous')),
                    response JSONB NULL,
                    created_at TIMESTAMPTZ NOT NULL,
                    updated_at TIMESTAMPTZ NOT NULL,
                    completed_at TIMESTAMPTZ NULL,
                    PRIMARY KEY (tenant_id, installation_id, execution_idempotency_key, request_idempotency_key),
                    CHECK ((status = 'completed' AND response IS NOT NULL AND completed_at IS NOT NULL)
                        OR (status <> 'completed' AND response IS NULL AND completed_at IS NULL))
                )",
                "CREATE INDEX module_artifact_http_effect_receipts_status_idx
                    ON module_artifact_http_effect_receipts (status, updated_at)",
                "ALTER TABLE module_artifact_http_effect_receipts ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_http_effect_receipts_scope
                    ON module_artifact_http_effect_receipts
                    USING (tenant_id::text = current_setting('rustok.tenant_id', true))
                    WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE FUNCTION module_artifact_http_effect_receipt_guard() RETURNS trigger
                    LANGUAGE plpgsql AS $$
                    BEGIN
                      IF TG_OP = 'DELETE' THEN
                        RAISE EXCEPTION 'artifact HTTP effect receipts are immutable';
                      END IF;
                      IF OLD.status <> 'dispatching'
                         OR NEW.status NOT IN ('completed', 'ambiguous')
                         OR ROW(NEW.tenant_id, NEW.installation_id, NEW.execution_idempotency_key,
                                NEW.request_idempotency_key, NEW.request_digest, NEW.created_at)
                            IS DISTINCT FROM
                            ROW(OLD.tenant_id, OLD.installation_id, OLD.execution_idempotency_key,
                                OLD.request_idempotency_key, OLD.request_digest, OLD.created_at) THEN
                        RAISE EXCEPTION 'invalid artifact HTTP effect receipt transition';
                      END IF;
                      RETURN NEW;
                    END $$",
                "CREATE TRIGGER module_artifact_http_effect_receipt_guard
                    BEFORE UPDATE OR DELETE ON module_artifact_http_effect_receipts
                    FOR EACH ROW EXECUTE FUNCTION module_artifact_http_effect_receipt_guard()",
            ],
            SupportedBackend::Sqlite => &[
                "CREATE TABLE module_artifact_http_effect_receipts (
                    tenant_id TEXT NOT NULL,
                    installation_id TEXT NOT NULL,
                    execution_idempotency_key TEXT NOT NULL,
                    request_idempotency_key TEXT NOT NULL,
                    request_digest TEXT NOT NULL CHECK (length(request_digest) = 71 AND request_digest LIKE 'sha256:%'),
                    status TEXT NOT NULL CHECK (status IN ('dispatching', 'completed', 'ambiguous')),
                    response TEXT NULL,
                    created_at TEXT NOT NULL,
                    updated_at TEXT NOT NULL,
                    completed_at TEXT NULL,
                    PRIMARY KEY (tenant_id, installation_id, execution_idempotency_key, request_idempotency_key),
                    CHECK ((status = 'completed' AND response IS NOT NULL AND completed_at IS NOT NULL)
                        OR (status <> 'completed' AND response IS NULL AND completed_at IS NULL))
                )",
                "CREATE INDEX module_artifact_http_effect_receipts_status_idx
                    ON module_artifact_http_effect_receipts (status, updated_at)",
                "CREATE TRIGGER module_artifact_http_effect_receipt_update_guard
                    BEFORE UPDATE ON module_artifact_http_effect_receipts
                    WHEN OLD.status <> 'dispatching'
                      OR NEW.status NOT IN ('completed', 'ambiguous')
                      OR NEW.tenant_id IS NOT OLD.tenant_id
                      OR NEW.installation_id IS NOT OLD.installation_id
                      OR NEW.execution_idempotency_key IS NOT OLD.execution_idempotency_key
                      OR NEW.request_idempotency_key IS NOT OLD.request_idempotency_key
                      OR NEW.request_digest IS NOT OLD.request_digest
                      OR NEW.created_at IS NOT OLD.created_at
                    BEGIN SELECT RAISE(ABORT, 'invalid artifact HTTP effect receipt transition'); END",
                "CREATE TRIGGER module_artifact_http_effect_receipt_delete_guard
                    BEFORE DELETE ON module_artifact_http_effect_receipts
                    BEGIN SELECT RAISE(ABORT, 'artifact HTTP effect receipts are immutable'); END",
            ],
        };
        for statement in statements {
            manager
                .get_connection()
                .execute_unprepared(statement)
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE module_artifact_http_effect_receipts")
            .await?;
        if backend == SupportedBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared("DROP FUNCTION module_artifact_http_effect_receipt_guard()")
                .await?;
        }
        Ok(())
    }
}
