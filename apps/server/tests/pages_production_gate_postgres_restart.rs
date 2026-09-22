#![cfg(feature = "mod-pages")]

use std::any::Any;
use std::env;
use std::error::Error as StdError;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rustok_cache::CacheService;
use rustok_core::events::EventHandler;
use rustok_core::{Error, EventTransport, MigrationSource, ReliabilityLevel};
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_outbox::entity::SysEventStatus;
use rustok_outbox::{
    OutboxModule, OutboxRelay, OutboxTransport, RelayConfig, SysEvents, TransactionalEventBus,
};
use rustok_pages::{
    PAGES_CACHE_ENTITY_KIND, PageCacheGenerationSnapshot, PageCacheInvalidationEventHandler,
    PageCacheScope, PagesCacheInvalidationRuntime, PagesCacheReadRuntime, PagesModule,
    page_cache_key, storefront_pages_cache_key,
};
use rustok_server::common::settings::RustokSettings;
use rustok_server::services::pages_cache_invalidation::ServerPagesCachePort;
use rustok_server::services::server_runtime_context::ServerRuntimeContext;
use rustok_server::services::tenant_cache_generation::start_tenant_cache_generation_listener;
use rustok_server::services::tenant_generation_delivery_gate::TenantGenerationDeliveryGate;
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, EntityTrait,
    Statement, TransactionTrait,
};
use sea_orm_migration::SchemaManager;
use serde_json::{Value, json};
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_PAGES_TEST_DATABASE_URL";
const PUBLISH_IDEMPOTENCY_KEY: &str = "pages-production-gate-postgres-publish-v1";
const ROLLBACK_IDEMPOTENCY_KEY: &str = "pages-production-gate-postgres-rollback-v1";

type TestResult<T> = Result<T, Box<dyn StdError + Send + Sync>>;

struct TestDatabase {
    control: DatabaseConnection,
    database_url: String,
    schema_name: String,
}

impl TestDatabase {
    async fn setup() -> TestResult<Option<Self>> {
        let Some(database_url) = database_url() else {
            eprintln!(
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping production Pages gate PostgreSQL harness"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let schema_name = format!("rustok_pages_production_gate_{}", Uuid::new_v4().simple());
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
struct FailNextTransport {
    failures_remaining: AtomicUsize,
    delivered: Mutex<Vec<EventEnvelope>>,
}

impl FailNextTransport {
    fn fail_next(&self) {
        self.failures_remaining.store(1, Ordering::SeqCst);
    }

    fn delivered_ids(&self) -> Vec<Uuid> {
        self.delivered
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .map(|envelope| envelope.id)
            .collect()
    }

    fn envelope(&self, event_id: Uuid) -> Option<EventEnvelope> {
        self.delivered
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .iter()
            .find(|envelope| envelope.id == event_id)
            .cloned()
    }
}

#[async_trait]
impl EventTransport for FailNextTransport {
    async fn publish(&self, envelope: EventEnvelope) -> rustok_core::Result<()> {
        if self
            .failures_remaining
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                value.checked_sub(1)
            })
            .is_ok()
        {
            return Err(Error::External(
                "synthetic downstream rejection after Pages generation rotation".to_string(),
            ));
        }
        self.delivered
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(envelope);
        Ok(())
    }

    fn reliability_level(&self) -> ReliabilityLevel {
        ReliabilityLevel::Outbox
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

struct KeyCycle {
    old_storefront_key: String,
    old_artifact_key: String,
    new_storefront_key: String,
    new_artifact_key: String,
    old_storefront: Value,
    old_artifact: Value,
}

#[tokio::test]
async fn production_gate_correlates_postgres_publish_rollback_and_restart_retry() -> TestResult<()>
{
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

    let outbox_transport: Arc<dyn EventTransport> = Arc::new(OutboxTransport::new(db.clone()));
    let event_bus = TransactionalEventBus::new(outbox_transport);
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
        read_receipt_version(&db, "page_publish_operations", publish_operation_id,).await?,
        2
    );

    let cache = CacheService::from_url(None);
    let runtime_ctx = ServerRuntimeContext::new(db.clone(), RustokSettings::default());
    start_tenant_cache_generation_listener(&runtime_ctx, cache.clone()).await?;
    let provider = Arc::new(ServerPagesCachePort::new(&cache));
    let reads = PagesCacheReadRuntime::new(provider.clone());
    let downstream = Arc::new(FailNextTransport::default());

    let publish_before = reads.generation_snapshot(tenant_id).await?;
    assert_eq!(publish_before, PageCacheGenerationSnapshot::default());
    let publish_keys = seed_old_keys(&reads, tenant_id, page_id, publish_before, "publish").await?;

    let publish_target: Arc<dyn EventTransport> = Arc::new(TenantGenerationDeliveryGate::new(
        downstream.clone(),
        runtime_ctx.clone(),
        cache.clone(),
    ));
    let publish_relay = OutboxRelay::new(db.clone(), publish_target)
        .with_config(relay_config("pages-production-gate-publish"));
    assert_eq!(publish_relay.process_pending_once(Some(1)).await?, 1);
    assert_dispatched(&db, publish_event_id, 0).await?;
    assert_eq!(downstream.delivered_ids(), vec![publish_event_id]);

    let publish_after = reads.generation_snapshot(tenant_id).await?;
    assert_eq!(publish_after, PageCacheGenerationSnapshot::new(1, 1, 1));
    assert_new_keys_miss_and_old_keys_remain(&reads, &publish_keys).await?;
    refill_new_keys(&reads, &publish_keys, "publish").await?;

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
        read_receipt_version(&db, "page_rollback_operations", rollback_operation_id,).await?,
        3
    );
    let rollback_envelope = read_envelope(&db, rollback_event_id, tenant_id, page_id).await?;
    let rollback_keys =
        seed_old_keys(&reads, tenant_id, page_id, publish_after, "rollback").await?;

    downstream.fail_next();
    let failing_target: Arc<dyn EventTransport> = Arc::new(TenantGenerationDeliveryGate::new(
        downstream.clone(),
        runtime_ctx.clone(),
        cache.clone(),
    ));
    let first_rollback_relay = OutboxRelay::new(db.clone(), failing_target)
        .with_config(relay_config("pages-production-gate-before-restart"));
    assert_eq!(first_rollback_relay.process_pending_once(Some(1)).await?, 1);
    assert_retrying(&db, rollback_event_id).await?;
    assert_eq!(downstream.delivered_ids(), vec![publish_event_id]);

    let after_failed_downstream = reads.generation_snapshot(tenant_id).await?;
    assert_eq!(
        after_failed_downstream,
        PageCacheGenerationSnapshot::new(2, 2, 2)
    );
    assert_new_keys_miss_and_old_keys_remain(&reads, &rollback_keys).await?;
    let first_metrics = first_rollback_relay.metrics();
    assert_eq!(first_metrics.failure_total, 1);
    assert_eq!(first_metrics.success_total, 0);
    assert_eq!(first_metrics.processed_total, 1);

    let restarted_target: Arc<dyn EventTransport> = Arc::new(TenantGenerationDeliveryGate::new(
        downstream.clone(),
        runtime_ctx,
        cache.clone(),
    ));
    let restarted_relay = OutboxRelay::new(db.clone(), restarted_target)
        .with_config(relay_config("pages-production-gate-after-restart"));
    assert_eq!(restarted_relay.process_pending_once(Some(1)).await?, 1);
    assert_dispatched(&db, rollback_event_id, 1).await?;
    assert_eq!(
        downstream.delivered_ids(),
        vec![publish_event_id, rollback_event_id]
    );
    assert_eq!(
        reads.generation_snapshot(tenant_id).await?,
        PageCacheGenerationSnapshot::new(2, 2, 2)
    );

    let delivered_rollback = downstream.envelope(rollback_event_id).ok_or_else(|| {
        std::io::Error::other("restarted relay did not deliver rollback envelope")
    })?;
    assert_eq!(delivered_rollback.id, rollback_envelope.id);
    assert_eq!(
        delivered_rollback.correlation_id,
        rollback_envelope.correlation_id
    );
    PageCacheInvalidationEventHandler::new(PagesCacheInvalidationRuntime::new(Arc::new(
        ServerPagesCachePort::new(&cache),
    )))
    .handle(&delivered_rollback)
    .await?;
    assert_eq!(
        reads.generation_snapshot(tenant_id).await?,
        PageCacheGenerationSnapshot::new(2, 2, 2)
    );

    refill_new_keys(&reads, &rollback_keys, "rollback").await?;
    assert_old_and_new_values(&reads, &rollback_keys).await?;
    let restarted_metrics = restarted_relay.metrics();
    assert_eq!(restarted_metrics.failure_total, 0);
    assert_eq!(restarted_metrics.success_total, 1);
    assert_eq!(restarted_metrics.processed_total, 1);

    assert_eq!(read_page_version(&db, tenant_id, page_id).await?, 3);
    database.cleanup().await
}

async fn seed_old_keys(
    reads: &PagesCacheReadRuntime,
    tenant_id: Uuid,
    page_id: Uuid,
    before: PageCacheGenerationSnapshot,
    operation: &str,
) -> TestResult<KeyCycle> {
    let storefront_variant = format!("{operation}|home|en|en|web");
    let artifact_variant = format!("{operation}|en|en|web");
    let old_storefront_key = storefront_pages_cache_key(tenant_id, before, &storefront_variant)?;
    let old_artifact_key = page_cache_key(
        PageCacheScope::Artifact,
        tenant_id,
        page_id,
        before.artifact,
        &artifact_variant,
    )?;
    let after =
        PageCacheGenerationSnapshot::new(before.route + 1, before.page + 1, before.artifact + 1);
    let new_storefront_key = storefront_pages_cache_key(tenant_id, after, &storefront_variant)?;
    let new_artifact_key = page_cache_key(
        PageCacheScope::Artifact,
        tenant_id,
        page_id,
        after.artifact,
        &artifact_variant,
    )?;
    let old_storefront = json!({"operation": operation, "generation": "before"});
    let old_artifact = json!({"operation": operation, "generation": "before"});
    reads
        .put_json(old_storefront_key.clone(), &old_storefront)
        .await?;
    reads
        .put_json(old_artifact_key.clone(), &old_artifact)
        .await?;
    Ok(KeyCycle {
        old_storefront_key,
        old_artifact_key,
        new_storefront_key,
        new_artifact_key,
        old_storefront,
        old_artifact,
    })
}

async fn assert_new_keys_miss_and_old_keys_remain(
    reads: &PagesCacheReadRuntime,
    keys: &KeyCycle,
) -> TestResult<()> {
    assert_ne!(keys.new_storefront_key, keys.old_storefront_key);
    assert_ne!(keys.new_artifact_key, keys.old_artifact_key);
    assert_eq!(
        reads.get_json::<Value>(&keys.new_storefront_key).await?,
        None
    );
    assert_eq!(reads.get_json::<Value>(&keys.new_artifact_key).await?, None);
    assert_eq!(
        reads.get_json::<Value>(&keys.old_storefront_key).await?,
        Some(keys.old_storefront.clone())
    );
    assert_eq!(
        reads.get_json::<Value>(&keys.old_artifact_key).await?,
        Some(keys.old_artifact.clone())
    );
    Ok(())
}

async fn refill_new_keys(
    reads: &PagesCacheReadRuntime,
    keys: &KeyCycle,
    operation: &str,
) -> TestResult<()> {
    reads
        .put_json(
            keys.new_storefront_key.clone(),
            &json!({"operation": operation, "generation": "after"}),
        )
        .await?;
    reads
        .put_json(
            keys.new_artifact_key.clone(),
            &json!({"operation": operation, "generation": "after"}),
        )
        .await?;
    Ok(())
}

async fn assert_old_and_new_values(
    reads: &PagesCacheReadRuntime,
    keys: &KeyCycle,
) -> TestResult<()> {
    assert!(
        reads
            .get_json::<Value>(&keys.old_storefront_key)
            .await?
            .is_some()
    );
    assert!(
        reads
            .get_json::<Value>(&keys.old_artifact_key)
            .await?
            .is_some()
    );
    assert!(
        reads
            .get_json::<Value>(&keys.new_storefront_key)
            .await?
            .is_some()
    );
    assert!(
        reads
            .get_json::<Value>(&keys.new_artifact_key)
            .await?
            .is_some()
    );
    Ok(())
}

async fn insert_page(db: &DatabaseConnection, tenant_id: Uuid, page_id: Uuid) -> TestResult<()> {
    db.execute_raw(Statement::from_sql_and_values(
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
        .execute_raw(Statement::from_sql_and_values(
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
    txn.execute_raw(Statement::from_sql_and_values(
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
        .execute_raw(Statement::from_sql_and_values(
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
    txn.execute_raw(Statement::from_sql_and_values(
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
            digest('e').into(),
            target_publish_operation_id.into(),
            digest('d').into(),
            digest('f').into(),
        ],
    ))
    .await?;
    txn.commit().await?;
    Ok(event_id)
}

async fn read_envelope(
    db: &DatabaseConnection,
    event_id: Uuid,
    tenant_id: Uuid,
    page_id: Uuid,
) -> TestResult<EventEnvelope> {
    let row = SysEvents::find_by_id(event_id)
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("durable Pages event is missing"))?;
    let envelope: EventEnvelope = serde_json::from_value(row.payload)?;
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

async fn assert_retrying(db: &DatabaseConnection, event_id: Uuid) -> TestResult<()> {
    let row = SysEvents::find_by_id(event_id)
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("retrying outbox row is missing"))?;
    assert_eq!(row.status, SysEventStatus::Pending);
    assert_eq!(row.retry_count, 1);
    assert!(row.last_error.is_some());
    assert!(row.next_attempt_at.is_some());
    assert!(row.claimed_by.is_none());
    assert!(row.claimed_at.is_none());
    assert!(row.dispatched_at.is_none());
    Ok(())
}

async fn assert_dispatched(
    db: &DatabaseConnection,
    event_id: Uuid,
    expected_retry_count: i32,
) -> TestResult<()> {
    let row = SysEvents::find_by_id(event_id)
        .one(db)
        .await?
        .ok_or_else(|| std::io::Error::other("dispatched outbox row is missing"))?;
    assert_eq!(row.status, SysEventStatus::Dispatched);
    assert_eq!(row.retry_count, expected_retry_count);
    assert!(row.dispatched_at.is_some());
    assert!(row.last_error.is_none());
    assert!(row.next_attempt_at.is_none());
    assert!(row.claimed_by.is_none());
    assert!(row.claimed_at.is_none());
    Ok(())
}

async fn read_receipt_version(
    db: &DatabaseConnection,
    table: &str,
    operation_id: Uuid,
) -> TestResult<i32> {
    let sql = match table {
        "page_publish_operations" => {
            "SELECT result_version FROM page_publish_operations WHERE id = $1"
        }
        "page_rollback_operations" => {
            "SELECT result_version FROM page_rollback_operations WHERE id = $1"
        }
        _ => return Err(std::io::Error::other("unsupported receipt table").into()),
    };
    let row = db
        .query_one_raw(Statement::from_sql_and_values(
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
        .query_one_raw(Statement::from_sql_and_values(
            DbBackend::Postgres,
            "SELECT version FROM pages WHERE tenant_id = $1 AND id = $2",
            vec![tenant_id.into(), page_id.into()],
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("page row is missing"))?;
    Ok(row.try_get("", "version")?)
}

fn relay_config(worker_id: &str) -> RelayConfig {
    RelayConfig {
        batch_size: 1,
        max_attempts: 3,
        backoff_base: Duration::ZERO,
        backoff_max: Duration::ZERO,
        max_concurrency: 1,
        claim_ttl: Duration::from_secs(1),
        worker_id: worker_id.to_string(),
    }
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
        .max_connections(4)
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
