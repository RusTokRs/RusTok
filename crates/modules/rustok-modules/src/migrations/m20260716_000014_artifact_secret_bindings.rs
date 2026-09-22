use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Persists tenant/owner/secret-instance logical bindings separately from
/// structured artifact data. Rows contain resolver references only, never
/// resolved secret values.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_secret_bindings (\
                    tenant_id UUID NOT NULL,\
                    data_owner_id UUID NOT NULL,\
                    secret_instance_id UUID NOT NULL,\
                    reference_name TEXT NOT NULL CHECK (length(reference_name) BETWEEN 1 AND 96),\
                    resolver_alias TEXT NOT NULL CHECK (length(resolver_alias) BETWEEN 1 AND 96),\
                    resolver_key TEXT NOT NULL CHECK (length(resolver_key) BETWEEN 1 AND 512),\
                    revision BIGINT NOT NULL CHECK (revision > 0),\
                    actor_id UUID NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    created_at TIMESTAMPTZ NOT NULL,\
                    updated_at TIMESTAMPTZ NOT NULL,\
                    PRIMARY KEY (tenant_id, data_owner_id, secret_instance_id, reference_name)\
                )",
                "ALTER TABLE module_artifact_secret_bindings ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_secret_bindings_scope ON module_artifact_secret_bindings \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_secret_binding_operations (\
                    tenant_id UUID NOT NULL,\
                    data_owner_id UUID NOT NULL,\
                    secret_instance_id UUID NOT NULL,\
                    idempotency_key UUID NOT NULL,\
                    reference_name TEXT NOT NULL CHECK (length(reference_name) BETWEEN 1 AND 96),\
                    resolver_alias TEXT NOT NULL CHECK (length(resolver_alias) BETWEEN 1 AND 96),\
                    resolver_key TEXT NOT NULL CHECK (length(resolver_key) BETWEEN 1 AND 512),\
                    expected_revision BIGINT NULL CHECK (expected_revision > 0),\
                    actor_id UUID NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0 AND length(trace_id) <= 512),\
                    correlation_id UUID NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    revision BIGINT NOT NULL CHECK (revision > 0),\
                    request_digest TEXT NOT NULL CHECK (length(request_digest) = 71 AND request_digest LIKE 'sha256:%'),\
                    completed_at TIMESTAMPTZ NOT NULL,\
                    PRIMARY KEY (tenant_id, data_owner_id, secret_instance_id, idempotency_key)\
                )",
                "ALTER TABLE module_artifact_secret_binding_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_secret_binding_operations_scope \
                 ON module_artifact_secret_binding_operations \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_secret_bindings (\
                    tenant_id TEXT NOT NULL,\
                    data_owner_id TEXT NOT NULL,\
                    secret_instance_id TEXT NOT NULL,\
                    reference_name TEXT NOT NULL CHECK (length(reference_name) BETWEEN 1 AND 96),\
                    resolver_alias TEXT NOT NULL CHECK (length(resolver_alias) BETWEEN 1 AND 96),\
                    resolver_key TEXT NOT NULL CHECK (length(resolver_key) BETWEEN 1 AND 512),\
                    revision INTEGER NOT NULL CHECK (revision > 0),\
                    actor_id TEXT NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    created_at TEXT NOT NULL,\
                    updated_at TEXT NOT NULL,\
                    PRIMARY KEY (tenant_id, data_owner_id, secret_instance_id, reference_name)\
                )",
                "CREATE TABLE module_artifact_secret_binding_operations (\
                    tenant_id TEXT NOT NULL,\
                    data_owner_id TEXT NOT NULL,\
                    secret_instance_id TEXT NOT NULL,\
                    idempotency_key TEXT NOT NULL,\
                    reference_name TEXT NOT NULL CHECK (length(reference_name) BETWEEN 1 AND 96),\
                    resolver_alias TEXT NOT NULL CHECK (length(resolver_alias) BETWEEN 1 AND 96),\
                    resolver_key TEXT NOT NULL CHECK (length(resolver_key) BETWEEN 1 AND 512),\
                    expected_revision INTEGER NULL CHECK (expected_revision > 0),\
                    actor_id TEXT NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0 AND length(trace_id) <= 512),\
                    correlation_id TEXT NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    revision INTEGER NOT NULL CHECK (revision > 0),\
                    request_digest TEXT NOT NULL CHECK (length(request_digest) = 71 AND request_digest LIKE 'sha256:%'),\
                    completed_at TEXT NOT NULL,\
                    PRIMARY KEY (tenant_id, data_owner_id, secret_instance_id, idempotency_key)\
                )",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact secret-binding migration does not support database backend {backend:?}"
                )));
            }
        };
        for statement in statements {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(
                    manager.get_database_backend(),
                    (*statement).to_string(),
                ))
                .await?;
        }
        let guards: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE FUNCTION module_artifact_secret_receipt_immutable() RETURNS trigger LANGUAGE plpgsql AS $$ BEGIN RAISE EXCEPTION 'secret binding receipts are immutable'; END $$",
                "CREATE TRIGGER module_artifact_secret_receipt_immutable BEFORE UPDATE OR DELETE ON module_artifact_secret_binding_operations FOR EACH ROW EXECUTE FUNCTION module_artifact_secret_receipt_immutable()",
            ],
            DbBackend::Sqlite => &[
                "CREATE TRIGGER module_artifact_secret_receipt_update BEFORE UPDATE ON module_artifact_secret_binding_operations BEGIN SELECT RAISE(ABORT, 'secret binding receipts are immutable'); END",
                "CREATE TRIGGER module_artifact_secret_receipt_delete BEFORE DELETE ON module_artifact_secret_binding_operations BEGIN SELECT RAISE(ABORT, 'secret binding receipts are immutable'); END",
            ],
            _ => unreachable!("unsupported backend rejected before creating tables"),
        };
        for statement in guards {
            manager
                .get_connection()
                .execute_unprepared(statement)
                .await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in [
            "module_artifact_secret_binding_operations",
            "module_artifact_secret_bindings",
        ] {
            manager
                .get_connection()
                .execute_unprepared(&format!("DROP TABLE {table}"))
                .await?;
        }
        if manager.get_database_backend() == DbBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared("DROP FUNCTION module_artifact_secret_receipt_immutable()")
                .await?;
        }
        Ok(())
    }
}
