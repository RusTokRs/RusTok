//! Runtime evidence for owner-scoped snapshot copies and isolated restore.
//! This does not attest post-purge recovery verification, cutover, or fleet fences.

use async_trait::async_trait;
use chrono::Utc;
use object_store::{ObjectStoreExt, path::Path};
use rustok_core::MigrationSource;
use rustok_modules::{
    ArtifactDataError, ArtifactDataQuota, ArtifactDataRestoreRequest, ArtifactDataScope,
    ArtifactDataSnapshotAuthorizer, ArtifactDataSnapshotCollectionAuthorizer,
    ArtifactDataSnapshotCollectionRequest, ArtifactDataSnapshotCollectionRule,
    ArtifactDataSnapshotCreateRequest, ArtifactDataSnapshotIntentService, ModuleCommandContext,
    ModulesModule, SeaOrmArtifactDataSnapshotCollectionService, SeaOrmArtifactDataSnapshotService,
    SnapshotArtifactDataSnapshotCollectionPolicy,
};
use rustok_storage::{LocalStorageConfig, ObjectKey, ObjectScope, ObjectZone, StorageRuntime};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use sha2::{Digest, Sha256};
use std::{path::PathBuf, time::Duration};
use uuid::Uuid;

struct FixturePolicy {
    db: DatabaseConnection,
    tenant_id: Uuid,
    actor_id: Uuid,
    owner_id: Uuid,
}

impl FixturePolicy {
    async fn check(
        &self,
        scope: &ArtifactDataScope,
        context: &ModuleCommandContext,
    ) -> Result<(), ArtifactDataError> {
        if scope.tenant_id != self.tenant_id
            || context.tenant_id != Some(self.tenant_id)
            || context.actor_id != self.actor_id
            || scope.data_owner_id != self.owner_id
            || scope.policy_revision != 1
        {
            return Err(ArtifactDataError::RestorePrecondition);
        }
        let row = self.db.query_one_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "SELECT 1 FROM module_artifact_data_namespaces WHERE tenant_id = ?1 AND data_owner_id = ?2
             AND namespace_instance_id = ?3 AND data_contract_digest = ?4",
            vec![scope.tenant_id.to_string().into(),scope.data_owner_id.to_string().into(),
                 scope.namespace_instance_id.to_string().into(),scope.data_contract_digest.clone().into()]))
            .await.map_err(|e| ArtifactDataError::Storage(e.to_string()))?;
        if row.is_none() {
            return Err(ArtifactDataError::RestorePrecondition);
        }
        Ok(())
    }
}

#[async_trait]
impl ArtifactDataSnapshotAuthorizer for FixturePolicy {
    async fn authorize_snapshot(
        &self,
        request: &ArtifactDataSnapshotCreateRequest,
    ) -> Result<(), ArtifactDataError> {
        self.check(&request.scope, &request.context).await
    }
    async fn authorize_restore(
        &self,
        request: &ArtifactDataRestoreRequest,
    ) -> Result<ArtifactDataQuota, ArtifactDataError> {
        self.check(&request.target, &request.context).await?;
        Ok(ArtifactDataQuota::default())
    }
}

#[async_trait]
impl ArtifactDataSnapshotCollectionAuthorizer for FixturePolicy {
    async fn authorize_collection(
        &self,
        request: &ArtifactDataSnapshotCollectionRequest,
    ) -> Result<(), ArtifactDataError> {
        if request.tenant_id != self.tenant_id
            || request.context.tenant_id != Some(self.tenant_id)
            || request.context.actor_id != self.actor_id
        {
            return Err(ArtifactDataError::SnapshotCollectionPrecondition);
        }
        Ok(())
    }
}

struct FixtureRoot(PathBuf);

impl FixtureRoot {
    fn new() -> Self {
        let target = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../target");
        std::fs::create_dir_all(&target).expect("workspace target");
        let target = target.canonicalize().expect("absolute workspace target");
        let path = target.join(format!("snapshot-fixture-{}", Uuid::new_v4()));
        std::fs::create_dir(&path).expect("fixture storage root");
        assert!(path.starts_with(&target));
        Self(path)
    }
}

impl Drop for FixtureRoot {
    fn drop(&mut self) {
        let target = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../target")
            .canonicalize()
            .expect("workspace target");
        assert!(self.0.starts_with(target));
        std::fs::remove_dir_all(&self.0).expect("remove isolated fixture root");
    }
}

async fn execute(db: &DatabaseConnection, sql: &str, values: Vec<sea_orm::Value>) {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        sql,
        values,
    ))
    .await
    .expect("fixture SQL");
}

async fn scalar(db: &DatabaseConnection, sql: &str) -> i64 {
    db.query_one_raw(Statement::from_string(DbBackend::Sqlite, sql))
        .await
        .expect("scalar SQL")
        .expect("scalar row")
        .try_get("", "value")
        .expect("scalar value")
}

#[tokio::test]
async fn snapshot_restore_reuses_reserved_keys_and_preserves_tombstones_and_holds() {
    let db = Database::connect("sqlite::memory:").await.expect("sqlite");
    rustok_outbox::SysEventsMigration
        .up(&SchemaManager::new(&db))
        .await
        .expect("outbox");
    for migration in ModulesModule.migrations() {
        migration
            .up(&SchemaManager::new(&db))
            .await
            .expect("module migration");
    }
    let root = FixtureRoot::new();
    let storage = StorageRuntime::local(&LocalStorageConfig {
        base_dir: root.0.display().to_string(),
        fsync: true,
        ..Default::default()
    })
    .expect("real local storage");
    let tenant_id = Uuid::new_v4();
    let owner_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let source = ArtifactDataScope {
        tenant_id,
        data_owner_id: owner_id,
        namespace_instance_id: Uuid::new_v4(),
        data_contract_digest: format!(
            "sha256:{}",
            hex::encode(Sha256::digest(b"fixture data contract"))
        ),
        module_slug: "snapshot_module".into(),
        data_contract_revision: 1,
        policy_revision: 1,
    };
    let target = ArtifactDataScope {
        namespace_instance_id: Uuid::new_v4(),
        ..source.clone()
    };
    let now = Utc::now();
    for (scope, state) in [(&source, "serving"), (&target, "staging")] {
        execute(&db,"INSERT INTO module_artifact_data_namespaces
            (tenant_id,data_owner_id,namespace_instance_id,module_slug,data_contract_revision,data_contract_digest,
             state,namespace_revision,purged_at,created_at,updated_at) VALUES (?1,?2,?3,?4,1,?5,?6,1,NULL,?7,?7)",
            vec![tenant_id.to_string().into(),owner_id.to_string().into(),scope.namespace_instance_id.to_string().into(),
                 scope.module_slug.clone().into(),scope.data_contract_digest.clone().into(),state.into(),now.into()]).await;
    }
    execute(
        &db,
        "INSERT INTO module_artifact_data_owner_references VALUES (?1,?2,?3,1)",
        vec![
            tenant_id.to_string().into(),
            owner_id.to_string().into(),
            source.namespace_instance_id.to_string().into(),
        ],
    )
    .await;
    execute(
        &db,
        "INSERT INTO module_artifact_data VALUES (?1,?2,?3,'record','{\"answer\":42}',13,3,?4)",
        vec![
            tenant_id.to_string().into(),
            owner_id.to_string().into(),
            source.namespace_instance_id.to_string().into(),
            now.into(),
        ],
    )
    .await;
    execute(
        &db,
        "INSERT INTO module_artifact_data_indexes VALUES (?1,?2,?3,'answer','42','record')",
        vec![
            tenant_id.to_string().into(),
            owner_id.to_string().into(),
            source.namespace_instance_id.to_string().into(),
        ],
    )
    .await;
    execute(
        &db,
        "INSERT INTO module_artifact_data_index_contracts VALUES (?1,?2,?3,?4,?5)",
        vec![
            tenant_id.to_string().into(),
            owner_id.to_string().into(),
            source.namespace_instance_id.to_string().into(),
            format!(
                "sha256:{}",
                hex::encode(Sha256::digest(b"fixture answer index contract"))
            )
            .into(),
            now.into(),
        ],
    )
    .await;
    let bytes = bytes::Bytes::from_static(b"actual snapshot payload");
    let digest = format!("sha256:{}", hex::encode(Sha256::digest(&bytes)));
    let source_key = ObjectKey::chronological(
        "module-artifact-data",
        ObjectZone::Objects,
        ObjectScope::Namespace {
            tenant_id,
            owner_id,
            instance_id: source.namespace_instance_id,
        },
        now,
        Uuid::new_v4(),
        "bin",
    )
    .expect("source private key")
    .to_string();
    storage
        .objects
        .put(&Path::from(source_key.as_str()), bytes.clone().into())
        .await
        .expect("publish source bytes");
    execute(&db,"INSERT INTO module_artifact_data_objects
        (tenant_id,data_owner_id,namespace_instance_id,object_name,storage_key,content_type,size_bytes,digest_sha256,
         revision,created_at,updated_at) VALUES (?1,?2,?3,'payload',?4,'application/octet-stream',?5,?6,4,?7,?7)",
        vec![tenant_id.to_string().into(),owner_id.to_string().into(),source.namespace_instance_id.to_string().into(),
             source_key.clone().into(),i64::try_from(bytes.len()).expect("size").into(),digest.into(),now.into()]).await;
    let context = ModuleCommandContext {
        actor_id,
        tenant_id: Some(tenant_id),
        trace_id: "test:snapshot-intents".into(),
        correlation_id: Uuid::new_v4(),
        idempotency_key: Uuid::new_v4(),
    };
    let service = SeaOrmArtifactDataSnapshotService::new(
        db.clone(),
        storage.clone(),
        FixturePolicy {
            db: db.clone(),
            tenant_id,
            actor_id,
            owner_id,
        },
    );
    let create = ArtifactDataSnapshotCreateRequest {
        scope: source.clone(),
        expected_namespace_revision: 1,
        context: context.clone(),
        reason: "Capture fixture".into(),
        retain_until: now + chrono::Duration::days(1),
        legal_hold: false,
    };
    let snapshot = service
        .create(create.clone())
        .await
        .expect("copy and finalize actual snapshot");
    assert_eq!(
        service.create(create).await.expect("exact snapshot replay"),
        snapshot
    );
    assert_eq!(
        scalar(
            &db,
            "SELECT count(*) AS value FROM module_artifact_data_snapshot_copy_intents"
        )
        .await,
        1
    );
    let snapshot_key: String = db
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT snapshot_storage_key FROM module_artifact_data_snapshot_objects",
        ))
        .await
        .expect("snapshot key SQL")
        .expect("snapshot object")
        .try_get("", "snapshot_storage_key")
        .expect("snapshot key");
    execute(
        &db,
        "UPDATE module_artifact_data_namespaces SET state='purged',purged_at=?1
        WHERE namespace_instance_id=?2",
        vec![now.into(), source.namespace_instance_id.to_string().into()],
    )
    .await;
    execute(
        &db,
        "DELETE FROM module_artifact_data_objects WHERE namespace_instance_id=?1",
        vec![source.namespace_instance_id.to_string().into()],
    )
    .await;
    storage
        .objects
        .delete(&Path::from(source_key))
        .await
        .expect("remove purged source bytes");
    storage
        .objects
        .put(
            &Path::from(snapshot_key.as_str()),
            bytes::Bytes::from_static(b"corrupt").into(),
        )
        .await
        .expect("corrupt bounded snapshot fixture");
    let restore = ArtifactDataRestoreRequest {
        snapshot_id: snapshot.snapshot_id,
        target: target.clone(),
        expected_namespace_revision: 1,
        context: ModuleCommandContext {
            idempotency_key: Uuid::new_v4(),
            ..context
        },
        reason: "Restore into empty staging".into(),
    };
    assert!(matches!(
        service.restore(restore.clone()).await,
        Err(ArtifactDataError::SnapshotIntegrity)
    ));
    assert_eq!(
        scalar(
            &db,
            "SELECT count(*) AS value FROM module_artifact_data_snapshot_holds WHERE released_at IS NULL"
        ).await,
        1
    );
    assert_eq!(
        scalar(
            &db,
            "SELECT count(*) AS value FROM module_artifact_data_restore_operations"
        )
        .await,
        0
    );
    execute(
        &db,
        "UPDATE module_artifact_data_snapshots SET retain_until=?1",
        vec![(now - chrono::Duration::days(1)).into()],
    )
    .await;
    let collector = SeaOrmArtifactDataSnapshotCollectionService::new(
        db.clone(),
        storage.clone(),
        FixturePolicy {
            db: db.clone(),
            tenant_id,
            actor_id,
            owner_id,
        },
    );
    let collection = ArtifactDataSnapshotCollectionRequest {
        tenant_id,
        context: ModuleCommandContext {
            actor_id,
            tenant_id: Some(tenant_id),
            trace_id: "test:snapshot-collection".into(),
            correlation_id: Uuid::new_v4(),
            idempotency_key: Uuid::new_v4(),
        },
        reason: "Collect expired fixture snapshot".into(),
        policy_snapshot_id: "fixture-approved-policy".into(),
        limit: 1,
    };
    let collection_policy = SnapshotArtifactDataSnapshotCollectionPolicy::new(
        collection.policy_snapshot_id.clone(),
        std::collections::HashMap::from([(
            snapshot.snapshot_id,
            ArtifactDataSnapshotCollectionRule {
                audit_hold: false,
                rollback_hold: false,
                collection_approved: true,
            },
        )]),
    )
    .expect("approved collection rule");
    assert_eq!(
        collector
            .collect(collection.clone(), &collection_policy)
            .await
            .expect("held collection")
            .collected,
        0
    );
    assert!(
        storage
            .objects
            .head(&Path::from(snapshot_key.as_str()))
            .await
            .is_ok()
    );
    let reserved: String = db.query_one_raw(Statement::from_string(DbBackend::Sqlite,
        "SELECT target_storage_key FROM module_artifact_data_snapshot_copy_intents WHERE operation_kind='restore'"))
        .await.expect("restore reservation SQL").expect("restore intent").try_get("","target_storage_key").expect("reserved key");
    storage
        .objects
        .put(&Path::from(snapshot_key), bytes.clone().into())
        .await
        .expect("repair fixture bytes");
    let restored = service
        .restore(restore.clone())
        .await
        .expect("resume reserved restore");
    assert_eq!(restored.restored_records, 1);
    assert_eq!(restored.restored_objects, 1);
    assert_eq!(
        service
            .restore(restore.clone())
            .await
            .expect("exact terminal replay"),
        restored
    );
    let mut conflict = restore;
    conflict.reason = "Changed replay evidence".into();
    assert!(matches!(
        service.restore(conflict).await,
        Err(ArtifactDataError::IdempotencyConflict)
    ));
    assert_eq!(
        storage
            .objects
            .get(&Path::from(reserved.as_str()))
            .await
            .expect("actual target")
            .bytes()
            .await
            .expect("target bytes"),
        bytes
    );
    assert_eq!(
        scalar(
            &db,
            "SELECT count(*) AS value FROM module_artifact_data_snapshot_copy_intents"
        )
        .await,
        2
    );
    assert_eq!(
        scalar(
            &db,
            "SELECT count(*) AS value FROM module_artifact_data_snapshot_copy_intents WHERE status='committed'"
        ).await,
        2
    );
    assert_eq!(
        scalar(
            &db,
            "SELECT count(*) AS value FROM module_artifact_data_snapshot_holds WHERE released_at IS NULL"
        ).await,
        0
    );
    assert_eq!(
        scalar(
            &db,
            "SELECT count(*) AS value FROM module_artifact_data_namespaces WHERE state='purged' AND purged_at IS NOT NULL"
        ).await,
        1
    );
    assert_eq!(
        scalar(
            &db,
            "SELECT count(*) AS value FROM module_artifact_data_namespaces WHERE state='verified' AND verified_manifest_digest IS NOT NULL"
        ).await,
        1
    );
    assert_eq!(scalar(&db,"SELECT count(*) AS value FROM module_artifact_data_owner_references reference
        JOIN module_artifact_data_namespaces namespace USING (tenant_id,data_owner_id,namespace_instance_id)
        WHERE namespace.state='purged'").await,1);
    for sql in [
        "DELETE FROM module_artifact_data WHERE namespace_instance_id=?1",
        "UPDATE module_artifact_data SET revision=revision+1 WHERE namespace_instance_id=?1",
        "DELETE FROM module_artifact_data_objects WHERE namespace_instance_id=?1",
        "UPDATE module_artifact_data_namespaces SET state='staging' WHERE namespace_instance_id=?1",
        "UPDATE module_artifact_data_namespaces SET verified_manifest_digest=NULL WHERE namespace_instance_id=?1",
        "DELETE FROM module_artifact_data_namespaces WHERE namespace_instance_id=?1",
    ] {
        assert!(
            db.execute_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                sql,
                vec![target.namespace_instance_id.to_string().into()]
            ))
            .await
            .is_err(),
            "sealed target: {sql}"
        );
    }
    assert_eq!(scalar(&db,"SELECT revision AS value FROM module_artifact_data record
        JOIN module_artifact_data_namespaces namespace USING (tenant_id,data_owner_id,namespace_instance_id)
        WHERE namespace.state='verified'").await,3);
    assert_eq!(scalar(&db,"SELECT revision AS value FROM module_artifact_data_objects object
        JOIN module_artifact_data_namespaces namespace USING (tenant_id,data_owner_id,namespace_instance_id)
        WHERE namespace.state='verified'").await,4);
    assert_eq!(scalar(&db,"SELECT count(*) AS value FROM module_artifact_data_indexes record
        JOIN module_artifact_data_namespaces namespace USING (tenant_id,data_owner_id,namespace_instance_id)
        WHERE namespace.state='verified' AND index_name='answer' AND index_value='42'").await,1);
    // Age and an absent metadata parent never authorize orphan deletion.
    execute(&db,"UPDATE module_artifact_data_snapshot_copy_intents SET status='staging',committed_at=NULL,created_at=?1
        WHERE operation_kind='restore'",vec![(now - chrono::Duration::minutes(10)).into()]).await;
    execute(
        &db,
        "DELETE FROM module_artifact_data_restore_operations",
        vec![],
    )
    .await;
    let reconciler = ArtifactDataSnapshotIntentService::new(db.clone(), storage.clone());
    let receipt = reconciler
        .reconcile_stale_intents(tenant_id, Duration::from_secs(300))
        .await
        .expect("reconcile unresolved");
    assert_eq!(receipt.retained_unresolved, 1);
    assert_eq!(receipt.committed_resumed, 0);
    assert!(
        storage
            .objects
            .head(&Path::from(reserved.as_str()))
            .await
            .is_ok()
    );
    assert_eq!(
        collector
            .collect(collection, &collection_policy)
            .await
            .expect("released collection")
            .collected,
        1
    );
    assert!(storage.objects.head(&Path::from(reserved)).await.is_ok());
}
