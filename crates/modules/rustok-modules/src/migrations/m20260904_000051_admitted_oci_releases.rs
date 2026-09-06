use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

/// Persists immutable admitted OCI releases, binding digest-pinned OCI identity,
/// verified descriptor metadata, streamed platform-CAS payload receipts, and
/// independently verified external prebuilt ingress evidence.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements: &[&str] = match manager.get_database_backend() {
            DbBackend::Postgres => &[
                "CREATE TABLE module_admitted_oci_releases (\
                    release_digest TEXT PRIMARY KEY CHECK (release_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('platform', 'tenant')),\
                    scope_tenant_key TEXT NOT NULL,\
                    registry TEXT NOT NULL,\
                    repository TEXT NOT NULL,\
                    slug TEXT NOT NULL,\
                    version TEXT NOT NULL,\
                    payload_digest TEXT NOT NULL CHECK (payload_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    payload_media_type TEXT NOT NULL,\
                    payload_size_bytes BIGINT NOT NULL CHECK (payload_size_bytes >= 0),\
                    descriptor_json TEXT NOT NULL,\
                    artifact_origin TEXT NOT NULL DEFAULT 'oci_admitted' CHECK (artifact_origin IN ('oci_admitted', 'external_prebuilt')),\
                    actor_id UUID NOT NULL,\
                    idempotency_key UUID NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512),\
                    correlation_id UUID NOT NULL,\
                    admitted_at TIMESTAMPTZ NOT NULL,\
                    UNIQUE (scope_kind, scope_tenant_key, actor_id, idempotency_key)\
                )",
                "CREATE INDEX idx_admitted_oci_releases_slug_version \
                 ON module_admitted_oci_releases (slug, version)",
                "CREATE INDEX idx_admitted_oci_releases_payload \
                 ON module_admitted_oci_releases (payload_digest)",
                "ALTER TABLE module_admitted_oci_releases ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_admitted_oci_releases_scope \
                 ON module_admitted_oci_releases \
                 USING (scope_kind = 'platform' OR scope_tenant_key = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (scope_kind = 'platform' OR scope_tenant_key = current_setting('rustok.tenant_id', true))",
                "CREATE TABLE module_external_prebuilt_ingress (\
                    release_digest TEXT PRIMARY KEY REFERENCES module_admitted_oci_releases(release_digest) ON DELETE RESTRICT,\
                    publisher_identity TEXT NOT NULL CHECK (length(trim(publisher_identity)) > 0),\
                    lineage_reference TEXT NOT NULL CHECK (length(trim(lineage_reference)) > 0),\
                    lineage_digest TEXT NOT NULL CHECK (lineage_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    signature_reference TEXT NOT NULL CHECK (length(trim(signature_reference)) > 0),\
                    signature_digest TEXT NOT NULL CHECK (signature_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    sbom_reference TEXT NOT NULL CHECK (length(trim(sbom_reference)) > 0),\
                    sbom_digest TEXT NOT NULL CHECK (sbom_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    provenance_reference TEXT NOT NULL CHECK (length(trim(provenance_reference)) > 0),\
                    provenance_digest TEXT NOT NULL CHECK (provenance_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    policy_revision BIGINT NOT NULL CHECK (policy_revision >= 0),\
                    license_policy_verified BOOLEAN NOT NULL CHECK (license_policy_verified = TRUE),\
                    vulnerability_policy_verified BOOLEAN NOT NULL CHECK (vulnerability_policy_verified = TRUE),\
                    abi_verified BOOLEAN NOT NULL CHECK (abi_verified = TRUE),\
                    capability_verified BOOLEAN NOT NULL CHECK (capability_verified = TRUE),\
                    native_promotion_denied BOOLEAN NOT NULL DEFAULT TRUE CHECK (native_promotion_denied = TRUE),\
                    ingress_at TIMESTAMPTZ NOT NULL\
                )",
                "ALTER TABLE module_external_prebuilt_ingress ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_external_prebuilt_ingress_scope \
                 ON module_external_prebuilt_ingress \
                 USING (\
                    EXISTS (\
                        SELECT 1 FROM module_admitted_oci_releases AS r \
                        WHERE r.release_digest = module_external_prebuilt_ingress.release_digest \
                          AND (r.scope_kind = 'platform' OR r.scope_tenant_key = current_setting('rustok.tenant_id', true))\
                    )\
                 ) \
                 WITH CHECK (\
                    EXISTS (\
                        SELECT 1 FROM module_admitted_oci_releases AS r \
                        WHERE r.release_digest = module_external_prebuilt_ingress.release_digest \
                          AND (r.scope_kind = 'platform' OR r.scope_tenant_key = current_setting('rustok.tenant_id', true))\
                    )\
                 )",
                "CREATE TABLE module_executor_readiness_receipts (\
                    id UUID PRIMARY KEY,\
                    operation_id UUID NOT NULL,\
                    installation_id UUID NOT NULL,\
                    release_digest TEXT NOT NULL CHECK (release_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    payload_digest TEXT NOT NULL CHECK (payload_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    runtime_fingerprint TEXT NOT NULL CHECK (runtime_fingerprint ~ '^sha256:[0-9a-f]{64}$'),\
                    pool_id TEXT NOT NULL CHECK (length(trim(pool_id)) > 0),\
                    pool_generation BIGINT NOT NULL CHECK (pool_generation > 0),\
                    placement TEXT NOT NULL CHECK (placement IN ('in_process', 'isolated_worker')),\
                    placement_policy_revision BIGINT NOT NULL CHECK (placement_policy_revision >= 0),\
                    capability_routes_verified BOOLEAN NOT NULL DEFAULT FALSE,\
                    smoke_passed BOOLEAN NOT NULL DEFAULT FALSE,\
                    evaluated_at TIMESTAMPTZ NOT NULL\
                )",
                "CREATE INDEX idx_executor_readiness_lookup \
                 ON module_executor_readiness_receipts (release_digest, runtime_fingerprint, pool_generation)",
                "CREATE INDEX idx_executor_readiness_operation \
                 ON module_executor_readiness_receipts (operation_id, installation_id)",
                "ALTER TABLE module_executor_readiness_receipts ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_executor_readiness_receipts_scope \
                 ON module_executor_readiness_receipts \
                 USING (\
                    EXISTS (\
                        SELECT 1 FROM module_admitted_oci_releases AS r \
                        WHERE r.release_digest = module_executor_readiness_receipts.release_digest \
                          AND (r.scope_kind = 'platform' OR r.scope_tenant_key = current_setting('rustok.tenant_id', true))\
                    )\
                 ) \
                 WITH CHECK (\
                    EXISTS (\
                        SELECT 1 FROM module_admitted_oci_releases AS r \
                        WHERE r.release_digest = module_executor_readiness_receipts.release_digest \
                          AND (r.scope_kind = 'platform' OR r.scope_tenant_key = current_setting('rustok.tenant_id', true))\
                    )\
                 )",
                "CREATE TABLE module_artifact_work_generations (\
                    installation_id UUID PRIMARY KEY REFERENCES module_artifact_installations(installation_id) ON DELETE RESTRICT,\
                    work_generation BIGINT NOT NULL CHECK (work_generation > 0),\
                    retired BOOLEAN NOT NULL DEFAULT FALSE,\
                    retired_at TIMESTAMPTZ NULL,\
                    updated_at TIMESTAMPTZ NOT NULL\
                )",
                "ALTER TABLE module_artifact_work_generations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_artifact_work_generations_scope \
                 ON module_artifact_work_generations \
                 USING (\
                    EXISTS (\
                        SELECT 1 FROM module_artifact_installations AS i \
                        WHERE i.installation_id = module_artifact_work_generations.installation_id \
                          AND (i.scope_kind = 'platform' OR i.tenant_id::text = current_setting('rustok.tenant_id', true))\
                    )\
                 ) \
                 WITH CHECK (\
                    EXISTS (\
                        SELECT 1 FROM module_artifact_installations AS i \
                        WHERE i.installation_id = module_artifact_work_generations.installation_id \
                          AND (i.scope_kind = 'platform' OR i.tenant_id::text = current_setting('rustok.tenant_id', true))\
                    )\
                 )",
                "CREATE TABLE module_production_operations (\
                    operation_id UUID PRIMARY KEY,\
                    installation_id UUID NOT NULL REFERENCES module_artifact_installations(installation_id) ON DELETE RESTRICT,\
                    action TEXT NOT NULL CHECK (action IN ('install', 'enable', 'update', 'disable', 'remove', 'uninstall', 'rollback', 'dynamic_artifact_data_purge', 'dynamic_artifact_settings_purge')),\
                    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('platform', 'tenant')),\
                    tenant_id UUID NULL,\
                    module_slug TEXT NOT NULL CHECK (length(trim(module_slug)) > 0),\
                    release_digest TEXT NOT NULL CHECK (release_digest ~ '^sha256:[0-9a-f]{64}$'),\
                    predecessor_installation_id UUID NULL,\
                    data_owner_id UUID NOT NULL,\
                    settings_instance_id UUID NOT NULL,\
                    work_generation BIGINT NOT NULL CHECK (work_generation > 0),\
                    status TEXT NOT NULL CHECK (status IN ('in_progress', 'converged', 'rolled_back', 'failed')),\
                    actor_id UUID NOT NULL,\
                    idempotency_key UUID NOT NULL UNIQUE,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512),\
                    correlation_id UUID NOT NULL,\
                    created_at TIMESTAMPTZ NOT NULL,\
                    updated_at TIMESTAMPTZ NOT NULL\
                )",
                "CREATE INDEX idx_production_operations_slug \
                 ON module_production_operations (module_slug, tenant_id, work_generation)",
                "ALTER TABLE module_production_operations ENABLE ROW LEVEL SECURITY",
                "CREATE POLICY module_production_operations_scope \
                 ON module_production_operations \
                 USING (scope_kind = 'platform' OR tenant_id::text = current_setting('rustok.tenant_id', true)) \
                 WITH CHECK (scope_kind = 'platform' OR tenant_id::text = current_setting('rustok.tenant_id', true))",
            ],
            DbBackend::Sqlite => &[
                "CREATE TABLE module_admitted_oci_releases (\
                    release_digest TEXT PRIMARY KEY NOT NULL CHECK (length(release_digest) = 71),\
                    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('platform', 'tenant')),\
                    scope_tenant_key TEXT NOT NULL,\
                    registry TEXT NOT NULL,\
                    repository TEXT NOT NULL,\
                    slug TEXT NOT NULL,\
                    version TEXT NOT NULL,\
                    payload_digest TEXT NOT NULL CHECK (length(payload_digest) = 71),\
                    payload_media_type TEXT NOT NULL,\
                    payload_size_bytes INTEGER NOT NULL CHECK (payload_size_bytes >= 0),\
                    descriptor_json TEXT NOT NULL,\
                    artifact_origin TEXT NOT NULL DEFAULT 'oci_admitted' CHECK (artifact_origin IN ('oci_admitted', 'external_prebuilt')),\
                    actor_id TEXT NOT NULL,\
                    idempotency_key TEXT NOT NULL,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512),\
                    correlation_id TEXT NOT NULL,\
                    admitted_at TEXT NOT NULL,\
                    UNIQUE (scope_kind, scope_tenant_key, actor_id, idempotency_key)\
                )",
                "CREATE INDEX idx_admitted_oci_releases_slug_version \
                 ON module_admitted_oci_releases (slug, version)",
                "CREATE INDEX idx_admitted_oci_releases_payload \
                 ON module_admitted_oci_releases (payload_digest)",
                "CREATE TABLE module_external_prebuilt_ingress (\
                    release_digest TEXT PRIMARY KEY NOT NULL REFERENCES module_admitted_oci_releases(release_digest) ON DELETE RESTRICT,\
                    publisher_identity TEXT NOT NULL CHECK (length(trim(publisher_identity)) > 0),\
                    lineage_reference TEXT NOT NULL CHECK (length(trim(lineage_reference)) > 0),\
                    lineage_digest TEXT NOT NULL CHECK (length(lineage_digest) = 71),\
                    signature_reference TEXT NOT NULL CHECK (length(trim(signature_reference)) > 0),\
                    signature_digest TEXT NOT NULL CHECK (length(signature_digest) = 71),\
                    sbom_reference TEXT NOT NULL CHECK (length(trim(sbom_reference)) > 0),\
                    sbom_digest TEXT NOT NULL CHECK (length(sbom_digest) = 71),\
                    provenance_reference TEXT NOT NULL CHECK (length(trim(provenance_reference)) > 0),\
                    provenance_digest TEXT NOT NULL CHECK (length(provenance_digest) = 71),\
                    policy_revision INTEGER NOT NULL CHECK (policy_revision >= 0),\
                    license_policy_verified INTEGER NOT NULL CHECK (license_policy_verified = 1),\
                    vulnerability_policy_verified INTEGER NOT NULL CHECK (vulnerability_policy_verified = 1),\
                    abi_verified INTEGER NOT NULL CHECK (abi_verified = 1),\
                    capability_verified INTEGER NOT NULL CHECK (capability_verified = 1),\
                    native_promotion_denied INTEGER NOT NULL DEFAULT 1 CHECK (native_promotion_denied = 1),\
                    ingress_at TEXT NOT NULL\
                )",
                "CREATE TABLE module_executor_readiness_receipts (\
                    id TEXT PRIMARY KEY NOT NULL,\
                    operation_id TEXT NOT NULL,\
                    installation_id TEXT NOT NULL,\
                    release_digest TEXT NOT NULL CHECK (length(release_digest) = 71),\
                    payload_digest TEXT NOT NULL CHECK (length(payload_digest) = 71),\
                    runtime_fingerprint TEXT NOT NULL CHECK (length(runtime_fingerprint) = 71),\
                    pool_id TEXT NOT NULL CHECK (length(trim(pool_id)) > 0),\
                    pool_generation INTEGER NOT NULL CHECK (pool_generation > 0),\
                    placement TEXT NOT NULL CHECK (placement IN ('in_process', 'isolated_worker')),\
                    placement_policy_revision INTEGER NOT NULL CHECK (placement_policy_revision >= 0),\
                    capability_routes_verified INTEGER NOT NULL DEFAULT 0,\
                    smoke_passed INTEGER NOT NULL DEFAULT 0,\
                    evaluated_at TEXT NOT NULL\
                )",
                "CREATE INDEX idx_executor_readiness_lookup \
                 ON module_executor_readiness_receipts (release_digest, runtime_fingerprint, pool_generation)",
                "CREATE INDEX idx_executor_readiness_operation \
                 ON module_executor_readiness_receipts (operation_id, installation_id)",
                "CREATE TABLE module_artifact_work_generations (\
                    installation_id TEXT PRIMARY KEY NOT NULL REFERENCES module_artifact_installations(installation_id) ON DELETE RESTRICT,\
                    work_generation INTEGER NOT NULL CHECK (work_generation > 0),\
                    retired INTEGER NOT NULL DEFAULT 0 CHECK (retired IN (0, 1)),\
                    retired_at TEXT NULL,\
                    updated_at TEXT NOT NULL\
                )",
                "CREATE TABLE module_production_operations (\
                    operation_id TEXT PRIMARY KEY NOT NULL,\
                    installation_id TEXT NOT NULL REFERENCES module_artifact_installations(installation_id) ON DELETE RESTRICT,\
                    action TEXT NOT NULL CHECK (action IN ('install', 'enable', 'update', 'disable', 'remove', 'uninstall', 'rollback', 'dynamic_artifact_data_purge', 'dynamic_artifact_settings_purge')),\
                    scope_kind TEXT NOT NULL CHECK (scope_kind IN ('platform', 'tenant')),\
                    tenant_id TEXT NULL,\
                    module_slug TEXT NOT NULL CHECK (length(trim(module_slug)) > 0),\
                    release_digest TEXT NOT NULL CHECK (length(release_digest) = 71),\
                    predecessor_installation_id TEXT NULL,\
                    data_owner_id TEXT NOT NULL,\
                    settings_instance_id TEXT NOT NULL,\
                    work_generation INTEGER NOT NULL CHECK (work_generation > 0),\
                    status TEXT NOT NULL CHECK (status IN ('in_progress', 'converged', 'rolled_back', 'failed')),\
                    actor_id TEXT NOT NULL,\
                    idempotency_key TEXT NOT NULL UNIQUE,\
                    trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512),\
                    correlation_id TEXT NOT NULL,\
                    created_at TEXT NOT NULL,\
                    updated_at TEXT NOT NULL\
                )",
                "CREATE INDEX idx_production_operations_slug \
                 ON module_production_operations (module_slug, tenant_id, work_generation)",
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

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let statements = &[
            "DROP TABLE IF EXISTS module_production_operations",
            "DROP TABLE IF EXISTS module_artifact_work_generations",
            "DROP TABLE IF EXISTS module_executor_readiness_receipts",
            "DROP TABLE IF EXISTS module_external_prebuilt_ingress",
            "DROP TABLE IF EXISTS module_admitted_oci_releases",
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

        Ok(())
    }
}
