use std::{
    collections::BTreeMap,
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
};

use async_graphql::{EmptySubscription, Request, Schema};
use async_trait::async_trait;
use rustok_api::Permission;
use rustok_core::{
    MigrationSource, ModuleRegistry, ModuleRuntimeExtensions, RusToKModule, UserRole,
};
use rustok_index::{
    EntityKey, EntityName, FieldCardinality, FieldName, IndexField, IndexModule, IndexMutation,
    IndexRecord, IndexSchema, IndexSource, IndexSourceFailure, IndexSourceLoadBatch,
    IndexSourceLoadRequest, IndexSourcePage, IndexSourceScanRequest, IndexValue, IndexValueType,
    LocaleMode, ModuleName, PostgresSchemaRegistrationStore, SchemaRef, SchemaVersion,
    SharedIndexSchemaRegistry, register_index_schema_source, register_index_source,
};
use sea_orm::{ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement};
use sea_orm_migration::{MigrationTrait, SchemaManager};
use tokio::sync::Notify;
use uuid::Uuid;

use super::index_replay::IndexReplayMutation;
use crate::context::{AuthContext, TenantContext};
use crate::services::app_lifecycle::StopHandle;
use crate::services::index_replay_runtime_composition::materialize_index_replay_runtime;
use crate::services::rbac_request_scope::{RbacRequestScope, with_rbac_request_scope};

const TENANT_ID: Uuid = Uuid::from_u128(1);
const ACTOR_ID: Uuid = Uuid::from_u128(2);
const ENTITY_ID: Uuid = Uuid::from_u128(101);
const EVENT_ID: Uuid = Uuid::from_u128(1001);
const SOURCE_NAME: &str = "shutdown-evidence-primary";

#[derive(Clone)]
struct ScanGate {
    started: Arc<Notify>,
    release: Arc<Notify>,
}

struct ShutdownReplayModule {
    gate: Option<ScanGate>,
    source_calls: Arc<AtomicUsize>,
}

struct ShutdownSource {
    gate: Option<ScanGate>,
    source_calls: Arc<AtomicUsize>,
}

#[async_trait]
impl IndexSource for ShutdownSource {
    async fn scan(
        &self,
        request: IndexSourceScanRequest,
    ) -> Result<IndexSourcePage, IndexSourceFailure> {
        self.source_calls.fetch_add(1, Ordering::SeqCst);
        if let Some(gate) = &self.gate {
            gate.started.notify_one();
            gate.release.notified().await;
        }

        let mutation = IndexMutation::Upsert {
            event_id: EVENT_ID,
            record: IndexRecord {
                key: EntityKey {
                    tenant_id: request.tenant_id(),
                    schema: shutdown_schema_ref(),
                    entity_id: ENTITY_ID,
                    locale: None,
                },
                source_version: 1,
                fields: BTreeMap::from([(
                    FieldName::new("id").expect("field name"),
                    IndexValue::Uuid(ENTITY_ID),
                )]),
                links: Vec::new(),
            },
        };

        Ok(IndexSourcePage::new(&request, vec![mutation], None)
            .expect("shutdown evidence page should satisfy source contract"))
    }

    async fn load(
        &self,
        request: IndexSourceLoadRequest,
    ) -> Result<IndexSourceLoadBatch, IndexSourceFailure> {
        Ok(IndexSourceLoadBatch::new(&request, Vec::new())
            .expect("empty targeted load should satisfy source contract"))
    }
}

impl MigrationSource for ShutdownReplayModule {
    fn migrations(&self) -> Vec<Box<dyn MigrationTrait>> {
        Vec::new()
    }
}

#[async_trait]
impl RusToKModule for ShutdownReplayModule {
    fn slug(&self) -> &'static str {
        "shutdown_evidence"
    }

    fn name(&self) -> &'static str {
        "Shutdown evidence"
    }

    fn description(&self) -> &'static str {
        "Deterministic Index replay GraphQL shutdown evidence source"
    }

    fn version(&self) -> &'static str {
        "0.1.0"
    }

    fn register_runtime_extensions(
        &self,
        extensions: &mut ModuleRuntimeExtensions,
    ) -> rustok_core::Result<()> {
        let schema = shutdown_schema();
        register_index_schema_source(extensions, self.slug(), schema.clone()).map_err(|error| {
            rustok_core::Error::Validation(format!(
                "shutdown evidence schema registration failed: {error}"
            ))
        })?;
        register_index_source(
            extensions,
            self.slug(),
            SOURCE_NAME,
            [schema.reference],
            ShutdownSource {
                gate: self.gate.clone(),
                source_calls: self.source_calls.clone(),
            },
        )
        .map_err(|error| {
            rustok_core::Error::Validation(format!(
                "shutdown evidence source registration failed: {error}"
            ))
        })
    }
}

#[derive(Default)]
struct EmptyQuery;

#[async_graphql::Object]
impl EmptyQuery {
    async fn _empty(&self) -> bool {
        true
    }
}

type ReplayTestSchema = Schema<EmptyQuery, IndexReplayMutation, EmptySubscription>;

struct ReplayGraphqlRuntime {
    schema: ReplayTestSchema,
    stop_handle: StopHandle,
    _stop_receiver: tokio::sync::watch::Receiver<bool>,
    source_calls: Arc<AtomicUsize>,
}

#[tokio::test]
async fn graphql_replay_observes_shared_stop_and_fresh_runtime_resumes_pending_job() {
    let db = replay_database().await;
    let gate = ScanGate {
        started: Arc::new(Notify::new()),
        release: Arc::new(Notify::new()),
    };

    let first = graphql_runtime(&db, Some(gate.clone())).await;
    let first_schema = first.schema.clone();
    let request_task = tokio::spawn(async move {
        with_rbac_request_scope(
            Some(operator_scope()),
            first_schema.execute(replay_request()),
        )
        .await
    });

    // The real source is already inside scan, so the pre-scan safe point has passed. Stop is
    // published deterministically while scan is pending; once released, the pre-mutation safe point
    // must observe the same shared StopHandle and yield the job without applying the mutation.
    gate.started.notified().await;
    first.stop_handle.stop().await;
    assert!(first.stop_handle.is_stopping());
    gate.release.notify_one();

    let first_response = request_task
        .await
        .expect("GraphQL replay task should join without panic");
    assert!(
        first_response.errors.is_empty(),
        "shutdown-yield GraphQL response must not contain errors: {:?}",
        first_response.errors
    );
    let first_data = first_response
        .data
        .into_json()
        .expect("GraphQL response data should convert to JSON");
    let first_run = &first_data["runIndexReplay"];
    assert_eq!(first_run["status"], "YIELDED");
    assert_eq!(first_run["pagesProcessed"], 0);
    assert_eq!(first_run["mutationsProcessed"], 0);
    assert_eq!(first_run["appliedCount"], 0);
    assert_eq!(first_run["duplicateCount"], 0);
    let job_id = Uuid::parse_str(
        first_run["jobId"]
            .as_str()
            .expect("yielded replay should expose job id"),
    )
    .expect("GraphQL job id should be a UUID");

    assert_eq!(first.source_calls.load(Ordering::SeqCst), 1);
    assert_eq!(job_state(&db).await, "pending");
    assert_eq!(job_attempt_count(&db).await, 1);
    assert_eq!(pending_uncancelled_lease_free_jobs(&db).await, 1);
    assert_eq!(checkpoint_count(&db).await, 0);
    assert_eq!(materialized_entity_count(&db).await, 0);
    assert_eq!(applied_inbox_count(&db).await, 0);

    // Fresh server/runtime/GraphQL composition gets a fresh non-stopping lifecycle handle while
    // reusing the same durable database. The same authorized command must reclaim the pending job as
    // attempt 2 and complete from the last committed checkpoint.
    let restarted = graphql_runtime(&db, None).await;
    assert!(!restarted.stop_handle.is_stopping());
    let second_response = with_rbac_request_scope(
        Some(operator_scope()),
        restarted.schema.execute(replay_request()),
    )
    .await;
    assert!(
        second_response.errors.is_empty(),
        "restart GraphQL response must not contain errors: {:?}",
        second_response.errors
    );
    let second_data = second_response
        .data
        .into_json()
        .expect("restart GraphQL response data should convert to JSON");
    let second_run = &second_data["runIndexReplay"];
    assert_eq!(second_run["status"], "COMPLETE");
    assert_eq!(second_run["jobId"], job_id.to_string());
    assert_eq!(second_run["pagesProcessed"], 1);
    assert_eq!(second_run["mutationsProcessed"], 1);
    assert_eq!(second_run["appliedCount"], 1);
    assert_eq!(second_run["duplicateCount"], 0);

    assert_eq!(restarted.source_calls.load(Ordering::SeqCst), 1);
    assert_eq!(job_state(&db).await, "succeeded");
    assert_eq!(job_attempt_count(&db).await, 2);
    assert_eq!(checkpoint_count(&db).await, 1);
    assert_eq!(materialized_entity_count(&db).await, 1);
    assert_eq!(applied_inbox_count(&db).await, 1);
}

async fn replay_database() -> DatabaseConnection {
    let db = Database::connect("sqlite::memory:")
        .await
        .expect("in-memory SQLite should connect");
    db.execute_unprepared("PRAGMA foreign_keys = ON")
        .await
        .expect("foreign keys should be enabled");
    db.execute_unprepared("CREATE TABLE tenants (id TEXT PRIMARY KEY)")
        .await
        .expect("tenant fixture should be created");
    db.execute_unprepared(&format!("INSERT INTO tenants (id) VALUES ('{TENANT_ID}')"))
        .await
        .expect("tenant fixture should be inserted");

    let manager = SchemaManager::new(&db);
    for migration in IndexModule.migrations() {
        migration
            .up(&manager)
            .await
            .unwrap_or_else(|error| panic!("{} should apply: {error}", migration.name()));
    }
    db
}

async fn graphql_runtime(db: &DatabaseConnection, gate: Option<ScanGate>) -> ReplayGraphqlRuntime {
    let source_calls = Arc::new(AtomicUsize::new(0));
    let registry = ModuleRegistry::new()
        .register(IndexModule)
        .register(ShutdownReplayModule {
            gate,
            source_calls: source_calls.clone(),
        });
    let mut extensions = rustok_distribution::build_runtime_extensions(&registry)
        .expect("shutdown evidence runtime extensions should compose");

    let schemas = extensions
        .get::<SharedIndexSchemaRegistry>()
        .cloned()
        .expect("shutdown evidence schema registry should be present");
    let schema_store = PostgresSchemaRegistrationStore::new(db.clone());
    for registered in schemas.registry().iter() {
        schema_store
            .register(TENANT_ID, &registered.schema)
            .await
            .expect("shutdown evidence schema should persist idempotently");
    }

    materialize_index_replay_runtime(&mut extensions, db.clone())
        .expect("guarded replay runtime should materialize");
    let extensions = Arc::new(extensions);
    let (stop_handle, stop_receiver) = StopHandle::new();
    let schema = Schema::build(EmptyQuery, IndexReplayMutation, EmptySubscription)
        .data(extensions)
        .data(stop_handle.clone())
        .finish();

    ReplayGraphqlRuntime {
        schema,
        stop_handle,
        _stop_receiver: stop_receiver,
        source_calls,
    }
}

fn replay_request() -> Request {
    Request::new(
        r#"
mutation {
  runIndexReplay(input: {
    moduleName: "shutdown-evidence"
    entityName: "item"
    schemaVersion: "1"
  }) {
    status
    jobId
    pagesProcessed
    mutationsProcessed
    appliedCount
    duplicateCount
    staleCount
  }
}
"#,
    )
    .data(AuthContext {
        user_id: ACTOR_ID,
        session_id: Uuid::new_v4(),
        tenant_id: TENANT_ID,
        permissions: vec![Permission::MODULES_MANAGE],
        client_id: None,
        scopes: Vec::new(),
        grant_type: "direct".to_string(),
    })
    .data(TenantContext {
        id: TENANT_ID,
        name: "Shutdown evidence".to_string(),
        slug: "shutdown-evidence".to_string(),
        domain: None,
        settings: serde_json::json!({}),
        default_locale: "en".to_string(),
        is_active: true,
    })
}

fn operator_scope() -> RbacRequestScope {
    RbacRequestScope::new(
        TENANT_ID,
        ACTOR_ID,
        vec![Permission::MODULES_MANAGE],
        UserRole::Admin,
    )
}

fn shutdown_schema_ref() -> SchemaRef {
    SchemaRef {
        module: ModuleName::new("shutdown-evidence").expect("module name"),
        entity: EntityName::new("item").expect("entity name"),
        version: SchemaVersion::INITIAL,
    }
}

fn shutdown_schema() -> IndexSchema {
    IndexSchema {
        reference: shutdown_schema_ref(),
        locale_mode: LocaleMode::None,
        fields: vec![IndexField {
            name: FieldName::new("id").expect("field name"),
            value_type: IndexValueType::Uuid,
            cardinality: FieldCardinality::One,
            nullable: false,
            selectable: true,
            filterable: true,
            sortable: true,
        }],
        links: Vec::new(),
    }
}

async fn scalar_i64(db: &DatabaseConnection, sql: &str) -> i64 {
    db.query_one_raw(Statement::from_string(DbBackend::Sqlite, sql.to_owned()))
        .await
        .expect("scalar query should execute")
        .expect("scalar query should return a row")
        .try_get("", "value")
        .expect("scalar value should be integer")
}

async fn scalar_string(db: &DatabaseConnection, sql: &str) -> String {
    db.query_one_raw(Statement::from_string(DbBackend::Sqlite, sql.to_owned()))
        .await
        .expect("scalar query should execute")
        .expect("scalar query should return a row")
        .try_get("", "value")
        .expect("scalar value should be text")
}

async fn job_state(db: &DatabaseConnection) -> String {
    scalar_string(
        db,
        "SELECT state AS value FROM index_jobs WHERE kind = 'rebuild'",
    )
    .await
}

async fn job_attempt_count(db: &DatabaseConnection) -> i64 {
    scalar_i64(
        db,
        "SELECT attempt_count AS value FROM index_jobs WHERE kind = 'rebuild'",
    )
    .await
}

async fn pending_uncancelled_lease_free_jobs(db: &DatabaseConnection) -> i64 {
    scalar_i64(
        db,
        "SELECT COUNT(*) AS value FROM index_jobs WHERE kind = 'rebuild' AND state = 'pending' AND cancel_requested = FALSE AND lease_owner IS NULL AND lease_expires_at IS NULL AND completed_at IS NULL",
    )
    .await
}

async fn checkpoint_count(db: &DatabaseConnection) -> i64 {
    scalar_i64(
        db,
        "SELECT COUNT(*) AS value FROM index_checkpoints WHERE checkpoint_kind = 'rebuild'",
    )
    .await
}

async fn materialized_entity_count(db: &DatabaseConnection) -> i64 {
    scalar_i64(
        db,
        "SELECT COUNT(*) AS value FROM index_entities WHERE tenant_id = '00000000-0000-0000-0000-000000000001' AND module_name = 'shutdown-evidence' AND entity_name = 'item' AND schema_version = 1 AND entity_id = '00000000-0000-0000-0000-000000000065' AND is_deleted = 0",
    )
    .await
}

async fn applied_inbox_count(db: &DatabaseConnection) -> i64 {
    scalar_i64(
        db,
        "SELECT COUNT(*) AS value FROM index_inbox WHERE tenant_id = '00000000-0000-0000-0000-000000000001' AND source_name = 'shutdown-evidence-primary' AND delivery_id = '00000000-0000-0000-0000-0000000003e9' AND state = 'applied'",
    )
    .await
}
