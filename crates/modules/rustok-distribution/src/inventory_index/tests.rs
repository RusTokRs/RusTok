//! Focused regression definitions. No test execution is claimed by the source-only handoff.
use std::sync::{
    Arc, Mutex,
    atomic::{AtomicBool, Ordering},
};

use async_trait::async_trait;
use rustok_api::{Permission, PortActor, PortContext, PortError};
use rustok_auth::{
    WorkerServiceContextPort, WorkerServiceContextRequest, WorkerServiceContextRuntime,
};
use rustok_core::{
    MigrationSource, ModuleEventListenerContext, ModuleEventListenerRegistry,
    ModuleRuntimeExtensions, RusToKModule,
    events::{ContractEventEnvelope, EventHandler},
};
use rustok_index::{
    EntityKey, FieldName, IndexModule, IndexMutation, IndexMutationEventCatalog,
    IndexSchemaSourceCatalog, IndexSource, IndexSourceCursor, IndexSourceLoadRequest,
    IndexSourceScanRequest, IndexValue, PostgresIndexSourceFactoryCatalog,
    materialize_index_schema_registry, materialize_index_source_registry,
    materialize_postgres_index_sources,
};
use rustok_inventory::source_history::{InventorySourceEvent, InventorySourceRecord};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
    TransactionTrait, Value as SqlValue,
};
use sea_orm_migration::SchemaManager;
use serde_json::json;
use uuid::Uuid;

use super::{
    INVENTORY_STOCK_INDEX_SOURCE,
    consumer::InventoryStockEventHandler,
    register,
    schema::{schema_ref, stock_schema},
    source::{InventoryStockSource, mutation},
};

const TENANT: Uuid = Uuid::from_u128(1);
const BASELINE: Uuid = Uuid::from_u128(2);
const VARIANT: Uuid = Uuid::from_u128(3);
const SERVICE: Uuid = Uuid::from_u128(30);

fn record() -> InventorySourceRecord {
    InventorySourceRecord {
        tenant_id: TENANT,
        baseline_id: BASELINE,
        variant_id: VARIANT,
        product_id: Uuid::from_u128(4),
        inventory_item_id: Uuid::from_u128(5),
        source_version: 2,
        deleted: false,
        stocked: 3,
        reserved: 8,
        incoming: 11,
        available: -5,
        change_id: Uuid::from_u128(102),
    }
}

fn key() -> EntityKey {
    EntityKey {
        tenant_id: TENANT,
        schema: schema_ref().unwrap(),
        entity_id: VARIANT,
        locale: None,
    }
}

fn changed(revision: u64, id: Uuid) -> ContractEventEnvelope {
    ContractEventEnvelope::new_with_envelope_id(
        id,
        TENANT,
        Some(Uuid::from_u128(99)), // Deliberately not the current authenticated service actor.
        InventorySourceEvent::Changed {
            baseline_id: BASELINE,
            variant_id: VARIANT,
            source_version: revision,
        },
    )
    .unwrap()
}

#[test]
fn selected_inventory_marker_contributes_one_native_source_schema_and_real_event_route() {
    let mut extensions = ModuleRuntimeExtensions::default();
    register(&mut extensions).unwrap();
    assert!(!extensions.contains::<PostgresIndexSourceFactoryCatalog>());
    extensions.insert(rustok_inventory::InventoryRuntimeSelected);
    register(&mut extensions).unwrap();
    let schemas = extensions.get::<IndexSchemaSourceCatalog>().unwrap();
    assert_eq!(schemas.len(), 1);
    assert_eq!(
        schemas.iter().next().unwrap().schema,
        stock_schema().unwrap()
    );
    let factories = extensions
        .get::<PostgresIndexSourceFactoryCatalog>()
        .unwrap();
    assert_eq!(factories.len(), 1);
    assert_eq!(
        factories.iter().next().unwrap().factory_name(),
        INVENTORY_STOCK_INDEX_SOURCE
    );
    let events = extensions.get::<IndexMutationEventCatalog>().unwrap();
    assert_eq!(events.len(), 1);
    assert!(events.get("inventory.source.changed").is_some());
}

#[test]
fn stock_projection_retains_original_uuid_owner_revision_and_signed_availability() {
    let row = record();
    let IndexMutation::Upsert { event_id, record } = mutation(&row, TENANT, BASELINE).unwrap()
    else {
        panic!("live record must be an upsert");
    };
    assert_eq!(event_id, row.change_id);
    assert_eq!(record.key, key());
    assert_eq!(record.source_version, 2);
    assert_eq!(
        record.fields[&FieldName::new("available").unwrap()],
        IndexValue::Integer(-5)
    );
    assert_eq!(record.fields.len(), 8);
    assert!(record.links.is_empty());
}

#[test]
fn retained_delete_requires_zero_individual_counters_and_keeps_original_key() {
    let mut row = record();
    row.deleted = true;
    row.stocked = 0;
    row.reserved = 0;
    row.incoming = 0;
    row.available = 0;
    assert!(matches!(mutation(&row, TENANT, BASELINE).unwrap(),
        IndexMutation::Delete { event_id, key: deleted_key, source_version: 2 }
            if event_id == row.change_id && deleted_key == key()));
    row.incoming = 1;
    assert!(mutation(&row, TENANT, BASELINE).is_err());
}

#[test]
fn projection_refuses_scope_identity_revision_and_counter_drift_without_zero_fallback() {
    let original = record();
    for kind in 0..11 {
        let mut row = original.clone();
        match kind {
            0 => row.tenant_id = Uuid::from_u128(900),
            1 => row.baseline_id = Uuid::from_u128(900),
            2 => row.variant_id = Uuid::nil(),
            3 => row.inventory_item_id = Uuid::nil(),
            4 => row.product_id = Uuid::nil(),
            5 => row.change_id = Uuid::nil(),
            6 => row.source_version = 0,
            7 => row.source_version = i64::MAX as u64 + 1,
            8 => row.stocked = -1,
            9 => row.reserved = -1,
            _ => row.available = 0,
        }
        assert!(
            mutation(&row, TENANT, BASELINE).is_err(),
            "invalid case {kind}"
        );
    }
}

#[derive(Clone, Default)]
struct CurrentAuthority {
    revoked: Arc<AtomicBool>,
    calls: Arc<Mutex<Vec<(Uuid, String, Uuid)>>>,
}

#[async_trait]
impl WorkerServiceContextPort for CurrentAuthority {
    async fn current_context(
        &self,
        request: WorkerServiceContextRequest,
    ) -> Result<PortContext, PortError> {
        self.calls.lock().unwrap().push((
            request.tenant_id(),
            request.consumer().into(),
            request.correlation_id(),
        ));
        if self.revoked.load(Ordering::SeqCst) {
            return Err(PortError::forbidden(
                "fixture.revoked",
                "Current service grant revoked",
            ));
        }
        Ok(PortContext::new(
            request.tenant_id().to_string(),
            PortActor::service(SERVICE.to_string()),
            "und",
            request.correlation_id().to_string(),
        )
        .with_claim(Permission::INVENTORY_READ.to_string())
        .with_deadline(request.budget()))
    }
}

struct Fixture {
    db: DatabaseConnection,
    authority: CurrentAuthority,
}

impl Fixture {
    async fn new() -> Self {
        Self::on(Database::connect("sqlite::memory:").await.unwrap()).await
    }

    async fn on(db: DatabaseConnection) -> Self {
        let tenant_type = if db.get_database_backend() == DbBackend::Postgres {
            "UUID"
        } else {
            "TEXT"
        };
        db.execute_unprepared(&format!("CREATE TABLE tenants (id {tenant_type} PRIMARY KEY); INSERT INTO tenants (id) VALUES ('{TENANT}')")).await.unwrap();
        let manager = SchemaManager::new(&db);
        for migration in IndexModule.migrations() {
            migration.up(&manager).await.unwrap();
        }
        let history = rustok_inventory::InventoryModule
            .migrations()
            .into_iter()
            .find(|migration| {
                migration.name() == "m20261101_000027_create_inventory_source_history"
            })
            .expect("canonical I027 migration must remain available");
        history.up(&manager).await.unwrap();
        let fixture = Self {
            db,
            authority: CurrentAuthority::default(),
        };
        // An empty, genuinely finalized baseline has a matching retained final-page receipt.
        let now = chrono::Utc::now().fixed_offset();
        fixture.exec("INSERT INTO inventory_source_baselines (tenant_id, baseline_id, actor_id, active, record_count, last_page_id, last_page_limit, created_at, updated_at) VALUES ($1,$2,$3,TRUE,0,$4,128,$5,$6)",
            vec![TENANT.into(), BASELINE.into(), Uuid::from_u128(99).into(), Uuid::from_u128(6).into(), now.into(), now.into()]).await;
        fixture.exec("INSERT INTO inventory_source_baseline_pages (tenant_id, page_id, baseline_id, page_limit, active, record_count, created_at) VALUES ($1,$2,$3,128,TRUE,0,$4)",
            vec![TENANT.into(), Uuid::from_u128(6).into(), BASELINE.into(), now.into()]).await;
        fixture
    }

    async fn exec(&self, sql: &str, values: Vec<SqlValue>) {
        self.db
            .execute_raw(Statement::from_sql_and_values(
                self.db.get_database_backend(),
                sql,
                values,
            ))
            .await
            .unwrap();
    }

    fn source(&self) -> InventoryStockSource {
        InventoryStockSource::new(
            self.db.clone(),
            Some(WorkerServiceContextRuntime::new(Arc::new(
                self.authority.clone(),
            ))),
        )
    }

    async fn put(&self, row: &InventorySourceRecord) {
        let transaction = self.db.begin().await.unwrap();
        let backend = self.db.get_database_backend();
        let now = chrono::Utc::now().fixed_offset();
        transaction.execute_raw(Statement::from_sql_and_values(backend,
            "INSERT INTO inventory_source_changes (change_id, tenant_id, variant_id, baseline_id, source_version, kind, created_at) VALUES ($1,$2,$3,$4,$5,$6,$7)",
            vec![row.change_id.into(), row.tenant_id.into(), row.variant_id.into(), row.baseline_id.into(), (row.source_version as i64).into(), if row.deleted { "deleted" } else { "changed" }.into(), now.into()])).await.unwrap();
        transaction.execute_raw(Statement::from_sql_and_values(backend,
            "INSERT INTO inventory_source_records (tenant_id, variant_id, baseline_id, inventory_item_id, product_id, source_version, deleted, stocked, reserved, incoming, binding_digest, last_change_id, updated_at) VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10,$11,$12,$13) ON CONFLICT(tenant_id, variant_id) DO UPDATE SET source_version=excluded.source_version, deleted=excluded.deleted, stocked=excluded.stocked, reserved=excluded.reserved, incoming=excluded.incoming, last_change_id=excluded.last_change_id, updated_at=excluded.updated_at",
            vec![row.tenant_id.into(), row.variant_id.into(), row.baseline_id.into(), row.inventory_item_id.into(), row.product_id.into(), (row.source_version as i64).into(), row.deleted.into(), row.stocked.into(), row.reserved.into(), row.incoming.into(), "0".repeat(64).into(), row.change_id.into(), now.into()])).await.unwrap();
        transaction.commit().await.unwrap();
    }

    fn extensions(&self) -> ModuleRuntimeExtensions {
        let mut extensions = ModuleRuntimeExtensions::default();
        extensions.insert(rustok_inventory::InventoryRuntimeSelected);
        extensions.insert(WorkerServiceContextRuntime::new(Arc::new(
            self.authority.clone(),
        )));
        register(&mut extensions).unwrap();
        extensions.insert(
            materialize_index_schema_registry(&extensions)
                .unwrap()
                .unwrap(),
        );
        materialize_postgres_index_sources(&mut extensions, self.db.clone()).unwrap();
        extensions.insert(
            materialize_index_source_registry(&extensions)
                .unwrap()
                .unwrap(),
        );
        extensions
    }

    fn handler(&self) -> InventoryStockEventHandler {
        InventoryStockEventHandler::new(&ModuleEventListenerContext {
            db: self.db.clone(),
            extensions: &self.extensions(),
        })
    }

    async fn inbox_count(&self) -> i64 {
        self.db
            .query_one_raw(Statement::from_string(
                self.db.get_database_backend(),
                "SELECT COUNT(*) AS n FROM index_inbox",
            ))
            .await
            .unwrap()
            .unwrap()
            .try_get("", "n")
            .unwrap()
    }
}

#[tokio::test]
async fn actual_index_listener_lifecycle_registers_the_selected_inventory_handler() {
    let fixture = Fixture::new().await;
    let extensions = fixture.extensions();
    let mut listeners = ModuleEventListenerRegistry::new();
    IndexModule.register_event_listeners(
        &mut listeners,
        &ModuleEventListenerContext {
            db: fixture.db.clone(),
            extensions: &extensions,
        },
    );
    let handlers = listeners.into_handlers();
    assert_eq!(handlers.len(), 1);
    assert_eq!(handlers[0].name(), "inventory_stock_index");
}

#[tokio::test]
async fn every_owner_invocation_reauthenticates_and_never_reuses_event_or_baseline_actor() {
    let fixture = Fixture::new().await;
    fixture.put(&record()).await;
    let source = fixture.source();
    source
        .load(IndexSourceLoadRequest::new(vec![key()]).unwrap())
        .await
        .unwrap();
    assert_eq!(fixture.authority.calls.lock().unwrap()[0].0, TENANT);
    assert_eq!(
        fixture.authority.calls.lock().unwrap()[0].1,
        INVENTORY_STOCK_INDEX_SOURCE
    );
    fixture.authority.revoked.store(true, Ordering::SeqCst);
    assert!(
        source
            .load(IndexSourceLoadRequest::new(vec![key()]).unwrap())
            .await
            .is_err()
    );
    assert_eq!(fixture.authority.calls.lock().unwrap().len(), 2);
    assert!(
        fixture
            .handler()
            .handle_contract(&changed(2, record().change_id))
            .await
            .is_err()
    );
    assert_eq!(fixture.inbox_count().await, 0);
}

#[tokio::test]
async fn missing_authority_or_missing_exact_key_never_synthesizes_stock_or_delete() {
    let fixture = Fixture::new().await;
    assert!(
        InventoryStockSource::new(fixture.db.clone(), None)
            .load(IndexSourceLoadRequest::new(vec![key()]).unwrap())
            .await
            .is_err()
    );
    assert!(
        fixture
            .source()
            .load(IndexSourceLoadRequest::new(vec![key()]).unwrap())
            .await
            .is_err()
    );
    assert_eq!(fixture.inbox_count().await, 0);
}

#[tokio::test]
async fn live_scan_cursor_is_tenant_and_baseline_bound_without_snapshot_or_absence_claim() {
    let fixture = Fixture::new().await;
    let mut row = record();
    for value in 10..13 {
        row.variant_id = Uuid::from_u128(value);
        row.change_id = Uuid::from_u128(value + 1000);
        fixture.put(&row).await;
    }
    let source = fixture.source();
    let first = source
        .scan(IndexSourceScanRequest::new(TENANT, schema_ref().unwrap(), None, 2).unwrap())
        .await
        .unwrap();
    assert_eq!(first.mutations().len(), 2);
    let cursor = first
        .next_cursor()
        .cloned()
        .expect("third live row requires continuation");
    let final_page = source
        .scan(
            IndexSourceScanRequest::new(TENANT, schema_ref().unwrap(), Some(cursor.clone()), 2)
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(final_page.mutations().len(), 1);
    assert!(final_page.next_cursor().is_none());
    for (field, value) in [
        ("tenant_id", Uuid::from_u128(99)),
        ("baseline_id", Uuid::from_u128(99)),
    ] {
        let mut wire = cursor.value().clone();
        wire[field] = json!(value);
        assert!(
            source
                .scan(
                    IndexSourceScanRequest::new(
                        TENANT,
                        schema_ref().unwrap(),
                        Some(IndexSourceCursor::new(wire).unwrap()),
                        2
                    )
                    .unwrap()
                )
                .await
                .is_err()
        );
    }
}

#[tokio::test]
async fn changed_persists_before_success_and_old_redelivery_keeps_advancing_owner_mutation_ids() {
    let fixture = Fixture::new().await;
    let mut row = record();
    fixture.put(&row).await;
    let handler = fixture.handler();
    let original = changed(2, row.change_id);
    handler.handle_contract(&original).await.unwrap();
    assert_eq!(fixture.inbox_count().await, 1);
    row.source_version = 3;
    row.change_id = Uuid::from_u128(103);
    row.stocked = 4;
    row.available = -4;
    fixture.put(&row).await;
    handler.handle_contract(&original).await.unwrap();
    assert_eq!(fixture.inbox_count().await, 2);
    handler.handle_contract(&original).await.unwrap();
    assert_eq!(fixture.inbox_count().await, 2);
    let saved = fixture
        .db
        .query_one_raw(Statement::from_string(
            fixture.db.get_database_backend(),
            "SELECT CAST(source_version AS TEXT) AS version FROM index_entities",
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.try_get::<String>("", "version").unwrap(), "3");
}

#[tokio::test]
async fn owner_tombstone_is_durable_and_wrong_change_or_storage_failure_withholds_success() {
    let fixture = Fixture::new().await;
    let mut row = record();
    fixture.put(&row).await;
    let handler = fixture.handler();
    assert!(
        handler
            .handle_contract(&changed(2, Uuid::from_u128(999)))
            .await
            .is_err()
    );
    assert_eq!(fixture.inbox_count().await, 0);
    handler
        .handle_contract(&changed(2, row.change_id))
        .await
        .unwrap();
    row.source_version = 3;
    row.change_id = Uuid::from_u128(103);
    row.deleted = true;
    row.stocked = 0;
    row.reserved = 0;
    row.incoming = 0;
    row.available = 0;
    fixture.put(&row).await;
    handler
        .handle_contract(&changed(2, Uuid::from_u128(102)))
        .await
        .unwrap();
    let saved = fixture
        .db
        .query_one_raw(Statement::from_string(
            fixture.db.get_database_backend(),
            "SELECT is_deleted FROM index_entities",
        ))
        .await
        .unwrap()
        .unwrap();
    assert!(saved.try_get::<bool>("", "is_deleted").unwrap());
    fixture
        .db
        .execute_unprepared("DROP TABLE index_inbox")
        .await
        .unwrap();
    assert!(
        handler
            .handle_contract(&changed(3, row.change_id))
            .await
            .is_err()
    );
}

#[tokio::test]
async fn activation_refuses_wrong_original_envelope_identity_before_any_replay_ack() {
    let fixture = Fixture::new().await;
    let event = ContractEventEnvelope::new_with_envelope_id(
        Uuid::from_u128(999),
        TENANT,
        Some(Uuid::from_u128(99)),
        InventorySourceEvent::BaselineActivated {
            baseline_id: BASELINE,
        },
    )
    .unwrap();
    assert!(fixture.handler().handle_contract(&event).await.is_err());
    assert!(fixture.authority.calls.lock().unwrap().is_empty());
}

#[tokio::test]
#[ignore = "requires isolated PostgreSQL credentials in RUSTOK_INVENTORY_INDEX_DATABASE_URL"]
async fn postgres_empty_activation_durably_enqueues_once_under_original_baseline_uuid() {
    let (fixture, schema) = postgres_fixture().await;
    let handler = fixture.handler();
    let event = ContractEventEnvelope::new_with_envelope_id(
        BASELINE,
        TENANT,
        Some(Uuid::from_u128(99)),
        InventorySourceEvent::BaselineActivated {
            baseline_id: BASELINE,
        },
    )
    .unwrap();
    handler.handle_contract(&event).await.unwrap();
    handler.handle_contract(&event).await.unwrap();
    let jobs = fixture
        .db
        .query_all_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT job_id, state, attempt_count FROM index_jobs WHERE kind='rebuild'",
        ))
        .await
        .unwrap();
    assert_eq!(jobs.len(), 1);
    assert_eq!(jobs[0].try_get::<Uuid>("", "job_id").unwrap(), BASELINE);
    assert_eq!(jobs[0].try_get::<String>("", "state").unwrap(), "pending");
    assert_eq!(jobs[0].try_get::<i32>("", "attempt_count").unwrap(), 0);
    fixture
        .db
        .execute_unprepared(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .await
        .unwrap();
}

#[tokio::test]
async fn changed_refuses_unregistered_wire_version_before_authority_or_persistence() {
    let fixture = Fixture::new().await;
    fixture.put(&record()).await;
    let mut wire = serde_json::to_value(changed(2, record().change_id)).unwrap();
    wire["schema_version"] = json!(2);
    let unregistered: ContractEventEnvelope = serde_json::from_value(wire).unwrap();
    assert!(
        fixture
            .handler()
            .handle_contract(&unregistered)
            .await
            .is_err()
    );
    assert!(fixture.authority.calls.lock().unwrap().is_empty());
    assert_eq!(fixture.inbox_count().await, 0);
}

#[tokio::test]
async fn explicitly_empty_active_baseline_returns_an_empty_final_live_page_not_synthetic_stock() {
    let fixture = Fixture::new().await;
    let page = fixture
        .source()
        .scan(IndexSourceScanRequest::new(TENANT, schema_ref().unwrap(), None, 128).unwrap())
        .await
        .unwrap();
    assert!(page.mutations().is_empty());
    assert!(page.next_cursor().is_none());
}

// Each PostgreSQL definition gets one isolated schema on one pinned connection.
// The authority fixture exercises the consumer boundary, not real OAuth/deployment readiness.
async fn postgres_fixture() -> (Fixture, String) {
    let url = std::env::var("RUSTOK_INVENTORY_INDEX_DATABASE_URL")
        .expect("explicit isolated PostgreSQL test database is required");
    assert!(url.starts_with("postgres://") || url.starts_with("postgresql://"));
    let mut options = ConnectOptions::new(url);
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    let db = Database::connect(options).await.unwrap();
    let schema = format!("inventory_index_{}", Uuid::new_v4().simple());
    db.execute_unprepared(&format!(
        "CREATE SCHEMA \"{schema}\"; SET search_path TO \"{schema}\""
    ))
    .await
    .unwrap();
    let fixture = Fixture::on(db).await;
    (fixture, schema)
}

#[tokio::test]
#[ignore = "requires explicit isolated RUSTOK_INVENTORY_INDEX_DATABASE_URL PostgreSQL database"]
async fn postgres_changed_redelivery_keeps_one_inbox_per_owner_change_including_tombstone() {
    let (fixture, schema) = postgres_fixture().await;
    let mut row = record();
    fixture.put(&row).await;
    let handler = fixture.handler();
    let original = changed(row.source_version, row.change_id);
    handler.handle_contract(&original).await.unwrap();
    handler.handle_contract(&original).await.unwrap();
    assert_eq!(fixture.inbox_count().await, 1);

    row.source_version = 3;
    row.change_id = Uuid::from_u128(103);
    row.deleted = true;
    row.stocked = 0;
    row.reserved = 0;
    row.incoming = 0;
    row.available = 0;
    fixture.put(&row).await;
    // Old notification loads and persists the newer owner UUID, then exact redelivery
    // must decode the retained inbox as a duplicate, not fail with an INT4/INT8 mismatch.
    handler.handle_contract(&original).await.unwrap();
    handler.handle_contract(&original).await.unwrap();
    assert_eq!(fixture.inbox_count().await, 2);
    let saved = fixture
        .db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT CAST(source_version AS TEXT) AS version, is_deleted FROM index_entities",
        ))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(saved.try_get::<String>("", "version").unwrap(), "3");
    assert!(saved.try_get::<bool>("", "is_deleted").unwrap());
    fixture
        .db
        .execute_unprepared(&format!("DROP SCHEMA \"{schema}\" CASCADE"))
        .await
        .unwrap();
}
