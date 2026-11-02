use crate::backend::SupportedBackend;
use sea_orm::{ConnectionTrait, Statement};
use sea_orm_migration::prelude::*;

/// Preserve each scope's immutable admission independently, including ingress evidence.
#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let backend = SupportedBackend::new(manager.get_database_backend())?;
        let statements: &[&str] = match backend {
            SupportedBackend::Postgres => &[
                "ALTER TABLE module_external_prebuilt_ingress ADD COLUMN scope_kind TEXT, ADD COLUMN scope_tenant_key TEXT",
                "UPDATE module_external_prebuilt_ingress e SET scope_kind = r.scope_kind, scope_tenant_key = r.scope_tenant_key FROM module_admitted_oci_releases r WHERE r.release_digest = e.release_digest",
                "ALTER TABLE module_external_prebuilt_ingress ALTER COLUMN scope_kind SET NOT NULL, ALTER COLUMN scope_tenant_key SET NOT NULL",
                "ALTER TABLE module_external_prebuilt_ingress DROP CONSTRAINT module_external_prebuilt_ingress_release_digest_fkey",
                "ALTER TABLE module_external_prebuilt_ingress DROP CONSTRAINT module_external_prebuilt_ingress_pkey",
                "ALTER TABLE module_admitted_oci_releases DROP CONSTRAINT module_admitted_oci_releases_pkey",
                "ALTER TABLE module_admitted_oci_releases ADD PRIMARY KEY (scope_kind, scope_tenant_key, release_digest)",
                "ALTER TABLE module_external_prebuilt_ingress ADD PRIMARY KEY (scope_kind, scope_tenant_key, release_digest), ADD FOREIGN KEY (scope_kind, scope_tenant_key, release_digest) REFERENCES module_admitted_oci_releases (scope_kind, scope_tenant_key, release_digest) ON DELETE RESTRICT",
                "DROP POLICY module_external_prebuilt_ingress_scope ON module_external_prebuilt_ingress",
                "CREATE POLICY module_external_prebuilt_ingress_scope ON module_external_prebuilt_ingress USING (scope_kind = 'platform' OR scope_tenant_key = current_setting('rustok.tenant_id', true)) WITH CHECK (scope_kind = 'platform' OR scope_tenant_key = current_setting('rustok.tenant_id', true))",
                "ALTER TABLE module_executor_readiness_receipts ADD COLUMN scope_kind TEXT, ADD COLUMN scope_tenant_key TEXT",
                "UPDATE module_executor_readiness_receipts receipt SET scope_kind = i.scope_kind, scope_tenant_key = CASE WHEN i.scope_kind = 'platform' THEN 'platform' ELSE i.tenant_id::text END FROM module_artifact_installations i WHERE i.installation_id = receipt.installation_id",
                "DROP POLICY module_executor_readiness_receipts_scope ON module_executor_readiness_receipts",
                "CREATE POLICY module_executor_readiness_receipts_scope ON module_executor_readiness_receipts USING (EXISTS (SELECT 1 FROM module_artifact_installations i JOIN module_admitted_oci_releases r ON r.scope_kind = i.scope_kind AND r.scope_tenant_key = CASE WHEN i.scope_kind = 'platform' THEN 'platform' ELSE i.tenant_id::text END AND r.release_digest = module_executor_readiness_receipts.release_digest WHERE i.installation_id = module_executor_readiness_receipts.installation_id AND module_executor_readiness_receipts.scope_kind = i.scope_kind AND module_executor_readiness_receipts.scope_tenant_key = r.scope_tenant_key AND i.manifest_digest = r.release_digest AND i.payload_digest = module_executor_readiness_receipts.payload_digest AND (i.scope_kind = 'platform' OR i.tenant_id::text = current_setting('rustok.tenant_id', true)))) WITH CHECK (EXISTS (SELECT 1 FROM module_artifact_installations i JOIN module_admitted_oci_releases r ON r.scope_kind = i.scope_kind AND r.scope_tenant_key = CASE WHEN i.scope_kind = 'platform' THEN 'platform' ELSE i.tenant_id::text END AND r.release_digest = module_executor_readiness_receipts.release_digest WHERE i.installation_id = module_executor_readiness_receipts.installation_id AND module_executor_readiness_receipts.scope_kind = i.scope_kind AND module_executor_readiness_receipts.scope_tenant_key = r.scope_tenant_key AND i.manifest_digest = r.release_digest AND i.payload_digest = module_executor_readiness_receipts.payload_digest AND (i.scope_kind = 'platform' OR i.tenant_id::text = current_setting('rustok.tenant_id', true))))",
            ],
            SupportedBackend::Sqlite => &[
                "CREATE TABLE module_admitted_oci_releases_scoped (release_digest TEXT NOT NULL CHECK (length(release_digest) = 71), scope_kind TEXT NOT NULL CHECK (scope_kind IN ('platform', 'tenant')), scope_tenant_key TEXT NOT NULL, registry TEXT NOT NULL, repository TEXT NOT NULL, slug TEXT NOT NULL, version TEXT NOT NULL, payload_digest TEXT NOT NULL CHECK (length(payload_digest) = 71), payload_media_type TEXT NOT NULL, payload_size_bytes INTEGER NOT NULL CHECK (payload_size_bytes >= 0), descriptor_json TEXT NOT NULL, artifact_origin TEXT NOT NULL DEFAULT 'oci_admitted' CHECK (artifact_origin IN ('oci_admitted', 'external_prebuilt')), actor_id TEXT NOT NULL, idempotency_key TEXT NOT NULL, trace_id TEXT NOT NULL CHECK (length(trim(trace_id)) BETWEEN 1 AND 512), correlation_id TEXT NOT NULL, request_digest TEXT NOT NULL CHECK (length(request_digest) = 71 AND substr(request_digest, 1, 7) = 'sha256:' AND substr(request_digest, 8) NOT GLOB '*[^0-9a-f]*'), admitted_at TEXT NOT NULL, PRIMARY KEY (scope_kind, scope_tenant_key, release_digest), UNIQUE (scope_kind, scope_tenant_key, actor_id, idempotency_key))",
                "INSERT INTO module_admitted_oci_releases_scoped SELECT * FROM module_admitted_oci_releases",
                "CREATE TABLE module_external_prebuilt_ingress_scoped (scope_kind TEXT NOT NULL, scope_tenant_key TEXT NOT NULL, release_digest TEXT NOT NULL, publisher_identity TEXT NOT NULL CHECK (length(trim(publisher_identity)) > 0), lineage_reference TEXT NOT NULL CHECK (length(trim(lineage_reference)) > 0), lineage_digest TEXT NOT NULL CHECK (length(lineage_digest) = 71), signature_reference TEXT NOT NULL CHECK (length(trim(signature_reference)) > 0), signature_digest TEXT NOT NULL CHECK (length(signature_digest) = 71), sbom_reference TEXT NOT NULL CHECK (length(trim(sbom_reference)) > 0), sbom_digest TEXT NOT NULL CHECK (length(sbom_digest) = 71), provenance_reference TEXT NOT NULL CHECK (length(trim(provenance_reference)) > 0), provenance_digest TEXT NOT NULL CHECK (length(provenance_digest) = 71), policy_revision INTEGER NOT NULL CHECK (policy_revision >= 0), license_policy_verified INTEGER NOT NULL CHECK (license_policy_verified = 1), vulnerability_policy_verified INTEGER NOT NULL CHECK (vulnerability_policy_verified = 1), abi_verified INTEGER NOT NULL CHECK (abi_verified = 1), capability_verified INTEGER NOT NULL CHECK (capability_verified = 1), native_promotion_denied INTEGER NOT NULL DEFAULT 1 CHECK (native_promotion_denied = 1), ingress_at TEXT NOT NULL, PRIMARY KEY (scope_kind, scope_tenant_key, release_digest), FOREIGN KEY (scope_kind, scope_tenant_key, release_digest) REFERENCES module_admitted_oci_releases_scoped (scope_kind, scope_tenant_key, release_digest) ON DELETE RESTRICT)",
                "INSERT INTO module_external_prebuilt_ingress_scoped SELECT r.scope_kind, r.scope_tenant_key, e.* FROM module_external_prebuilt_ingress e JOIN module_admitted_oci_releases r ON r.release_digest = e.release_digest",
                "DROP TABLE module_external_prebuilt_ingress",
                "DROP TABLE module_admitted_oci_releases",
                "ALTER TABLE module_admitted_oci_releases_scoped RENAME TO module_admitted_oci_releases",
                "ALTER TABLE module_external_prebuilt_ingress_scoped RENAME TO module_external_prebuilt_ingress",
                "CREATE INDEX idx_admitted_oci_releases_slug_version ON module_admitted_oci_releases (slug, version)",
                "CREATE INDEX idx_admitted_oci_releases_payload ON module_admitted_oci_releases (payload_digest)",
                "ALTER TABLE module_executor_readiness_receipts ADD COLUMN scope_kind TEXT",
                "ALTER TABLE module_executor_readiness_receipts ADD COLUMN scope_tenant_key TEXT",
                "UPDATE module_executor_readiness_receipts SET scope_kind = (SELECT i.scope_kind FROM module_artifact_installations i WHERE i.installation_id = module_executor_readiness_receipts.installation_id), scope_tenant_key = (SELECT CASE WHEN i.scope_kind = 'platform' THEN 'platform' ELSE i.tenant_id END FROM module_artifact_installations i WHERE i.installation_id = module_executor_readiness_receipts.installation_id)",
            ],
        };
        for statement in statements {
            manager
                .get_connection()
                .execute_raw(Statement::from_string(
                    backend.db_backend(),
                    (*statement).to_string(),
                ))
                .await?;
        }
        Ok(())
    }

    async fn down(&self, _: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "Scoped admission identities cannot be downgraded without losing tenant evidence"
                .into(),
        ))
    }
}

#[cfg(test)]
mod tests {
    use crate::ModulesModule;
    use rustok_core::MigrationSource;
    use sea_orm::{ConnectionTrait, Database, DbBackend, Statement, TransactionTrait};
    use sea_orm_migration::{MigrationTrait, SchemaManager};
    use sha2::{Digest, Sha256};
    use uuid::Uuid;

    fn sha256_digest(bytes: &[u8]) -> String {
        format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
    }

    /// Run explicitly with a privileged PostgreSQL test connection. All DDL
    /// and fixture data are rolled back with the isolated schema transaction.
    #[tokio::test]
    #[ignore = "requires RUSTOK_MODULES_TEST_DATABASE_URL and PostgreSQL SET ROLE privileges"]
    async fn postgres_scoped_admission_migration_enforces_tenant_rls() {
        let url = std::env::var("RUSTOK_MODULES_TEST_DATABASE_URL")
            .expect("RUSTOK_MODULES_TEST_DATABASE_URL is required");
        let db = Database::connect(url).await.unwrap();
        let tx = db.begin().await.unwrap();
        let schema = format!("module_admission_test_{}", Uuid::new_v4().simple());
        tx.execute_unprepared(&format!(
            "CREATE SCHEMA {schema}; GRANT USAGE ON SCHEMA {schema} TO pg_read_all_data; \
             SET LOCAL search_path TO {schema}, public; \
             CREATE TABLE module_artifact_installations (installation_id uuid PRIMARY KEY, \
             scope_kind text, tenant_id uuid, manifest_digest text, payload_digest text)"
        ))
        .await
        .unwrap();
        let manager = SchemaManager::new(&tx);
        super::super::m20261022_000057_admitted_oci_releases::Migration
            .up(&manager)
            .await
            .unwrap();
        super::super::m20261027_000077_executor_readiness_validity::Migration
            .up(&manager)
            .await
            .unwrap();
        let tenant_a = Uuid::new_v4().to_string();
        let tenant_b = Uuid::new_v4().to_string();
        let digest = sha256_digest(b"shared-release");
        let payload = sha256_digest(b"payload");
        let actor = Uuid::new_v4();
        let key = Uuid::new_v4();
        let correlation = Uuid::new_v4();
        let request_digest = sha256_digest(b"request");
        tx.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            "INSERT INTO module_admitted_oci_releases (release_digest, scope_kind, scope_tenant_key, registry, repository, slug, version, payload_digest, payload_media_type, payload_size_bytes, descriptor_json, artifact_origin, actor_id, idempotency_key, trace_id, correlation_id, request_digest, admitted_at) VALUES ($1, 'tenant', $2, 'registry', 'repo', 'module', '1', $3, 'media', 1, '{}', 'external_prebuilt', $4, $5, 'trace', $6, $7, now())",
            vec![digest.clone().into(), tenant_a.clone().into(), payload.clone().into(), actor.into(), key.into(), correlation.into(), request_digest.into()])).await.unwrap();
        tx.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            "INSERT INTO module_external_prebuilt_ingress (release_digest, publisher_identity, lineage_reference, lineage_digest, signature_reference, signature_digest, sbom_reference, sbom_digest, provenance_reference, provenance_digest, policy_revision, license_policy_verified, vulnerability_policy_verified, abi_verified, capability_verified, native_promotion_denied, ingress_at) VALUES ($1, 'publisher-a', 'lineage', $2, 'signature', $2, 'sbom', $2, 'provenance', $2, 1, true, true, true, true, true, now())",
            vec![digest.clone().into(), payload.into()])).await.unwrap();
        super::Migration.up(&manager).await.unwrap();
        tx.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            "INSERT INTO module_admitted_oci_releases SELECT release_digest, 'tenant', $1, registry, repository, slug, version, payload_digest, payload_media_type, payload_size_bytes, descriptor_json, artifact_origin, actor_id, idempotency_key, trace_id, correlation_id, request_digest, admitted_at FROM module_admitted_oci_releases WHERE scope_tenant_key = $2",
            vec![tenant_b.clone().into(), tenant_a.clone().into()])).await.unwrap();
        tx.execute_raw(Statement::from_sql_and_values(DbBackend::Postgres,
            "INSERT INTO module_external_prebuilt_ingress SELECT release_digest, 'publisher-b', lineage_reference, lineage_digest, signature_reference, signature_digest, sbom_reference, sbom_digest, provenance_reference, provenance_digest, policy_revision, license_policy_verified, vulnerability_policy_verified, abi_verified, capability_verified, native_promotion_denied, ingress_at, 'tenant', $1 FROM module_external_prebuilt_ingress WHERE scope_tenant_key = $2",
            vec![tenant_b.clone().into(), tenant_a.clone().into()])).await.unwrap();
        tx.execute_unprepared(&format!(
            "GRANT INSERT ON {schema}.module_admitted_oci_releases TO pg_read_all_data"
        ))
        .await
        .unwrap();
        tx.execute_unprepared("SET LOCAL ROLE pg_read_all_data")
            .await
            .unwrap();
        let role = tx
            .query_one_raw(Statement::from_string(
                DbBackend::Postgres,
                "SELECT NOT rolsuper AND NOT rolbypassrls AS rls_enforced FROM pg_roles WHERE rolname = current_user".to_string(),
            ))
            .await
            .unwrap()
            .expect("test role exists");
        assert!(role.try_get::<bool>("", "rls_enforced").unwrap());
        for (tenant, expected_publisher) in [(&tenant_a, "publisher-a"), (&tenant_b, "publisher-b")]
        {
            tx.execute_raw(Statement::from_sql_and_values(
                DbBackend::Postgres,
                "SELECT set_config('rustok.tenant_id', $1, true)",
                vec![tenant.clone().into()],
            ))
            .await
            .unwrap();
            let admissions = tx
                .query_all_raw(Statement::from_string(
                    DbBackend::Postgres,
                    "SELECT scope_tenant_key FROM module_admitted_oci_releases".to_string(),
                ))
                .await
                .unwrap();
            let ingress = tx
                .query_all_raw(Statement::from_string(
                    DbBackend::Postgres,
                    "SELECT publisher_identity FROM module_external_prebuilt_ingress".to_string(),
                ))
                .await
                .unwrap();
            assert_eq!(admissions.len(), 1);
            assert_eq!(
                admissions[0]
                    .try_get::<String>("", "scope_tenant_key")
                    .unwrap(),
                *tenant
            );
            assert_eq!(ingress.len(), 1);
            assert_eq!(
                ingress[0]
                    .try_get::<String>("", "publisher_identity")
                    .unwrap(),
                expected_publisher
            );
        }
        tx.execute_unprepared(
            "DO $$ BEGIN BEGIN \
             INSERT INTO module_admitted_oci_releases \
             SELECT release_digest, 'tenant', '00000000-0000-0000-0000-000000000000', \
             registry, repository, slug, version, payload_digest, payload_media_type, \
             payload_size_bytes, descriptor_json, artifact_origin, actor_id, idempotency_key, \
             trace_id, correlation_id, request_digest, admitted_at FROM module_admitted_oci_releases; \
             RAISE EXCEPTION 'foreign tenant insertion was accepted'; \
             EXCEPTION WHEN insufficient_privilege THEN NULL; END; END $$",
        )
        .await
        .unwrap();
        tx.rollback().await.unwrap();
    }

    #[tokio::test]
    async fn scoped_admission_migration_preserves_existing_release_and_ingress() {
        let db = Database::connect("sqlite::memory:").await.unwrap();
        db.execute_unprepared("PRAGMA foreign_keys = ON")
            .await
            .unwrap();
        let manager = SchemaManager::new(&db);
        rustok_outbox::SysEventsMigration
            .up(&manager)
            .await
            .unwrap();
        let migrations = ModulesModule.migrations();
        for migration in migrations
            .iter()
            .filter(|m| !m.name().ends_with("scoped_admission"))
        {
            migration.up(&manager).await.unwrap();
        }
        let digest = sha256_digest(b"prior-manifest");
        let payload_digest = sha256_digest(b"prior-payload");
        let actor = Uuid::new_v4().to_string();
        let key = Uuid::new_v4().to_string();
        let correlation = Uuid::new_v4().to_string();
        let tenant = Uuid::new_v4().to_string();
        db.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
        "INSERT INTO module_admitted_oci_releases (release_digest, scope_kind, scope_tenant_key, registry, repository, slug, version, payload_digest, payload_media_type, payload_size_bytes, descriptor_json, artifact_origin, actor_id, idempotency_key, trace_id, correlation_id, request_digest, admitted_at) VALUES (?1, 'tenant', ?2, 'registry', 'repository', 'module', '1.0', ?3, 'media', 1, '{}', 'external_prebuilt', ?4, ?5, 'trace', ?6, ?7, '2026-01-01T00:00:00Z')",
        vec![digest.clone().into(), tenant.clone().into(), payload_digest.clone().into(), actor.into(), key.into(), correlation.into(), sha256_digest(b"request").into()])).await.unwrap();
        db.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
        "INSERT INTO module_external_prebuilt_ingress (release_digest, publisher_identity, lineage_reference, lineage_digest, signature_reference, signature_digest, sbom_reference, sbom_digest, provenance_reference, provenance_digest, policy_revision, license_policy_verified, vulnerability_policy_verified, abi_verified, capability_verified, native_promotion_denied, ingress_at) VALUES (?1, 'publisher', 'lineage', ?2, 'signature', ?2, 'sbom', ?2, 'provenance', ?2, 1, 1, 1, 1, 1, 1, '2026-01-01T00:00:00Z')",
        vec![digest.clone().into(), payload_digest.into()])).await.unwrap();
        migrations
            .iter()
            .find(|m| m.name().ends_with("scoped_admission"))
            .unwrap()
            .up(&manager)
            .await
            .unwrap();
        let row = db.query_one_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
        "SELECT scope_kind, scope_tenant_key, publisher_identity FROM module_external_prebuilt_ingress WHERE release_digest = ?1", vec![digest.into()])).await.unwrap().unwrap();
        assert_eq!(row.try_get::<String>("", "scope_kind").unwrap(), "tenant");
        assert_eq!(
            row.try_get::<String>("", "scope_tenant_key").unwrap(),
            tenant
        );
        assert_eq!(
            row.try_get::<String>("", "publisher_identity").unwrap(),
            "publisher"
        );
        assert!(
            db.query_all_raw(Statement::from_string(
                DbBackend::Sqlite,
                "PRAGMA foreign_key_check".to_string()
            ))
            .await
            .unwrap()
            .is_empty()
        );
    }
}
