use std::{sync::Arc, time::Duration as StdDuration};

use async_trait::async_trait;
use chrono::{Duration, Utc};
use rustok_api::{
    Action, ModuleWorkError, ModuleWorkHandler, ModuleWorkItem, ModuleWorkOutcome,
    ModuleWorkSource, Permission, PortActor, PortContext, Resource,
};
use rustok_runtime::{HostRuntimeContext, ModuleWorkRegistration, ModuleWorkScheduler};
use rustok_storage::StorageRuntime;
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, QuerySelect,
    QueryTrait,
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    PurgeMemoryEntryInput, TombstoneMemoryEntryInput, TranslationError, TranslationMemoryService,
    entities::{exchange_job, machine_memory_binding, memory_entry},
    exchange::{expire_interchange_artifact, next_expired_interchange_artifact},
};

pub const TRANSLATION_MEMORY_RETENTION_WORKER: &str = "translation_memory_retention";
pub const TRANSLATION_INTERCHANGE_ARTIFACT_EXPIRY_WORKER: &str =
    "translation_interchange_artifact_expiry";
const PURGE_GRACE_HOURS: i64 = 24;

#[derive(Clone, Copy, Debug, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
enum RetentionAction {
    Tombstone,
    Purge,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
struct RetentionWorkPayload {
    action: RetentionAction,
    expected_revision: i64,
}

#[derive(Clone)]
pub struct TranslationMemoryRetentionWorkAdapter {
    database: DatabaseConnection,
}

impl TranslationMemoryRetentionWorkAdapter {
    pub fn new(database: DatabaseConnection) -> Self {
        Self { database }
    }

    pub async fn register_with(
        self,
        scheduler: &ModuleWorkScheduler,
    ) -> Result<(), ModuleWorkError> {
        let adapter = Arc::new(self);
        scheduler.register(adapter.clone(), adapter).await
    }

    async fn next_tombstone_candidate(
        &self,
    ) -> Result<Option<memory_entry::Model>, ModuleWorkError> {
        let now = Utc::now().fixed_offset();
        memory_entry::Entity::find()
            .filter(memory_entry::Column::TombstonedAt.is_null())
            .filter(
                Condition::any()
                    .add(
                        Condition::all()
                            .add(memory_entry::Column::RetentionPolicy.eq("retain_until"))
                            .add(memory_entry::Column::RetainUntil.lte(now)),
                    )
                    .add(
                        Condition::all()
                            .add(memory_entry::Column::RetentionPolicy.eq("owner_lifecycle"))
                            .add(memory_entry::Column::OwnerDeletedAt.is_not_null()),
                    ),
            )
            .order_by_asc(memory_entry::Column::UpdatedAt)
            .order_by_asc(memory_entry::Column::Id)
            .one(&self.database)
            .await
            .map_err(|error| ModuleWorkError::Source(error.to_string()))
    }

    async fn next_purge_candidate(&self) -> Result<Option<memory_entry::Model>, ModuleWorkError> {
        let now = Utc::now().fixed_offset();
        let purge_before = now - Duration::hours(PURGE_GRACE_HOURS);
        let pinned_memory_entries = machine_memory_binding::Entity::find()
            .select_only()
            .column(machine_memory_binding::Column::MemoryEntryId)
            .into_query();

        memory_entry::Entity::find()
            .filter(memory_entry::Column::TombstonedAt.lte(purge_before))
            .filter(
                Condition::any()
                    .add(
                        Condition::all()
                            .add(memory_entry::Column::RetentionPolicy.eq("retain_until"))
                            .add(memory_entry::Column::RetainUntil.lte(now)),
                    )
                    .add(
                        Condition::all()
                            .add(memory_entry::Column::RetentionPolicy.eq("owner_lifecycle"))
                            .add(memory_entry::Column::OwnerDeletedAt.is_not_null()),
                    ),
            )
            .filter(memory_entry::Column::Id.not_in_subquery(pinned_memory_entries))
            .order_by_asc(memory_entry::Column::TombstonedAt)
            .order_by_asc(memory_entry::Column::Id)
            .one(&self.database)
            .await
            .map_err(|error| ModuleWorkError::Source(error.to_string()))
    }

    fn work_item(entry: memory_entry::Model, action: RetentionAction) -> ModuleWorkItem {
        ModuleWorkItem {
            id: entry.id,
            tenant_id: entry.tenant_id,
            worker_slug: TRANSLATION_MEMORY_RETENTION_WORKER.to_string(),
            lease_token: Uuid::new_v4().to_string(),
            payload: serde_json::to_value(RetentionWorkPayload {
                action,
                expected_revision: entry.revision,
            })
            .expect("translation retention payload must serialize"),
        }
    }

    fn payload(item: &ModuleWorkItem) -> Result<RetentionWorkPayload, ModuleWorkError> {
        serde_json::from_value(item.payload.clone()).map_err(|_| {
            ModuleWorkError::Handler(
                "translation memory retention work payload is invalid".to_string(),
            )
        })
    }

    fn worker_context(item: &ModuleWorkItem, payload: &RetentionWorkPayload) -> PortContext {
        let action = match payload.action {
            RetentionAction::Tombstone => "tombstone",
            RetentionAction::Purge => "purge",
        };
        PortContext::new(
            item.tenant_id.to_string(),
            PortActor::system(),
            "en",
            format!("translation-memory-retention:{}:{action}", item.id),
        )
        .with_claim(Permission::new(Resource::TranslationMemory, Action::Delete).to_string())
        .with_claim(Permission::new(Resource::TranslationMemory, Action::Manage).to_string())
        .with_channel("module_work")
        .with_idempotency_key(format!(
            "translation-memory-retention:{}:{action}:{}",
            item.id, payload.expected_revision
        ))
        .with_deadline(StdDuration::from_secs(30))
    }

    fn stale_transition(error: &TranslationError) -> bool {
        matches!(
            error,
            TranslationError::MemoryEntryNotFound
                | TranslationError::MemoryRevisionConflict { .. }
                | TranslationError::MemoryLifecycleConflict(_)
                | TranslationError::MemoryRetentionConflict(_)
        )
    }
}

pub(crate) struct TranslationMemoryRetentionWorkRegistration;

#[async_trait]
impl ModuleWorkRegistration for TranslationMemoryRetentionWorkRegistration {
    async fn register(
        &self,
        host: &HostRuntimeContext,
        scheduler: &ModuleWorkScheduler,
    ) -> Result<(), ModuleWorkError> {
        TranslationMemoryRetentionWorkAdapter::new(host.db_clone())
            .register_with(scheduler)
            .await
    }
}

#[async_trait]
impl ModuleWorkSource for TranslationMemoryRetentionWorkAdapter {
    async fn claim(&self, worker_slug: &str) -> Result<Option<ModuleWorkItem>, ModuleWorkError> {
        if worker_slug != TRANSLATION_MEMORY_RETENTION_WORKER {
            return Ok(None);
        }
        if let Some(entry) = self.next_tombstone_candidate().await? {
            return Ok(Some(Self::work_item(entry, RetentionAction::Tombstone)));
        }
        Ok(self
            .next_purge_candidate()
            .await?
            .map(|entry| Self::work_item(entry, RetentionAction::Purge)))
    }

    async fn complete(
        &self,
        _item: &ModuleWorkItem,
        _outcome: ModuleWorkOutcome,
    ) -> Result<(), ModuleWorkError> {
        // The entry revision and mutation receipt are the durable completion
        // evidence. Failed work remains eligible for the next scheduler pass.
        Ok(())
    }
}

#[async_trait]
impl ModuleWorkHandler for TranslationMemoryRetentionWorkAdapter {
    fn worker_slug(&self) -> &'static str {
        TRANSLATION_MEMORY_RETENTION_WORKER
    }

    async fn execute(&self, item: ModuleWorkItem) -> Result<ModuleWorkOutcome, ModuleWorkError> {
        if item.worker_slug != TRANSLATION_MEMORY_RETENTION_WORKER {
            return Err(ModuleWorkError::Handler(
                "wrong translation memory retention worker slug".to_string(),
            ));
        }
        let payload = Self::payload(&item)?;
        let service = TranslationMemoryService::new(self.database.clone());
        let result = match payload.action {
            RetentionAction::Tombstone => {
                service
                    .tombstone_entry(
                        Self::worker_context(&item, &payload),
                        TombstoneMemoryEntryInput {
                            entry_id: item.id,
                            expected_revision: payload.expected_revision,
                        },
                    )
                    .await
            }
            RetentionAction::Purge => {
                service
                    .purge_entry(
                        Self::worker_context(&item, &payload),
                        PurgeMemoryEntryInput {
                            entry_id: item.id,
                            expected_revision: payload.expected_revision,
                        },
                    )
                    .await
            }
        };
        match result {
            Ok(_) => Ok(ModuleWorkOutcome::Completed),
            Err(error) if Self::stale_transition(&error) => Ok(ModuleWorkOutcome::Completed),
            Err(error) => Err(ModuleWorkError::Handler(error.to_string())),
        }
    }
}

/// Removes expired private interchange artifacts even when their tenant has no
/// subsequent Translation request. Metadata remains as content-free audit
/// evidence after object deletion.
#[derive(Clone)]
pub struct TranslationInterchangeArtifactExpiryWorkAdapter {
    database: DatabaseConnection,
    storage: StorageRuntime,
}

impl TranslationInterchangeArtifactExpiryWorkAdapter {
    pub fn new(database: DatabaseConnection, storage: StorageRuntime) -> Self {
        Self { database, storage }
    }

    pub async fn register_with(
        self,
        scheduler: &ModuleWorkScheduler,
    ) -> Result<(), ModuleWorkError> {
        let adapter = Arc::new(self);
        scheduler.register(adapter.clone(), adapter).await
    }
}

pub(crate) struct TranslationInterchangeArtifactExpiryWorkRegistration;

#[async_trait]
impl ModuleWorkRegistration for TranslationInterchangeArtifactExpiryWorkRegistration {
    async fn register(
        &self,
        host: &HostRuntimeContext,
        scheduler: &ModuleWorkScheduler,
    ) -> Result<(), ModuleWorkError> {
        let Some(storage) = host.shared_get::<StorageRuntime>() else {
            return Ok(());
        };
        TranslationInterchangeArtifactExpiryWorkAdapter::new(host.db_clone(), storage)
            .register_with(scheduler)
            .await
    }
}

#[async_trait]
impl ModuleWorkSource for TranslationInterchangeArtifactExpiryWorkAdapter {
    async fn claim(&self, worker_slug: &str) -> Result<Option<ModuleWorkItem>, ModuleWorkError> {
        if worker_slug != TRANSLATION_INTERCHANGE_ARTIFACT_EXPIRY_WORKER {
            return Ok(None);
        }
        let candidate = next_expired_interchange_artifact(&self.database)
            .await
            .map_err(|error| ModuleWorkError::Source(error.to_string()))?;
        Ok(candidate.map(|artifact| ModuleWorkItem {
            id: artifact.id,
            tenant_id: artifact.tenant_id,
            worker_slug: TRANSLATION_INTERCHANGE_ARTIFACT_EXPIRY_WORKER.to_string(),
            lease_token: Uuid::new_v4().to_string(),
            payload: serde_json::Value::Null,
        }))
    }

    async fn complete(
        &self,
        _item: &ModuleWorkItem,
        _outcome: ModuleWorkOutcome,
    ) -> Result<(), ModuleWorkError> {
        // The storage-deleted timestamp is durable completion evidence. A
        // transient object-store failure leaves it unset and therefore
        // eligible for a later scheduler pass.
        Ok(())
    }
}

#[async_trait]
impl ModuleWorkHandler for TranslationInterchangeArtifactExpiryWorkAdapter {
    fn worker_slug(&self) -> &'static str {
        TRANSLATION_INTERCHANGE_ARTIFACT_EXPIRY_WORKER
    }

    async fn execute(&self, item: ModuleWorkItem) -> Result<ModuleWorkOutcome, ModuleWorkError> {
        if item.worker_slug != TRANSLATION_INTERCHANGE_ARTIFACT_EXPIRY_WORKER {
            return Err(ModuleWorkError::Handler(
                "wrong translation interchange artifact expiry worker slug".to_string(),
            ));
        }
        let Some(artifact) = exchange_job::Entity::find_by_id(item.id)
            .filter(exchange_job::Column::TenantId.eq(item.tenant_id))
            .one(&self.database)
            .await
            .map_err(|error| ModuleWorkError::Source(error.to_string()))?
        else {
            return Ok(ModuleWorkOutcome::Completed);
        };
        if artifact.storage_deleted_at.is_some() || artifact.expires_at > Utc::now().fixed_offset()
        {
            return Ok(ModuleWorkOutcome::Completed);
        }
        expire_interchange_artifact(&self.database, &self.storage, &artifact)
            .await
            .map_err(|error| ModuleWorkError::Handler(error.to_string()))?;
        Ok(ModuleWorkOutcome::Completed)
    }
}

#[cfg(test)]
mod tests {
    use std::path::{Path, PathBuf};

    use bytes::Bytes;
    use chrono::{DateTime, FixedOffset};
    use rustok_api::HostRuntimeContext;
    use rustok_core::{ModuleRuntimeExtensions, RusToKModule};
    use rustok_storage::{
        LocalStorageConfig,
        object_store::{ObjectStore, path::Path as ObjectPath},
    };
    use sea_orm::{
        ActiveModelTrait, ConnectOptions, ConnectionTrait, Database, DbBackend, EntityTrait,
        PaginatorTrait, Set, Statement,
    };
    use sea_orm_migration::SchemaManager;
    use tokio::process::Command;

    use super::*;
    use crate::{
        TranslationModule,
        entities::{exchange_job, job, machine_memory_binding, memory_entry, memory_receipt},
        migrations,
    };

    struct SqliteTestFileGuard(PathBuf);

    impl Drop for SqliteTestFileGuard {
        fn drop(&mut self) {
            for path in sqlite_test_files(&self.0) {
                if path.exists() {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    }

    async fn fixture() -> (DatabaseConnection, Uuid) {
        let database = Database::connect("sqlite::memory:").await.unwrap();
        initialize_fixture(database).await
    }

    async fn fixture_at(database_path: &Path) -> (DatabaseConnection, Uuid) {
        let database = connect_file(database_path, true).await;
        initialize_fixture(database).await
    }

    async fn connect_file(database_path: &Path, create_if_missing: bool) -> DatabaseConnection {
        let database_path = database_path.to_path_buf();
        let mut options =
            ConnectOptions::new("sqlite://translation-retention-placeholder.sqlite?mode=rwc");
        options
            .max_connections(4)
            .min_connections(1)
            .sqlx_logging(false)
            .map_sqlx_sqlite_opts(move |options| {
                options
                    .filename(database_path.clone())
                    .create_if_missing(create_if_missing)
                    .busy_timeout(StdDuration::from_secs(5))
            });
        let database = Database::connect(options).await.unwrap();
        database
            .execute_unprepared("PRAGMA foreign_keys = ON")
            .await
            .unwrap();
        database
    }

    async fn initialize_fixture(database: DatabaseConnection) -> (DatabaseConnection, Uuid) {
        database
            .execute_unprepared("CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL)")
            .await
            .unwrap();
        let manager = SchemaManager::new(&database);
        for migration in migrations::migrations() {
            migration.up(&manager).await.unwrap();
        }
        let tenant_id = Uuid::new_v4();
        database
            .execute(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO tenants (id) VALUES (?)",
                [tenant_id.into()],
            ))
            .await
            .unwrap();
        (database, tenant_id)
    }

    fn sqlite_test_files(database_path: &Path) -> [PathBuf; 4] {
        [
            database_path.to_path_buf(),
            PathBuf::from(format!("{}-journal", database_path.display())),
            PathBuf::from(format!("{}-shm", database_path.display())),
            PathBuf::from(format!("{}-wal", database_path.display())),
        ]
    }

    async fn insert_memory_entry(
        database: &DatabaseConnection,
        tenant_id: Uuid,
        retention_policy: &str,
        retain_until: Option<DateTime<FixedOffset>>,
        owner_deleted_at: Option<DateTime<FixedOffset>>,
        tombstoned_at: Option<DateTime<FixedOffset>>,
    ) -> Uuid {
        let id = Uuid::new_v4();
        let now = Utc::now().fixed_offset();
        memory_entry::ActiveModel {
            id: Set(id),
            tenant_id: Set(tenant_id),
            source_locale: Set("en".to_string()),
            target_locale: Set("de".to_string()),
            owner_slug: Set("pages".to_string()),
            resource_kind: Set("page".to_string()),
            resource_id: Set(Uuid::new_v4().to_string()),
            subresource_id: Set(None),
            field_key: Set("title".to_string()),
            source_text: Set("Source".to_string()),
            target_text: Set("Target".to_string()),
            source_key: Set(Uuid::new_v4().simple().to_string()),
            source_hash: Set(Uuid::new_v4().simple().to_string()),
            target_hash: Set(Uuid::new_v4().simple().to_string()),
            context_fingerprint: Set(Uuid::new_v4().simple().to_string()),
            segmentation_version: Set("owner-field-v1".to_string()),
            origin: Set("manual".to_string()),
            quality_state: Set("human_approved_applied".to_string()),
            reviewer_actor_kind: Set("system".to_string()),
            reviewer_actor_id: Set("test".to_string()),
            proposal_id: Set(Uuid::new_v4()),
            apply_receipt_id: Set(Uuid::new_v4()),
            retention_policy: Set(retention_policy.to_string()),
            retain_until: Set(retain_until),
            owner_lifecycle_revision: Set(owner_deleted_at.map(|_| "deleted-7".to_string())),
            owner_deleted_at: Set(owner_deleted_at),
            tombstoned_at: Set(tombstoned_at),
            revision: Set(1),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(database)
        .await
        .unwrap();
        id
    }

    async fn run_adapter_once(database: &DatabaseConnection) -> usize {
        let scheduler = ModuleWorkScheduler::new();
        TranslationMemoryRetentionWorkAdapter::new(database.clone())
            .register_with(&scheduler)
            .await
            .unwrap();
        scheduler.run_once().await.unwrap()
    }

    async fn artifact_storage() -> (StorageRuntime, tempfile::TempDir) {
        let directory = tempfile::tempdir().unwrap();
        let storage = StorageRuntime::local(&LocalStorageConfig {
            base_dir: directory.path().display().to_string(),
            base_url: "/private".to_string(),
            fsync: false,
        })
        .unwrap();
        (storage, directory)
    }

    async fn insert_expired_interchange_artifact(
        database: &DatabaseConnection,
        tenant_id: Uuid,
        storage: &StorageRuntime,
    ) -> (Uuid, ObjectPath) {
        let now = Utc::now().fixed_offset();
        let job_id = Uuid::new_v4();
        job::ActiveModel {
            id: Set(job_id),
            tenant_id: Set(tenant_id),
            source_locale: Set("en".to_string()),
            target_locale: Set("de".to_string()),
            glossary_id: Set(None),
            glossary_revision: Set(None),
            status: Set("open".to_string()),
            created_by_actor_kind: Set("system".to_string()),
            created_by_actor_id: Set("scheduler-test".to_string()),
            idempotency_key: Set(format!("job-{}", job_id.simple())),
            request_hash: Set("a".repeat(64)),
            revision: Set(0),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(database)
        .await
        .unwrap();

        let artifact_id = Uuid::new_v4();
        let object_path = ObjectPath::from(format!(
            "translation/objects/scheduler-test/{artifact_id}.json"
        ));
        storage
            .objects
            .put(&object_path, Bytes::from_static(b"{}").into())
            .await
            .unwrap();
        exchange_job::ActiveModel {
            id: Set(artifact_id),
            tenant_id: Set(tenant_id),
            job_id: Set(job_id),
            direction: Set("export".to_string()),
            status: Set("ready".to_string()),
            object_key: Set(object_path.to_string()),
            content_length: Set(2),
            checksum_sha256: Set("b".repeat(64)),
            created_by_actor_kind: Set("system".to_string()),
            created_by_actor_id: Set("scheduler-test".to_string()),
            idempotency_key: Set(format!("artifact-{}", artifact_id.simple())),
            request_hash: Set("c".repeat(64)),
            processing_idempotency_key: Set(None),
            processing_request_hash: Set(None),
            processed_by_actor_kind: Set(None),
            processed_by_actor_id: Set(None),
            processing_lease_token: Set(None),
            processing_lease_expires_at: Set(None),
            processed_at: Set(None),
            report: Set(serde_json::Value::Null),
            expires_at: Set(now - Duration::minutes(1)),
            storage_deleted_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(database)
        .await
        .unwrap();
        (artifact_id, object_path)
    }

    #[test]
    fn translation_module_publishes_retention_work_registration() {
        let mut extensions = ModuleRuntimeExtensions::default();
        TranslationModule
            .register_runtime_extensions(&mut extensions)
            .unwrap();
        assert!(
            !extensions
                .get::<rustok_runtime::ModuleWorkRegistrations>()
                .unwrap()
                .is_empty()
        );
    }

    #[tokio::test]
    async fn scheduled_artifact_expiry_deletes_private_object_for_an_inactive_tenant() {
        let (database, tenant_id) = fixture().await;
        let (storage, _storage_directory) = artifact_storage().await;
        let (artifact_id, object_path) =
            insert_expired_interchange_artifact(&database, tenant_id, &storage).await;
        let scheduler = ModuleWorkScheduler::new();
        TranslationInterchangeArtifactExpiryWorkAdapter::new(database.clone(), storage.clone())
            .register_with(&scheduler)
            .await
            .unwrap();

        assert_eq!(scheduler.run_once().await.unwrap(), 1);
        let artifact = exchange_job::Entity::find_by_id(artifact_id)
            .one(&database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(artifact.status, "expired");
        assert!(artifact.storage_deleted_at.is_some());
        assert!(storage.objects.head(&object_path).await.is_err());
        assert_eq!(scheduler.run_once().await.unwrap(), 0);
    }

    #[tokio::test]
    async fn duplicate_artifact_expiry_claims_converge_on_one_deleted_object() {
        let (database, tenant_id) = fixture().await;
        let (storage, _storage_directory) = artifact_storage().await;
        let (artifact_id, object_path) =
            insert_expired_interchange_artifact(&database, tenant_id, &storage).await;
        let first =
            TranslationInterchangeArtifactExpiryWorkAdapter::new(database.clone(), storage.clone());
        let second =
            TranslationInterchangeArtifactExpiryWorkAdapter::new(database.clone(), storage.clone());
        let first_item = first
            .claim(TRANSLATION_INTERCHANGE_ARTIFACT_EXPIRY_WORKER)
            .await
            .unwrap()
            .unwrap();
        let second_item = second
            .claim(TRANSLATION_INTERCHANGE_ARTIFACT_EXPIRY_WORKER)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(first_item.id, artifact_id);
        assert_eq!(second_item.id, artifact_id);

        let (first_outcome, second_outcome) =
            tokio::join!(first.execute(first_item), second.execute(second_item));
        assert_eq!(first_outcome.unwrap(), ModuleWorkOutcome::Completed);
        assert_eq!(second_outcome.unwrap(), ModuleWorkOutcome::Completed);
        let artifact = exchange_job::Entity::find_by_id(artifact_id)
            .one(&database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(artifact.status, "expired");
        assert!(artifact.storage_deleted_at.is_some());
        assert!(storage.objects.head(&object_path).await.is_err());
    }

    #[tokio::test]
    async fn expiry_registration_is_optional_without_storage_but_uses_the_host_handle_when_present()
    {
        let (database, tenant_id) = fixture().await;
        let (storage, _storage_directory) = artifact_storage().await;
        let (artifact_id, object_path) =
            insert_expired_interchange_artifact(&database, tenant_id, &storage).await;
        let scheduler_without_storage = ModuleWorkScheduler::new();
        TranslationInterchangeArtifactExpiryWorkRegistration
            .register(
                &HostRuntimeContext::new(database.clone()),
                &scheduler_without_storage,
            )
            .await
            .unwrap();
        assert_eq!(scheduler_without_storage.run_once().await.unwrap(), 0);

        let scheduler = ModuleWorkScheduler::new();
        TranslationInterchangeArtifactExpiryWorkRegistration
            .register(
                &HostRuntimeContext::new(database.clone()).with_shared_value(storage.clone()),
                &scheduler,
            )
            .await
            .unwrap();
        assert_eq!(scheduler.run_once().await.unwrap(), 1);
        assert!(storage.objects.head(&object_path).await.is_err());
        assert!(
            exchange_job::Entity::find_by_id(artifact_id)
                .one(&database)
                .await
                .unwrap()
                .unwrap()
                .storage_deleted_at
                .is_some()
        );
    }

    #[tokio::test]
    async fn expired_retain_until_is_tombstoned_with_a_system_receipt() {
        let (database, tenant_id) = fixture().await;
        let entry_id = insert_memory_entry(
            &database,
            tenant_id,
            "retain_until",
            Some(Utc::now().fixed_offset() - Duration::minutes(1)),
            None,
            None,
        )
        .await;

        assert_eq!(run_adapter_once(&database).await, 1);
        let entry = memory_entry::Entity::find_by_id(entry_id)
            .one(&database)
            .await
            .unwrap()
            .unwrap();
        assert!(entry.tombstoned_at.is_some());
        assert_eq!(entry.revision, 2);
        let receipt = memory_receipt::Entity::find()
            .one(&database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(receipt.operation, "tombstone");
        assert_eq!(receipt.requested_by_actor_kind, "system");
        assert_eq!(receipt.requested_by_actor_id, "system");
    }

    #[tokio::test]
    async fn duplicate_replica_claims_converge_on_one_transition_and_receipt() {
        let (database, tenant_id) = fixture().await;
        let entry_id = insert_memory_entry(
            &database,
            tenant_id,
            "retain_until",
            Some(Utc::now().fixed_offset() - Duration::minutes(1)),
            None,
            None,
        )
        .await;
        let first = TranslationMemoryRetentionWorkAdapter::new(database.clone());
        let second = TranslationMemoryRetentionWorkAdapter::new(database.clone());
        let first_item = first
            .claim(TRANSLATION_MEMORY_RETENTION_WORKER)
            .await
            .unwrap()
            .unwrap();
        let second_item = second
            .claim(TRANSLATION_MEMORY_RETENTION_WORKER)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(first_item.id, entry_id);
        assert_eq!(second_item.id, entry_id);

        assert_eq!(
            first.execute(first_item).await.unwrap(),
            ModuleWorkOutcome::Completed
        );
        assert_eq!(
            second.execute(second_item).await.unwrap(),
            ModuleWorkOutcome::Completed
        );
        let entry = memory_entry::Entity::find_by_id(entry_id)
            .one(&database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(entry.revision, 2);
        assert_eq!(
            memory_receipt::Entity::find()
                .all(&database)
                .await
                .unwrap()
                .len(),
            1
        );
    }

    #[tokio::test]
    async fn independent_replica_pools_converge_on_one_retention_receipt() {
        let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("workspace path");
        let evidence_dir = workspace.join("target/translation-retention-process-tests");
        std::fs::create_dir_all(&evidence_dir)
            .expect("Translation retention process evidence directory");
        let evidence_dir = evidence_dir
            .canonicalize()
            .expect("Translation retention process evidence path");
        assert!(evidence_dir.starts_with(workspace.join("target")));
        let database_path = evidence_dir.join(format!(
            "rustok-translation-retention-{}.sqlite",
            Uuid::new_v4()
        ));
        let _database_cleanup = SqliteTestFileGuard(database_path.clone());
        let (database, tenant_id) = fixture_at(&database_path).await;
        let entry_id = insert_memory_entry(
            &database,
            tenant_id,
            "retain_until",
            Some(Utc::now().fixed_offset() - Duration::minutes(1)),
            None,
            None,
        )
        .await;
        database.close().await.unwrap();

        let first_database = connect_file(&database_path, false).await;
        let second_database = connect_file(&database_path, false).await;
        let first = TranslationMemoryRetentionWorkAdapter::new(first_database.clone());
        let second = TranslationMemoryRetentionWorkAdapter::new(second_database.clone());
        let first_item = first
            .claim(TRANSLATION_MEMORY_RETENTION_WORKER)
            .await
            .unwrap()
            .unwrap();
        let second_item = second
            .claim(TRANSLATION_MEMORY_RETENTION_WORKER)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(first_item.id, entry_id);
        assert_eq!(second_item.id, entry_id);

        let (first_outcome, second_outcome) =
            tokio::join!(first.execute(first_item), second.execute(second_item));
        assert_eq!(first_outcome.unwrap(), ModuleWorkOutcome::Completed);
        assert_eq!(second_outcome.unwrap(), ModuleWorkOutcome::Completed);
        assert_eq!(
            memory_entry::Entity::find_by_id(entry_id)
                .one(&first_database)
                .await
                .unwrap()
                .unwrap()
                .revision,
            2
        );
        assert_eq!(
            memory_receipt::Entity::find()
                .count(&first_database)
                .await
                .unwrap(),
            1
        );
        first_database.close().await.unwrap();
        second_database.close().await.unwrap();
    }

    #[tokio::test]
    async fn separate_process_recovers_claimed_retention_and_completes_purge() {
        let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("workspace path");
        let evidence_dir = workspace.join("target/translation-retention-process-tests");
        std::fs::create_dir_all(&evidence_dir)
            .expect("Translation retention process evidence directory");
        let evidence_dir = evidence_dir
            .canonicalize()
            .expect("Translation retention process evidence path");
        assert!(evidence_dir.starts_with(workspace.join("target")));
        let database_path = evidence_dir.join(format!(
            "rustok-translation-retention-{}.sqlite",
            Uuid::new_v4()
        ));
        let _database_cleanup = SqliteTestFileGuard(database_path.clone());
        let (database, tenant_id) = fixture_at(&database_path).await;
        let tombstone_id = insert_memory_entry(
            &database,
            tenant_id,
            "retain_until",
            Some(Utc::now().fixed_offset() - Duration::minutes(1)),
            None,
            None,
        )
        .await;
        let purge_id = insert_memory_entry(
            &database,
            tenant_id,
            "owner_lifecycle",
            None,
            Some(Utc::now().fixed_offset() - Duration::hours(30)),
            Some(Utc::now().fixed_offset() - Duration::hours(25)),
        )
        .await;
        let claimed = TranslationMemoryRetentionWorkAdapter::new(database.clone())
            .claim(TRANSLATION_MEMORY_RETENTION_WORKER)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(claimed.id, tombstone_id);
        database.close().await.unwrap();

        for expected_entry_id in [tombstone_id, purge_id] {
            let output =
                Command::new(std::env::current_exe().expect("Translation test executable"))
                    .args([
                        "--exact",
                        "scheduler::tests::retention_recovery_child_process",
                        "--ignored",
                        "--nocapture",
                        "--test-threads=1",
                    ])
                    .env("RUSTOK_TRANSLATION_TEST_RETENTION_DB_PATH", &database_path)
                    .env(
                        "RUSTOK_TRANSLATION_TEST_RETENTION_ENTRY_ID",
                        expected_entry_id.to_string(),
                    )
                    .output()
                    .await
                    .expect("Translation retention child process");
            assert!(
                output.status.success(),
                "Translation retention child failed for entry {expected_entry_id}:\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );
        }

        let database = connect_file(&database_path, false).await;
        let tombstoned = memory_entry::Entity::find_by_id(tombstone_id)
            .one(&database)
            .await
            .unwrap()
            .unwrap();
        assert!(tombstoned.tombstoned_at.is_some());
        assert_eq!(tombstoned.revision, 2);
        assert!(
            memory_entry::Entity::find_by_id(purge_id)
                .one(&database)
                .await
                .unwrap()
                .is_none()
        );
        assert_eq!(
            memory_receipt::Entity::find()
                .count(&database)
                .await
                .unwrap(),
            2
        );
        assert_eq!(run_adapter_once(&database).await, 0);
        database.close().await.unwrap();
    }

    #[tokio::test]
    #[ignore = "internal child process for Translation retention recovery evidence"]
    async fn retention_recovery_child_process() {
        let Some(database_path) = std::env::var_os("RUSTOK_TRANSLATION_TEST_RETENTION_DB_PATH")
        else {
            return;
        };
        let expected_entry_id = Uuid::parse_str(
            &std::env::var("RUSTOK_TRANSLATION_TEST_RETENTION_ENTRY_ID")
                .expect("Translation retention child entry id"),
        )
        .expect("Translation retention child entry UUID");
        let database = connect_file(Path::new(&database_path), false).await;
        let adapter = TranslationMemoryRetentionWorkAdapter::new(database.clone());
        let item = adapter
            .claim(TRANSLATION_MEMORY_RETENTION_WORKER)
            .await
            .unwrap()
            .expect("child process must reclaim retention work");
        assert_eq!(item.id, expected_entry_id);
        assert_eq!(
            adapter.execute(item).await.unwrap(),
            ModuleWorkOutcome::Completed
        );
        database.close().await.unwrap();
    }

    #[tokio::test]
    async fn owner_deletion_is_tombstoned_but_legal_hold_is_not_claimed() {
        let (database, tenant_id) = fixture().await;
        let deleted_at = Utc::now().fixed_offset() - Duration::minutes(1);
        let owner_entry_id = insert_memory_entry(
            &database,
            tenant_id,
            "owner_lifecycle",
            None,
            Some(deleted_at),
            None,
        )
        .await;
        let legal_hold_id = insert_memory_entry(
            &database,
            tenant_id,
            "legal_hold",
            None,
            Some(deleted_at),
            None,
        )
        .await;

        assert_eq!(run_adapter_once(&database).await, 1);
        assert!(
            memory_entry::Entity::find_by_id(owner_entry_id)
                .one(&database)
                .await
                .unwrap()
                .unwrap()
                .tombstoned_at
                .is_some()
        );
        assert!(
            memory_entry::Entity::find_by_id(legal_hold_id)
                .one(&database)
                .await
                .unwrap()
                .unwrap()
                .tombstoned_at
                .is_none()
        );
        assert_eq!(run_adapter_once(&database).await, 0);
    }

    #[tokio::test]
    async fn purge_waits_for_grace_then_preserves_content_free_receipt() {
        let (database, tenant_id) = fixture().await;
        let recent_id = insert_memory_entry(
            &database,
            tenant_id,
            "owner_lifecycle",
            None,
            Some(Utc::now().fixed_offset() - Duration::hours(2)),
            Some(Utc::now().fixed_offset() - Duration::hours(23)),
        )
        .await;
        assert_eq!(run_adapter_once(&database).await, 0);

        let purge_id = insert_memory_entry(
            &database,
            tenant_id,
            "owner_lifecycle",
            None,
            Some(Utc::now().fixed_offset() - Duration::hours(30)),
            Some(Utc::now().fixed_offset() - Duration::hours(25)),
        )
        .await;
        assert_eq!(run_adapter_once(&database).await, 1);
        assert!(
            memory_entry::Entity::find_by_id(purge_id)
                .one(&database)
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            memory_entry::Entity::find_by_id(recent_id)
                .one(&database)
                .await
                .unwrap()
                .is_some()
        );
        let receipt = memory_receipt::Entity::find()
            .one(&database)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(receipt.operation, "purge");
        assert_eq!(receipt.entry_id, purge_id);
        assert!(!receipt.response.to_string().contains("Source"));
        assert!(!receipt.response.to_string().contains("Target"));
    }

    #[tokio::test]
    async fn purge_source_excludes_machine_operation_pins() {
        let (database, tenant_id) = fixture().await;
        let entry_id = insert_memory_entry(
            &database,
            tenant_id,
            "owner_lifecycle",
            None,
            Some(Utc::now().fixed_offset() - Duration::hours(30)),
            Some(Utc::now().fixed_offset() - Duration::hours(25)),
        )
        .await;
        database
            .execute_unprepared("PRAGMA foreign_keys = OFF")
            .await
            .unwrap();
        machine_memory_binding::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            operation_id: Set(Uuid::new_v4()),
            unit_id: Set("unit-1".to_string()),
            batch_ordinal: Set(0),
            unit_ordinal: Set(0),
            memory_entry_id: Set(entry_id),
            score_basis_points: Set(10_000),
            created_at: Set(Utc::now().fixed_offset()),
        }
        .insert(&database)
        .await
        .unwrap();
        database
            .execute_unprepared("PRAGMA foreign_keys = ON")
            .await
            .unwrap();

        assert_eq!(run_adapter_once(&database).await, 0);
        assert!(
            memory_entry::Entity::find_by_id(entry_id)
                .one(&database)
                .await
                .unwrap()
                .is_some()
        );
    }
}
