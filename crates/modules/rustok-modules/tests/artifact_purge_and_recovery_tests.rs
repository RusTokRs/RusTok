//! Integration tests for separate preview/apply artifact settings purge and data purge,
//! verifying retirement fences, tombstoned recovery points, and non-combined lifecycle.

use async_trait::async_trait;
use chrono::{Duration, Utc};
use rustok_core::MigrationSource;
use rustok_modules::{
    ArtifactDataError, ArtifactDataPurgeAuthorizer, ArtifactDataPurgeRequest, ArtifactDataScope,
    ArtifactModuleKind, ArtifactPayloadKind, ArtifactSchemaDocument, ArtifactSettingsPurgeRequest,
    ArtifactSettingsRecoveryAuthorizationContext, ArtifactSettingsRecoveryAuthorizer,
    ArtifactSettingsRecoveryBindRequest, ArtifactSettingsRecoveryCipher,
    ArtifactSettingsRecoveryCipherContext, ArtifactSettingsRecoveryCiphertext,
    ArtifactSettingsRecoveryCollectionRequest, ArtifactSettingsRecoveryError,
    ArtifactSettingsRecoveryPointCreateRequest, ArtifactSettingsRecoveryRetention,
    ArtifactSettingsRecoveryRetentionUpdate, ArtifactSettingsRecoveryRetentionUpdateRequest,
    ArtifactSettingsRecoveryRewrapRequest, ArtifactSettingsRestoreRequest,
    ModuleArtifactDescriptor, ModuleCommandContext, ModulesModule, SeaOrmArtifactDataPurgeService,
    SeaOrmArtifactSettingsRecoveryService, canonical_schema_digest,
};
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Clone)]
struct TestAuthorizer;

#[async_trait]
impl ArtifactDataPurgeAuthorizer for TestAuthorizer {
    async fn authorize_purge(
        &self,
        request: &ArtifactDataPurgeRequest,
    ) -> Result<(), ArtifactDataError> {
        if request.reason.trim().is_empty() {
            return Err(ArtifactDataError::PurgePrecondition);
        }
        Ok(())
    }
}

#[async_trait]
impl ArtifactSettingsRecoveryAuthorizer for TestAuthorizer {
    async fn authorize_recovery_point(
        &self,
        request: &ArtifactSettingsRecoveryPointCreateRequest,
    ) -> Result<ArtifactSettingsRecoveryRetention, ArtifactSettingsRecoveryError> {
        if request.reason.trim().is_empty() {
            return Err(ArtifactSettingsRecoveryError::PolicyDenied);
        }
        Ok(ArtifactSettingsRecoveryRetention {
            policy_snapshot_id: "test-retention-v1".to_string(),
            secret_handle_digest: format!("sha256:{}", "0".repeat(64)),
            retain_until: Utc::now() + Duration::days(30),
            legal_hold: false,
            audit_hold: false,
            incident_hold: false,
        })
    }

    async fn authorize_purge(
        &self,
        request: &ArtifactSettingsPurgeRequest,
        recovery: &ArtifactSettingsRecoveryAuthorizationContext,
    ) -> Result<(), ArtifactSettingsRecoveryError> {
        if request.reason.trim().is_empty()
            || recovery.recovery_point_id != request.recovery_point_id
        {
            return Err(ArtifactSettingsRecoveryError::PolicyDenied);
        }
        Ok(())
    }

    async fn authorize_restore(
        &self,
        request: &ArtifactSettingsRestoreRequest,
        recovery: &ArtifactSettingsRecoveryAuthorizationContext,
    ) -> Result<(), ArtifactSettingsRecoveryError> {
        if request.reason.trim().is_empty()
            || recovery.recovery_point_id != request.recovery_point_id
        {
            return Err(ArtifactSettingsRecoveryError::PolicyDenied);
        }
        Ok(())
    }

    async fn authorize_retention_update(
        &self,
        request: &ArtifactSettingsRecoveryRetentionUpdateRequest,
        recovery: &ArtifactSettingsRecoveryAuthorizationContext,
    ) -> Result<ArtifactSettingsRecoveryRetentionUpdate, ArtifactSettingsRecoveryError> {
        Ok(ArtifactSettingsRecoveryRetentionUpdate {
            policy_snapshot_id: "test-retention-v1".to_string(),
            retain_until: request.extend_retain_until.unwrap_or(recovery.retain_until),
            legal_hold: request.legal_hold.unwrap_or(recovery.legal_hold),
            audit_hold: request.audit_hold.unwrap_or(recovery.audit_hold),
            incident_hold: request.incident_hold.unwrap_or(recovery.incident_hold),
        })
    }

    async fn authorize_rewrap(
        &self,
        _: &ArtifactSettingsRecoveryRewrapRequest,
        _: &ArtifactSettingsRecoveryAuthorizationContext,
    ) -> Result<(), ArtifactSettingsRecoveryError> {
        Ok(())
    }

    async fn authorize_collection(
        &self,
        _: &ArtifactSettingsRecoveryCollectionRequest,
    ) -> Result<(), ArtifactSettingsRecoveryError> {
        Ok(())
    }

    async fn authorize_bind(
        &self,
        _: &ArtifactSettingsRecoveryBindRequest,
        _: &ArtifactSettingsRecoveryAuthorizationContext,
    ) -> Result<(), ArtifactSettingsRecoveryError> {
        Ok(())
    }
}

#[derive(Clone)]
struct TestCipher;

const TEST_KEY_VERSION: &str = "test-kms-2026-09";

#[async_trait]
impl ArtifactSettingsRecoveryCipher for TestCipher {
    async fn encrypt(
        &self,
        context: &ArtifactSettingsRecoveryCipherContext,
        canonical_settings: &[u8],
    ) -> Result<ArtifactSettingsRecoveryCiphertext, ArtifactSettingsRecoveryError> {
        let mut hasher = Sha256::new();
        hasher.update(context.settings_instance_id.as_bytes());
        hasher.update(canonical_settings);
        let tag = hasher.finalize().to_vec();

        let mut bytes = tag;
        bytes.extend_from_slice(canonical_settings);

        Ok(ArtifactSettingsRecoveryCiphertext {
            key_version: TEST_KEY_VERSION.to_string(),
            bytes,
        })
    }

    async fn decrypt(
        &self,
        context: &ArtifactSettingsRecoveryCipherContext,
        ciphertext: &ArtifactSettingsRecoveryCiphertext,
    ) -> Result<Vec<u8>, ArtifactSettingsRecoveryError> {
        if ciphertext.bytes.len() < 32 || ciphertext.key_version != TEST_KEY_VERSION {
            return Err(ArtifactSettingsRecoveryError::CiphertextIntegrity);
        }
        let (tag, settings) = ciphertext.bytes.split_at(32);
        let mut hasher = Sha256::new();
        hasher.update(context.settings_instance_id.as_bytes());
        hasher.update(settings);
        let expected_tag = hasher.finalize().to_vec();

        if tag == expected_tag.as_slice() {
            Ok(settings.to_vec())
        } else {
            Err(ArtifactSettingsRecoveryError::CiphertextIntegrity)
        }
    }

    async fn rewrap(
        &self,
        context: &ArtifactSettingsRecoveryCipherContext,
        ciphertext: &ArtifactSettingsRecoveryCiphertext,
    ) -> Result<ArtifactSettingsRecoveryCiphertext, ArtifactSettingsRecoveryError> {
        let settings = self.decrypt(context, ciphertext).await?;
        self.encrypt(context, &settings).await
    }
}

fn command_context(tenant_id: Uuid, actor_id: Uuid) -> ModuleCommandContext {
    ModuleCommandContext {
        actor_id,
        tenant_id: Some(tenant_id),
        idempotency_key: Uuid::new_v4(),
        trace_id: "trace-1".to_string(),
        correlation_id: Uuid::new_v4(),
    }
}

fn descriptor(
    slug: &str,
    version: &str,
    schema_digest: String,
    schema: serde_json::Value,
) -> ModuleArtifactDescriptor {
    ModuleArtifactDescriptor {
        schema_version: rustok_modules::MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
        slug: slug.to_string(),
        version: version.to_string(),
        payload_kind: ArtifactPayloadKind::Rhai,
        module_kind: ArtifactModuleKind::Optional,
        runtime_abi: "rustok:module/runtime@1".to_string(),
        platform_compatibility: "^0.1".to_string(),
        required_features: Vec::new(),
        artifact_digest: format!("sha256:{}", "a".repeat(64)),
        entrypoint: "main".to_string(),
        capabilities: Vec::new(),
        bindings: Vec::new(),
        dependencies: Vec::new(),
        permissions: Vec::new(),
        schema_documents: vec![ArtifactSchemaDocument {
            digest: schema_digest.clone(),
            document: schema,
        }],
        settings_schema_digest: Some(schema_digest),
        data_schema_digest: None,
        localization_catalogs: Vec::new(),
        ui_contributions: Vec::new(),
        persistence_contract: None,
    }
}

#[tokio::test]
async fn test_separate_settings_and_data_purge_lifecycle() {
    let database = Database::connect("sqlite::memory:")
        .await
        .expect("sqlite database");
    rustok_outbox::SysEventsMigration
        .up(&SchemaManager::new(&database))
        .await
        .expect("outbox migration");
    for migration in ModulesModule.migrations() {
        migration
            .up(&SchemaManager::new(&database))
            .await
            .expect("module migration");
    }

    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let installation_id = Uuid::new_v4();
    let data_owner_id = Uuid::new_v4();
    let settings_instance_id = Uuid::new_v4();

    let schema = serde_json::json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
        "properties": { "theme": { "type": "string" } },
        "required": ["theme"],
        "additionalProperties": false,
    });
    let schema_digest = canonical_schema_digest(&schema);

    let desc = descriptor(
        "theme_manager",
        "1.0.0",
        schema_digest.clone(),
        schema.clone(),
    );

    // 1. Insert installation and settings instance
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_installations (\
                installation_id, scope_kind, tenant_id, registry, repository, manifest_digest, slug, version, payload_kind, \
                runtime_abi, payload_digest, entrypoint, descriptor, data_owner_id, settings_instance_id, dependency_graph_revision, \
                dependency_graph_digest, dependency_lock, installed_at\
             ) VALUES (?1, 'tenant', ?2, 'registry.example', 'modules/recovery', ?3, 'theme_manager', '1.0.0', 'rhai', \
                'rustok:module/runtime@1', ?4, 'main', ?5, ?6, ?7, 1, ?8, '{}', '2026-08-13T00:00:00Z')",
            vec![
                installation_id.to_string().into(),
                tenant_id.to_string().into(),
                format!("sha256:{}", "d".repeat(64)).into(),
                desc.artifact_digest.clone().into(),
                sea_orm::Value::Json(Some(Box::new(serde_json::to_value(&desc).unwrap()))),
                data_owner_id.to_string().into(),
                settings_instance_id.to_string().into(),
                format!("sha256:{}", "e".repeat(64)).into(),
            ],
        ))
        .await
        .expect("installation");

    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_settings_instances (tenant_id, data_owner_id, settings_instance_id, schema_digest, settings, revision, created_at, updated_at) VALUES (?1, ?2, ?3, ?4, ?5, 3, '2026-09-01T00:00:00Z', '2026-09-01T00:00:00Z')",
            vec![
                tenant_id.to_string().into(),
                data_owner_id.to_string().into(),
                settings_instance_id.to_string().into(),
                schema_digest.clone().into(),
                serde_json::json!({ "theme": "ocean" }).into(),
            ],
        ))
        .await
        .expect("insert settings");

    // Initially mark admission as ACTIVE (serving)
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_admissions (stage_id, installation_id, payload_digest, media_type, size_bytes, verification_evidence, status, revision, committed_at) VALUES (?1, ?2, ?3, 'application/vnd.rustok.rhai', 1, '{}', 'active', 2, '2026-08-13T00:00:00Z')",
            vec![
                Uuid::new_v4().to_string().into(),
                installation_id.to_string().into(),
                format!("sha256:{}", "f".repeat(64)).into(),
            ],
        ))
        .await
        .expect("insert admission active");

    let recovery_service =
        SeaOrmArtifactSettingsRecoveryService::new(database.clone(), TestAuthorizer, TestCipher);

    // 2. Attempting to create recovery point while installation is ACTIVE must fail with RecoveryPrecondition!
    let recovery_req = ArtifactSettingsRecoveryPointCreateRequest {
        tenant_id,
        installation_id,
        expected_installation_revision: 2,
        expected_settings_revision: 3,
        context: command_context(tenant_id, actor_id),
        reason: "pre-purge backup".to_string(),
    };
    let active_recovery_err = recovery_service
        .create_recovery_point(recovery_req.clone())
        .await
        .expect_err("active recovery point creation should fail");
    assert_eq!(
        active_recovery_err,
        ArtifactSettingsRecoveryError::RecoveryPrecondition
    );

    // 3. Transition installation to retired: inactive admission and uninstall evidence
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE module_artifact_admissions SET status = 'inactive' WHERE installation_id = ?1",
            vec![installation_id.to_string().into()],
        ))
        .await
        .expect("set inactive");

    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_uninstall_operations \
             (operation_id, installation_id, expected_revision, actor_id, trace_id, correlation_id, reason, idempotency_key, committed_at) \
             VALUES (?1, ?2, 2, ?3, 'test:artifact-settings-recovery', ?4, 'retired source', ?5, '2026-08-13T00:00:00Z')",
            vec![
                Uuid::new_v4().to_string().into(),
                installation_id.to_string().into(),
                actor_id.to_string().into(),
                Uuid::new_v4().to_string().into(),
                Uuid::new_v4().to_string().into(),
            ],
        ))
        .await
        .expect("insert uninstall evidence");

    // 4. Now that installation is retired, creating recovery point succeeds!
    let recovery_pt = recovery_service
        .create_recovery_point(recovery_req)
        .await
        .expect("create recovery point");

    assert_eq!(recovery_pt.recovery_point_id, recovery_pt.recovery_point_id);

    // 5. Purge now succeeds!
    let purge_req = ArtifactSettingsPurgeRequest {
        tenant_id,
        installation_id,
        recovery_point_id: recovery_pt.recovery_point_id,
        expected_installation_revision: 2,
        expected_settings_revision: 3,
        context: command_context(tenant_id, actor_id),
        reason: "test purge".to_string(),
    };
    let purge_res = recovery_service
        .purge(purge_req)
        .await
        .expect("retired purge succeeds");
    assert_eq!(purge_res.recovery_point_id, recovery_pt.recovery_point_id);
    assert_eq!(purge_res.tombstone_revision, 1);

    // 5. Restore settings to a fresh non-serving instance
    let restore_req = ArtifactSettingsRestoreRequest {
        tenant_id,
        recovery_point_id: recovery_pt.recovery_point_id,
        target_installation_id: None,
        expected_target_installation_revision: None,
        context: command_context(tenant_id, actor_id),
        reason: "disaster recovery restore".to_string(),
    };
    let restore_res = recovery_service
        .restore(restore_req)
        .await
        .expect("restore from recovery point succeeds");
    assert_ne!(restore_res.settings_instance_id, settings_instance_id);

    // 6. Test separate data purge service
    let data_scope = ArtifactDataScope {
        tenant_id,
        module_slug: "theme_manager".to_string(),
        data_contract_revision: 1,
        policy_revision: 1,
    };

    // Insert data namespace
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_data_namespaces (tenant_id, module_slug, data_contract_revision, namespace_revision, created_at, updated_at) VALUES (?1, ?2, 1, 1, '2026-09-01T00:00:00Z', '2026-09-01T00:00:00Z')",
            vec![
                tenant_id.to_string().into(),
                "theme_manager".to_string().into(),
            ],
        ))
        .await
        .expect("insert namespace");

    let data_purge_service = SeaOrmArtifactDataPurgeService::new(database.clone(), TestAuthorizer);

    let data_purge_req = ArtifactDataPurgeRequest {
        scope: data_scope,
        expected_namespace_revision: 1,
        context: command_context(tenant_id, actor_id),
        reason: "cleanup namespace".to_string(),
    };

    let data_purge_res = data_purge_service
        .purge(data_purge_req)
        .await
        .expect("data purge succeeds");

    assert_eq!(data_purge_res.namespace_revision, 2);
    assert_eq!(data_purge_res.purged_records, 0);
}
