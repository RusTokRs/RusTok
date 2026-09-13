use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Persists crash-safe snapshot/restore intents, staging receipts, and post-purge
/// namespace recovery operations with verified CAS cutover.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_data_snapshot_copy_intents (\
                    intent_id UUID PRIMARY KEY,\
                    tenant_id UUID NOT NULL,\
                    data_owner_id UUID NOT NULL,\
                    namespace_instance_id UUID NOT NULL,\
                    snapshot_id UUID NOT NULL,\
                    operation_id UUID NOT NULL,\
                    operation_request_digest TEXT NOT NULL CHECK (length(operation_request_digest) = 71),\
                    request_digest TEXT NOT NULL CHECK (length(request_digest) = 71),\
                    operation_kind TEXT NOT NULL CHECK (operation_kind IN ('snapshot', 'restore')),\
                    object_name TEXT NOT NULL CHECK (length(object_name) BETWEEN 1 AND 256),\
                    source_storage_key TEXT NOT NULL,\
                    target_storage_key TEXT NOT NULL,\
                    digest_sha256 TEXT NOT NULL CHECK (digest_sha256 ~ '^sha256:[0-9a-f]{64}$'),\
                    size_bytes BIGINT NOT NULL CHECK (size_bytes >= 0),\
                    status TEXT NOT NULL CHECK (status IN ('intent', 'staging', 'committed')),\
                    created_at TIMESTAMPTZ NOT NULL,\
                    committed_at TIMESTAMPTZ NULL,\
                    UNIQUE (tenant_id, data_owner_id, namespace_instance_id, operation_id, operation_kind, object_name)\
                )",
                "CREATE INDEX idx_snapshot_copy_intents_scope \
                 ON module_artifact_data_snapshot_copy_intents (tenant_id, snapshot_id, status)",
                "ALTER TABLE module_artifact_data_snapshot_copy_intents ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_snapshot_copy_intents_scope \
                 ON module_artifact_data_snapshot_copy_intents \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_data_snapshot_holds (\
                    tenant_id UUID NOT NULL,\
                    snapshot_id UUID NOT NULL,\
                    holder_kind TEXT NOT NULL CHECK (holder_kind IN ('restore', 'recovery')),\
                    holder_id UUID NOT NULL,\
                    data_owner_id UUID NOT NULL,\
                    namespace_instance_id UUID NOT NULL,\
                    request_digest TEXT NOT NULL CHECK (length(request_digest) = 71),\
                    created_at TIMESTAMPTZ NOT NULL,\
                    released_at TIMESTAMPTZ NULL,\
                    PRIMARY KEY (tenant_id, snapshot_id, holder_kind, holder_id)\
                )",
                "CREATE INDEX idx_artifact_data_snapshot_active_holds \
                 ON module_artifact_data_snapshot_holds (tenant_id, snapshot_id, released_at)",
                "ALTER TABLE module_artifact_data_snapshot_holds ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_snapshot_holds_scope ON module_artifact_data_snapshot_holds \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_data_namespace_recovery_operations (\
                    recovery_id UUID PRIMARY KEY NOT NULL,\
                    tenant_id UUID NOT NULL,\
                    installation_id UUID NOT NULL,\
                    data_owner_id UUID NOT NULL,\
                    source_namespace_instance_id UUID NOT NULL,\
                    namespace_instance_id UUID NOT NULL,\
                    source_snapshot_id UUID NOT NULL,\
                    source_retention_revision BIGINT NOT NULL CHECK (source_retention_revision > 0),\
                    expected_reference_revision BIGINT NOT NULL CHECK (expected_reference_revision > 0),\
                    tombstone_namespace_revision BIGINT NOT NULL CHECK (tombstone_namespace_revision > 0),\
                    target_namespace_revision BIGINT NOT NULL CHECK (target_namespace_revision > 0),\
                    status TEXT NOT NULL CHECK (status IN ('staging', 'verified', 'cutover')),\
                    records_restored BIGINT NOT NULL CHECK (records_restored >= 0),\
                    objects_restored BIGINT NOT NULL CHECK (objects_restored >= 0),\
                    source_manifest_digest TEXT NOT NULL CHECK (length(source_manifest_digest) = 71),\
                    verified_manifest_digest TEXT NULL CHECK (verified_manifest_digest IS NULL OR length(verified_manifest_digest) = 71),\
                    request_digest TEXT NOT NULL CHECK (length(request_digest) = 71),\
                    prepare_request_json TEXT NOT NULL,\
                    authorization_json TEXT NOT NULL,\
                    idempotency_key UUID NOT NULL,\
                    created_at TIMESTAMPTZ NOT NULL,\
                    verified_at TIMESTAMPTZ NULL,\
                    cutover_at TIMESTAMPTZ NULL,\
                    cutover_request_digest TEXT NULL CHECK (cutover_request_digest IS NULL OR length(cutover_request_digest) = 71),\
                    cutover_request_json TEXT NULL,\
                    active_reference_revision BIGINT NULL CHECK (active_reference_revision > 0),\
                    active_namespace_revision BIGINT NULL CHECK (active_namespace_revision > 0),\
                    UNIQUE (tenant_id, installation_id, idempotency_key),\
                    FOREIGN KEY (tenant_id, data_owner_id, source_namespace_instance_id) REFERENCES module_artifact_data_namespaces(tenant_id, data_owner_id, namespace_instance_id),\
                    FOREIGN KEY (tenant_id, data_owner_id, namespace_instance_id) REFERENCES module_artifact_data_namespaces(tenant_id, data_owner_id, namespace_instance_id),\
                    CHECK (status = 'staging' OR (verified_manifest_digest IS NOT NULL AND verified_at IS NOT NULL)),\
                    CHECK (status <> 'cutover' OR (cutover_request_digest IS NOT NULL AND cutover_request_json IS NOT NULL AND cutover_at IS NOT NULL AND active_reference_revision IS NOT NULL AND active_namespace_revision IS NOT NULL))\
                )",
                "CREATE INDEX idx_namespace_recovery_ops_scope \
                 ON module_artifact_data_namespace_recovery_operations (tenant_id, data_owner_id, namespace_instance_id, status)",
                "ALTER TABLE module_artifact_data_namespace_recovery_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_data_namespace_recovery_operations_scope \
                 ON module_artifact_data_namespace_recovery_operations \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_data_snapshot_copy_intents (\
                    intent_id TEXT PRIMARY KEY NOT NULL,\
                    tenant_id TEXT NOT NULL,\
                    data_owner_id TEXT NOT NULL,\
                    namespace_instance_id TEXT NOT NULL,\
                    snapshot_id TEXT NOT NULL,\
                    operation_id TEXT NOT NULL,\
                    operation_request_digest TEXT NOT NULL CHECK (length(operation_request_digest) = 71),\
                    request_digest TEXT NOT NULL CHECK (length(request_digest) = 71),\
                    operation_kind TEXT NOT NULL CHECK (operation_kind IN ('snapshot', 'restore')),\
                    object_name TEXT NOT NULL CHECK (length(object_name) BETWEEN 1 AND 256),\
                    source_storage_key TEXT NOT NULL,\
                    target_storage_key TEXT NOT NULL,\
                    digest_sha256 TEXT NOT NULL CHECK (length(digest_sha256) = 71),\
                    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),\
                    status TEXT NOT NULL CHECK (status IN ('intent', 'staging', 'committed')),\
                    created_at TEXT NOT NULL,\
                    committed_at TEXT NULL,\
                    UNIQUE (tenant_id, data_owner_id, namespace_instance_id, operation_id, operation_kind, object_name)\
                )",
                "CREATE INDEX idx_snapshot_copy_intents_scope \
                 ON module_artifact_data_snapshot_copy_intents (tenant_id, snapshot_id, status)",
                "CREATE TABLE module_artifact_data_snapshot_holds (\
                    tenant_id TEXT NOT NULL,\
                    snapshot_id TEXT NOT NULL,\
                    holder_kind TEXT NOT NULL CHECK (holder_kind IN ('restore', 'recovery')),\
                    holder_id TEXT NOT NULL,\
                    data_owner_id TEXT NOT NULL,\
                    namespace_instance_id TEXT NOT NULL,\
                    request_digest TEXT NOT NULL CHECK (length(request_digest) = 71),\
                    created_at TEXT NOT NULL,\
                    released_at TEXT NULL,\
                    PRIMARY KEY (tenant_id, snapshot_id, holder_kind, holder_id)\
                )",
                "CREATE INDEX idx_artifact_data_snapshot_active_holds \
                 ON module_artifact_data_snapshot_holds (tenant_id, snapshot_id, released_at)",
                "CREATE TABLE module_artifact_data_namespace_recovery_operations (\
                    recovery_id TEXT PRIMARY KEY NOT NULL,\
                    tenant_id TEXT NOT NULL,\
                    installation_id TEXT NOT NULL,\
                    data_owner_id TEXT NOT NULL,\
                    source_namespace_instance_id TEXT NOT NULL,\
                    namespace_instance_id TEXT NOT NULL,\
                    source_snapshot_id TEXT NOT NULL,\
                    source_retention_revision INTEGER NOT NULL CHECK (source_retention_revision > 0),\
                    expected_reference_revision INTEGER NOT NULL CHECK (expected_reference_revision > 0),\
                    tombstone_namespace_revision INTEGER NOT NULL CHECK (tombstone_namespace_revision > 0),\
                    target_namespace_revision INTEGER NOT NULL CHECK (target_namespace_revision > 0),\
                    status TEXT NOT NULL CHECK (status IN ('staging', 'verified', 'cutover')),\
                    records_restored INTEGER NOT NULL CHECK (records_restored >= 0),\
                    objects_restored INTEGER NOT NULL CHECK (objects_restored >= 0),\
                    source_manifest_digest TEXT NOT NULL CHECK (length(source_manifest_digest) = 71),\
                    verified_manifest_digest TEXT NULL CHECK (verified_manifest_digest IS NULL OR length(verified_manifest_digest) = 71),\
                    request_digest TEXT NOT NULL CHECK (length(request_digest) = 71),\
                    prepare_request_json TEXT NOT NULL,\
                    authorization_json TEXT NOT NULL,\
                    idempotency_key TEXT NOT NULL,\
                    created_at TEXT NOT NULL,\
                    verified_at TEXT NULL,\
                    cutover_at TEXT NULL,\
                    cutover_request_digest TEXT NULL CHECK (cutover_request_digest IS NULL OR length(cutover_request_digest) = 71),\
                    cutover_request_json TEXT NULL,\
                    active_reference_revision INTEGER NULL CHECK (active_reference_revision > 0),\
                    active_namespace_revision INTEGER NULL CHECK (active_namespace_revision > 0),\
                    UNIQUE (tenant_id, installation_id, idempotency_key),\
                    FOREIGN KEY (tenant_id, data_owner_id, source_namespace_instance_id) REFERENCES module_artifact_data_namespaces(tenant_id, data_owner_id, namespace_instance_id),\
                    FOREIGN KEY (tenant_id, data_owner_id, namespace_instance_id) REFERENCES module_artifact_data_namespaces(tenant_id, data_owner_id, namespace_instance_id),\
                    CHECK (status = 'staging' OR (verified_manifest_digest IS NOT NULL AND verified_at IS NOT NULL)),\
                    CHECK (status <> 'cutover' OR (cutover_request_digest IS NOT NULL AND cutover_request_json IS NOT NULL AND cutover_at IS NOT NULL AND active_reference_revision IS NOT NULL AND active_namespace_revision IS NOT NULL))\
                )",
                "CREATE INDEX idx_namespace_recovery_ops_scope \
                 ON module_artifact_data_namespace_recovery_operations (tenant_id, data_owner_id, namespace_instance_id, status)",
            ],
            _ => return Err(DbErr::Custom("Unsupported database backend".to_string())),
        };

        for sql in statements {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(
                    manager.get_database_backend(),
                    (*sql).to_string(),
                ))
                .await?;
        }

        protect_recovery_operations(manager).await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements = &[
            "DROP TABLE IF EXISTS module_artifact_data_namespace_recovery_operations",
            "DROP TABLE IF EXISTS module_artifact_data_snapshot_copy_intents",
            "DROP TABLE IF EXISTS module_artifact_data_snapshot_holds",
        ];

        for sql in statements {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(
                    manager.get_database_backend(),
                    (*sql).to_string(),
                ))
                .await?;
        }

        if manager.get_database_backend() == DbBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared("DROP FUNCTION module_artifact_data_recovery_guard()")
                .await?;
        }
        Ok(())
    }
}

async fn protect_recovery_operations(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let identity = [
        "recovery_id",
        "tenant_id",
        "installation_id",
        "data_owner_id",
        "source_namespace_instance_id",
        "namespace_instance_id",
        "source_snapshot_id",
        "source_retention_revision",
        "expected_reference_revision",
        "tombstone_namespace_revision",
        "source_manifest_digest",
        "request_digest",
        "prepare_request_json",
        "authorization_json",
        "idempotency_key",
        "created_at",
    ];
    let evidence = [
        "target_namespace_revision",
        "records_restored",
        "objects_restored",
        "verified_manifest_digest",
        "verified_at",
    ];
    let statements = match manager.get_database_backend() {
        DbBackend::Postgres => {
            let current = identity
                .iter()
                .map(|field| format!("NEW.{field}"))
                .collect::<Vec<_>>()
                .join(",");
            let previous = identity
                .iter()
                .map(|field| format!("OLD.{field}"))
                .collect::<Vec<_>>()
                .join(",");
            let evidence_current = evidence
                .iter()
                .map(|field| format!("NEW.{field}"))
                .collect::<Vec<_>>()
                .join(",");
            let evidence_previous = evidence
                .iter()
                .map(|field| format!("OLD.{field}"))
                .collect::<Vec<_>>()
                .join(",");
            vec![format!(
                "CREATE FUNCTION module_artifact_data_recovery_guard() RETURNS TRIGGER LANGUAGE plpgsql AS $$ BEGIN
                   IF OLD.status='cutover' THEN
                     IF TG_OP='DELETE' THEN
                       RAISE EXCEPTION 'Terminal recovery receipt is immutable' USING ERRCODE='23514';
                     ELSIF NEW IS DISTINCT FROM OLD THEN
                       RAISE EXCEPTION 'Terminal recovery receipt is immutable' USING ERRCODE='23514';
                     END IF;
                   END IF;
                   IF TG_OP='UPDATE' THEN
                     IF ROW({current}) IS DISTINCT FROM ROW({previous}) THEN
                       RAISE EXCEPTION 'Recovery command identity is immutable' USING ERRCODE='23514';
                     END IF;
                     IF OLD.status='verified' AND (NEW.status='staging' OR
                        ROW({evidence_current}) IS DISTINCT FROM ROW({evidence_previous})) THEN
                       RAISE EXCEPTION 'Recovery verification evidence is immutable' USING ERRCODE='23514';
                     END IF;
                   END IF;
                   IF TG_OP='DELETE' THEN RETURN OLD; ELSE RETURN NEW; END IF;
                 END $$"),
                "CREATE TRIGGER module_artifact_data_recovery_identity BEFORE UPDATE OR DELETE
                 ON module_artifact_data_namespace_recovery_operations FOR EACH ROW
                 EXECUTE FUNCTION module_artifact_data_recovery_guard()".into(),
            ]
        }
        DbBackend::Sqlite => {
            let immutable = identity
                .iter()
                .map(|field| format!("NEW.{field} IS NOT OLD.{field}"))
                .collect::<Vec<_>>()
                .join(" OR ");
            let verified = evidence
                .iter()
                .map(|field| format!("NEW.{field} IS NOT OLD.{field}"))
                .collect::<Vec<_>>()
                .join(" OR ");
            vec![
                format!("CREATE TRIGGER module_artifact_data_recovery_identity BEFORE UPDATE
                    ON module_artifact_data_namespace_recovery_operations WHEN {immutable}
                    BEGIN SELECT RAISE(ABORT, 'Recovery command identity is immutable'); END"),
                format!("CREATE TRIGGER module_artifact_data_recovery_verified BEFORE UPDATE
                    ON module_artifact_data_namespace_recovery_operations
                    WHEN OLD.status='verified' AND (NEW.status='staging' OR {verified})
                    BEGIN SELECT RAISE(ABORT, 'Recovery verification evidence is immutable'); END"),
                "CREATE TRIGGER module_artifact_data_recovery_terminal_update BEFORE UPDATE
                    ON module_artifact_data_namespace_recovery_operations
                    WHEN OLD.status='cutover' AND (NEW.recovery_id IS NOT OLD.recovery_id OR NEW.tenant_id IS NOT OLD.tenant_id OR NEW.installation_id IS NOT OLD.installation_id OR NEW.data_owner_id IS NOT OLD.data_owner_id OR NEW.source_namespace_instance_id IS NOT OLD.source_namespace_instance_id OR NEW.namespace_instance_id IS NOT OLD.namespace_instance_id OR NEW.source_snapshot_id IS NOT OLD.source_snapshot_id OR NEW.source_retention_revision IS NOT OLD.source_retention_revision OR NEW.expected_reference_revision IS NOT OLD.expected_reference_revision OR NEW.tombstone_namespace_revision IS NOT OLD.tombstone_namespace_revision OR NEW.source_manifest_digest IS NOT OLD.source_manifest_digest OR NEW.request_digest IS NOT OLD.request_digest OR NEW.prepare_request_json IS NOT OLD.prepare_request_json OR NEW.authorization_json IS NOT OLD.authorization_json OR NEW.idempotency_key IS NOT OLD.idempotency_key OR NEW.created_at IS NOT OLD.created_at OR NEW.target_namespace_revision IS NOT OLD.target_namespace_revision OR NEW.records_restored IS NOT OLD.records_restored OR NEW.objects_restored IS NOT OLD.objects_restored OR NEW.verified_manifest_digest IS NOT OLD.verified_manifest_digest OR NEW.verified_at IS NOT OLD.verified_at OR NEW.status IS NOT OLD.status OR NEW.cutover_at IS NOT OLD.cutover_at OR NEW.cutover_request_digest IS NOT OLD.cutover_request_digest OR NEW.cutover_request_json IS NOT OLD.cutover_request_json OR NEW.active_reference_revision IS NOT OLD.active_reference_revision OR NEW.active_namespace_revision IS NOT OLD.active_namespace_revision)
                    BEGIN SELECT RAISE(ABORT, 'Terminal recovery receipt is immutable'); END".into(),
                "CREATE TRIGGER module_artifact_data_recovery_terminal_delete BEFORE DELETE
                    ON module_artifact_data_namespace_recovery_operations WHEN OLD.status='cutover'
                    BEGIN SELECT RAISE(ABORT, 'Terminal recovery receipt is immutable'); END".into(),
            ]
        }
        _ => {
            return Err(DbErr::Migration(
                "Unsupported recovery guard backend".into(),
            ));
        }
    };
    for statement in statements {
        manager
            .get_connection()
            .execute_unprepared(&statement)
            .await?;
    }
    Ok(())
}
