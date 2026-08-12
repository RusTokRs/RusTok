use std::collections::HashMap;
use std::env;
use std::error::Error;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rustok_core::MigrationSource;
use rustok_core::events::{EventHandler, EventTransport};
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_outbox::entity::SysEventStatus;
use rustok_outbox::{OutboxModule, OutboxTransport, SysEvents, TransactionalEventBus};
use rustok_pages::{
    PAGES_CACHE_ENTITY_KIND, PageCacheError, PageCacheGenerationSnapshot,
    PageCacheInvalidationCause, PageCacheInvalidationEventHandler, PageCacheInvalidationPort,
    PageCacheInvalidationReceipt, PageCacheInvalidationRequest, PageCacheScope,
    PagesCacheInvalidationRuntime, PagesCacheReadPort, PagesCacheReadRuntime, PagesModule,
    page_cache_key, storefront_pages_cache_key,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait,
    Statement, TransactionTrait,
};
use sea_orm_migration::SchemaManager;
use serde_json::{Value, json};
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_PAGES_TEST_DATABASE_URL";
const PUBLISH_IDEMPOTENCY_KEY: &str = "pages-postgres-publish-v1";
const ROLLBACK_IDEMPOTENCY_KEY: &str = "pages-postgres-rollback-v1";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

struct CacheRotationRefillInput<'a> {
    handler: &'a PageCacheInvalidationEventHandler,
    reads: &'a PagesCacheReadRuntime,
    port: &'a DurableCachePort,
    envelope: &'a EventEnvelope,
    tenant_id: Uuid,
    page_id: Uuid,
    operation: &'a str,
    expected_receipt_count: usize,
}

struct TestDatabase {
    control: DatabaseConnection,
    database_url: String,
    schema_name: String,
}

impl TestDatabase {
    async fn setup() -> TestResult<Option<Self>> {
        let Some(database_url) = database_url() else {
            eprintln!(
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Pages outbox/cache harness"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let schema_name = format!("rustok_pages_outbox_cache_{}", Uuid::new_v4().simple());
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;

        let db = scoped_connection(&database_url, &schema_name).await?;
        let manager = SchemaManager::new(&db);
        for migration in OutboxModule
            .migrations()
            .into_iter()
            .chain(PagesModule.migrations())
        {
            migration.up(&manager).await?;
        }

        Ok(Some(Self {
            control,
            database_url,
            schema_name,
        }))
    }

    async fn connection(&self) -> TestResult<DatabaseConnection> {
        scoped_connection(&self.database_url, &self.schema_name).await
    }

    async fn cleanup(self) -> TestResult<()> {
        self.control
            .execute_unprepared(&format!(
                r#"DROP SCHEMA IF EXISTS "{}" CASCADE"#,
                self.schema_name
            ))
            .await?;
        Ok(())
    }
}

#[derive(Default)]
struct DurableCacheState {
    generations: PageCacheGenerationSnapshot,
    values: HashMap<String, Vec<u8>>,
    requests: Vec<PageCacheInvalidationRequest>,
    receipts: Vec<PageCacheInvalidationReceipt>,
}

struct DurableCachePort {
    state: Mutex<DurableCacheState>,
}

impl DurableCachePort {
    fn new(generations: PageCacheGenerationSnapshot) -> Self {
        Self {
            state: Mutex::new(DurableCacheState {
                generations,
                ..DurableCacheState::default()
            }),
        }
    }

    fn recorded(
        &self,
    ) -> (
        PageCacheGenerationSnapshot,
        Vec<PageCacheInvalidationRequest>,
        Vec<PageCacheInvalidationReceipt>,
    ) {
        let state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        (
            state.generations,
            state.requests.clone(),
            state.receipts.clone(),
        )
    }
}

#[async_trait]
impl PageCacheInvalidationPort for DurableCachePort {
    async fn invalidate(
        &self,
        request: PageCacheInvalidationRequest,
    ) -> Result<PageCacheInvalidationReceipt, PageCacheError> {
        let mut state = self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        state.requests.push(request.clone());

        let mut receipt = PageCacheInvalidationReceipt::new(&request);
        for scope in request.scopes() {
            let next = state.generations.generation(*scope) + 1;
            state.generations.record(*scope, next);
            receipt.record(*scope, next);
        }
        state.receipts.push(receipt.clone());
        Ok(receipt)
    }
}

#[async_trait]
impl PagesCacheReadPort for DurableCachePort {
    async fn generation_snapshot(
        &self,
        _tenant_id: Uuid,
    ) -> Result<PageCacheGenerationSnapshot, PageCacheError> {
        Ok(self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .generations)
    }

    async fn get(&self, key: &str) -> Result<Option<Vec<u8>>, PageCacheError> {
        Ok(self
            .state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values
            .get(key)
            .cloned())
    }

    async fn put(&self, key: String, value: Vec<u8>, _ttl: Duration) -> Result<(), PageCacheError> {
        self.state
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .values
            .insert(key, value);
        Ok(())
    }
}

#[tokio::test]
async fn publish_and_rollback_receipts_correlate_with_durable_outbox_and_cache_rotation_on_postgres()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let page_id = Uuid::new_v4();
    let publish_operation_id = Uuid::new_v4();
    let rollback_operation_id = Uuid::new_v4();
    insert_page(&db, tenant_id, page_id).await?;

    let transport: Arc<dyn EventTransport> = Arc::new(OutboxTransport::new(db.clone()));
    let event_bus = TransactionalEventBus::new(transport);
    let cache_port = Arc::new(DurableCachePort::new(PageCacheGenerationSnapshot::new(
        3, 5, 7,
    )));
    let invalidation_port: Arc<dyn PageCacheInvalidationPort> = cache_port.clone();
    let read_port: Arc<dyn PagesCacheReadPort> = cache_port.clone();
    let handler = PageCacheInvalidationEventHandler::new(PagesCacheInvalidationRuntime::new(
        invalidation_port,
    ));
    let reads = PagesCacheReadRuntime::new(read_port);

    let publish_event_id = persist_publish_receipt_and_event(
        &db,
        &event_bus,
        tenant_id,
        actor_id,
        page_id,
        publish_operation_id,
    )
    .await?;
    assert_eq!(
        read_publish_receipt_version(&db, publish_operation_id).await?,
        2
    );
    let publish_envelope =
        read_published_envelope(&db, publish_event_id, tenant_id, page_id).await?;
    let publish_generations = rotate_and_refill(CacheRotationRefillInput {
        handler: &handler,
        reads: &reads,
        port: cache_port.as_ref(),
        envelope: &publish_envelope,
        tenant_id,
        page_id,
        operation: "publish",
        expected_receipt_count: 1,
    })
    .await?;
    assert_eq!(
        publish_generations,
        PageCacheGenerationSnapshot::new(4, 6, 8)
    );

    let rolled_back_event_id =
        persist_conflicting_publish_and_rollback(&db, &event_bus, tenant_id, actor_id, page_id)
            .await?;
    assert_event_absent(&db, rolled_back_event_id).await?;
    assert_eq!(
        count_publish_receipts_by_idempotency(&db, PUBLISH_IDEMPOTENCY_KEY).await?,
        1
    );

    let rollback_event_id = persist_rollback_receipt_and_event(
        &db,
        &event_bus,
        tenant_id,
        actor_id,
        page_id,
        publish_operation_id,
        rollback_operation_id,
    )
    .await?;
    assert_eq!(
        read_rollback_receipt_version(&db, rollback_operation_id).await?,
        3
    );
    let rollback_envelope =
        read_published_envelope(&db, rollback_event_id, tenant_id, page_id).await?;
    let rollback_generations = rotate_and_refill(CacheRotationRefillInput {
        handler: &handler,
        reads: &reads,
        port: cache_port.as_ref(),
        envelope: &rollback_envelope,
        tenant_id,
        page_id,
        operation: "rollback",
        expected_receipt_count: 2,
    })
    .await?;
    assert_eq!(
        rollback_generations,
        PageCacheGenerationSnapshot::new(5, 7, 9)
    );
    assert_eq!(read_page_version(&db, tenant_id, page_id).await?, 3);

    database.cleanup().await
}

async fn insert_page(db: &DatabaseConnection, tenant_id: Uuid, page_id: Uuid) -> TestResult<()> {
    db.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
INSERT INTO pages (
    id, tenant_id, author_id, status, template, metadata,
    created_at, updated_at, published_at, archived_at, version
) VALUES (
    $1, $2, NULL, 'draft', 'default', '{}'::jsonb,
    CURRENT_TIMESTAMP, CURRENT_TIMESTAMP, NULL, NULL, 1
)
"#,
        vec![page_id.into(), tenant_id.into()],
    ))
    .await?;
    Ok(())
}

async fn persist_publish_receipt_and_event(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    actor_id: Uuid,
    page_id: Uuid,
    operation_id: Uuid,
) -> TestResult<Uuid> {
    let txn = db.begin().await?;
    let updated = txn
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE pages SET status = 'published', published_at = CURRENT_TIMESTAMP, updated_at = CURRENT_TIMESTAMP, version = 2 WHERE tenant_id = $1 AND id = $2 AND version = 1",
            vec![tenant_id.into(), page_id.into()],
        ))
        .await?;
    assert_eq!(updated.rows_affected(), 1);

    let event_id = event_bus
        .publish_in_tx_with_envelope_id(
            &txn,
            tenant_id,
            Some(actor_id),
            DomainEvent::NodePublished {
                node_id: page_id,
                kind: PAGES_CACHE_ENTITY_KIND.to_string(),
            },
        )
        .await?;
    txn.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
INSERT INTO page_publish_operations (
    id, tenant_id, page_id, idempotency_key, request_hash, review_hash,
    sanitized_set_hash, artifact_set_hash, result_version, published_at, created_at
) VALUES (
    $1, $2, $3, $4, $5, $6, $7, $8, 2, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
)
"#,
        vec![
            operation_id.into(),
            tenant_id.into(),
            page_id.into(),
            PUBLISH_IDEMPOTENCY_KEY.into(),
            digest('a').into(),
            digest('b').into(),
            digest('c').into(),
            digest('d').into(),
        ],
    ))
    .await?;
    txn.commit().await?;
    Ok(event_id)
}

async fn persist_conflicting_publish_and_rollback(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    actor_id: Uuid,
    page_id: Uuid,
) -> TestResult<Uuid> {
    let txn = db.begin().await?;
    let event_id = event_bus
        .publish_in_tx_with_envelope_id(
            &txn,
            tenant_id,
            Some(actor_id),
            DomainEvent::NodePublished {
                node_id: page_id,
                kind: PAGES_CACHE_ENTITY_KIND.to_string(),
            },
        )
        .await?;
    let duplicate = txn
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            r#"
INSERT INTO page_publish_operations (
    id, tenant_id, page_id, idempotency_key, request_hash, review_hash,
    sanitized_set_hash, artifact_set_hash, result_version, published_at, created_at
) VALUES (
    $1, $2, $3, $4, $5, $6, $7, $8, 99, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
)
"#,
            vec![
                Uuid::new_v4().into(),
                tenant_id.into(),
                page_id.into(),
                PUBLISH_IDEMPOTENCY_KEY.into(),
                digest('e').into(),
                digest('f').into(),
                digest('1').into(),
                digest('2').into(),
            ],
        ))
        .await;
    assert!(duplicate.is_err());
    txn.rollback().await?;
    Ok(event_id)
}

async fn persist_rollback_receipt_and_event(
    db: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    tenant_id: Uuid,
    actor_id: Uuid,
    page_id: Uuid,
    target_publish_operation_id: Uuid,
    operation_id: Uuid,
) -> TestResult<Uuid> {
    let txn = db.begin().await?;
    let updated = txn
        .execute(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "UPDATE pages SET updated_at = CURRENT_TIMESTAMP, version = 3 WHERE tenant_id = $1 AND id = $2 AND version = 2 AND status = 'published'",
            vec![tenant_id.into(), page_id.into()],
        ))
        .await?;
    assert_eq!(updated.rows_affected(), 1);

    let event_id = event_bus
        .publish_in_tx_with_envelope_id(
            &txn,
            tenant_id,
            Some(actor_id),
            DomainEvent::NodePublished {
                node_id: page_id,
                kind: PAGES_CACHE_ENTITY_KIND.to_string(),
            },
        )
        .await?;
    txn.execute(Statement::from_sql_and_values(
        DbBackend::Postgres,
        r#"
INSERT INTO page_rollback_operations (
    id, tenant_id, page_id, idempotency_key, request_hash,
    target_publish_operation_id, source_artifact_set_hash,
    target_artifact_set_hash, result_version, rolled_back_at, created_at
) VALUES (
    $1, $2, $3, $4, $5, $6, $7, $8, 3, CURRENT_TIMESTAMP, CURRENT_TIMESTAMP
)
"#,
        vec![
            operation_id.into(),
            tenant_id.into(),
            page_id.into(),
            ROLLBACK_IDEMPOTENCY_KEY.into(),
            digest('3').into(),
            target_publish_operation_id.into(),
            digest('d').into(),
            digest('4').into(),
        ],
    ))
    .await?;
    txn.commit().await?;
    Ok(event_id)
}

async fn read_published_envelope(
    db: &DatabaseConnection,
    event_id: Uuid,
    tenant_id: Uuid,
    page_id: Uuid,
) -> TestResult<EventEnvelope> {
    let stored = SysEvents::find_by_id(event_id)
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("durable NodePublished outbox row is missing"))?;
    assert_eq!(stored.status, SysEventStatus::Pending);
    let envelope: EventEnvelope = serde_json::from_value(stored.payload)?;
    envelope.validate_registered_schema()?;
    assert_eq!(envelope.id, event_id);
    assert_eq!(envelope.correlation_id, event_id);
    assert_eq!(envelope.tenant_id, tenant_id);
    match &envelope.event {
        DomainEvent::NodePublished { node_id, kind } => {
            assert_eq!(*node_id, page_id);
            assert_eq!(kind, PAGES_CACHE_ENTITY_KIND);
        }
        other => panic!("expected durable NodePublished envelope, got {other:?}"),
    }
    Ok(envelope)
}

async fn rotate_and_refill(
    input: CacheRotationRefillInput<'_>,
) -> TestResult<PageCacheGenerationSnapshot> {
    let before = input.reads.generation_snapshot(input.tenant_id).await?;
    let storefront_variant = format!("{}|home|en|en|web", input.operation);
    let artifact_variant = format!("{}|en|en|web", input.operation);
    let old_storefront_key =
        storefront_pages_cache_key(input.tenant_id, before, &storefront_variant)?;
    let old_artifact_key = page_cache_key(
        PageCacheScope::Artifact,
        input.tenant_id,
        input.page_id,
        before.artifact,
        &artifact_variant,
    )?;
    let old_storefront = json!({"operation": input.operation, "generation": "before"});
    let old_artifact = json!({"operation": input.operation, "generation": "before"});
    input
        .reads
        .put_json(old_storefront_key.clone(), &old_storefront)
        .await?;
    input
        .reads
        .put_json(old_artifact_key.clone(), &old_artifact)
        .await?;

    input.handler.handle(input.envelope).await?;
    let after = input.reads.generation_snapshot(input.tenant_id).await?;
    assert_eq!(after.route, before.route + 1);
    assert_eq!(after.page, before.page + 1);
    assert_eq!(after.artifact, before.artifact + 1);

    let (recorded_generations, requests, receipts) = input.port.recorded();
    assert_eq!(recorded_generations, after);
    assert_eq!(requests.len(), input.expected_receipt_count);
    assert_eq!(receipts.len(), input.expected_receipt_count);
    let request = requests
        .last()
        .ok_or_else(|| std::io::Error::other("cache invalidation request is missing"))?;
    let receipt = receipts
        .last()
        .ok_or_else(|| std::io::Error::other("cache invalidation receipt is missing"))?;
    assert_eq!(request.event_id, input.envelope.id);
    assert_eq!(request.correlation_id, input.envelope.correlation_id);
    assert_eq!(request.cause, PageCacheInvalidationCause::Published);
    assert_eq!(receipt.event_id, input.envelope.id);
    assert_eq!(receipt.correlation_id, input.envelope.correlation_id);
    assert_eq!(receipt.route_generation, Some(after.route));
    assert_eq!(receipt.page_generation, Some(after.page));
    assert_eq!(receipt.artifact_generation, Some(after.artifact));

    let new_storefront_key =
        storefront_pages_cache_key(input.tenant_id, after, &storefront_variant)?;
    let new_artifact_key = page_cache_key(
        PageCacheScope::Artifact,
        input.tenant_id,
        input.page_id,
        after.artifact,
        &artifact_variant,
    )?;
    assert_ne!(new_storefront_key, old_storefront_key);
    assert_ne!(new_artifact_key, old_artifact_key);
    assert_eq!(
        input.reads.get_json::<Value>(&new_storefront_key).await?,
        None
    );
    assert_eq!(
        input.reads.get_json::<Value>(&new_artifact_key).await?,
        None
    );

    let refilled_storefront = json!({"operation": input.operation, "generation": "after"});
    let refilled_artifact = json!({"operation": input.operation, "generation": "after"});
    input
        .reads
        .put_json(new_storefront_key.clone(), &refilled_storefront)
        .await?;
    input
        .reads
        .put_json(new_artifact_key.clone(), &refilled_artifact)
        .await?;
    assert_eq!(
        input.reads.get_json::<Value>(&new_storefront_key).await?,
        Some(refilled_storefront)
    );
    assert_eq!(
        input.reads.get_json::<Value>(&new_artifact_key).await?,
        Some(refilled_artifact)
    );
    assert_eq!(
        input.reads.get_json::<Value>(&old_storefront_key).await?,
        Some(old_storefront)
    );
    assert_eq!(
        input.reads.get_json::<Value>(&old_artifact_key).await?,
        Some(old_artifact)
    );
    Ok(after)
}

async fn read_publish_receipt_version(
    db: &DatabaseConnection,
    operation_id: Uuid,
) -> TestResult<i32> {
    read_version_row(
        db,
        "SELECT result_version FROM page_publish_operations WHERE id = $1",
        operation_id,
    )
    .await
}

async fn read_rollback_receipt_version(
    db: &DatabaseConnection,
    operation_id: Uuid,
) -> TestResult<i32> {
    read_version_row(
        db,
        "SELECT result_version FROM page_rollback_operations WHERE id = $1",
        operation_id,
    )
    .await
}

async fn read_version_row(
    db: &DatabaseConnection,
    sql: &str,
    operation_id: Uuid,
) -> TestResult<i32> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            sql,
            vec![operation_id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("operation receipt row is missing"))?;
    Ok(row.try_get("", "result_version")?)
}

async fn read_page_version(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    page_id: Uuid,
) -> TestResult<i32> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT version FROM pages WHERE tenant_id = $1 AND id = $2",
            vec![tenant_id.into(), page_id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("page row is missing"))?;
    Ok(row.try_get("", "version")?)
}

async fn count_publish_receipts_by_idempotency(
    db: &DatabaseConnection,
    idempotency_key: &str,
) -> TestResult<i64> {
    let row = db
        .query_one(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT COUNT(*)::bigint AS value FROM page_publish_operations WHERE idempotency_key = $1",
            vec![idempotency_key.to_owned().into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("publish receipt count returned no row"))?;
    Ok(row.try_get("", "value")?)
}

async fn assert_event_absent(db: &DatabaseConnection, event_id: Uuid) -> TestResult<()> {
    assert!(SysEvents::find_by_id(event_id).one(db).await?.is_none());
    Ok(())
}

fn digest(value: char) -> String {
    value.to_string().repeat(64)
}

fn database_url() -> Option<String> {
    env::var(DATABASE_ENV)
        .or_else(|_| env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

async fn connect(database_url: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    Ok(Database::connect(options).await?)
}

async fn scoped_connection(
    database_url: &str,
    schema_name: &str,
) -> TestResult<DatabaseConnection> {
    let db = connect(database_url).await?;
    db.execute_unprepared(&format!(r#"SET search_path TO "{schema_name}""#))
        .await?;
    Ok(db)
}
