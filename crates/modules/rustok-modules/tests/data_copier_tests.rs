//! Actual owner-transaction copying, exact replay, schema/index integrity, and rollback.
//! Persisted fixture policy is bounded evidence, not a production maintenance fence.

use async_trait::async_trait;
use chrono::Utc;
use rustok_core::MigrationSource;
use rustok_modules::{
    ArtifactDataCopier, ArtifactDataCopyAuthorization, ArtifactDataCopyAuthorizer,
    ArtifactDataCopyError as Error, ArtifactDataCopyRequest as Request, ArtifactDataIndexField,
    ArtifactDataIndexValueType, ArtifactDataNamespace, ArtifactDataQuota, ArtifactDataScope,
    ArtifactModuleKind, ArtifactPayloadKind, ArtifactPersistenceContract, ArtifactSchemaDocument,
    MigrationPreflightInput, ModuleArtifactDescriptor, ModuleCommandContext, ModulesModule,
    UpdateMode, canonical_schema_digest, evaluate_migration_preflight,
};
use sea_orm::{
    ConnectionTrait, Database, DatabaseConnection, DatabaseTransaction, DbBackend, Statement, Value,
};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use sha2::{Digest, Sha256};
use std::path::PathBuf;
use uuid::Uuid;

fn digest(value: &impl serde::Serialize) -> String {
    format!(
        "sha256:{}",
        hex::encode(Sha256::digest(
            serde_json::to_vec(value).expect("canonical JSON")
        ))
    )
}

async fn execute(db: &impl ConnectionTrait, sql: &str, values: Vec<Value>) {
    db.execute_raw(Statement::from_sql_and_values(
        DbBackend::Sqlite,
        sql,
        values,
    ))
    .await
    .expect("fixture write");
}
async fn scalar(db: &impl ConnectionTrait, sql: &str) -> i64 {
    db.query_one_raw(Statement::from_string(DbBackend::Sqlite, sql))
        .await
        .expect("query")
        .expect("row")
        .try_get("", "value")
        .expect("scalar")
}

struct FixturePolicy;
#[async_trait]
impl ArtifactDataCopyAuthorizer for FixturePolicy {
    async fn authorize_copy_on(
        &self,
        transaction: &DatabaseTransaction,
        request: &Request,
        source: &ArtifactDataNamespace,
        target: &ArtifactDataNamespace,
    ) -> Result<ArtifactDataCopyAuthorization, Error> {
        let row = transaction.query_one_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "SELECT descriptor,quota,policy_revision FROM fixture_copy_authority
             WHERE tenant_id=?1 AND data_owner_id=?2 AND actor_id=?3 AND source_namespace_instance_id=?4
              AND target_namespace_instance_id=?5 AND enabled=1",
            vec![request.tenant_id.to_string().into(),request.data_owner_id.to_string().into(),
                request.context.actor_id.to_string().into(),source.namespace_instance_id.to_string().into(),
                target.namespace_instance_id.to_string().into()]))
            .await.map_err(|error|Error::Storage(error.to_string()))?.ok_or(Error::PolicyDenied)?;
        if source.tenant_id != request.tenant_id
            || target.tenant_id != source.tenant_id
            || source.data_owner_id != request.data_owner_id
            || target.data_owner_id != source.data_owner_id
        {
            return Err(Error::PolicyDenied);
        }
        let descriptor: ModuleArtifactDescriptor = serde_json::from_str(
            &row.try_get::<String>("", "descriptor")
                .map_err(|error| Error::Storage(error.to_string()))?,
        )
        .map_err(|_| Error::PolicyDenied)?;
        let quota: ArtifactDataQuota = serde_json::from_str(
            &row.try_get::<String>("", "quota")
                .map_err(|error| Error::Storage(error.to_string()))?,
        )
        .map_err(|_| Error::PolicyDenied)?;
        let policy_revision = u64::try_from(
            row.try_get::<i64>("", "policy_revision")
                .map_err(|error| Error::Storage(error.to_string()))?,
        )
        .map_err(|_| Error::PolicyDenied)?;
        Ok(ArtifactDataCopyAuthorization {
            target_scope: ArtifactDataScope {
                tenant_id: target.tenant_id,
                data_owner_id: target.data_owner_id,
                namespace_instance_id: target.namespace_instance_id,
                module_slug: target.module_slug.clone(),
                data_contract_revision: target.data_contract_revision,
                data_contract_digest: target.data_contract_digest.clone(),
                policy_revision,
            },
            target_descriptor: descriptor,
            quota,
        })
    }
}

fn descriptor() -> ModuleArtifactDescriptor {
    let schema = serde_json::json!({
        "$schema":"https://json-schema.org/draft/2020-12/schema","type":"object",
        "properties":{"name":{"type":"string"},"stock":{"type":"integer"}},
        "required":["name","stock"],"additionalProperties":false
    });
    let schema_digest = canonical_schema_digest(&schema);
    let descriptor = ModuleArtifactDescriptor {
        schema_version: rustok_modules::MODULE_ARTIFACT_DESCRIPTOR_SCHEMA_VERSION,
        slug: "copy_fixture".into(),
        version: "1.0.0".into(),
        payload_kind: ArtifactPayloadKind::Rhai,
        module_kind: ArtifactModuleKind::Optional,
        runtime_abi: "rustok:module/runtime@1".into(),
        platform_compatibility: "^0.1".into(),
        required_features: vec![],
        artifact_digest: format!(
            "sha256:{}",
            hex::encode(Sha256::digest(b"fn main() { return (); }"))
        ),
        entrypoint: "main".into(),
        capabilities: vec![],
        bindings: vec![],
        dependencies: vec![],
        permissions: vec![],
        schema_documents: vec![ArtifactSchemaDocument {
            digest: schema_digest.clone(),
            document: schema,
        }],
        settings_schema_digest: None,
        data_schema_digest: Some(schema_digest.clone()),
        localization_catalogs: vec![],
        ui_contributions: vec![],
        persistence_contract: Some(ArtifactPersistenceContract {
            revision: 2,
            schema_digest,
            indexes: vec![ArtifactDataIndexField {
                name: "stock".into(),
                json_pointer: "/stock".into(),
                value_type: ArtifactDataIndexValueType::Number,
            }],
        }),
    };
    descriptor.validate().expect("real descriptor validation");
    descriptor
}

struct Fixture {
    db: DatabaseConnection,
    request: Request,
    root: PathBuf,
    url: String,
}
impl Fixture {
    async fn new(count: u32) -> Self {
        Self::with_descriptor(count, descriptor()).await
    }

    async fn with_descriptor(count: u32, target: ModuleArtifactDescriptor) -> Self {
        let workspace_target = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../../target");
        std::fs::create_dir_all(&workspace_target).expect("target directory");
        let workspace_target = workspace_target
            .canonicalize()
            .expect("resolved workspace target");
        let root = workspace_target.join(format!("data-copy-fixture-{}", Uuid::new_v4()));
        assert!(
            root.is_absolute() && root.starts_with(&workspace_target) && root != workspace_target
        );
        std::fs::create_dir(&root).expect("isolated fixture directory");
        let database_path = root.join("control-plane.db").display().to_string();
        let database_path = database_path
            .strip_prefix(r"\\?\")
            .unwrap_or(&database_path);
        let url = format!("sqlite://{}?mode=rwc", database_path.replace('\\', "/"));
        let db = Database::connect(url.as_str())
            .await
            .expect("durable SQLite");
        rustok_outbox::SysEventsMigration
            .up(&SchemaManager::new(&db))
            .await
            .expect("real outbox migration");
        for migration in ModulesModule.migrations() {
            migration
                .up(&SchemaManager::new(&db))
                .await
                .expect("canonical owner migration");
        }
        let tenant_id = Uuid::new_v4();
        let request = Request {
            tenant_id,
            data_owner_id: Uuid::new_v4(),
            source_namespace_instance_id: Uuid::new_v4(),
            target_namespace_instance_id: Uuid::new_v4(),
            expected_source_namespace_revision: 1,
            expected_target_namespace_revision: 1,
            page_size: 2,
            page_cursor: None,
            context: ModuleCommandContext {
                tenant_id: Some(tenant_id),
                actor_id: Uuid::new_v4(),
                idempotency_key: Uuid::new_v4(),
                trace_id: "test:atomic-record-copy".into(),
                correlation_id: Uuid::new_v4(),
            },
            reason: "Copy sealed fixture records into a non-serving instance".into(),
        };
        let source_contract = ArtifactPersistenceContract {
            revision: 1,
            indexes: vec![],
            schema_digest: target.data_schema_digest.clone().expect("data schema"),
        };
        for (instance, revision, contract) in [
            (
                request.source_namespace_instance_id,
                1,
                digest(&source_contract),
            ),
            (
                request.target_namespace_instance_id,
                2,
                digest(
                    target
                        .persistence_contract
                        .as_ref()
                        .expect("target contract"),
                ),
            ),
        ] {
            execute(&db,"INSERT INTO module_artifact_data_namespaces
                (tenant_id,data_owner_id,namespace_instance_id,module_slug,data_contract_revision,data_contract_digest,state,namespace_revision,created_at,updated_at)
                VALUES (?1,?2,?3,'copy_fixture',?4,?5,'staging',1,?6,?6)",
                vec![tenant_id.to_string().into(),request.data_owner_id.to_string().into(),instance.to_string().into(),revision.into(),contract.into(),Utc::now().into()]).await;
        }
        let mut inventory = Vec::new();
        for index in 1..=count {
            let key = format!("item_{index:02}");
            let value = serde_json::json!({"name":format!("Item {index}"),"stock":index*10});
            let encoded = serde_json::to_string(&value).expect("value JSON");
            execute(&db,"INSERT INTO module_artifact_data
                (tenant_id,data_owner_id,namespace_instance_id,data_key,value,value_size_bytes,revision,updated_at) VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
                vec![tenant_id.to_string().into(),request.data_owner_id.to_string().into(),request.source_namespace_instance_id.to_string().into(),
                    key.clone().into(),encoded.clone().into(),i64::try_from(encoded.len()).expect("size").into(),i64::from(index+10).into(),Utc::now().into()]).await;
            inventory.push((key, value, index + 10));
        }
        // Seal the actual bounded inventory. This is not a production full-snapshot attestation.
        execute(&db,"UPDATE module_artifact_data_namespaces SET state='verified',verified_manifest_digest=?1 WHERE namespace_instance_id=?2",
            vec![digest(&inventory).into(),request.source_namespace_instance_id.to_string().into()]).await;
        execute(&db,"CREATE TABLE fixture_copy_authority (
            tenant_id TEXT NOT NULL,data_owner_id TEXT NOT NULL,actor_id TEXT NOT NULL,source_namespace_instance_id TEXT NOT NULL,
            target_namespace_instance_id TEXT NOT NULL,descriptor TEXT NOT NULL,quota TEXT NOT NULL,
            policy_revision INTEGER NOT NULL CHECK(policy_revision>0),enabled INTEGER NOT NULL CHECK(enabled IN (0,1)))",vec![]).await;
        execute(
            &db,
            "INSERT INTO fixture_copy_authority VALUES (?1,?2,?3,?4,?5,?6,?7,1,1)",
            vec![
                tenant_id.to_string().into(),
                request.data_owner_id.to_string().into(),
                request.context.actor_id.to_string().into(),
                request.source_namespace_instance_id.to_string().into(),
                request.target_namespace_instance_id.to_string().into(),
                serde_json::to_string(&target).expect("descriptor").into(),
                serde_json::to_string(&ArtifactDataQuota::default())
                    .expect("quota")
                    .into(),
            ],
        )
        .await;
        Self {
            db,
            request,
            root,
            url,
        }
    }
    fn copier(&self) -> ArtifactDataCopier<FixturePolicy> {
        ArtifactDataCopier::new(self.db.clone(), FixturePolicy)
    }
    async fn target_count(&self) -> i64 {
        self.db
            .query_one_raw(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "SELECT COUNT(*) AS value FROM module_artifact_data WHERE namespace_instance_id=?1",
                vec![self.request.target_namespace_instance_id.to_string().into()],
            ))
            .await
            .expect("target count")
            .expect("row")
            .try_get("", "value")
            .expect("count")
    }
    async fn target_revision(&self) -> i64 {
        self.db.query_one_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
            "SELECT namespace_revision AS value FROM module_artifact_data_namespaces WHERE namespace_instance_id=?1",
            vec![self.request.target_namespace_instance_id.to_string().into()]))
            .await.expect("target revision").expect("row").try_get("","value").expect("revision")
    }
    async fn reopen(&mut self) {
        self.db.clone().close().await.expect("close owner pool");
        self.db = Database::connect(self.url.as_str())
            .await
            .expect("reopen durable owner database");
    }
    async fn finish(self) {
        self.db.close().await.expect("close fixture");
        let workspace_target = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../../target")
            .canonicalize()
            .expect("target");
        let resolved = self
            .root
            .canonicalize()
            .expect("resolved fixture directory");
        assert!(
            resolved == self.root
                && resolved.starts_with(&workspace_target)
                && resolved != workspace_target
        );
        std::fs::remove_dir_all(resolved).expect("remove isolated fixture");
    }
}

async fn assert_namespaces_held(fixture: &Fixture) {
    let target_has_records = fixture.target_count().await > 0;
    for instance in [
        fixture.request.source_namespace_instance_id,
        fixture.request.target_namespace_instance_id,
    ] {
        let is_source = instance == fixture.request.source_namespace_instance_id;
        let instance = instance.to_string();
        for sql in [
            "UPDATE module_artifact_data_namespaces SET namespace_revision=namespace_revision+1 WHERE namespace_instance_id=?1",
            "DELETE FROM module_artifact_data_namespaces WHERE namespace_instance_id=?1",
            "UPDATE module_artifact_data SET updated_at='2026-09-14T00:00:00Z' WHERE namespace_instance_id=?1",
            "DELETE FROM module_artifact_data WHERE namespace_instance_id=?1",
            "UPDATE module_artifact_data_index_contracts SET contract_digest=contract_digest WHERE namespace_instance_id=?1",
            "DELETE FROM module_artifact_data_index_contracts WHERE namespace_instance_id=?1",
            "UPDATE module_artifact_data_indexes SET data_key=data_key WHERE namespace_instance_id=?1",
            "DELETE FROM module_artifact_data_indexes WHERE namespace_instance_id=?1",
        ] {
            // Empty tables have no row-level effect. Root mutation and real rows must reject.
            if sql.contains("index") && (is_source || !target_has_records) {
                continue;
            }
            if sql.contains("module_artifact_data ") && !is_source && !target_has_records {
                continue;
            }
            let result = fixture
                .db
                .execute_raw(Statement::from_sql_and_values(
                    DbBackend::Sqlite,
                    sql,
                    vec![instance.clone().into()],
                ))
                .await;
            let error = result.expect_err("held namespace must reject mutation");
            assert!(
                error.to_string().contains("held"),
                "hold guard rejected {sql}: {error}"
            );
        }
        let insert = fixture.db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_data
             (tenant_id,data_owner_id,namespace_instance_id,data_key,value,value_size_bytes,revision,updated_at)
             VALUES (?1,?2,?3,'unowned_write','{}',2,1,?4)",
            vec![fixture.request.tenant_id.to_string().into(),
                fixture.request.data_owner_id.to_string().into(),instance.clone().into(),Utc::now().into()]
        )).await;
        assert!(
            insert
                .expect_err("unrelated record must reject")
                .to_string()
                .contains("held")
        );
        let reference = fixture.db.execute_raw(Statement::from_sql_and_values(
            DbBackend::Sqlite,
            "INSERT INTO module_artifact_data_owner_references
             (tenant_id,data_owner_id,namespace_instance_id,reference_revision) VALUES (?1,?2,?3,1)",
            vec![fixture.request.tenant_id.to_string().into(),
                fixture.request.data_owner_id.to_string().into(),instance.into()]
        )).await;
        assert!(
            reference
                .expect_err("serving reference must reject")
                .to_string()
                .contains("held")
        );
    }
}

#[test]
fn preflight_classifies_required_data_copy_as_maintenance() {
    let receipt = evaluate_migration_preflight(MigrationPreflightInput {
        operation_id: Uuid::new_v4(),
        module_slug: "inventory".into(),
        source_schema_digest: canonical_schema_digest(&serde_json::json!({"type":"object"})),
        target_schema_digest: descriptor()
            .data_schema_digest
            .expect("target schema digest"),
        migration_plan_digest: digest(
            &serde_json::json!({"module_slug":"inventory","requires_data_copy":true}),
        ),
        is_additive_safe: true,
        migration_reasons: vec![],
        settings_guard_installed: true,
        has_irreversible_external_effects: false,
        requires_cross_revision_data_copy: true,
    });
    assert_eq!(receipt.mode, UpdateMode::Maintenance);
    assert!(
        receipt
            .denial_reasons
            .iter()
            .any(|reason| reason.contains("maintenance-only"))
    );
}

#[tokio::test]
async fn pages_preserve_records_indexes_continuation_and_exact_replay_after_reopen() {
    let mut fixture = Fixture::new(4).await;
    let copier = fixture.copier();
    let first = copier
        .copy_page(fixture.request.clone())
        .await
        .expect("first page");
    assert_eq!(first.items_copied, 2);
    assert_eq!(first.next_page_cursor.as_deref(), Some("item_02"));
    assert!(!first.is_terminal_page);
    assert_eq!(first.target_namespace_revision, 2);
    let mut second_request = fixture.request.clone();
    second_request.context.idempotency_key = Uuid::new_v4();
    second_request.expected_target_namespace_revision = 2;
    second_request.page_cursor = first.next_page_cursor.clone();
    let second = copier.copy_page(second_request).await.expect("second page");
    assert_eq!(second.items_copied, 2);
    assert!(second.is_terminal_page);
    assert!(second.next_page_cursor.is_none());
    assert_eq!(fixture.target_count().await, 4);
    assert_eq!(fixture.target_revision().await, 3);
    let target_rows=fixture.db.query_all_raw(Statement::from_sql_and_values(DbBackend::Sqlite,
        "SELECT data_key,revision,CAST(value AS TEXT) AS value_text FROM module_artifact_data WHERE namespace_instance_id=?1 ORDER BY data_key",
        vec![fixture.request.target_namespace_instance_id.to_string().into()])).await.expect("target inventory");
    for (index, row) in target_rows.iter().enumerate() {
        let index = u32::try_from(index).expect("index") + 1;
        assert_eq!(
            row.try_get::<i64>("", "revision")
                .expect("preserved revision"),
            i64::from(index + 10)
        );
        let value: serde_json::Value =
            serde_json::from_str(&row.try_get::<String>("", "value_text").expect("JSON"))
                .expect("value");
        assert_eq!(
            value,
            serde_json::json!({"name":format!("Item {index}"),"stock":index*10})
        );
    }
    assert_eq!(
        scalar(
            &fixture.db,
            "SELECT count(*) AS value FROM module_artifact_data_indexes"
        )
        .await,
        4
    );
    assert_eq!(
        scalar(
            &fixture.db,
            "SELECT count(*) AS value FROM module_artifact_data_index_contracts"
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
    execute(
        &fixture.db,
        "UPDATE fixture_copy_authority SET enabled=0",
        vec![],
    )
    .await;
    execute(&fixture.db,"UPDATE module_artifact_data_namespaces SET state='purged',purged_at=?1,namespace_revision=2 WHERE namespace_instance_id=?2",
        vec![Utc::now().into(),fixture.request.source_namespace_instance_id.to_string().into()]).await;
    drop(copier);
    fixture.reopen().await;
    assert_eq!(
        fixture
            .copier()
            .copy_page(fixture.request.clone())
            .await
            .expect("exact replay after source purge and revocation"),
        first
    );
    assert_eq!(fixture.target_revision().await, 3);
    for sql in [
        "UPDATE module_artifact_data_copy_operations SET reason='changed'",
        "DELETE FROM module_artifact_data_copy_operations",
    ] {
        assert!(
            fixture.db.execute_unprepared(sql).await.is_err(),
            "immutable receipt rejected {sql}"
        );
    }
    fixture.finish().await;
}

#[tokio::test]
async fn changed_request_identity_never_replays_or_mutates() {
    let fixture = Fixture::new(3).await;
    let copier = fixture.copier();
    let original = copier
        .copy_page(fixture.request.clone())
        .await
        .expect("first page");
    let mut variants = Vec::new();
    let mut changed = fixture.request.clone();
    changed.reason.push_str(" changed");
    variants.push(changed);
    let mut changed = fixture.request.clone();
    changed.page_size = 3;
    variants.push(changed);
    let mut changed = fixture.request.clone();
    changed.page_cursor = Some("item_01".into());
    variants.push(changed);
    let mut changed = fixture.request.clone();
    changed.expected_source_namespace_revision = 2;
    variants.push(changed);
    let mut changed = fixture.request.clone();
    changed.expected_target_namespace_revision = 2;
    variants.push(changed);
    let mut changed = fixture.request.clone();
    changed.target_namespace_instance_id = Uuid::new_v4();
    variants.push(changed);
    let mut changed = fixture.request.clone();
    changed.source_namespace_instance_id = Uuid::new_v4();
    variants.push(changed);
    let mut changed = fixture.request.clone();
    changed.context.actor_id = Uuid::new_v4();
    variants.push(changed);
    let mut changed = fixture.request.clone();
    changed.context.correlation_id = Uuid::new_v4();
    variants.push(changed);
    let mut changed = fixture.request.clone();
    changed.context.trace_id.push_str(":changed");
    variants.push(changed);
    for changed in variants {
        assert_eq!(
            copier.copy_page(changed).await,
            Err(Error::IdempotencyConflict)
        );
    }
    assert_eq!(
        copier
            .copy_page(fixture.request.clone())
            .await
            .expect("original response"),
        original
    );
    assert_eq!(fixture.target_count().await, 2);
    assert_eq!(fixture.target_revision().await, 2);
    assert_eq!(
        scalar(
            &fixture.db,
            "SELECT count(*) AS value FROM module_artifact_data_copy_operations"
        )
        .await,
        1
    );
    drop(copier);
    fixture.finish().await;
}

#[tokio::test]
async fn policy_namespace_and_cursor_checks_precede_page_writes() {
    let fixture = Fixture::new(3).await;
    let copier = fixture.copier();
    let mut changed = fixture.request.clone();
    changed.context.actor_id = Uuid::new_v4();
    assert_eq!(copier.copy_page(changed).await, Err(Error::PolicyDenied));
    let mut changed = fixture.request.clone();
    changed.page_size = 0;
    assert_eq!(copier.copy_page(changed).await, Err(Error::InvalidCommand));
    let mut changed = fixture.request.clone();
    changed.expected_source_namespace_revision = u64::MAX;
    assert_eq!(copier.copy_page(changed).await, Err(Error::InvalidCommand));
    let mut changed = fixture.request.clone();
    changed.expected_target_namespace_revision = 2;
    assert_eq!(
        copier.copy_page(changed).await,
        Err(Error::NamespacePrecondition)
    );
    let mut changed = fixture.request.clone();
    changed.tenant_id = Uuid::new_v4();
    changed.context.tenant_id = Some(changed.tenant_id);
    assert_eq!(
        copier.copy_page(changed).await,
        Err(Error::NamespacePrecondition)
    );
    let mut changed = fixture.request.clone();
    changed.page_cursor = Some("item_02".into());
    assert_eq!(
        copier.copy_page(changed).await,
        Err(Error::NamespacePrecondition)
    );
    execute(
        &fixture.db,
        "UPDATE fixture_copy_authority SET enabled=0",
        vec![],
    )
    .await;
    assert_eq!(
        copier.copy_page(fixture.request.clone()).await,
        Err(Error::PolicyDenied)
    );
    assert_eq!(fixture.target_count().await, 0);
    assert_eq!(fixture.target_revision().await, 1);
    assert_eq!(
        scalar(
            &fixture.db,
            "SELECT count(*) AS value FROM module_artifact_data_copy_operations"
        )
        .await,
        0
    );
    drop(copier);
    fixture.finish().await;
}

#[tokio::test]
async fn quota_denial_precedes_admission_and_namespace_holds_protect_continuation() {
    let fixture = Fixture::new(5).await;
    let copier = fixture.copier();
    let quota = ArtifactDataQuota {
        max_structured_records: 1,
        ..Default::default()
    };
    execute(
        &fixture.db,
        "UPDATE fixture_copy_authority SET quota=?1",
        vec![serde_json::to_string(&quota).expect("quota").into()],
    )
    .await;
    assert_eq!(
        copier.copy_page(fixture.request.clone()).await,
        Err(Error::QuotaExceeded)
    );
    assert_eq!(fixture.target_count().await, 0);
    assert_eq!(fixture.target_revision().await, 1);
    assert_eq!(
        scalar(
            &fixture.db,
            "SELECT count(*) AS value FROM module_artifact_data_indexes"
        )
        .await,
        0
    );
    assert_eq!(
        scalar(
            &fixture.db,
            "SELECT count(*) AS value FROM module_artifact_data_index_contracts"
        )
        .await,
        0
    );
    assert_eq!(
        scalar(
            &fixture.db,
            "SELECT count(*) AS value FROM module_artifact_data_copy_operations"
        )
        .await,
        0
    );
    execute(
        &fixture.db,
        "UPDATE fixture_copy_authority SET quota=?1",
        vec![
            serde_json::to_string(&ArtifactDataQuota::default())
                .expect("quota")
                .into(),
        ],
    )
    .await;
    let first = copier
        .copy_page(fixture.request.clone())
        .await
        .expect("first page");
    assert!(!first.is_terminal_page);
    assert_namespaces_held(&fixture).await;
    let mut request = fixture.request.clone();
    request.page_cursor = first.next_page_cursor;
    request.expected_target_namespace_revision = 2;
    request.context.idempotency_key = Uuid::new_v4();
    let second = copier
        .copy_page(request)
        .await
        .expect("authorized next page");
    assert!(!second.is_terminal_page);
    assert_eq!(fixture.target_count().await, 4);
    assert_eq!(fixture.target_revision().await, 3);
    assert_eq!(
        scalar(
            &fixture.db,
            "SELECT count(*) AS value FROM module_artifact_data_indexes"
        )
        .await,
        4
    );
    assert_eq!(
        scalar(
            &fixture.db,
            "SELECT count(*) AS value FROM module_artifact_data_copy_operations"
        )
        .await,
        2
    );
    assert_namespaces_held(&fixture).await;
    drop(copier);
    fixture.finish().await;
}

#[tokio::test]
async fn changed_admitted_contract_preserves_the_previous_checkpoint() {
    let fixture = Fixture::new(3).await;
    let copier = fixture.copier();
    let first = copier
        .copy_page(fixture.request.clone())
        .await
        .expect("first page");
    let mut target = descriptor();
    let schema = &mut target.schema_documents[0].document;
    schema["properties"]["stock"]["maximum"] = serde_json::json!(20);
    let schema_digest = canonical_schema_digest(schema);
    target.schema_documents[0].digest = schema_digest.clone();
    target.data_schema_digest = Some(schema_digest.clone());
    target
        .persistence_contract
        .as_mut()
        .expect("contract")
        .schema_digest = schema_digest;
    // The owner rejects changed admitted contract evidence before a record can be copied.
    execute(
        &fixture.db,
        "UPDATE fixture_copy_authority SET descriptor=?1",
        vec![serde_json::to_string(&target).expect("descriptor").into()],
    )
    .await;
    let mut request = fixture.request.clone();
    request.context.idempotency_key = Uuid::new_v4();
    request.expected_target_namespace_revision = 2;
    request.page_cursor = first.next_page_cursor;
    assert_eq!(copier.copy_page(request).await, Err(Error::PolicyDenied));
    assert_eq!(fixture.target_count().await, 2);
    assert_eq!(fixture.target_revision().await, 2);
    assert_eq!(
        scalar(
            &fixture.db,
            "SELECT count(*) AS value FROM module_artifact_data_copy_operations"
        )
        .await,
        1
    );
    drop(copier);
    fixture.finish().await;
}

#[tokio::test]
async fn empty_source_commits_one_terminal_page_and_cannot_restart_with_another_key() {
    let fixture = Fixture::new(0).await;
    let copier = fixture.copier();
    let receipt = copier
        .copy_page(fixture.request.clone())
        .await
        .expect("empty terminal page");
    assert_eq!(receipt.items_copied, 0);
    assert!(receipt.is_terminal_page);
    assert_eq!(receipt.target_namespace_revision, 2);
    let mut request = fixture.request.clone();
    request.context.idempotency_key = Uuid::new_v4();
    request.expected_target_namespace_revision = 2;
    assert_eq!(
        copier.copy_page(request).await,
        Err(Error::NamespacePrecondition)
    );
    assert_eq!(
        copier
            .copy_page(fixture.request.clone())
            .await
            .expect("exact empty replay"),
        receipt
    );
    drop(copier);
    fixture.finish().await;
}

#[tokio::test]
async fn target_schema_violation_rolls_back_the_initial_index_binding() {
    let mut target = descriptor();
    target.schema_documents[0].document["properties"]["stock"]["maximum"] = serde_json::json!(20);
    let schema_digest = canonical_schema_digest(&target.schema_documents[0].document);
    target.schema_documents[0].digest = schema_digest.clone();
    target.data_schema_digest = Some(schema_digest.clone());
    target
        .persistence_contract
        .as_mut()
        .expect("contract")
        .schema_digest = schema_digest;
    target.validate().expect("restricted target descriptor");
    let fixture = Fixture::with_descriptor(3, target).await;
    let copier = fixture.copier();
    let mut request = fixture.request.clone();
    request.page_size = 3;
    assert_eq!(copier.copy_page(request).await, Err(Error::SchemaViolation));
    assert_eq!(fixture.target_count().await, 0);
    assert_eq!(fixture.target_revision().await, 1);
    assert_eq!(
        scalar(
            &fixture.db,
            "SELECT count(*) AS value FROM module_artifact_data_index_contracts"
        )
        .await,
        0
    );
    assert_eq!(
        scalar(
            &fixture.db,
            "SELECT count(*) AS value FROM module_artifact_data_copy_operations"
        )
        .await,
        0
    );
    drop(copier);
    fixture.finish().await;
}

#[tokio::test]
async fn receipt_commit_failure_preserves_intent_and_rolls_back_all_page_effects() {
    let mut fixture = Fixture::new(2).await;
    let copier = fixture.copier();
    execute(&fixture.db,
        "CREATE TRIGGER fixture_reject_copy_receipt BEFORE UPDATE ON module_artifact_data_copy_operations
         WHEN NEW.status='committed'
         BEGIN SELECT RAISE(ABORT,'Injected receipt storage failure'); END", vec![]).await;
    assert!(matches!(
        copier.copy_page(fixture.request.clone()).await,
        Err(Error::Storage(_))
    ));
    assert_eq!(fixture.target_count().await, 0);
    assert_eq!(fixture.target_revision().await, 1);
    assert_eq!(
        scalar(
            &fixture.db,
            "SELECT count(*) AS value FROM module_artifact_data_indexes"
        )
        .await,
        0
    );
    assert_eq!(
        scalar(
            &fixture.db,
            "SELECT count(*) AS value FROM module_artifact_data_index_contracts"
        )
        .await,
        0
    );
    assert_eq!(
        scalar(
            &fixture.db,
            "SELECT count(*) AS value FROM module_artifact_data_copy_operations WHERE status='preparing' AND receipt_json IS NULL"
        )
        .await,
        1
    );
    assert_namespaces_held(&fixture).await;
    let reservation = copier
        .reserve_page(fixture.request.clone())
        .await
        .expect("persisted admission");
    assert!(!reservation.is_committed);
    drop(copier);
    fixture.reopen().await;
    let copier = fixture.copier();
    execute(
        &fixture.db,
        "DROP TRIGGER fixture_reject_copy_receipt",
        vec![],
    )
    .await;
    let receipt = copier
        .copy_page(fixture.request.clone())
        .await
        .expect("retry after storage recovery");
    assert_eq!(receipt.operation_id, reservation.operation_id);
    assert!(receipt.is_terminal_page);
    assert_eq!(fixture.target_count().await, 2);
    assert_eq!(fixture.target_revision().await, 2);
    drop(copier);
    fixture.finish().await;
}

#[tokio::test]
async fn pending_page_survives_reopen_and_reconciliation_rechecks_current_authority() {
    let mut fixture = Fixture::new(3).await;
    let copier = fixture.copier();
    let reservation = copier
        .reserve_page(fixture.request.clone())
        .await
        .expect("durable admission");
    assert!(!reservation.is_committed);
    assert_eq!(fixture.target_count().await, 0);
    assert_eq!(fixture.target_revision().await, 1);
    let row = fixture.db.query_one_raw(Statement::from_string(DbBackend::Sqlite,
        "SELECT request_json,frozen_page_json FROM module_artifact_data_copy_operations WHERE status='preparing'"))
        .await.expect("intent query").expect("durable intent");
    assert_eq!(
        serde_json::from_str::<Request>(
            &row.try_get::<String>("", "request_json")
                .expect("request JSON")
        )
        .expect("exact original request"),
        fixture.request
    );
    let frozen: serde_json::Value = serde_json::from_str(
        &row.try_get::<String>("", "frozen_page_json")
            .expect("frozen JSON"),
    )
    .expect("frozen records");
    assert_eq!(frozen["records"].as_array().expect("records").len(), 2);
    assert_eq!(frozen["records"][0]["key"], "item_01");
    assert_eq!(frozen["records"][0]["revision"], 11);
    assert_eq!(frozen["reservation_scope"]["policy_revision"], 1);
    let mut changed = fixture.request.clone();
    changed.reason.push_str(" changed");
    assert_eq!(
        copier.reserve_page(changed).await,
        Err(Error::IdempotencyConflict)
    );
    let mut competing = fixture.request.clone();
    competing.context.idempotency_key = Uuid::new_v4();
    assert_eq!(
        copier.reserve_page(competing).await,
        Err(Error::NamespacePrecondition)
    );
    for sql in [
        "UPDATE module_artifact_data_copy_operations SET request_json='{}'",
        "UPDATE module_artifact_data_copy_operations SET frozen_page_json='{}'",
        "UPDATE module_artifact_data_copy_operations SET status='committed'",
        "DELETE FROM module_artifact_data_copy_operations",
    ] {
        assert!(
            fixture.db.execute_unprepared(sql).await.is_err(),
            "durable evidence rejected {sql}"
        );
    }
    assert_namespaces_held(&fixture).await;
    execute(
        &fixture.db,
        "UPDATE fixture_copy_authority SET enabled=0",
        vec![],
    )
    .await;
    drop(copier);
    fixture.reopen().await;
    let copier = fixture.copier();
    let results = copier
        .reconcile_pending_pages(fixture.request.tenant_id, fixture.request.data_owner_id)
        .await
        .expect("scoped reconciliation");
    assert_eq!(results, vec![Err(Error::PolicyDenied)]);
    assert_eq!(fixture.target_count().await, 0);
    assert_namespaces_held(&fixture).await;
    let quota = ArtifactDataQuota {
        max_structured_records: 1,
        ..Default::default()
    };
    execute(
        &fixture.db,
        "UPDATE fixture_copy_authority SET enabled=1,policy_revision=2,quota=?1",
        vec![serde_json::to_string(&quota).expect("quota").into()],
    )
    .await;
    assert_eq!(
        copier
            .reconcile_pending_pages(fixture.request.tenant_id, fixture.request.data_owner_id)
            .await
            .expect("quota reconciliation"),
        vec![Err(Error::QuotaExceeded)]
    );
    assert_eq!(fixture.target_count().await, 0);
    assert_eq!(fixture.target_revision().await, 1);
    execute(
        &fixture.db,
        "UPDATE fixture_copy_authority SET quota=?1",
        vec![
            serde_json::to_string(&ArtifactDataQuota::default())
                .expect("quota")
                .into(),
        ],
    )
    .await;
    let mut results = copier
        .reconcile_pending_pages(fixture.request.tenant_id, fixture.request.data_owner_id)
        .await
        .expect("authorized reconciliation");
    assert_eq!(results.len(), 1);
    let first = results.remove(0).expect("committed original page");
    assert_eq!(first.operation_id, reservation.operation_id);
    assert_eq!(first.policy_revision, 2);
    assert_eq!(first.page_digest, reservation.page_digest);
    assert_eq!(first.items_copied, 2);
    assert!(!first.is_terminal_page);
    assert_eq!(fixture.target_count().await, 2);
    assert_eq!(fixture.target_revision().await, 2);
    assert!(
        copier
            .reconcile_pending_pages(fixture.request.tenant_id, fixture.request.data_owner_id)
            .await
            .expect("no pending intent")
            .is_empty()
    );
    assert_namespaces_held(&fixture).await;
    let mut next = fixture.request.clone();
    next.page_cursor = first.next_page_cursor;
    next.expected_target_namespace_revision = first.target_namespace_revision;
    next.context.idempotency_key = Uuid::new_v4();
    let terminal = copier.copy_page(next).await.expect("terminal continuation");
    assert!(terminal.is_terminal_page);
    assert_eq!(fixture.target_count().await, 3);
    assert_eq!(fixture.target_revision().await, 3);
    execute(&fixture.db,
        "UPDATE module_artifact_data_namespaces SET state='purged',purged_at=?1,namespace_revision=2 WHERE namespace_instance_id=?2",
        vec![Utc::now().into(),fixture.request.source_namespace_instance_id.to_string().into()]).await;
    assert_eq!(
        scalar(
            &fixture.db,
            "SELECT count(*) AS value FROM module_artifact_data_owner_references"
        )
        .await,
        0
    );
    drop(copier);
    fixture.finish().await;
}

#[tokio::test]
async fn initial_copy_rejects_prepopulated_or_serving_target_and_skipped_continuation() {
    let fixture = Fixture::new(5).await;
    let copier = fixture.copier();
    execute(
        &fixture.db,
        "UPDATE module_artifact_data_namespaces SET state='serving' WHERE namespace_instance_id=?1",
        vec![
            fixture
                .request
                .target_namespace_instance_id
                .to_string()
                .into(),
        ],
    )
    .await;
    assert_eq!(
        copier.copy_page(fixture.request.clone()).await,
        Err(Error::NamespacePrecondition)
    );
    execute(
        &fixture.db,
        "UPDATE module_artifact_data_namespaces SET state='staging' WHERE namespace_instance_id=?1",
        vec![
            fixture
                .request
                .target_namespace_instance_id
                .to_string()
                .into(),
        ],
    )
    .await;
    execute(&fixture.db, "INSERT INTO module_artifact_data
        (tenant_id,data_owner_id,namespace_instance_id,data_key,value,value_size_bytes,revision,updated_at)
        VALUES (?1,?2,?3,'unrelated','{}',2,1,?4)",
        vec![fixture.request.tenant_id.to_string().into(),fixture.request.data_owner_id.to_string().into(),
            fixture.request.target_namespace_instance_id.to_string().into(),Utc::now().into()]).await;
    assert_eq!(
        copier.copy_page(fixture.request.clone()).await,
        Err(Error::NamespacePrecondition)
    );
    execute(
        &fixture.db,
        "DELETE FROM module_artifact_data WHERE namespace_instance_id=?1",
        vec![
            fixture
                .request
                .target_namespace_instance_id
                .to_string()
                .into(),
        ],
    )
    .await;
    copier
        .copy_page(fixture.request.clone())
        .await
        .expect("initial empty target copy");
    let mut request = fixture.request.clone();
    request.context.idempotency_key = Uuid::new_v4();
    request.expected_target_namespace_revision = 2;
    request.page_cursor = Some("item_03".into());
    assert_eq!(
        copier.copy_page(request).await,
        Err(Error::NamespacePrecondition)
    );
    assert_eq!(fixture.target_count().await, 2);
    assert_eq!(fixture.target_revision().await, 2);
    assert_eq!(
        scalar(
            &fixture.db,
            "SELECT count(*) AS value FROM module_artifact_data_copy_operations"
        )
        .await,
        1
    );
    drop(copier);
    fixture.finish().await;
}
