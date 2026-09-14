//! Real-byte migration, persisted replay, policy denial, and pending namespace holds.
//! Fixture authority does not attest host maintenance, fleet fences, or recovery CAS.

use async_trait::async_trait;
use bytes::Bytes;
use chrono::Utc;
use object_store::{ObjectStoreExt, path::Path};
use rustok_core::MigrationSource;
use rustok_modules::{
    ArtifactDataNamespace, ArtifactDataObjectMigrationAuthorizer,
    ArtifactDataObjectMigrationError as Error, ArtifactDataObjectMigrationRequest as Request,
    ArtifactDataObjectMigrationService, ModuleCommandContext, ModulesModule,
};
use rustok_storage::{LocalStorageConfig, ObjectKey, ObjectScope, ObjectZone, StorageRuntime};
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DatabaseTransaction, DbBackend, Statement, Value,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use uuid::Uuid;

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{}", hex::encode(Sha256::digest(bytes)))
}

/// A bounded persisted fixture policy, evaluated on the supplied owner transaction.
/// It models actor/scope/current-grant rejection; it is not a production maintenance fence.
struct FixtureAuthority;

#[async_trait]
impl ArtifactDataObjectMigrationAuthorizer for FixtureAuthority {
    async fn authorize_object_migration_on(
        &self,
        transaction: &DatabaseTransaction,
        request: &Request,
        source: &ArtifactDataNamespace,
        target: &ArtifactDataNamespace,
    ) -> Result<(), Error> {
        if request.context.tenant_id != Some(source.tenant_id)
            || request.tenant_id != source.tenant_id
            || target.tenant_id != source.tenant_id
            || request.data_owner_id != source.data_owner_id
            || target.data_owner_id != source.data_owner_id
            || request.source_namespace_instance_id != source.namespace_instance_id
            || request.target_namespace_instance_id != target.namespace_instance_id
            || request.expected_source_namespace_revision != source.namespace_revision
            || request.expected_target_namespace_revision != target.namespace_revision
        {
            return Err(Error::PolicyDenied);
        }
        let row = transaction
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT 1 FROM fixture_migration_authority
             WHERE tenant_id=?1 AND data_owner_id=?2 AND actor_id=?3
              AND source_namespace_instance_id=?4 AND target_namespace_instance_id=?5
              AND source_revision=?6 AND target_revision=?7 AND enabled=1",
                vec![
                    request.tenant_id.to_string().into(),
                    request.data_owner_id.to_string().into(),
                    request.context.actor_id.to_string().into(),
                    source.namespace_instance_id.to_string().into(),
                    target.namespace_instance_id.to_string().into(),
                    i64::try_from(source.namespace_revision)
                        .map_err(|_| Error::PolicyDenied)?
                        .into(),
                    i64::try_from(target.namespace_revision)
                        .map_err(|_| Error::PolicyDenied)?
                        .into(),
                ],
            ))
            .await
            .map_err(|error| Error::Storage(error.to_string()))?;
        if row.is_none() {
            return Err(Error::PolicyDenied);
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
        let root = target.join(format!("object-migration-fixture-{}", Uuid::new_v4()));
        assert!(root.is_absolute() && root.starts_with(&target) && root != target);
        std::fs::create_dir(&root).expect("isolated fixture root");
        Self(root)
    }
}

impl Drop for FixtureRoot {
    fn drop(&mut self) {
        let target = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../target")
            .canonicalize()
            .expect("workspace target");
        let root = self.0.canonicalize().expect("resolved fixture root");
        assert!(
            root.is_absolute() && root.starts_with(&target) && root != target && root == self.0
        );
        std::fs::remove_dir_all(root).expect("remove checked isolated fixture root");
    }
}

async fn execute<C: ConnectionTrait>(db: &C, sql: &str, values: Vec<Value>) {
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
        .expect("scalar query")
        .expect("scalar row")
        .try_get("", "value")
        .expect("scalar value")
}

struct SourceObject {
    name: String,
    key: String,
    bytes: Bytes,
}

struct Fixture {
    db: DatabaseConnection,
    storage: StorageRuntime,
    request: Request,
    objects: Vec<SourceObject>,
    database_url: String,
    root: FixtureRoot,
}

impl Fixture {
    async fn new(object_count: usize) -> Self {
        let root = FixtureRoot::new();
        let database_path = root.0.join("control-plane.db").display().to_string();
        // SQLite's URI parser interprets the question mark in Windows extended
        // paths as a query delimiter. Use the same absolute drive path without
        // the extended prefix; the checked fixture root still owns the file.
        let database_path = database_path
            .strip_prefix(r"\\?\")
            .unwrap_or(&database_path);
        let database_url = format!("sqlite://{}?mode=rwc", database_path.replace('\\', "/"));
        let db = Database::connect(database_url.as_str())
            .await
            .expect("file-backed sqlite");
        rustok_outbox::SysEventsMigration
            .up(&SchemaManager::new(&db))
            .await
            .expect("outbox migration");
        for migration in ModulesModule.migrations() {
            migration
                .up(&SchemaManager::new(&db))
                .await
                .expect("canonical module migration");
        }
        let storage = StorageRuntime::local(&LocalStorageConfig {
            base_dir: root.0.join("objects").display().to_string(),
            fsync: true,
            ..Default::default()
        })
        .expect("real file storage");
        let request = Request {
            tenant_id: Uuid::new_v4(),
            data_owner_id: Uuid::new_v4(),
            source_namespace_instance_id: Uuid::new_v4(),
            target_namespace_instance_id: Uuid::new_v4(),
            expected_source_namespace_revision: 1,
            expected_target_namespace_revision: 1,
            context: ModuleCommandContext {
                actor_id: Uuid::new_v4(),
                tenant_id: None,
                idempotency_key: Uuid::new_v4(),
                trace_id: "test:real-object-migration".into(),
                correlation_id: Uuid::new_v4(),
            },
            reason: "Copy verified fixture inventory into a non-serving empty instance".into(),
        };
        let mut request = request;
        request.context.tenant_id = Some(request.tenant_id);
        let contract = digest(br#"{"type":"object","additionalProperties":false}"#);
        for (namespace, revision) in [
            (request.source_namespace_instance_id, 1_i64),
            (request.target_namespace_instance_id, 2_i64),
        ] {
            execute(&db,"INSERT INTO module_artifact_data_namespaces
                (tenant_id,data_owner_id,namespace_instance_id,module_slug,data_contract_revision,data_contract_digest,
                 state,namespace_revision,created_at,updated_at)
                VALUES (?1,?2,?3,'migration_fixture',?4,?5,'staging',1,?6,?6)",
                vec![request.tenant_id.to_string().into(),request.data_owner_id.to_string().into(),
                     namespace.to_string().into(),revision.into(),contract.clone().into(),Utc::now().into()]).await;
        }
        execute(&db,"CREATE TABLE fixture_migration_authority (
            tenant_id TEXT NOT NULL,data_owner_id TEXT NOT NULL,actor_id TEXT NOT NULL,
            source_namespace_instance_id TEXT NOT NULL,target_namespace_instance_id TEXT NOT NULL,
            source_revision INTEGER NOT NULL,target_revision INTEGER NOT NULL,enabled INTEGER NOT NULL CHECK(enabled IN (0,1)))",vec![]).await;
        execute(
            &db,
            "INSERT INTO fixture_migration_authority VALUES (?1,?2,?3,?4,?5,1,1,1)",
            vec![
                request.tenant_id.to_string().into(),
                request.data_owner_id.to_string().into(),
                request.context.actor_id.to_string().into(),
                request.source_namespace_instance_id.to_string().into(),
                request.target_namespace_instance_id.to_string().into(),
            ],
        )
        .await;
        let mut objects = Vec::new();
        let mut inventory = Vec::new();
        for index in 0..object_count {
            let name = format!("object-{index}.bin");
            let bytes = Bytes::from(format!("actual immutable object payload {index}"));
            let key = ObjectKey::chronological(
                "module-data",
                ObjectZone::Objects,
                ObjectScope::Namespace {
                    tenant_id: request.tenant_id,
                    owner_id: request.data_owner_id,
                    instance_id: request.source_namespace_instance_id,
                },
                Utc::now(),
                Uuid::new_v4(),
                "bin",
            )
            .expect("canonical source key")
            .to_string();
            storage
                .objects
                .put(&Path::from(key.as_str()), bytes.clone().into())
                .await
                .expect("publish source bytes");
            let actual = storage
                .objects
                .get(&Path::from(key.as_str()))
                .await
                .expect("read source")
                .bytes()
                .await
                .expect("source bytes");
            assert_eq!(actual, bytes);
            let size = i64::try_from(actual.len()).expect("checked object size");
            let hash = digest(&actual);
            execute(&db,"INSERT INTO module_artifact_data_objects
                (tenant_id,data_owner_id,namespace_instance_id,object_name,storage_key,content_type,size_bytes,digest_sha256,revision,created_at,updated_at)
                VALUES (?1,?2,?3,?4,?5,'application/octet-stream',?6,?7,1,?8,?8)",
                vec![request.tenant_id.to_string().into(),request.data_owner_id.to_string().into(),
                     request.source_namespace_instance_id.to_string().into(),name.clone().into(),key.clone().into(),
                     size.into(),hash.clone().into(),Utc::now().into()]).await;
            inventory
                .push(serde_json::json!({"name":name,"size":size,"digest":hash,"storage_key":key}));
            objects.push(SourceObject { name, key, bytes });
        }
        // Seal this bounded fixture only after checking its complete actual object inventory.
        // Production snapshot verification and host freeze authority have separate gates.
        let proof = digest(
            &serde_json::to_vec(&serde_json::json!({
                "tenant_id":request.tenant_id,"data_owner_id":request.data_owner_id,
                "namespace_instance_id":request.source_namespace_instance_id,"contract":contract,
                "records":[],"indexes":[],"objects":inventory
            }))
            .expect("complete fixture inventory"),
        );
        execute(&db,"UPDATE module_artifact_data_namespaces SET state='verified',verified_manifest_digest=?1
            WHERE namespace_instance_id=?2",vec![proof.into(),request.source_namespace_instance_id.to_string().into()]).await;
        Self {
            db,
            storage,
            request,
            objects,
            database_url,
            root,
        }
    }

    fn service(&self) -> ArtifactDataObjectMigrationService<FixtureAuthority> {
        ArtifactDataObjectMigrationService::new(
            self.db.clone(),
            self.storage.clone(),
            FixtureAuthority,
        )
    }

    async fn keys(&self) -> Vec<String> {
        self.db.query_all_raw(Statement::from_string(DbBackend::Sqlite,
            "SELECT target_storage_key FROM module_artifact_data_object_copy_operations ORDER BY object_name"))
            .await.expect("reserved keys").iter().map(|row|row.try_get("","target_storage_key").expect("key")).collect()
    }

    async fn assert_copies(&self) {
        let keys = self.keys().await;
        assert_eq!(keys.len(), self.objects.len());
        for (object, key) in self.objects.iter().zip(&keys) {
            assert_ne!(key, &object.key);
            let prefix = format!(
                "module-data/objects/tenants/{}/owners/{}/instances/{}/",
                self.request.tenant_id,
                self.request.data_owner_id,
                self.request.target_namespace_instance_id
            );
            assert!(
                key.starts_with(&prefix),
                "copy must use the exact target namespace"
            );
            let object_id = key
                .rsplit('/')
                .next()
                .expect("object component")
                .strip_suffix(".bin")
                .expect("canonical extension");
            assert!(
                !Uuid::parse_str(object_id)
                    .expect("opaque object identity")
                    .is_nil()
            );
            let bytes = self
                .storage
                .objects
                .get(&Path::from(key.as_str()))
                .await
                .expect("target object")
                .bytes()
                .await
                .expect("target bytes");
            assert_eq!(bytes, object.bytes);
            let row = self
                .db
                .query_one_raw(Statement::from_sql_and_values(
                    DbBackend::Sqlite,
                    "SELECT storage_key,digest_sha256,size_bytes FROM module_artifact_data_objects
                 WHERE namespace_instance_id=?1 AND object_name=?2",
                    vec![
                        self.request.target_namespace_instance_id.to_string().into(),
                        object.name.clone().into(),
                    ],
                ))
                .await
                .expect("target reference")
                .expect("target metadata");
            assert_eq!(
                row.try_get::<String>("", "storage_key").expect("ref key"),
                *key
            );
            assert_eq!(
                row.try_get::<String>("", "digest_sha256")
                    .expect("ref digest"),
                digest(&bytes)
            );
            assert_eq!(
                row.try_get::<i64>("", "size_bytes").expect("ref size"),
                i64::try_from(bytes.len()).expect("size")
            );
        }
    }

    async fn reopen(&mut self) {
        self.db.clone().close().await.expect("close original pool");
        self.db = Database::connect(self.database_url.as_str())
            .await
            .expect("reopen durable owner journal");
        self.storage = StorageRuntime::local(&LocalStorageConfig {
            base_dir: self.root.0.join("objects").display().to_string(),
            fsync: true,
            ..Default::default()
        })
        .expect("reconstituted file storage");
    }

    async fn finish(self) {
        self.db.close().await.expect("close fixture database");
        drop(self.storage);
        drop(self.root);
    }
}

#[tokio::test]
async fn migration_copies_actual_bytes_and_replays_terminal_receipt_before_mutable_state() {
    let fixture = Fixture::new(3).await;
    let service = fixture.service();
    assert_eq!(
        service
            .count_unmigrated_live_objects(
                fixture.request.tenant_id,
                fixture.request.data_owner_id,
                fixture.request.source_namespace_instance_id,
                fixture.request.target_namespace_instance_id
            )
            .await
            .expect("unmigrated object count"),
        3
    );
    let receipt = service
        .migrate_objects(fixture.request.clone())
        .await
        .expect("actual migration");
    assert!(receipt.accepted);
    assert_eq!(receipt.objects_migrated, 3);
    fixture.assert_copies().await;
    assert_eq!(
        scalar(
            &fixture.db,
            "SELECT count(*) AS value FROM module_artifact_data_namespaces WHERE state='staging'"
        )
        .await,
        1
    );
    assert_eq!(
        scalar(
            &fixture.db,
            "SELECT count(*) AS value FROM module_artifact_data_owner_references"
        )
        .await,
        0
    );
    assert_eq!(
        service
            .count_unmigrated_live_objects(
                fixture.request.tenant_id,
                fixture.request.data_owner_id,
                fixture.request.source_namespace_instance_id,
                fixture.request.target_namespace_instance_id,
            )
            .await
            .expect("logical guard cleared"),
        0
    );
    let keys = fixture.keys().await;
    execute(
        &fixture.db,
        "UPDATE fixture_migration_authority SET enabled=0",
        vec![],
    )
    .await;
    execute(&fixture.db,"UPDATE module_artifact_data_namespaces SET state='purged',purged_at=?1,namespace_revision=2
        WHERE namespace_instance_id=?2",vec![Utc::now().into(),fixture.request.source_namespace_instance_id.to_string().into()]).await;
    assert_eq!(
        service
            .migrate_objects(fixture.request.clone())
            .await
            .expect("terminal replay"),
        receipt
    );
    let mut changed = Vec::new();
    let mut request = fixture.request.clone();
    request.reason.push_str(" changed");
    changed.push(request);
    let mut request = fixture.request.clone();
    request.context.actor_id = Uuid::new_v4();
    changed.push(request);
    let mut request = fixture.request.clone();
    request.context.trace_id.push_str(":changed");
    changed.push(request);
    let mut request = fixture.request.clone();
    request.context.correlation_id = Uuid::new_v4();
    changed.push(request);
    let mut request = fixture.request.clone();
    request.expected_source_namespace_revision = 2;
    changed.push(request);
    let mut request = fixture.request.clone();
    request.target_namespace_instance_id = Uuid::new_v4();
    changed.push(request);
    for request in changed {
        assert_eq!(
            service.migrate_objects(request).await,
            Err(Error::IdempotencyConflict)
        );
    }
    assert_eq!(fixture.keys().await, keys);
    assert_eq!(
        scalar(
            &fixture.db,
            "SELECT count(*) AS value FROM module_artifact_data_object_migration_operations"
        )
        .await,
        1
    );
    drop(service);
    fixture.finish().await;
}

#[tokio::test]
async fn interrupted_publication_retains_keys_holds_and_current_policy_across_reopen() {
    let mut fixture = Fixture::new(2).await;
    fixture
        .storage
        .objects
        .put(
            &Path::from(fixture.objects[1].key.as_str()),
            Bytes::from_static(b"corrupt source").into(),
        )
        .await
        .expect("source corruption injection");
    let service = fixture.service();
    assert_eq!(
        service.migrate_objects(fixture.request.clone()).await,
        Err(Error::Integrity)
    );
    let keys = fixture.keys().await;
    assert_eq!(keys.len(), 2, "all intents must precede any publication");
    assert_eq!(
        scalar(
            &fixture.db,
            "SELECT count(*) AS value FROM module_artifact_data_object_migration_operations WHERE status='preparing'"
        ).await,
        1
    );
    assert_eq!(scalar(&fixture.db,"SELECT count(*) AS value FROM module_artifact_data_objects WHERE namespace_instance_id IN
        (SELECT target_namespace_instance_id FROM module_artifact_data_object_migration_operations)").await,0);
    let id: String = fixture
        .db
        .query_one_raw(Statement::from_string(
            DbBackend::Sqlite,
            "SELECT operation_id FROM module_artifact_data_object_migration_operations",
        ))
        .await
        .expect("operation query")
        .expect("operation")
        .try_get("", "operation_id")
        .expect("id");
    for (sql, values) in [
        (
            "UPDATE module_artifact_data_namespaces SET state='purged',purged_at=?1,namespace_revision=2 WHERE namespace_instance_id=?2",
            vec![
                Utc::now().into(),
                fixture
                    .request
                    .source_namespace_instance_id
                    .to_string()
                    .into(),
            ],
        ),
        (
            "INSERT INTO module_artifact_data_owner_references VALUES (?1,?2,?3,1)",
            vec![
                fixture.request.tenant_id.to_string().into(),
                fixture.request.data_owner_id.to_string().into(),
                fixture
                    .request
                    .target_namespace_instance_id
                    .to_string()
                    .into(),
            ],
        ),
        (
            "UPDATE module_artifact_data_namespaces SET state='serving' WHERE namespace_instance_id=?1",
            vec![
                fixture
                    .request
                    .target_namespace_instance_id
                    .to_string()
                    .into(),
            ],
        ),
        (
            "DELETE FROM module_artifact_data_object_migration_operations",
            vec![],
        ),
        (
            "UPDATE module_artifact_data_object_migration_operations SET request_json='{}'",
            vec![],
        ),
        (
            "UPDATE module_artifact_data_object_copy_operations SET target_storage_key='changed'",
            vec![],
        ),
        (
            "INSERT INTO module_artifact_data VALUES (?1,?2,?3,'late','{}',2,1,?4)",
            vec![
                fixture.request.tenant_id.to_string().into(),
                fixture.request.data_owner_id.to_string().into(),
                fixture
                    .request
                    .target_namespace_instance_id
                    .to_string()
                    .into(),
                Utc::now().into(),
            ],
        ),
    ] {
        assert!(
            fixture
                .db
                .execute_raw(Statement::from_sql_and_values(
                    DbBackend::Sqlite,
                    sql,
                    values
                ))
                .await
                .is_err(),
            "pending operation must block mutation: {sql}"
        );
    }
    fixture
        .storage
        .objects
        .put(
            &Path::from(keys[0].as_str()),
            Bytes::from_static(b"corrupt reserved target").into(),
        )
        .await
        .expect("target corruption injection");
    assert_eq!(
        service.migrate_objects(fixture.request.clone()).await,
        Err(Error::Integrity)
    );
    assert_eq!(
        fixture
            .storage
            .objects
            .get(&Path::from(keys[0].as_str()))
            .await
            .expect("reserved target")
            .bytes()
            .await
            .expect("reserved bytes"),
        Bytes::from_static(b"corrupt reserved target"),
        "migration cannot overwrite uncertain bytes"
    );
    fixture
        .storage
        .objects
        .put(
            &Path::from(keys[0].as_str()),
            fixture.objects[0].bytes.clone().into(),
        )
        .await
        .expect("restore injected target");
    fixture
        .storage
        .objects
        .put(
            &Path::from(fixture.objects[1].key.as_str()),
            fixture.objects[1].bytes.clone().into(),
        )
        .await
        .expect("restore injected source");
    execute(
        &fixture.db,
        "UPDATE fixture_migration_authority SET enabled=0",
        vec![],
    )
    .await;
    assert_eq!(
        service
            .reconcile_stale_intents(fixture.request.tenant_id, fixture.request.data_owner_id)
            .await,
        Err(Error::PolicyDenied)
    );
    assert_eq!(fixture.keys().await, keys);
    drop(service);
    fixture.reopen().await;
    let service = fixture.service();
    assert_eq!(
        service.migrate_objects(fixture.request.clone()).await,
        Err(Error::PolicyDenied)
    );
    execute(
        &fixture.db,
        "UPDATE fixture_migration_authority SET enabled=1",
        vec![],
    )
    .await;
    assert_eq!(
        service
            .reconcile_stale_intents(fixture.request.tenant_id, fixture.request.data_owner_id)
            .await
            .expect("exact durable reconciliation"),
        1
    );
    fixture.assert_copies().await;
    assert_eq!(fixture.keys().await, keys);
    let receipt = service
        .migrate_objects(fixture.request.clone())
        .await
        .expect("one terminal receipt");
    assert_eq!(receipt.operation_id.to_string(), id);
    assert_eq!(receipt.objects_migrated, 2);
    assert!(receipt.accepted);
    assert_eq!(
        scalar(
            &fixture.db,
            "SELECT count(*) AS value FROM module_artifact_data_object_copy_operations WHERE status='checkpointed'"
        ).await,
        2
    );
    drop(service);
    fixture.finish().await;
}

#[tokio::test]
async fn empty_inventory_still_requires_exact_scope_policy_and_durable_parent() {
    let fixture = Fixture::new(0).await;
    let service = fixture.service();
    let mut wrong_actor = fixture.request.clone();
    wrong_actor.context.actor_id = Uuid::new_v4();
    assert_eq!(
        service.migrate_objects(wrong_actor).await,
        Err(Error::PolicyDenied)
    );
    let mut stale = fixture.request.clone();
    stale.expected_target_namespace_revision = 2;
    assert_eq!(
        service.migrate_objects(stale).await,
        Err(Error::NamespacePrecondition)
    );
    let mut overflow = fixture.request.clone();
    overflow.expected_source_namespace_revision = u64::MAX;
    assert_eq!(
        service.migrate_objects(overflow).await,
        Err(Error::InvalidCommand)
    );
    let mut mismatch = fixture.request.clone();
    mismatch.context.tenant_id = Some(Uuid::new_v4());
    assert_eq!(
        service.migrate_objects(mismatch).await,
        Err(Error::TenantMismatch)
    );
    assert_eq!(
        scalar(
            &fixture.db,
            "SELECT count(*) AS value FROM module_artifact_data_object_migration_operations"
        )
        .await,
        0
    );
    let receipt = service
        .migrate_objects(fixture.request.clone())
        .await
        .expect("authorized empty migration");
    assert!(receipt.accepted);
    assert_eq!(receipt.objects_migrated, 0);
    assert_eq!(
        scalar(
            &fixture.db,
            "SELECT count(*) AS value FROM module_artifact_data_object_migration_operations WHERE status='committed'"
        ).await,
        1
    );
    assert_eq!(
        scalar(
            &fixture.db,
            "SELECT count(*) AS value FROM module_artifact_data_object_copy_operations"
        )
        .await,
        0
    );
    assert_eq!(
        service
            .migrate_objects(fixture.request.clone())
            .await
            .expect("empty replay"),
        receipt
    );
    let mut reused_target = fixture.request.clone();
    reused_target.context.idempotency_key = Uuid::new_v4();
    assert_eq!(
        service.migrate_objects(reused_target).await,
        Err(Error::NamespacePrecondition)
    );
    drop(service);
    fixture.finish().await;
}
