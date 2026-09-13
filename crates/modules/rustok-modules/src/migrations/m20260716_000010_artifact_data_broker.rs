use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Creates the host-owned structured-value namespace for untrusted artifacts.
/// Guest artifacts address logical keys only; physical tables and storage paths
/// remain outside their contract.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_data_namespaces (\
                    tenant_id UUID NOT NULL,\
                    data_owner_id UUID NOT NULL,\
                    namespace_instance_id UUID NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision BIGINT NOT NULL CHECK (data_contract_revision > 0),\
                    data_contract_digest TEXT NOT NULL CHECK (length(data_contract_digest) = 71),\
                    state TEXT NOT NULL CHECK (state IN ('staging', 'verified', 'serving', 'purged')),\
                    namespace_revision BIGINT NOT NULL CHECK (namespace_revision > 0),\
                    verified_manifest_digest TEXT NULL CHECK (verified_manifest_digest IS NULL OR length(verified_manifest_digest) = 71),\
                    purged_at TIMESTAMPTZ NULL,\
                    created_at TIMESTAMPTZ NOT NULL,\
                    updated_at TIMESTAMPTZ NOT NULL,\
                    PRIMARY KEY (tenant_id, data_owner_id, namespace_instance_id),\
                    CHECK ((state = 'purged') = (purged_at IS NOT NULL)),\
                    CHECK (state <> 'verified' OR verified_manifest_digest IS NOT NULL)\
                )",
                "ALTER TABLE module_artifact_data_namespaces ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_namespaces_scope ON module_artifact_data_namespaces \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE FUNCTION module_artifact_data_namespace_tombstone_guard() RETURNS TRIGGER \
                 LANGUAGE plpgsql AS $$ BEGIN \
                   IF TG_OP = 'DELETE' AND OLD.state = 'verified' THEN \
                     RAISE EXCEPTION 'Verified artifact data namespace is sealed' USING ERRCODE = '23514'; \
                   END IF; \
                   IF TG_OP = 'UPDATE' AND ( \
                     (OLD.state = 'verified' AND NEW.state = 'staging') OR \
                     (OLD.verified_manifest_digest IS NOT NULL AND NEW.verified_manifest_digest IS DISTINCT FROM OLD.verified_manifest_digest)) THEN \
                     RAISE EXCEPTION 'Verified artifact data namespace evidence is immutable' USING ERRCODE = '23514'; \
                   END IF; \
                   IF TG_OP = 'UPDATE' AND ROW(NEW.tenant_id, NEW.data_owner_id, NEW.namespace_instance_id, NEW.module_slug, NEW.data_contract_revision, NEW.data_contract_digest) \
                      IS DISTINCT FROM ROW(OLD.tenant_id, OLD.data_owner_id, OLD.namespace_instance_id, OLD.module_slug, OLD.data_contract_revision, OLD.data_contract_digest) THEN \
                     RAISE EXCEPTION 'Artifact data namespace identity is immutable' USING ERRCODE = '23514'; \
                   END IF; \
                   IF OLD.purged_at IS NOT NULL THEN \
                     IF TG_OP = 'DELETE' THEN \
                       RAISE EXCEPTION 'Purged artifact data namespace is immutable' USING ERRCODE = '23514'; \
                     ELSIF NEW IS DISTINCT FROM OLD THEN \
                       RAISE EXCEPTION 'Purged artifact data namespace is immutable' USING ERRCODE = '23514'; \
                     END IF; \
                   END IF; \
                   IF TG_OP = 'DELETE' THEN RETURN OLD; ELSE RETURN NEW; END IF; \
                 END $$",
                "CREATE TRIGGER module_artifact_data_namespace_tombstone \
                 BEFORE UPDATE OR DELETE ON module_artifact_data_namespaces \
                 FOR EACH ROW EXECUTE FUNCTION module_artifact_data_namespace_tombstone_guard()",
                "CREATE TABLE module_artifact_data_owner_references (\
                    tenant_id UUID NOT NULL,\
                    data_owner_id UUID NOT NULL,\
                    namespace_instance_id UUID NOT NULL,\
                    reference_revision BIGINT NOT NULL CHECK (reference_revision > 0),\
                    PRIMARY KEY (tenant_id, data_owner_id),\
                    FOREIGN KEY (tenant_id, data_owner_id, namespace_instance_id) REFERENCES module_artifact_data_namespaces(tenant_id, data_owner_id, namespace_instance_id)\
                )",
                "ALTER TABLE module_artifact_data_owner_references ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_owner_references_scope ON module_artifact_data_owner_references \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_data (\
                    tenant_id UUID NOT NULL,\
                    data_owner_id UUID NOT NULL,\
                    namespace_instance_id UUID NOT NULL,\
                    data_key TEXT NOT NULL CHECK (length(data_key) BETWEEN 1 AND 256),\
                    value JSONB NOT NULL,\
                    value_size_bytes BIGINT NOT NULL CHECK (value_size_bytes > 0 AND value_size_bytes <= 65536),\
                    revision BIGINT NOT NULL CHECK (revision > 0),\
                    updated_at TIMESTAMPTZ NOT NULL,\
                    PRIMARY KEY (tenant_id, data_owner_id, namespace_instance_id, data_key),\
                    FOREIGN KEY (tenant_id, data_owner_id, namespace_instance_id) REFERENCES module_artifact_data_namespaces(tenant_id, data_owner_id, namespace_instance_id)\
                )",
                "CREATE INDEX module_artifact_data_namespace_idx ON module_artifact_data (tenant_id, data_owner_id, namespace_instance_id)",
                "ALTER TABLE module_artifact_data ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_scope ON module_artifact_data \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_data_operations (\
                    tenant_id UUID NOT NULL,\
                    data_owner_id UUID NOT NULL,\
                    namespace_instance_id UUID NOT NULL,\
                    policy_revision BIGINT NOT NULL CHECK (policy_revision > 0),\
                    idempotency_key UUID NOT NULL,\
                    data_key TEXT NOT NULL CHECK (length(data_key) BETWEEN 1 AND 256),\
                    value JSONB NOT NULL,\
                    expected_revision BIGINT NULL CHECK (expected_revision > 0),\
                    revision BIGINT NOT NULL CHECK (revision > 0),\
                    completed_at TIMESTAMPTZ NOT NULL,\
                    PRIMARY KEY (tenant_id, data_owner_id, namespace_instance_id, policy_revision, idempotency_key),\
                    FOREIGN KEY (tenant_id, data_owner_id, namespace_instance_id) REFERENCES module_artifact_data_namespaces(tenant_id, data_owner_id, namespace_instance_id)\
                )",
                "ALTER TABLE module_artifact_data_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_operations_scope ON module_artifact_data_operations \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_data_namespaces (\
                    tenant_id TEXT NOT NULL,\
                    data_owner_id TEXT NOT NULL,\
                    namespace_instance_id TEXT NOT NULL,\
                    module_slug TEXT NOT NULL,\
                    data_contract_revision INTEGER NOT NULL CHECK (data_contract_revision > 0),\
                    data_contract_digest TEXT NOT NULL CHECK (length(data_contract_digest) = 71),\
                    state TEXT NOT NULL CHECK (state IN ('staging', 'verified', 'serving', 'purged')),\
                    namespace_revision INTEGER NOT NULL CHECK (namespace_revision > 0),\
                    verified_manifest_digest TEXT NULL CHECK (verified_manifest_digest IS NULL OR length(verified_manifest_digest) = 71),\
                    purged_at TEXT NULL,\
                    created_at TEXT NOT NULL,\
                    updated_at TEXT NOT NULL,\
                    PRIMARY KEY (tenant_id, data_owner_id, namespace_instance_id),\
                    CHECK ((state = 'purged') = (purged_at IS NOT NULL)),\
                    CHECK (state <> 'verified' OR verified_manifest_digest IS NOT NULL)\
                )",
                "CREATE TRIGGER module_artifact_data_namespace_verified_update \
                 BEFORE UPDATE ON module_artifact_data_namespaces \
                 WHEN (OLD.state = 'verified' AND NEW.state = 'staging') OR \
                      (OLD.verified_manifest_digest IS NOT NULL AND NEW.verified_manifest_digest IS NOT OLD.verified_manifest_digest) \
                 BEGIN SELECT RAISE(ABORT, 'Verified artifact data namespace evidence is immutable'); END",
                "CREATE TRIGGER module_artifact_data_namespace_verified_delete \
                 BEFORE DELETE ON module_artifact_data_namespaces WHEN OLD.state = 'verified' \
                 BEGIN SELECT RAISE(ABORT, 'Verified artifact data namespace is sealed'); END",
                "CREATE TRIGGER module_artifact_data_namespace_identity_update \
                 BEFORE UPDATE ON module_artifact_data_namespaces \
                 WHEN NEW.tenant_id IS NOT OLD.tenant_id OR NEW.data_owner_id IS NOT OLD.data_owner_id OR \
                      NEW.namespace_instance_id IS NOT OLD.namespace_instance_id OR NEW.module_slug IS NOT OLD.module_slug OR \
                      NEW.data_contract_revision IS NOT OLD.data_contract_revision OR NEW.data_contract_digest IS NOT OLD.data_contract_digest \
                 BEGIN SELECT RAISE(ABORT, 'Artifact data namespace identity is immutable'); END",
                "CREATE TRIGGER module_artifact_data_namespace_tombstone_update \
                 BEFORE UPDATE ON module_artifact_data_namespaces \
                 WHEN OLD.purged_at IS NOT NULL AND (\
                   NEW.tenant_id IS NOT OLD.tenant_id OR \
                   NEW.data_owner_id IS NOT OLD.data_owner_id OR \
                   NEW.namespace_instance_id IS NOT OLD.namespace_instance_id OR \
                   NEW.module_slug IS NOT OLD.module_slug OR \
                   NEW.data_contract_revision IS NOT OLD.data_contract_revision OR \
                   NEW.data_contract_digest IS NOT OLD.data_contract_digest OR \
                   NEW.state IS NOT OLD.state OR \
                   NEW.namespace_revision IS NOT OLD.namespace_revision OR \
                   NEW.verified_manifest_digest IS NOT OLD.verified_manifest_digest OR \
                   NEW.purged_at IS NOT OLD.purged_at OR \
                   NEW.created_at IS NOT OLD.created_at OR \
                   NEW.updated_at IS NOT OLD.updated_at\
                 ) BEGIN SELECT RAISE(ABORT, 'Purged artifact data namespace is immutable'); END",
                "CREATE TRIGGER module_artifact_data_namespace_tombstone_delete \
                 BEFORE DELETE ON module_artifact_data_namespaces \
                 WHEN OLD.purged_at IS NOT NULL \
                 BEGIN SELECT RAISE(ABORT, 'Purged artifact data namespace is immutable'); END",
                "CREATE TABLE module_artifact_data_owner_references (\
                    tenant_id TEXT NOT NULL,\
                    data_owner_id TEXT NOT NULL,\
                    namespace_instance_id TEXT NOT NULL,\
                    reference_revision INTEGER NOT NULL CHECK (reference_revision > 0),\
                    PRIMARY KEY (tenant_id, data_owner_id),\
                    FOREIGN KEY (tenant_id, data_owner_id, namespace_instance_id) REFERENCES module_artifact_data_namespaces(tenant_id, data_owner_id, namespace_instance_id)\
                )",
                "CREATE TABLE module_artifact_data (\
                    tenant_id TEXT NOT NULL,\
                    data_owner_id TEXT NOT NULL,\
                    namespace_instance_id TEXT NOT NULL,\
                    data_key TEXT NOT NULL CHECK (length(data_key) BETWEEN 1 AND 256),\
                    value JSON NOT NULL,\
                    value_size_bytes INTEGER NOT NULL CHECK (value_size_bytes > 0 AND value_size_bytes <= 65536),\
                    revision INTEGER NOT NULL CHECK (revision > 0),\
                    updated_at TEXT NOT NULL,\
                    PRIMARY KEY (tenant_id, data_owner_id, namespace_instance_id, data_key),\
                    FOREIGN KEY (tenant_id, data_owner_id, namespace_instance_id) REFERENCES module_artifact_data_namespaces(tenant_id, data_owner_id, namespace_instance_id)\
                )",
                "CREATE INDEX module_artifact_data_namespace_idx ON module_artifact_data (tenant_id, data_owner_id, namespace_instance_id)",
                "CREATE TABLE module_artifact_data_operations (\
                    tenant_id TEXT NOT NULL,\
                    data_owner_id TEXT NOT NULL,\
                    namespace_instance_id TEXT NOT NULL,\
                    policy_revision INTEGER NOT NULL CHECK (policy_revision > 0),\
                    idempotency_key TEXT NOT NULL,\
                    data_key TEXT NOT NULL CHECK (length(data_key) BETWEEN 1 AND 256),\
                    value JSON NOT NULL,\
                    expected_revision INTEGER NULL CHECK (expected_revision > 0),\
                    revision INTEGER NOT NULL CHECK (revision > 0),\
                    completed_at TEXT NOT NULL,\
                    PRIMARY KEY (tenant_id, data_owner_id, namespace_instance_id, policy_revision, idempotency_key),\
                    FOREIGN KEY (tenant_id, data_owner_id, namespace_instance_id) REFERENCES module_artifact_data_namespaces(tenant_id, data_owner_id, namespace_instance_id)\
                )",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "artifact data broker migration does not support database backend {backend:?}"
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
        super::protect_artifact_data_namespace_writes(manager, "module_artifact_data").await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in [
            "module_artifact_data_operations",
            "module_artifact_data",
            "module_artifact_data_owner_references",
            "module_artifact_data_namespaces",
        ] {
            manager
                .get_connection()
                .execute_unprepared(&format!("DROP TABLE {table}"))
                .await?;
        }
        if manager.get_database_backend() == DbBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared(
                    "DROP FUNCTION module_artifact_data_namespace_tombstone_guard()",
                )
                .await?;
        }
        super::drop_artifact_data_namespace_write_guard(manager, "module_artifact_data").await?;
        Ok(())
    }
}
