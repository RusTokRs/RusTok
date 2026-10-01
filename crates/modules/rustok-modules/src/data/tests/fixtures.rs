//! Shared fixtures and mocks for artifact data test suites.
#![allow(unused_imports)]

pub(crate) use std::{
    collections::HashMap,
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, Ordering},
    },
};

pub(crate) use async_trait::async_trait;
pub(crate) use bytes::Bytes;
pub(crate) use rustok_core::MigrationSource;
pub(crate) use rustok_sandbox::{
    CapabilityCall, CapabilityCallContext, CapabilityName, ExecutionPhase, SandboxSubject,
};
pub(crate) use rustok_storage::{LocalStorageConfig, StorageRuntime};
pub(crate) use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DatabaseTransaction, DbBackend, EntityTrait,
    QueryResult, Statement, TransactionTrait,
};
pub(crate) use sea_orm_migration::MigrationTrait;
pub(crate) use serde_json::{Value, json};
pub(crate) use uuid::Uuid;

pub(crate) use crate::{
    ArtifactModuleKind, ArtifactPayloadKind, ArtifactPersistenceContract, ArtifactSchemaDocument,
    ModuleArtifactDescriptor, ModuleBindingIdempotency, ModulesModule, canonical_schema_digest,
};

pub(crate) use super::super::*;
pub(crate) use super::super::broker::*;
pub(crate) use super::super::capabilities::*;
pub(crate) use super::super::constants::*;
pub(crate) use super::super::error::*;
pub(crate) use super::super::export::*;
pub(crate) use super::super::gc::*;
pub(crate) use super::super::helpers::*;
pub(crate) use super::super::object_capabilities::*;
pub(crate) use super::super::objects::*;
pub(crate) use super::super::objects_persistence::*;
pub(crate) use super::super::purge::*;
pub(crate) use super::super::structured_persistence::*;
pub(crate) use super::super::traits::*;
pub(crate) use super::super::types::*;
pub(crate) use super::super::upgrade::*;
pub(crate) use super::super::upload::*;
pub(crate) use super::super::upload_sessions::*;
pub(crate) use super::super::validation::*;

#[derive(Clone)]
pub(crate) struct CompletedPageBroker {
    pub(crate) completed: Arc<AtomicBool>,
}

#[async_trait]
impl ArtifactDataBroker for CompletedPageBroker {
    async fn get(
        &self,
        _: &ArtifactDataScope,
        _: &str,
    ) -> Result<Option<ArtifactDataRecord>, ArtifactDataError> {
        Err(ArtifactDataError::Storage(
            "not used by upgrade planning".to_string(),
        ))
    }

    async fn put(
        &self,
        _: &ArtifactDataScope,
        _: ArtifactDataWrite,
    ) -> Result<ArtifactDataRecord, ArtifactDataError> {
        Err(ArtifactDataError::Storage(
            "not used by upgrade planning".to_string(),
        ))
    }

    async fn put_batch(
        &self,
        _: &ArtifactDataScope,
        _: ArtifactDataBatchWrite,
    ) -> Result<Vec<ArtifactDataRecord>, ArtifactDataError> {
        Err(ArtifactDataError::Storage(
            "not used by upgrade planning".to_string(),
        ))
    }

    async fn delete(
        &self,
        _: &ArtifactDataScope,
        _: ArtifactDataDeleteRequest,
    ) -> Result<ArtifactDataDeleteResult, ArtifactDataError> {
        Err(ArtifactDataError::Storage(
            "not used by upgrade planning".to_string(),
        ))
    }

    async fn list(
        &self,
        _: &ArtifactDataScope,
        _: ArtifactDataPageRequest,
    ) -> Result<ArtifactDataPage, ArtifactDataError> {
        self.completed.store(true, Ordering::SeqCst);
        Ok(ArtifactDataPage {
            records: vec![ArtifactDataRecord {
                key: "state/current".to_string(),
                value: json!({ "version": 1 }),
                revision: 7,
            }],
            next_after_key: Some("state/current".to_string()),
        })
    }
}

#[derive(Clone)]
pub(crate) struct UpgradeHook {
    pub(crate) read_completed: Arc<AtomicBool>,
}

#[async_trait]
impl ArtifactDataUpgradeHook for UpgradeHook {
    async fn transform_data(
        &self,
        hook_binding_id: &str,
        input: ArtifactDataUpgradeInput,
    ) -> Result<Value, ArtifactDataError> {
        assert!(self.read_completed.load(Ordering::SeqCst));
        assert_eq!(hook_binding_id, "upgrade.v2");
        assert_eq!(input.record.key, "state/current");
        Ok(json!({ "version": 2 }))
    }
}

#[derive(Clone)]
pub(crate) struct AcceptingSchemaValidator;

#[async_trait]
impl ArtifactDataSchemaValidator for AcceptingSchemaValidator {
    async fn validate_data_value(
        &self,
        scope: &ArtifactDataScope,
        value: &Value,
    ) -> Result<(), ArtifactDataError> {
        assert_eq!(scope.data_contract_revision, 2);
        assert_eq!(value, &json!({ "version": 2 }));
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct AllowSchemaValidator;

#[async_trait]
impl ArtifactDataSchemaValidator for AllowSchemaValidator {
    async fn validate_data_value(
        &self,
        _: &ArtifactDataScope,
        _: &Value,
    ) -> Result<(), ArtifactDataError> {
        Ok(())
    }
}

#[derive(Clone)]
pub(crate) struct DataPurgeFixturePolicy {
    pub(crate) scope: ArtifactDataScope,
    pub(crate) actor_id: Uuid,
    pub(crate) installation_id: Uuid,
}

#[async_trait]
impl ArtifactDataPurgeAuthorizer for DataPurgeFixturePolicy {
    async fn authorize_purge_on(
        &self,
        transaction: &DatabaseTransaction,
        request: &ArtifactDataPurgeRequest,
        owner: &ArtifactDataPurgeAuthorizationContext,
    ) -> Result<(), ArtifactDataError> {
        if owner.scope != self.scope
            || request.context.actor_id != self.actor_id
            || request.installation_id != self.installation_id
            || owner.installation_id != self.installation_id
            || request.context.tenant_id != Some(self.scope.tenant_id)
        {
            return Err(ArtifactDataError::PolicyDenied);
        }
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                transaction.get_database_backend(),
                "SELECT 1 FROM module_artifact_data_namespaces
             WHERE tenant_id=?1 AND data_owner_id=?2 AND namespace_instance_id=?3
               AND purged_at IS NULL AND namespace_revision=?4",
                vec![
                    self.scope.tenant_id.to_string().into(),
                    self.scope.data_owner_id.to_string().into(),
                    self.scope.namespace_instance_id.to_string().into(),
                    revision_value(request.expected_namespace_revision)?,
                ],
            ))
            .await
            .map_err(storage_error)?;
        if row.is_none() {
            return Err(ArtifactDataError::PolicyDenied);
        }
        Ok(())
    }
}

pub(crate) async fn seed_retired_data_purge_installation(
    database: &DatabaseConnection,
    scope: &ArtifactDataScope,
) -> Uuid {
    let installation_id = Uuid::new_v4();
    let schema = json!({
        "$schema": "https://json-schema.org/draft/2020-12/schema",
        "type": "object",
    });
    let schema_digest = canonical_schema_digest(&schema);
    let descriptor = ModuleArtifactDescriptor {
        schema_version: crate::MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
        slug: scope.module_slug.clone(),
        version: "1.0.0".to_string(),
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
        settings_schema_digest: None,
        data_schema_digest: Some(schema_digest.clone()),
        localization_catalogs: Vec::new(),
        ui_contributions: Vec::new(),
        persistence_contract: Some(ArtifactPersistenceContract {
            revision: scope.data_contract_revision,
            schema_digest,
            indexes: Vec::new(),
        }),
    };
    descriptor.validate().expect("valid data purge descriptor");
    let actor_id = Uuid::new_v4();
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_installations (\
                installation_id, scope_kind, tenant_id, registry, repository, manifest_digest, slug, version, payload_kind, \
                runtime_abi, payload_digest, entrypoint, descriptor, data_owner_id, settings_instance_id, namespace_instance_id, dependency_graph_revision, \
                dependency_graph_digest, dependency_lock, capability_grant_revision, installed_at\
             ) VALUES (?1, 'tenant', ?2, 'registry.example', 'modules/purge', ?3, ?4, '1.0.0', 'rhai', \
                'rustok:module/runtime@1', ?5, 'main', ?6, ?7, ?8, ?11, 1, ?9, '{}', ?10, '2026-09-01T00:00:00Z')"
                .to_string(),
            vec![
                installation_id.to_string().into(),
                scope.tenant_id.to_string().into(),
                format!("sha256:{}", "b".repeat(64)).into(),
                scope.module_slug.clone().into(),
                descriptor.artifact_digest.clone().into(),
                sea_orm::Value::Json(Some(Box::new(
                    serde_json::to_value(&descriptor).expect("descriptor value"),
                ))),
                scope.data_owner_id.to_string().into(),
                Uuid::new_v4().to_string().into(),
                format!("sha256:{}", "c".repeat(64)).into(),
                i64::try_from(scope.policy_revision)
                    .expect("policy revision fits SQLite integer")
                    .into(),
                scope.namespace_instance_id.to_string().into(),
            ],
        ))
        .await
        .expect("retired data purge installation");
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_admissions (\
                stage_id, installation_id, payload_digest, media_type, size_bytes, verification_evidence, status, revision, committed_at\
             ) VALUES (?1, ?2, ?3, 'application/vnd.rustok.rhai', 1, '{}', 'inactive', 1, '2026-09-01T00:00:00Z')"
                .to_string(),
            vec![
                Uuid::new_v4().to_string().into(),
                installation_id.to_string().into(),
                format!("sha256:{}", "d".repeat(64)).into(),
            ],
        ))
        .await
        .expect("retired data purge admission");
    database
        .execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_uninstall_operations \
             (operation_id, installation_id, expected_revision, actor_id, trace_id, correlation_id, reason, idempotency_key, committed_at) \
             VALUES (?1, ?2, 1, ?3, 'test:artifact-data-purge', ?4, 'retired data purge fixture', ?5, '2026-09-01T00:00:00Z')"
                .to_string(),
            vec![
                Uuid::new_v4().to_string().into(),
                installation_id.to_string().into(),
                actor_id.to_string().into(),
                Uuid::new_v4().to_string().into(),
                Uuid::new_v4().to_string().into(),
            ],
        ))
        .await
        .expect("retired data purge uninstall evidence");
    installation_id
}

#[derive(Clone)]
pub(crate) struct AllowExportAuthorizer;

#[async_trait]
impl ArtifactDataExportAuthorizer for AllowExportAuthorizer {
    async fn authorize_export(
        &self,
        _: &ArtifactDataExportRequest,
    ) -> Result<(), ArtifactDataError> {
        Ok(())
    }
}

pub(crate) type UpgradeBindingCall = (String, String, ExecutionPhase, Value);

#[derive(Clone)]
pub(crate) struct RecordingUpgradeBindingExecutor {
    pub(crate) calls: Arc<Mutex<Vec<UpgradeBindingCall>>>,
}

#[async_trait]
impl ArtifactBindingExecutor for RecordingUpgradeBindingExecutor {
    fn supports_payload_kind(&self, _payload_kind: crate::ArtifactPayloadKind) -> bool {
        true
    }

    async fn dispatch_binding(
        &self,
        dispatch: ArtifactBindingDispatch<'_>,
    ) -> Result<Value, String> {
        self.calls.lock().expect("calls lock").push((
            dispatch.release.slug.clone(),
            dispatch.binding.id.clone(),
            dispatch.phase,
            dispatch.input,
        ));
        Ok(json!({ "version": 2 }))
    }
}

#[derive(Clone)]
pub(crate) struct UpgradeApplyBroker {
    pub(crate) source: ArtifactDataRecord,
    pub(crate) target: Arc<Mutex<HashMap<String, (ArtifactDataRecord, Uuid)>>>,
}

#[async_trait]
impl ArtifactDataBroker for UpgradeApplyBroker {
    async fn get(
        &self,
        scope: &ArtifactDataScope,
        key: &str,
    ) -> Result<Option<ArtifactDataRecord>, ArtifactDataError> {
        if scope.data_contract_revision == 1 {
            return Ok((key == self.source.key).then(|| self.source.clone()));
        }
        Ok(self
            .target
            .lock()
            .expect("target lock")
            .get(key)
            .map(|(record, _)| record.clone()))
    }

    async fn put(
        &self,
        scope: &ArtifactDataScope,
        write: ArtifactDataWrite,
    ) -> Result<ArtifactDataRecord, ArtifactDataError> {
        assert_eq!(scope.data_contract_revision, 2);
        assert!(write.create_only);
        let mut target = self.target.lock().expect("target lock");
        if let Some((record, idempotency_key)) = target.get(&write.key) {
            if *idempotency_key == write.idempotency_key
                && record.value == write.value
                && write.expected_revision.is_none()
            {
                return Ok(record.clone());
            }
            return Err(ArtifactDataError::RevisionConflict);
        }
        let record = ArtifactDataRecord {
            key: write.key.clone(),
            value: write.value,
            revision: 1,
        };
        target.insert(write.key, (record.clone(), write.idempotency_key));
        Ok(record)
    }

    async fn put_batch(
        &self,
        _: &ArtifactDataScope,
        _: ArtifactDataBatchWrite,
    ) -> Result<Vec<ArtifactDataRecord>, ArtifactDataError> {
        Err(ArtifactDataError::Storage(
            "not used by upgrade application".to_string(),
        ))
    }

    async fn delete(
        &self,
        _: &ArtifactDataScope,
        _: ArtifactDataDeleteRequest,
    ) -> Result<ArtifactDataDeleteResult, ArtifactDataError> {
        Err(ArtifactDataError::Storage(
            "not used by upgrade application".to_string(),
        ))
    }

    async fn list(
        &self,
        _: &ArtifactDataScope,
        _: ArtifactDataPageRequest,
    ) -> Result<ArtifactDataPage, ArtifactDataError> {
        Err(ArtifactDataError::Storage(
            "not used by upgrade application".to_string(),
        ))
    }
}

#[derive(Clone)]
pub(crate) struct RecordingCheckpointStore {
    pub(crate) requests: Arc<Mutex<Vec<ArtifactMigrationCheckpointRequest>>>,
    pub(crate) fail_first: Arc<AtomicBool>,
}

impl Default for RecordingCheckpointStore {
    fn default() -> Self {
        Self {
            requests: Arc::new(Mutex::new(Vec::new())),
            fail_first: Arc::new(AtomicBool::new(true)),
        }
    }
}

#[async_trait]
impl ArtifactDataMigrationCheckpointStore for RecordingCheckpointStore {
    async fn record_data_upgrade_checkpoint(
        &self,
        request: ArtifactMigrationCheckpointRequest,
    ) -> Result<u64, ArtifactDataError> {
        if self.fail_first.swap(false, Ordering::SeqCst) {
            return Err(ArtifactDataError::MigrationCheckpoint(
                "simulated retryable checkpoint failure".to_string(),
            ));
        }
        let revision = request.expected_revision + 1;
        self.requests.lock().expect("checkpoint lock").push(request);
        Ok(revision)
    }
}

pub(crate) fn test_scope(tenant_id: Uuid, module_slug: &str) -> ArtifactDataScope {
    ArtifactDataScope {
        tenant_id,
        data_owner_id: Uuid::new_v4(),
        namespace_instance_id: Uuid::new_v4(),
        data_contract_digest:
            "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef".to_string(),
        module_slug: module_slug.to_string(),
        data_contract_revision: 1,
        policy_revision: 1,
    }
}

pub(crate) async fn setup_test_serving_namespace<C: ConnectionTrait>(
    connection: &C,
    scope: &ArtifactDataScope,
) {
    let backend = connection.get_database_backend();
    connection
        .execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_artifact_data_namespaces
                     (tenant_id, data_owner_id, namespace_instance_id, module_slug,
                      data_contract_revision, data_contract_digest, state,
                      namespace_revision, created_at, updated_at)
                     VALUES ({}, {}, {}, {}, {}, {}, 'serving', 1, {}, {})",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
                placeholder(backend, 4),
                placeholder(backend, 5),
                placeholder(backend, 6),
                now_expression(backend),
                now_expression(backend),
            ),
            vec![
                uuid_value(scope.tenant_id, backend),
                uuid_value(scope.data_owner_id, backend),
                uuid_value(scope.namespace_instance_id, backend),
                scope.module_slug.clone().into(),
                revision_value(scope.data_contract_revision).expect("revision"),
                scope.data_contract_digest.clone().into(),
            ],
        ))
        .await
        .expect("insert test namespace");
    connection
        .execute_raw(Statement::from_sql_and_values(
            backend,
            format!(
                "INSERT INTO module_artifact_data_owner_references
                     (tenant_id, data_owner_id, namespace_instance_id, reference_revision)
                     VALUES ({}, {}, {}, 1)",
                placeholder(backend, 1),
                placeholder(backend, 2),
                placeholder(backend, 3),
            ),
            vec![
                uuid_value(scope.tenant_id, backend),
                uuid_value(scope.data_owner_id, backend),
                uuid_value(scope.namespace_instance_id, backend),
            ],
        ))
        .await
        .expect("insert test owner reference");
}
