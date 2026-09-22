//! Integration tests for separate preview/apply artifact settings purge and data purge,
//! verifying retirement fences, tombstoned recovery points, and non-combined lifecycle.

use async_trait::async_trait;
use chrono::{Duration, Utc};
use object_store::{ObjectStoreExt, path::Path};
use rustok_core::MigrationSource;
use rustok_modules::{
    ArtifactDataError, ArtifactDataPostPurgeRecoveryService, ArtifactDataPurgeAuthorizationContext,
    ArtifactDataPurgeAuthorizer, ArtifactDataPurgePreviewService, ArtifactDataPurgeRequest,
    ArtifactDataQuota, ArtifactDataRecoveryAuthorizationContext, ArtifactDataRecoveryAuthorizer,
    ArtifactDataRestoreRequest, ArtifactDataScope, ArtifactDataSnapshotAuthorizer,
    ArtifactDataSnapshotCreateRequest, ArtifactModuleKind, ArtifactPayloadKind,
    ArtifactPersistenceContract, ArtifactSchemaDocument, ArtifactSettingsPurgePreviewService,
    ArtifactSettingsPurgeRequest, ArtifactSettingsRecoveryAuthorizationContext,
    ArtifactSettingsRecoveryAuthorizer, ArtifactSettingsRecoveryBindRequest,
    ArtifactSettingsRecoveryCipher, ArtifactSettingsRecoveryCipherContext,
    ArtifactSettingsRecoveryCiphertext, ArtifactSettingsRecoveryCollectionRequest,
    ArtifactSettingsRecoveryError, ArtifactSettingsRecoveryPointCreateRequest,
    ArtifactSettingsRecoveryRetention, ArtifactSettingsRecoveryRetentionUpdate,
    ArtifactSettingsRecoveryRetentionUpdateRequest, ArtifactSettingsRecoveryRewrapRequest,
    ArtifactSettingsRestoreRequest, ModuleArtifactDescriptor, ModuleCommandContext, ModulesModule,
    PostPurgeRecoveryCutoverRequest, PostPurgeRecoveryError, PrepareRecoveryRequest,
    SeaOrmArtifactDataPurgeService, SeaOrmArtifactDataSnapshotService,
    SeaOrmArtifactSettingsRecoveryService, canonical_schema_digest,
};
use rustok_storage::{LocalStorageConfig, ObjectKey, ObjectScope, ObjectZone, StorageRuntime};
use sea_orm::{ConnectionTrait, Database, DbBackend, Statement};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use sha2::{Digest, Sha256};
use uuid::Uuid;

#[derive(Clone)]
struct RecoveryFixturePolicy {
    tenant_id: Uuid,
    actor_id: Uuid,
    installation_id: Uuid,
    data_owner_id: Uuid,
}

impl RecoveryFixturePolicy {
    fn check_scope(&self, context: &ModuleCommandContext, scope: &ArtifactDataScope) -> bool {
        context.tenant_id == Some(self.tenant_id)
            && context.actor_id == self.actor_id
            && scope.tenant_id == self.tenant_id
            && scope.data_owner_id == self.data_owner_id
    }
    async fn check_owner(
        &self,
        transaction: &sea_orm::DatabaseTransaction,
        context: &ModuleCommandContext,
        owner: &ArtifactDataRecoveryAuthorizationContext,
    ) -> Result<(), PostPurgeRecoveryError> {
        if !self.check_scope(context, &owner.original.scope)
            || !self.check_scope(context, &owner.target)
            || owner.original.installation_id != self.installation_id
            || owner.original.scope.namespace_instance_id == owner.target.namespace_instance_id
        {
            return Err(PostPurgeRecoveryError::AuthorizationDenied);
        }
        let row = transaction.query_one_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "SELECT 1 FROM module_artifact_data_namespaces WHERE tenant_id=?1 AND data_owner_id=?2
             AND namespace_instance_id=?3 AND state='purged' AND purged_at IS NOT NULL",
            vec![self.tenant_id.to_string().into(),self.data_owner_id.to_string().into(),
                 owner.original.scope.namespace_instance_id.to_string().into()]))
            .await.map_err(|e|PostPurgeRecoveryError::Storage(e.to_string()))?;
        if row.is_none() {
            return Err(PostPurgeRecoveryError::AuthorizationDenied);
        }
        Ok(())
    }
}

#[async_trait]
impl ArtifactDataSnapshotAuthorizer for RecoveryFixturePolicy {
    async fn authorize_snapshot(
        &self,
        request: &ArtifactDataSnapshotCreateRequest,
    ) -> Result<(), ArtifactDataError> {
        if !self.check_scope(&request.context, &request.scope) {
            return Err(ArtifactDataError::SnapshotPrecondition);
        }
        Ok(())
    }
    async fn authorize_restore(
        &self,
        request: &ArtifactDataRestoreRequest,
    ) -> Result<ArtifactDataQuota, ArtifactDataError> {
        if !self.check_scope(&request.context, &request.target) {
            return Err(ArtifactDataError::RestorePrecondition);
        }
        Ok(ArtifactDataQuota::default())
    }
}
#[async_trait]
impl ArtifactDataRecoveryAuthorizer for RecoveryFixturePolicy {
    async fn authorize_prepare_on(
        &self,
        transaction: &sea_orm::DatabaseTransaction,
        request: &PrepareRecoveryRequest,
        owner: &ArtifactDataRecoveryAuthorizationContext,
    ) -> Result<(), PostPurgeRecoveryError> {
        self.check_owner(transaction, &request.context, owner).await
    }
    async fn authorize_cutover_on(
        &self,
        transaction: &sea_orm::DatabaseTransaction,
        request: &PostPurgeRecoveryCutoverRequest,
        owner: &ArtifactDataRecoveryAuthorizationContext,
    ) -> Result<(), PostPurgeRecoveryError> {
        self.check_owner(transaction, &request.context, owner).await
    }
}

#[derive(Clone)]
struct TestAuthorizer;

#[async_trait]
impl ArtifactDataPurgeAuthorizer for RecoveryFixturePolicy {
    async fn authorize_purge_on(
        &self,
        transaction: &sea_orm::DatabaseTransaction,
        request: &ArtifactDataPurgeRequest,
        owner: &ArtifactDataPurgeAuthorizationContext,
    ) -> Result<(), ArtifactDataError> {
        if !self.check_scope(&request.context, &owner.scope)
            || owner.installation_id != self.installation_id
            || request.installation_id != self.installation_id
        {
            return Err(ArtifactDataError::PolicyDenied);
        }
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT 1 FROM module_artifact_data_namespaces
             WHERE tenant_id=?1 AND data_owner_id=?2 AND namespace_instance_id=?3
               AND purged_at IS NULL AND namespace_revision=?4",
                vec![
                    self.tenant_id.to_string().into(),
                    self.data_owner_id.to_string().into(),
                    owner.scope.namespace_instance_id.to_string().into(),
                    i64::try_from(request.expected_namespace_revision)
                        .map_err(|_| ArtifactDataError::PurgePrecondition)?
                        .into(),
                ],
            ))
            .await
            .map_err(|error| ArtifactDataError::Storage(error.to_string()))?;
        if row.is_none() {
            return Err(ArtifactDataError::PolicyDenied);
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
        settings_schema_digest: Some(schema_digest.clone()),
        data_schema_digest: Some(schema_digest.clone()),
        localization_catalogs: Vec::new(),
        ui_contributions: Vec::new(),
        persistence_contract: Some(ArtifactPersistenceContract {
            revision: 1,
            schema_digest,
            indexes: Vec::new(),
        }),
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
    let namespace_instance_id = Uuid::new_v4();
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
    let data_contract_digest = format!(
        "sha256:{}",
        hex::encode(Sha256::digest(
            serde_json::to_vec(desc.persistence_contract.as_ref().unwrap()).unwrap()
        ))
    );

    // 1. Insert installation and settings instance
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_installations (\
                installation_id, scope_kind, tenant_id, registry, repository, manifest_digest, slug, version, payload_kind, \
                runtime_abi, payload_digest, entrypoint, descriptor, data_owner_id, settings_instance_id, dependency_graph_revision, \
                dependency_graph_digest, dependency_lock, installed_at, namespace_instance_id\
             ) VALUES (?1, 'tenant', ?2, 'registry.example', 'modules/recovery', ?3, 'theme_manager', '1.0.0', 'rhai', \
                'rustok:module/runtime@1', ?4, 'main', ?5, ?6, ?7, 1, ?8, '{}', '2026-08-13T00:00:00Z', ?9)",
            vec![
                installation_id.to_string().into(),
                tenant_id.to_string().into(),
                format!("sha256:{}", "d".repeat(64)).into(),
                desc.artifact_digest.clone().into(),
                sea_orm::Value::Json(Some(Box::new(serde_json::to_value(&desc).unwrap()))),
                data_owner_id.to_string().into(),
                settings_instance_id.to_string().into(),
                format!("sha256:{}", "e".repeat(64)).into(),
                namespace_instance_id.to_string().into(),
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

    // Owner data scope is derived from this admitted descriptor. The same
    // namespace must be blocked while this installation is still serving.
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_data_namespaces
             (tenant_id, data_owner_id, namespace_instance_id, module_slug, data_contract_revision,
              data_contract_digest, state, namespace_revision, created_at, updated_at)
             VALUES (?1, ?2, ?3, 'theme_manager', 1, ?4, 'serving', 1, '2026-09-01T00:00:00Z', '2026-09-01T00:00:00Z')",
            vec![
                tenant_id.to_string().into(),
                data_owner_id.to_string().into(),
                namespace_instance_id.to_string().into(),
                data_contract_digest.clone().into(),
            ],
        ))
        .await
        .expect("insert data namespace");
    database.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
        "INSERT INTO module_artifact_data_owner_references
         (tenant_id, data_owner_id, namespace_instance_id, reference_revision) VALUES (?1, ?2, ?3, 1)",
        vec![tenant_id.to_string().into(), data_owner_id.to_string().into(), namespace_instance_id.to_string().into()]
    )).await.expect("insert owner serving reference");
    database.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
        "INSERT INTO module_artifact_data (tenant_id, data_owner_id, namespace_instance_id, data_key,
                                          value, value_size_bytes, revision, updated_at)
         VALUES (?1, ?2, ?3, 'theme', ?4, length(?4), 1, '2026-09-13T00:00:00Z')",
        vec![tenant_id.to_string().into(), data_owner_id.to_string().into(), namespace_instance_id.to_string().into(),
            serde_json::json!({"theme": "ocean"}).to_string().into()]
    )).await.expect("source owner value fixture");
    let storage_parent = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../../target")
        .canonicalize()
        .expect("absolute workspace target");
    let storage_root = storage_parent.join(format!("recovery-fixture-{}", Uuid::new_v4()));
    assert!(storage_root.starts_with(&storage_parent));
    let storage = StorageRuntime::local(&LocalStorageConfig {
        base_dir: storage_root.display().to_string(),
        fsync: true,
        ..Default::default()
    })
    .expect("actual local storage");
    let recovery_policy = RecoveryFixturePolicy {
        tenant_id,
        actor_id,
        installation_id,
        data_owner_id,
    };
    let snapshot_service = SeaOrmArtifactDataSnapshotService::new(
        database.clone(),
        storage.clone(),
        recovery_policy.clone(),
    );
    let object_bytes = bytes::Bytes::from_static(b"actual recovery object payload");
    let object_digest = format!("sha256:{}", hex::encode(Sha256::digest(&object_bytes)));
    let object_key = ObjectKey::chronological(
        "module-artifact-data",
        ObjectZone::Objects,
        ObjectScope::Namespace {
            tenant_id,
            owner_id: data_owner_id,
            instance_id: namespace_instance_id,
        },
        Utc::now(),
        Uuid::new_v4(),
        "bin",
    )
    .expect("owner-scoped source key")
    .to_string();
    storage
        .objects
        .put(
            &Path::from(object_key.as_str()),
            object_bytes.clone().into(),
        )
        .await
        .expect("source object bytes");
    database.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
        "INSERT INTO module_artifact_data_objects
         (tenant_id,data_owner_id,namespace_instance_id,object_name,storage_key,content_type,size_bytes,digest_sha256,
          revision,created_at,updated_at) VALUES (?1,?2,?3,'recovery-payload',?4,'application/octet-stream',?5,?6,1,?7,?7)",
        vec![tenant_id.to_string().into(),data_owner_id.to_string().into(),namespace_instance_id.to_string().into(),
             object_key.into(),i64::try_from(object_bytes.len()).unwrap().into(),object_digest.into(),Utc::now().into()]))
        .await.expect("source object metadata");
    let snapshot = snapshot_service
        .create(ArtifactDataSnapshotCreateRequest {
            scope: ArtifactDataScope {
                tenant_id,
                data_owner_id,
                namespace_instance_id,
                data_contract_digest,
                module_slug: "theme_manager".into(),
                data_contract_revision: 1,
                policy_revision: 1,
            },
            expected_namespace_revision: 1,
            context: command_context(tenant_id, actor_id),
            reason: "Capture actual source data".into(),
            retain_until: Utc::now() + Duration::days(30),
            legal_hold: false,
        })
        .await
        .expect("actual source snapshot before purge");
    let data_purge_service = SeaOrmArtifactDataPurgeService::new(
        database.clone(),
        RecoveryFixturePolicy {
            tenant_id,
            actor_id,
            installation_id,
            data_owner_id,
        },
    );
    let data_purge_req = ArtifactDataPurgeRequest {
        installation_id,
        expected_namespace_revision: 1,
        context: command_context(tenant_id, actor_id),
        reason: "cleanup namespace".to_string(),
    };
    let data_preview_service = ArtifactDataPurgePreviewService::new(database.clone());
    let active_data_preview = data_preview_service
        .preview(tenant_id, installation_id)
        .await
        .expect("active data preview");
    assert!(!active_data_preview.can_purge);
    let settings_preview_service = ArtifactSettingsPurgePreviewService::new(database.clone());
    let active_settings_preview = settings_preview_service
        .preview(tenant_id, installation_id)
        .await
        .expect("active settings preview");
    assert!(!active_settings_preview.can_purge);
    assert!(matches!(
        data_purge_service.purge(data_purge_req.clone()).await,
        Err(ArtifactDataError::PurgePrecondition)
    ));

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

    // A serving successor that retains this exact owner/instance blocks purge,
    // independently of its release version and the retired source installation.
    let conflicting_installation_id = Uuid::new_v4();
    let conflicting_descriptor = descriptor(
        "theme_manager",
        "1.0.1",
        schema_digest.clone(),
        schema.clone(),
    );
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_installations (\
                installation_id, scope_kind, tenant_id, registry, repository, manifest_digest, slug, version, payload_kind, \
                runtime_abi, payload_digest, entrypoint, descriptor, data_owner_id, settings_instance_id, dependency_graph_revision, \
                dependency_graph_digest, dependency_lock, installed_at, namespace_instance_id\
             ) VALUES (?1, 'tenant', ?2, 'registry.example', 'modules/recovery', ?3, 'theme_manager', '1.0.1', 'rhai', \
                'rustok:module/runtime@1', ?4, 'main', ?5, ?6, ?7, 1, ?8, '{}', '2026-08-13T00:00:00Z', ?9)",
            vec![
                conflicting_installation_id.to_string().into(),
                tenant_id.to_string().into(),
                format!("sha256:{}", "1".repeat(64)).into(),
                conflicting_descriptor.artifact_digest.clone().into(),
                sea_orm::Value::Json(Some(Box::new(
                    serde_json::to_value(&conflicting_descriptor)
                        .expect("conflicting descriptor JSON"),
                ))),
                data_owner_id.to_string().into(),
                Uuid::new_v4().to_string().into(),
                format!("sha256:{}", "2".repeat(64)).into(),
                namespace_instance_id.to_string().into(),
            ],
        ))
        .await
        .expect("conflicting active installation");
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_admissions (stage_id, installation_id, payload_digest, media_type, size_bytes, verification_evidence, status, revision, committed_at) VALUES (?1, ?2, ?3, 'application/vnd.rustok.rhai', 1, '{}', 'active', 1, '2026-08-13T00:00:00Z')",
            vec![
                Uuid::new_v4().to_string().into(),
                conflicting_installation_id.to_string().into(),
                format!("sha256:{}", "3".repeat(64)).into(),
            ],
        ))
        .await
        .expect("conflicting active admission");
    let conflicting_data_preview = data_preview_service
        .preview(tenant_id, installation_id)
        .await
        .expect("conflicting data preview");
    assert!(!conflicting_data_preview.can_purge);
    assert!(matches!(
        data_purge_service.purge(data_purge_req.clone()).await,
        Err(ArtifactDataError::PurgePrecondition)
    ));
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE module_artifact_admissions SET status = 'inactive' WHERE installation_id = ?1",
            vec![conflicting_installation_id.to_string().into()],
        ))
        .await
        .expect("retire conflicting installation");

    // Another owner can serve the same display slug without sharing bytes.
    let foreign_installation_id = Uuid::new_v4();
    let foreign_owner_id = Uuid::new_v4();
    let foreign_namespace_id = Uuid::new_v4();
    database.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
        "INSERT INTO module_artifact_installations
         (installation_id, scope_kind, tenant_id, registry, repository, manifest_digest, slug, version, payload_kind,
          runtime_abi, payload_digest, entrypoint, descriptor, data_owner_id, settings_instance_id,
          dependency_graph_revision, dependency_graph_digest, dependency_lock, installed_at, namespace_instance_id)
         SELECT ?1, scope_kind, tenant_id, registry, repository, ?5, slug, version, payload_kind,
                runtime_abi, payload_digest, entrypoint, descriptor, ?2, ?6,
                dependency_graph_revision, dependency_graph_digest, dependency_lock, installed_at, ?3
         FROM module_artifact_installations WHERE installation_id = ?4",
        vec![foreign_installation_id.to_string().into(), foreign_owner_id.to_string().into(),
            foreign_namespace_id.to_string().into(), conflicting_installation_id.to_string().into(),
            format!("sha256:{}", "4".repeat(64)).into(), Uuid::new_v4().to_string().into()]
    )).await.expect("foreign owner installation fixture");
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_data_namespaces
         (tenant_id, data_owner_id, namespace_instance_id, module_slug, data_contract_revision,
          data_contract_digest, state, namespace_revision, purged_at, created_at, updated_at)
         SELECT tenant_id, ?2, ?3, module_slug, data_contract_revision,
                data_contract_digest, 'serving', 1, NULL, created_at, updated_at
         FROM module_artifact_data_namespaces WHERE tenant_id = ?1 AND data_owner_id = ?4",
            vec![
                tenant_id.to_string().into(),
                foreign_owner_id.to_string().into(),
                foreign_namespace_id.to_string().into(),
                data_owner_id.to_string().into(),
            ],
        ))
        .await
        .expect("foreign owner namespace fixture");
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_data_owner_references VALUES (?1, ?2, ?3, 1)",
            vec![
                tenant_id.to_string().into(),
                foreign_owner_id.to_string().into(),
                foreign_namespace_id.to_string().into(),
            ],
        ))
        .await
        .expect("foreign owner reference fixture");
    database.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
        "INSERT INTO module_artifact_admissions
         (stage_id, installation_id, payload_digest, media_type, size_bytes, verification_evidence, status, revision, committed_at)
         SELECT ?2, ?3, payload_digest, media_type, size_bytes, verification_evidence, 'active', 1, committed_at
         FROM module_artifact_admissions WHERE installation_id = ?1",
        vec![conflicting_installation_id.to_string().into(), Uuid::new_v4().to_string().into(), foreign_installation_id.to_string().into()]
    )).await.expect("foreign owner serving admission fixture");
    database.execute_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
        "INSERT INTO module_artifact_data (tenant_id, data_owner_id, namespace_instance_id, data_key,
                                          value, value_size_bytes, revision, updated_at)
         VALUES (?1, ?2, ?3, 'theme', ?4, length(?4), 1, '2026-09-13T00:00:00Z')",
        vec![tenant_id.to_string().into(), foreign_owner_id.to_string().into(), foreign_namespace_id.to_string().into(),
            serde_json::json!({"theme": "foreign-owner"}).to_string().into()]
    )).await.expect("foreign owner value fixture");

    let retired_data_preview = data_preview_service
        .preview(tenant_id, installation_id)
        .await
        .expect("retired data preview");
    assert!(retired_data_preview.can_purge);
    assert_eq!(retired_data_preview.namespace_revision, 1);
    let mut denied_purge = data_purge_req.clone();
    denied_purge.context = command_context(tenant_id, Uuid::new_v4());
    assert_eq!(
        data_purge_service.purge(denied_purge).await,
        Err(ArtifactDataError::PolicyDenied)
    );
    let after_denial = data_preview_service
        .preview(tenant_id, installation_id)
        .await
        .unwrap();
    assert!(after_denial.can_purge);
    assert_eq!(after_denial.namespace_revision, 1);

    // 4. Now that installation is retired, creating recovery point succeeds!
    let recovery_pt = recovery_service
        .create_recovery_point(recovery_req)
        .await
        .expect("create recovery point");

    assert_eq!(recovery_pt.data_owner_id, data_owner_id);
    let eligible_settings_preview = settings_preview_service
        .preview(tenant_id, installation_id)
        .await
        .expect("eligible settings preview");
    assert!(eligible_settings_preview.can_purge);
    assert_eq!(
        eligible_settings_preview.recovery_point_id,
        Some(recovery_pt.recovery_point_id)
    );

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

    // 6. Data purge uses the same exact installation ID and succeeds only
    // after the owner lifecycle fence has become retired.
    let data_purge_res = data_purge_service
        .purge(data_purge_req.clone())
        .await
        .expect("data purge succeeds");

    assert_eq!(data_purge_res.namespace_revision, 2);
    assert_eq!(data_purge_res.purged_records, 2);
    assert_eq!(
        data_purge_service
            .purge(data_purge_req)
            .await
            .expect("exact terminal replay"),
        data_purge_res
    );

    let foreign_value = database
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT data.value, namespace.purged_at, admission.status
         FROM module_artifact_data data JOIN module_artifact_data_namespaces namespace
           ON namespace.tenant_id = data.tenant_id AND namespace.data_owner_id = data.data_owner_id
          AND namespace.namespace_instance_id = data.namespace_instance_id
         JOIN module_artifact_admissions admission ON admission.installation_id = ?4
         WHERE data.tenant_id = ?1 AND data.data_owner_id = ?2 AND data.namespace_instance_id = ?3",
            vec![
                tenant_id.to_string().into(),
                foreign_owner_id.to_string().into(),
                foreign_namespace_id.to_string().into(),
                foreign_installation_id.to_string().into(),
            ],
        ))
        .await
        .expect("foreign data query")
        .expect("foreign owner data must survive purge");
    assert_eq!(
        foreign_value.try_get::<String>("", "status").unwrap(),
        "active"
    );
    assert!(
        foreign_value
            .try_get::<Option<String>>("", "purged_at")
            .unwrap()
            .is_none()
    );
    assert_eq!(
        foreign_value
            .try_get::<serde_json::Value>("", "value")
            .unwrap(),
        serde_json::json!({"theme": "foreign-owner"})
    );

    for sql in [
        "UPDATE module_artifact_data_namespaces SET purged_at = NULL WHERE tenant_id = ?1",
        "UPDATE module_artifact_data_namespaces SET namespace_revision = namespace_revision + 1 WHERE tenant_id = ?1",
        "DELETE FROM module_artifact_data_namespaces WHERE tenant_id = ?1",
    ] {
        assert!(
            database
                .execute_raw(Statement::from_sql_and_values(
                    DbBackend::Sqlite,
                    sql,
                    vec![tenant_id.to_string().into()],
                ))
                .await
                .is_err(),
            "direct storage mutation must not change a purged namespace: {sql}"
        );
    }
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE module_artifact_data_namespaces SET namespace_revision = namespace_revision WHERE tenant_id = ?1",
            vec![tenant_id.to_string().into()],
        ))
        .await
        .expect("an unchanged write may acquire the SQLite lifecycle lock");
    let tombstone = database
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT namespace_revision, purged_at FROM module_artifact_data_namespaces WHERE tenant_id = ?1 AND data_owner_id = ?2",
            vec![tenant_id.to_string().into(), data_owner_id.to_string().into()],
        ))
        .await
        .expect("tombstone query")
        .expect("tombstone must remain present");
    assert_eq!(
        tombstone.try_get::<i64>("", "namespace_revision").unwrap(),
        2
    );
    assert!(
        tombstone
            .try_get::<Option<String>>("", "purged_at")
            .unwrap()
            .is_some()
    );
    let data_recovery = ArtifactDataPostPurgeRecoveryService::new(
        database.clone(),
        storage.clone(),
        recovery_policy.clone(),
        recovery_policy,
    );
    let prepare = PrepareRecoveryRequest {
        installation_id,
        expected_reference_revision: 1,
        expected_tombstone_revision: 2,
        source_snapshot_id: snapshot.snapshot_id,
        context: command_context(tenant_id, actor_id),
        reason: "Restore exact retired owner".into(),
    };
    let staged = data_recovery
        .prepare_recovery(prepare.clone())
        .await
        .expect("restore and verify actual recovery target");
    assert_eq!(staged.data_owner_id, data_owner_id);
    assert_eq!(staged.source_namespace_instance_id, namespace_instance_id);
    assert_ne!(staged.namespace_instance_id, namespace_instance_id);
    assert_eq!(staged.target_namespace_revision, 2);
    assert_eq!(staged.records_restored, 1);
    assert_eq!(staged.objects_restored, 1);
    assert_eq!(
        data_recovery
            .prepare_recovery(prepare.clone())
            .await
            .expect("exact prepared replay"),
        staged
    );
    let row = database.query_one_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
        "SELECT namespace.state,data.value,reference.namespace_instance_id AS active_instance
         FROM module_artifact_data_namespaces namespace
         JOIN module_artifact_data data USING (tenant_id,data_owner_id,namespace_instance_id)
         JOIN module_artifact_data_owner_references reference USING (tenant_id,data_owner_id)
         WHERE namespace.tenant_id=?1 AND namespace.data_owner_id=?2 AND namespace.namespace_instance_id=?3",
        vec![tenant_id.to_string().into(),data_owner_id.to_string().into(),staged.namespace_instance_id.to_string().into()]))
        .await.expect("restored target SQL").expect("actual target row");
    assert_eq!(row.try_get::<String>("", "state").unwrap(), "verified");
    assert_eq!(
        row.try_get::<serde_json::Value>("", "value").unwrap(),
        serde_json::json!({"theme":"ocean"})
    );
    assert_eq!(
        row.try_get::<String>("", "active_instance").unwrap(),
        namespace_instance_id.to_string()
    );
    let cutover = PostPurgeRecoveryCutoverRequest {
        recovery_id: staged.recovery_id,
        expected_reference_revision: 1,
        expected_tombstone_revision: 2,
        expected_target_namespace_revision: staged.target_namespace_revision,
        verified_manifest_digest: staged.verified_manifest_digest.clone(),
        context: command_context(tenant_id, actor_id),
        reason: "Authorize exact verified reference".into(),
    };
    let mut denied = cutover.clone();
    denied.context.actor_id = Uuid::new_v4();
    assert_eq!(
        data_recovery.execute_cas_cutover(denied).await.unwrap_err(),
        PostPurgeRecoveryError::AuthorizationDenied
    );
    let mut stale = cutover.clone();
    stale.expected_reference_revision = 2;
    assert_eq!(
        data_recovery.execute_cas_cutover(stale).await.unwrap_err(),
        PostPurgeRecoveryError::CasCutoverConflict
    );
    let row = database
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT storage_key FROM module_artifact_data_objects
         WHERE tenant_id=?1 AND data_owner_id=?2 AND namespace_instance_id=?3",
            vec![
                tenant_id.to_string().into(),
                data_owner_id.to_string().into(),
                staged.namespace_instance_id.to_string().into(),
            ],
        ))
        .await
        .expect("target object SQL")
        .expect("target object row");
    let target_key: String = row.try_get("", "storage_key").unwrap();
    assert_eq!(
        storage
            .objects
            .get(&Path::from(target_key.as_str()))
            .await
            .unwrap()
            .bytes()
            .await
            .unwrap(),
        object_bytes
    );
    storage
        .objects
        .put(
            &Path::from(target_key.as_str()),
            bytes::Bytes::from_static(b"corrupt target").into(),
        )
        .await
        .expect("corrupt bounded target fixture");
    assert_eq!(
        data_recovery
            .execute_cas_cutover(cutover.clone())
            .await
            .unwrap_err(),
        PostPurgeRecoveryError::Integrity
    );
    let row=database.query_one_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
        "SELECT namespace_instance_id FROM module_artifact_data_owner_references WHERE tenant_id=?1 AND data_owner_id=?2",
        vec![tenant_id.to_string().into(),data_owner_id.to_string().into()])).await.unwrap().unwrap();
    assert_eq!(
        row.try_get::<String>("", "namespace_instance_id").unwrap(),
        namespace_instance_id.to_string()
    );
    storage
        .objects
        .put(&Path::from(target_key), object_bytes.into())
        .await
        .expect("repair bounded target fixture");
    let activated = data_recovery
        .execute_cas_cutover(cutover.clone())
        .await
        .expect("separately authorized reference CAS");
    assert_eq!(
        activated.namespace_instance_id,
        staged.namespace_instance_id
    );
    assert_eq!(activated.active_reference_revision, 2);
    assert_eq!(activated.active_namespace_revision, 3);
    // Later lifecycle changes cannot invalidate exact terminal receipt replay.
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "UPDATE module_artifact_admissions SET status='active' WHERE installation_id=?1",
            vec![installation_id.to_string().into()],
        ))
        .await
        .expect("later lifecycle");
    assert_eq!(
        data_recovery
            .prepare_recovery(prepare.clone())
            .await
            .expect("prepared receipt after cutover/lifecycle"),
        staged
    );
    assert_eq!(
        data_recovery
            .execute_cas_cutover(cutover.clone())
            .await
            .expect("terminal cutover replay"),
        activated
    );
    let mut conflict = cutover;
    conflict.context.correlation_id = Uuid::new_v4();
    assert_eq!(
        data_recovery
            .execute_cas_cutover(conflict)
            .await
            .unwrap_err(),
        PostPurgeRecoveryError::IdempotencyConflict
    );
    let mut conflict = prepare;
    conflict.reason = "Changed prepared command".into();
    assert_eq!(
        data_recovery.prepare_recovery(conflict).await.unwrap_err(),
        PostPurgeRecoveryError::IdempotencyConflict
    );
    let row=database.query_one_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
        "SELECT reference.namespace_instance_id,namespace.state,namespace.namespace_revision,
          (SELECT count(*) FROM module_artifact_data_snapshot_holds hold WHERE hold.tenant_id=reference.tenant_id
           AND hold.holder_kind='recovery' AND hold.released_at IS NULL) AS active_holds
         FROM module_artifact_data_owner_references reference
         JOIN module_artifact_data_namespaces namespace USING (tenant_id,data_owner_id,namespace_instance_id)
         WHERE reference.tenant_id=?1 AND reference.data_owner_id=?2",
        vec![tenant_id.to_string().into(),data_owner_id.to_string().into()])).await.expect("final reference SQL").expect("final reference");
    assert_eq!(
        row.try_get::<String>("", "namespace_instance_id").unwrap(),
        staged.namespace_instance_id.to_string()
    );
    assert_eq!(row.try_get::<String>("", "state").unwrap(), "serving");
    assert_eq!(row.try_get::<i64>("", "active_holds").unwrap(), 0);
    let row = database
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "SELECT namespace_revision,purged_at FROM module_artifact_data_namespaces
         WHERE tenant_id=?1 AND data_owner_id=?2 AND namespace_instance_id=?3",
            vec![
                tenant_id.to_string().into(),
                data_owner_id.to_string().into(),
                namespace_instance_id.to_string().into(),
            ],
        ))
        .await
        .expect("permanent source SQL")
        .expect("permanent source tombstone");
    assert_eq!(row.try_get::<i64>("", "namespace_revision").unwrap(), 2);
    assert!(
        row.try_get::<Option<String>>("", "purged_at")
            .unwrap()
            .is_some()
    );
    for sql in [
        "DELETE FROM module_artifact_data_namespace_recovery_operations WHERE recovery_id=?1",
        "UPDATE module_artifact_data_namespace_recovery_operations SET status='verified' WHERE recovery_id=?1",
        "UPDATE module_artifact_data_namespace_recovery_operations SET objects_restored=objects_restored+1 WHERE recovery_id=?1",
    ] {
        assert!(
            database
                .execute_raw(Statement::from_sql_and_values(
                    DbBackend::Sqlite,
                    sql,
                    vec![staged.recovery_id.to_string().into()]
                ))
                .await
                .is_err(),
            "immutable receipt: {sql}"
        );
    }
    drop(data_recovery);
    drop(snapshot_service);
    drop(storage);
    let absolute_root = storage_root.canonicalize().expect("absolute isolated root");
    assert!(absolute_root.starts_with(&storage_parent));
    std::fs::remove_dir_all(absolute_root).expect("remove isolated fixture storage");
}
