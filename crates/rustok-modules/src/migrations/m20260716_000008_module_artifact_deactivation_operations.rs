use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Records durable scoped artifact activation and deactivation operations.
/// Both preserve immutable admission evidence and make their lifecycle result
/// replayable without selecting a mutable release again.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_artifact_deactivation_operations (\
                    operation_id UUID PRIMARY KEY,\
                    installation_id UUID NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    expected_revision BIGINT NOT NULL CHECK (expected_revision > 0),\
                    actor_id UUID NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0),\
                    correlation_id UUID NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    idempotency_key UUID NOT NULL UNIQUE,\
                    committed_at TIMESTAMPTZ NOT NULL\
                )",
                "CREATE INDEX module_artifact_deactivation_operations_installation_idx \
                 ON module_artifact_deactivation_operations (installation_id, committed_at DESC)",
                "ALTER TABLE module_artifact_deactivation_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_deactivation_operations_scope \
                 ON module_artifact_deactivation_operations USING (EXISTS (\
                    SELECT 1 FROM module_artifact_installations installation \
                    WHERE installation.installation_id = module_artifact_deactivation_operations.installation_id \
                    AND (installation.scope_kind = 'platform' OR installation.tenant_id::text = current_setting('rustok.tenant_id', true))\
                 ))",
                "CREATE TABLE module_artifact_activation_locks (\
                    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('platform', 'tenant')),\
                    scope_tenant_key TEXT NOT NULL,\
                    slug TEXT NOT NULL CHECK (length(trim(slug)) > 0),\
                    PRIMARY KEY (scope_kind, scope_tenant_key, slug)\
                )",
                "ALTER TABLE module_artifact_activation_locks ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_activation_locks_scope \
                 ON module_artifact_activation_locks USING (\
                    scope_kind = 'platform' OR scope_tenant_key = current_setting('rustok.tenant_id', true)\
                 ) WITH CHECK (\
                    scope_kind = 'platform' OR scope_tenant_key = current_setting('rustok.tenant_id', true)\
                 )",
                "CREATE TABLE module_artifact_activation_operations (\
                    operation_id UUID PRIMARY KEY,\
                    installation_id UUID NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    predecessor_installation_id UUID NULL REFERENCES module_artifact_installations(installation_id),\
                    expected_revision BIGINT NOT NULL CHECK (expected_revision > 0),\
                    installation_revision BIGINT NOT NULL CHECK (installation_revision > 0),\
                    predecessor_revision BIGINT NULL CHECK (predecessor_revision IS NULL OR predecessor_revision > 0),\
                    actor_id UUID NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0),\
                    correlation_id UUID NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    idempotency_key UUID NOT NULL UNIQUE,\
                    committed_at TIMESTAMPTZ NOT NULL,\
                    CHECK ((predecessor_installation_id IS NULL AND predecessor_revision IS NULL) \
                        OR (predecessor_installation_id IS NOT NULL AND predecessor_revision IS NOT NULL))\
                )",
                "CREATE INDEX module_artifact_activation_operations_installation_idx \
                 ON module_artifact_activation_operations (installation_id, committed_at DESC)",
                "ALTER TABLE module_artifact_activation_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_activation_operations_scope \
                 ON module_artifact_activation_operations USING (EXISTS (\
                    SELECT 1 FROM module_artifact_installations installation \
                    WHERE installation.installation_id = module_artifact_activation_operations.installation_id \
                    AND (installation.scope_kind = 'platform' OR installation.tenant_id::text = current_setting('rustok.tenant_id', true))\
                 ))",
                "CREATE TABLE module_artifact_settings_instances (\
                    tenant_id UUID NOT NULL,\
                    data_owner_id UUID NOT NULL,\
                    settings_instance_id UUID NOT NULL,\
                    schema_digest TEXT NOT NULL CHECK (schema_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    settings JSONB NOT NULL,\
                    revision BIGINT NOT NULL CHECK (revision > 0),\
                    created_at TIMESTAMPTZ NOT NULL,\
                    updated_at TIMESTAMPTZ NOT NULL,\
                    PRIMARY KEY (tenant_id, data_owner_id, settings_instance_id)\
                )",
                "ALTER TABLE module_artifact_settings_instances ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_settings_instances_scope \
                 ON module_artifact_settings_instances \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_settings_recovery_points (\
                    recovery_point_id UUID PRIMARY KEY,\
                    tenant_id UUID NOT NULL,\
                    installation_id UUID NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    data_owner_id UUID NOT NULL,\
                    settings_instance_id UUID NOT NULL,\
                    settings_revision BIGINT NOT NULL CHECK (settings_revision > 0),\
                    schema_digest TEXT NOT NULL CHECK (schema_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    descriptor_digest TEXT NOT NULL CHECK (descriptor_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    value_digest TEXT NOT NULL CHECK (value_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    key_version TEXT NOT NULL CHECK (length(trim(key_version)) > 0 AND length(key_version) <= 256),\
                    ciphertext BYTEA NULL CHECK (ciphertext IS NULL OR (octet_length(ciphertext) > 0 AND octet_length(ciphertext) <= 131072)),\
                    retention_revision BIGINT NOT NULL CHECK (retention_revision > 0),\
                    policy_snapshot_id TEXT NOT NULL CHECK (length(trim(policy_snapshot_id)) > 0 AND length(policy_snapshot_id) <= 128),\
                    secret_handle_digest TEXT NOT NULL CHECK (secret_handle_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    retain_until TIMESTAMPTZ NOT NULL,\
                    legal_hold BOOLEAN NOT NULL,\
                    audit_hold BOOLEAN NOT NULL,\
                    incident_hold BOOLEAN NOT NULL,\
                    state TEXT NOT NULL CHECK (state IN ('ready', 'collecting', 'collected')),\
                    restored_at TIMESTAMPTZ NULL,\
                    restored_installation_id UUID NULL REFERENCES module_artifact_installations(installation_id),\
                    restored_settings_instance_id UUID NULL,\
                    collected_at TIMESTAMPTZ NULL,\
                    created_at TIMESTAMPTZ NOT NULL,\
                    CHECK ((restored_at IS NULL AND restored_settings_instance_id IS NULL) \
                        OR (restored_at IS NOT NULL AND restored_settings_instance_id IS NOT NULL)),\
                    CHECK ((state IN ('ready', 'collecting') AND ciphertext IS NOT NULL AND collected_at IS NULL) \
                        OR (state = 'collected' AND ciphertext IS NULL AND collected_at IS NOT NULL))\
                )",
                "CREATE UNIQUE INDEX module_artifact_settings_recovery_points_tenant_recovery_idx \
                 ON module_artifact_settings_recovery_points (tenant_id, recovery_point_id)",
                "CREATE INDEX module_artifact_settings_recovery_points_scope_idx \
                 ON module_artifact_settings_recovery_points (tenant_id, installation_id, created_at DESC)",
                "ALTER TABLE module_artifact_settings_recovery_points ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_settings_recovery_points_scope \
                 ON module_artifact_settings_recovery_points \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_settings_recovery_operations (\
                    operation_id UUID PRIMARY KEY,\
                    tenant_id UUID NOT NULL,\
                    installation_id UUID NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    expected_installation_revision BIGINT NOT NULL CHECK (expected_installation_revision > 0),\
                    expected_settings_revision BIGINT NOT NULL CHECK (expected_settings_revision > 0),\
                    recovery_point_id UUID NOT NULL UNIQUE,\
                    actor_id UUID NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0 AND length(trace_id) <= 512),\
                    correlation_id UUID NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0 AND length(reason) <= 2000),\
                    idempotency_key UUID NOT NULL,\
                    committed_at TIMESTAMPTZ NOT NULL,\
                    UNIQUE (tenant_id, idempotency_key),\
                    UNIQUE (tenant_id, operation_id),\
                    FOREIGN KEY (tenant_id, recovery_point_id) REFERENCES module_artifact_settings_recovery_points(tenant_id, recovery_point_id)\
                )",
                "ALTER TABLE module_artifact_settings_recovery_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_settings_recovery_operations_scope \
                 ON module_artifact_settings_recovery_operations \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_settings_purge_operations (\
                    operation_id UUID PRIMARY KEY,\
                    tenant_id UUID NOT NULL,\
                    installation_id UUID NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    recovery_point_id UUID NOT NULL UNIQUE,\
                    expected_installation_revision BIGINT NOT NULL CHECK (expected_installation_revision > 0),\
                    expected_settings_revision BIGINT NOT NULL CHECK (expected_settings_revision > 0),\
                    tombstone_revision BIGINT NOT NULL CHECK (tombstone_revision > 0),\
                    actor_id UUID NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0 AND length(trace_id) <= 512),\
                    correlation_id UUID NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0 AND length(reason) <= 2000),\
                    idempotency_key UUID NOT NULL,\
                    committed_at TIMESTAMPTZ NOT NULL,\
                    UNIQUE (tenant_id, idempotency_key),\
                    UNIQUE (tenant_id, operation_id),\
                    FOREIGN KEY (tenant_id, recovery_point_id) REFERENCES module_artifact_settings_recovery_points(tenant_id, recovery_point_id)\
                )",
                "ALTER TABLE module_artifact_settings_purge_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_settings_purge_operations_scope \
                 ON module_artifact_settings_purge_operations \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_settings_tombstones (\
                    tenant_id UUID NOT NULL,\
                    data_owner_id UUID NOT NULL,\
                    settings_instance_id UUID NOT NULL,\
                    tombstone_revision BIGINT NOT NULL CHECK (tombstone_revision > 0),\
                    recovery_point_id UUID NOT NULL UNIQUE,\
                    purge_operation_id UUID NOT NULL UNIQUE,\
                    purged_at TIMESTAMPTZ NOT NULL,\
                    PRIMARY KEY (tenant_id, data_owner_id, settings_instance_id),\
                    FOREIGN KEY (tenant_id, recovery_point_id) REFERENCES module_artifact_settings_recovery_points(tenant_id, recovery_point_id),\
                    FOREIGN KEY (tenant_id, purge_operation_id) REFERENCES module_artifact_settings_purge_operations(tenant_id, operation_id)\
                )",
                "ALTER TABLE module_artifact_settings_tombstones ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_settings_tombstones_scope \
                 ON module_artifact_settings_tombstones \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_settings_restore_operations (\
                    operation_id UUID PRIMARY KEY,\
                    tenant_id UUID NOT NULL,\
                    recovery_point_id UUID NOT NULL UNIQUE,\
                    target_installation_id UUID NULL REFERENCES module_artifact_installations(installation_id),\
                    expected_target_installation_revision BIGINT NULL CHECK (expected_target_installation_revision IS NULL OR expected_target_installation_revision > 0),\
                    settings_instance_id UUID NOT NULL,\
                    actor_id UUID NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0 AND length(trace_id) <= 512),\
                    correlation_id UUID NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0 AND length(reason) <= 2000),\
                    idempotency_key UUID NOT NULL,\
                    committed_at TIMESTAMPTZ NOT NULL,\
                    CHECK ((target_installation_id IS NULL) = (expected_target_installation_revision IS NULL)),\
                    UNIQUE (tenant_id, idempotency_key),\
                    UNIQUE (tenant_id, operation_id),\
                    FOREIGN KEY (tenant_id, recovery_point_id) REFERENCES module_artifact_settings_recovery_points(tenant_id, recovery_point_id)\
                )",
                "ALTER TABLE module_artifact_settings_restore_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_settings_restore_operations_scope \
                 ON module_artifact_settings_restore_operations \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_settings_recovery_retention_operations (\
                    operation_id UUID PRIMARY KEY,\
                    tenant_id UUID NOT NULL,\
                    recovery_point_id UUID NOT NULL,\
                    idempotency_key UUID NOT NULL,\
                    request_digest TEXT NOT NULL CHECK (request_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    expected_retention_revision BIGINT NOT NULL CHECK (expected_retention_revision > 0),\
                    retention_revision BIGINT NOT NULL CHECK (retention_revision > 0),\
                    retain_until TIMESTAMPTZ NOT NULL,\
                    legal_hold BOOLEAN NOT NULL,\
                    audit_hold BOOLEAN NOT NULL,\
                    incident_hold BOOLEAN NOT NULL,\
                    policy_snapshot_id TEXT NOT NULL CHECK (length(trim(policy_snapshot_id)) > 0 AND length(policy_snapshot_id) <= 128),\
                    actor_id UUID NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0 AND length(trace_id) <= 512),\
                    correlation_id UUID NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0 AND length(reason) <= 2000),\
                    committed_at TIMESTAMPTZ NOT NULL,\
                    UNIQUE (tenant_id, idempotency_key),\
                    FOREIGN KEY (tenant_id, recovery_point_id) REFERENCES module_artifact_settings_recovery_points(tenant_id, recovery_point_id)\
                )",
                "ALTER TABLE module_artifact_settings_recovery_retention_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_settings_recovery_retention_operations_scope \
                 ON module_artifact_settings_recovery_retention_operations \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_settings_recovery_rewrap_operations (\
                    operation_id UUID PRIMARY KEY,\
                    tenant_id UUID NOT NULL,\
                    recovery_point_id UUID NOT NULL,\
                    idempotency_key UUID NOT NULL,\
                    previous_key_version TEXT NOT NULL CHECK (length(trim(previous_key_version)) > 0 AND length(previous_key_version) <= 256),\
                    key_version TEXT NOT NULL CHECK (length(trim(key_version)) > 0 AND length(key_version) <= 256),\
                    actor_id UUID NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0 AND length(trace_id) <= 512),\
                    correlation_id UUID NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0 AND length(reason) <= 2000),\
                    committed_at TIMESTAMPTZ NOT NULL,\
                    UNIQUE (tenant_id, idempotency_key),\
                    FOREIGN KEY (tenant_id, recovery_point_id) REFERENCES module_artifact_settings_recovery_points(tenant_id, recovery_point_id)\
                )",
                "ALTER TABLE module_artifact_settings_recovery_rewrap_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_settings_recovery_rewrap_operations_scope \
                 ON module_artifact_settings_recovery_rewrap_operations \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_settings_recovery_collections (\
                    collection_id UUID PRIMARY KEY,\
                    tenant_id UUID NOT NULL,\
                    recovery_point_id UUID NOT NULL UNIQUE,\
                    policy_snapshot_id TEXT NOT NULL CHECK (length(trim(policy_snapshot_id)) > 0 AND length(policy_snapshot_id) <= 128),\
                    actor_id UUID NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0 AND length(trace_id) <= 512),\
                    correlation_id UUID NOT NULL,\
                    idempotency_key UUID NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0 AND length(reason) <= 2000),\
                    collecting_at TIMESTAMPTZ NOT NULL,\
                    completed_at TIMESTAMPTZ NULL,\
                    FOREIGN KEY (tenant_id, recovery_point_id) REFERENCES module_artifact_settings_recovery_points(tenant_id, recovery_point_id)\
                )",
                "ALTER TABLE module_artifact_settings_recovery_collections ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_settings_recovery_collections_scope \
                 ON module_artifact_settings_recovery_collections \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_artifact_settings_recovery_bind_operations (\
                    operation_id UUID PRIMARY KEY,\
                    tenant_id UUID NOT NULL,\
                    recovery_point_id UUID NOT NULL UNIQUE,\
                    target_installation_id UUID NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    expected_target_installation_revision BIGINT NOT NULL CHECK (expected_target_installation_revision > 0),\
                    settings_instance_id UUID NOT NULL,\
                    actor_id UUID NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0 AND length(trace_id) <= 512),\
                    correlation_id UUID NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0 AND length(reason) <= 2000),\
                    idempotency_key UUID NOT NULL,\
                    committed_at TIMESTAMPTZ NOT NULL,\
                    UNIQUE (tenant_id, idempotency_key),\
                    FOREIGN KEY (tenant_id, recovery_point_id) REFERENCES module_artifact_settings_recovery_points(tenant_id, recovery_point_id)\
                )",
                "ALTER TABLE module_artifact_settings_recovery_bind_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_settings_recovery_bind_operations_scope \
                 ON module_artifact_settings_recovery_bind_operations \
                 USING (tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_artifact_deactivation_operations (\
                    operation_id TEXT PRIMARY KEY NOT NULL,\
                    installation_id TEXT NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    expected_revision INTEGER NOT NULL CHECK (expected_revision > 0),\
                    actor_id TEXT NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0),\
                    correlation_id TEXT NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    idempotency_key TEXT NOT NULL UNIQUE,\
                    committed_at TEXT NOT NULL\
                )",
                "CREATE INDEX module_artifact_deactivation_operations_installation_idx \
                 ON module_artifact_deactivation_operations (installation_id, committed_at DESC)",
                "CREATE TABLE module_artifact_activation_locks (\
                    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('platform', 'tenant')),\
                    scope_tenant_key TEXT NOT NULL,\
                    slug TEXT NOT NULL CHECK (length(trim(slug)) > 0),\
                    PRIMARY KEY (scope_kind, scope_tenant_key, slug)\
                )",
                "CREATE TABLE module_artifact_activation_operations (\
                    operation_id TEXT PRIMARY KEY NOT NULL,\
                    installation_id TEXT NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    predecessor_installation_id TEXT NULL REFERENCES module_artifact_installations(installation_id),\
                    expected_revision INTEGER NOT NULL CHECK (expected_revision > 0),\
                    installation_revision INTEGER NOT NULL CHECK (installation_revision > 0),\
                    predecessor_revision INTEGER NULL CHECK (predecessor_revision IS NULL OR predecessor_revision > 0),\
                    actor_id TEXT NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0),\
                    correlation_id TEXT NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0),\
                    idempotency_key TEXT NOT NULL UNIQUE,\
                    committed_at TEXT NOT NULL,\
                    CHECK ((predecessor_installation_id IS NULL AND predecessor_revision IS NULL) \
                        OR (predecessor_installation_id IS NOT NULL AND predecessor_revision IS NOT NULL))\
                )",
                "CREATE INDEX module_artifact_activation_operations_installation_idx \
                 ON module_artifact_activation_operations (installation_id, committed_at DESC)",
                "CREATE TABLE module_artifact_settings_instances (\
                    tenant_id TEXT NOT NULL,\
                    data_owner_id TEXT NOT NULL,\
                    settings_instance_id TEXT NOT NULL,\
                    schema_digest TEXT NOT NULL CHECK (length(schema_digest) = 71 AND substr(schema_digest, 1, 7) = 'sha256:' AND substr(schema_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    settings JSON NOT NULL,\
                    revision INTEGER NOT NULL CHECK (revision > 0),\
                    created_at TEXT NOT NULL,\
                    updated_at TEXT NOT NULL,\
                    PRIMARY KEY (tenant_id, data_owner_id, settings_instance_id)\
                )",
                "CREATE TABLE module_artifact_settings_recovery_points (\
                    recovery_point_id TEXT PRIMARY KEY NOT NULL,\
                    tenant_id TEXT NOT NULL,\
                    installation_id TEXT NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    data_owner_id TEXT NOT NULL,\
                    settings_instance_id TEXT NOT NULL,\
                    settings_revision INTEGER NOT NULL CHECK (settings_revision > 0),\
                    schema_digest TEXT NOT NULL CHECK (length(schema_digest) = 71 AND substr(schema_digest, 1, 7) = 'sha256:' AND substr(schema_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    descriptor_digest TEXT NOT NULL CHECK (length(descriptor_digest) = 71 AND substr(descriptor_digest, 1, 7) = 'sha256:' AND substr(descriptor_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    value_digest TEXT NOT NULL CHECK (length(value_digest) = 71 AND substr(value_digest, 1, 7) = 'sha256:' AND substr(value_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    key_version TEXT NOT NULL CHECK (length(trim(key_version)) > 0 AND length(key_version) <= 256),\
                    ciphertext BLOB NULL CHECK (ciphertext IS NULL OR (length(ciphertext) > 0 AND length(ciphertext) <= 131072)),\
                    retention_revision INTEGER NOT NULL CHECK (retention_revision > 0),\
                    policy_snapshot_id TEXT NOT NULL CHECK (length(trim(policy_snapshot_id)) > 0 AND length(policy_snapshot_id) <= 128),\
                    secret_handle_digest TEXT NOT NULL CHECK (length(secret_handle_digest) = 71 AND substr(secret_handle_digest, 1, 7) = 'sha256:' AND substr(secret_handle_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    retain_until TEXT NOT NULL,\
                    legal_hold INTEGER NOT NULL CHECK (legal_hold IN (0, 1)),\
                    audit_hold INTEGER NOT NULL CHECK (audit_hold IN (0, 1)),\
                    incident_hold INTEGER NOT NULL CHECK (incident_hold IN (0, 1)),\
                    state TEXT NOT NULL CHECK (state IN ('ready', 'collecting', 'collected')),\
                    restored_at TEXT NULL,\
                    restored_installation_id TEXT NULL REFERENCES module_artifact_installations(installation_id),\
                    restored_settings_instance_id TEXT NULL,\
                    collected_at TEXT NULL,\
                    created_at TEXT NOT NULL,\
                    CHECK ((restored_at IS NULL AND restored_settings_instance_id IS NULL) \
                        OR (restored_at IS NOT NULL AND restored_settings_instance_id IS NOT NULL)),\
                    CHECK ((state IN ('ready', 'collecting') AND ciphertext IS NOT NULL AND collected_at IS NULL) \
                        OR (state = 'collected' AND ciphertext IS NULL AND collected_at IS NOT NULL))\
                )",
                "CREATE UNIQUE INDEX module_artifact_settings_recovery_points_tenant_recovery_idx \
                 ON module_artifact_settings_recovery_points (tenant_id, recovery_point_id)",
                "CREATE INDEX module_artifact_settings_recovery_points_scope_idx \
                 ON module_artifact_settings_recovery_points (tenant_id, installation_id, created_at DESC)",
                "CREATE TABLE module_artifact_settings_recovery_operations (\
                    operation_id TEXT PRIMARY KEY NOT NULL,\
                    tenant_id TEXT NOT NULL,\
                    installation_id TEXT NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    expected_installation_revision INTEGER NOT NULL CHECK (expected_installation_revision > 0),\
                    expected_settings_revision INTEGER NOT NULL CHECK (expected_settings_revision > 0),\
                    recovery_point_id TEXT NOT NULL UNIQUE,\
                    actor_id TEXT NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0 AND length(trace_id) <= 512),\
                    correlation_id TEXT NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0 AND length(reason) <= 2000),\
                    idempotency_key TEXT NOT NULL,\
                    committed_at TEXT NOT NULL,\
                    UNIQUE (tenant_id, idempotency_key),\
                    FOREIGN KEY (tenant_id, recovery_point_id) REFERENCES module_artifact_settings_recovery_points(tenant_id, recovery_point_id)\
                )",
                "CREATE TABLE module_artifact_settings_purge_operations (\
                    operation_id TEXT PRIMARY KEY NOT NULL,\
                    tenant_id TEXT NOT NULL,\
                    installation_id TEXT NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    recovery_point_id TEXT NOT NULL UNIQUE,\
                    expected_installation_revision INTEGER NOT NULL CHECK (expected_installation_revision > 0),\
                    expected_settings_revision INTEGER NOT NULL CHECK (expected_settings_revision > 0),\
                    tombstone_revision INTEGER NOT NULL CHECK (tombstone_revision > 0),\
                    actor_id TEXT NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0 AND length(trace_id) <= 512),\
                    correlation_id TEXT NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0 AND length(reason) <= 2000),\
                    idempotency_key TEXT NOT NULL,\
                    committed_at TEXT NOT NULL,\
                    UNIQUE (tenant_id, idempotency_key),\
                    UNIQUE (tenant_id, operation_id),\
                    FOREIGN KEY (tenant_id, recovery_point_id) REFERENCES module_artifact_settings_recovery_points(tenant_id, recovery_point_id)\
                )",
                "CREATE TABLE module_artifact_settings_tombstones (\
                    tenant_id TEXT NOT NULL,\
                    data_owner_id TEXT NOT NULL,\
                    settings_instance_id TEXT NOT NULL,\
                    tombstone_revision INTEGER NOT NULL CHECK (tombstone_revision > 0),\
                    recovery_point_id TEXT NOT NULL UNIQUE,\
                    purge_operation_id TEXT NOT NULL UNIQUE,\
                    purged_at TEXT NOT NULL,\
                    PRIMARY KEY (tenant_id, data_owner_id, settings_instance_id),\
                    FOREIGN KEY (tenant_id, recovery_point_id) REFERENCES module_artifact_settings_recovery_points(tenant_id, recovery_point_id),\
                    FOREIGN KEY (tenant_id, purge_operation_id) REFERENCES module_artifact_settings_purge_operations(tenant_id, operation_id)\
                )",
                "CREATE TABLE module_artifact_settings_restore_operations (\
                    operation_id TEXT PRIMARY KEY NOT NULL,\
                    tenant_id TEXT NOT NULL,\
                    recovery_point_id TEXT NOT NULL UNIQUE,\
                    target_installation_id TEXT NULL REFERENCES module_artifact_installations(installation_id),\
                    expected_target_installation_revision INTEGER NULL CHECK (expected_target_installation_revision IS NULL OR expected_target_installation_revision > 0),\
                    settings_instance_id TEXT NOT NULL,\
                    actor_id TEXT NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0 AND length(trace_id) <= 512),\
                    correlation_id TEXT NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0 AND length(reason) <= 2000),\
                    idempotency_key TEXT NOT NULL,\
                    committed_at TEXT NOT NULL,\
                    CHECK ((target_installation_id IS NULL) = (expected_target_installation_revision IS NULL)),\
                    UNIQUE (tenant_id, idempotency_key),\
                    UNIQUE (tenant_id, operation_id),\
                    FOREIGN KEY (tenant_id, recovery_point_id) REFERENCES module_artifact_settings_recovery_points(tenant_id, recovery_point_id)\
                )",
                "CREATE TABLE module_artifact_settings_recovery_retention_operations (\
                    operation_id TEXT PRIMARY KEY NOT NULL,\
                    tenant_id TEXT NOT NULL,\
                    recovery_point_id TEXT NOT NULL,\
                    idempotency_key TEXT NOT NULL,\
                    request_digest TEXT NOT NULL CHECK (length(request_digest) = 71 AND substr(request_digest, 1, 7) = 'sha256:' AND substr(request_digest, 8) NOT GLOB '*[^0-9a-f]*'),\
                    expected_retention_revision INTEGER NOT NULL CHECK (expected_retention_revision > 0),\
                    retention_revision INTEGER NOT NULL CHECK (retention_revision > 0),\
                    retain_until TEXT NOT NULL,\
                    legal_hold INTEGER NOT NULL CHECK (legal_hold IN (0, 1)),\
                    audit_hold INTEGER NOT NULL CHECK (audit_hold IN (0, 1)),\
                    incident_hold INTEGER NOT NULL CHECK (incident_hold IN (0, 1)),\
                    policy_snapshot_id TEXT NOT NULL CHECK (length(trim(policy_snapshot_id)) > 0 AND length(policy_snapshot_id) <= 128),\
                    actor_id TEXT NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0 AND length(trace_id) <= 512),\
                    correlation_id TEXT NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0 AND length(reason) <= 2000),\
                    committed_at TEXT NOT NULL,\
                    UNIQUE (tenant_id, idempotency_key),\
                    FOREIGN KEY (tenant_id, recovery_point_id) REFERENCES module_artifact_settings_recovery_points(tenant_id, recovery_point_id)\
                )",
                "CREATE TABLE module_artifact_settings_recovery_rewrap_operations (\
                    operation_id TEXT PRIMARY KEY NOT NULL,\
                    tenant_id TEXT NOT NULL,\
                    recovery_point_id TEXT NOT NULL,\
                    idempotency_key TEXT NOT NULL,\
                    previous_key_version TEXT NOT NULL CHECK (length(trim(previous_key_version)) > 0 AND length(previous_key_version) <= 256),\
                    key_version TEXT NOT NULL CHECK (length(trim(key_version)) > 0 AND length(key_version) <= 256),\
                    actor_id TEXT NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0 AND length(trace_id) <= 512),\
                    correlation_id TEXT NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0 AND length(reason) <= 2000),\
                    committed_at TEXT NOT NULL,\
                    UNIQUE (tenant_id, idempotency_key),\
                    FOREIGN KEY (tenant_id, recovery_point_id) REFERENCES module_artifact_settings_recovery_points(tenant_id, recovery_point_id)\
                )",
                "CREATE TABLE module_artifact_settings_recovery_collections (\
                    collection_id TEXT PRIMARY KEY NOT NULL,\
                    tenant_id TEXT NOT NULL,\
                    recovery_point_id TEXT NOT NULL UNIQUE,\
                    policy_snapshot_id TEXT NOT NULL CHECK (length(trim(policy_snapshot_id)) > 0 AND length(policy_snapshot_id) <= 128),\
                    actor_id TEXT NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0 AND length(trace_id) <= 512),\
                    correlation_id TEXT NOT NULL,\
                    idempotency_key TEXT NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0 AND length(reason) <= 2000),\
                    collecting_at TEXT NOT NULL,\
                    completed_at TEXT NULL,\
                    FOREIGN KEY (tenant_id, recovery_point_id) REFERENCES module_artifact_settings_recovery_points(tenant_id, recovery_point_id)\
                )",
                "CREATE TABLE module_artifact_settings_recovery_bind_operations (\
                    operation_id TEXT PRIMARY KEY NOT NULL,\
                    tenant_id TEXT NOT NULL,\
                    recovery_point_id TEXT NOT NULL UNIQUE,\
                    target_installation_id TEXT NOT NULL REFERENCES module_artifact_installations(installation_id),\
                    expected_target_installation_revision INTEGER NOT NULL CHECK (expected_target_installation_revision > 0),\
                    settings_instance_id TEXT NOT NULL,\
                    actor_id TEXT NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) > 0 AND length(trace_id) <= 512),\
                    correlation_id TEXT NOT NULL,\
                    reason TEXT NOT NULL CHECK (length(trim(reason)) > 0 AND length(reason) <= 2000),\
                    idempotency_key TEXT NOT NULL,\
                    committed_at TEXT NOT NULL,\
                    UNIQUE (tenant_id, idempotency_key),\
                    FOREIGN KEY (tenant_id, recovery_point_id) REFERENCES module_artifact_settings_recovery_points(tenant_id, recovery_point_id)\
                )",
            ],
            backend => {
                return Err(DbErr::Migration(format!(
                    "module artifact deactivation-operation migration does not support database backend {backend:?}"
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
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for statement in [
            "DROP TABLE module_artifact_settings_restore_operations",
            "DROP TABLE module_artifact_settings_recovery_bind_operations",
            "DROP TABLE module_artifact_settings_recovery_collections",
            "DROP TABLE module_artifact_settings_recovery_rewrap_operations",
            "DROP TABLE module_artifact_settings_recovery_retention_operations",
            "DROP TABLE module_artifact_settings_tombstones",
            "DROP TABLE module_artifact_settings_purge_operations",
            "DROP TABLE module_artifact_settings_recovery_operations",
            "DROP TABLE module_artifact_settings_recovery_points",
            "DROP TABLE module_artifact_settings_instances",
            "DROP TABLE module_artifact_activation_operations",
            "DROP TABLE module_artifact_activation_locks",
            "DROP TABLE module_artifact_deactivation_operations",
        ] {
            manager
                .get_connection()
                .execute_unprepared(statement)
                .await?;
        }
        Ok(())
    }
}
