use std::any::Any;
use std::env;
use std::error::Error as StdError;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rustok_core::events::EventHandler;
use rustok_core::{Error, EventTransport, MigrationSource, ReliabilityLevel};
use rustok_events::{DomainEvent, EventEnvelope};
use rustok_outbox::entity::SysEventStatus;
use rustok_outbox::{
    OutboxModule, OutboxRelay, OutboxTransport, RelayConfig, SysEvents, TransactionalEventBus,
};
use rustok_pages::{
    PAGES_CACHE_ENTITY_KIND, PageCacheError, PageCacheGenerationSnapshot,
    PageCacheInvalidationEventHandler, PageCacheInvalidationPort, PageCacheInvalidationReceipt,
    PageCacheInvalidationRequest, PageCacheScope, PagesCacheInvalidationRuntime,
};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, EntityTrait, TransactionTrait,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_PAGES_TEST_DATABASE_URL";

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
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Pages relay restart harness"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let schema_name = format!("rustok_pages_relay_restart_{}", Uuid::new_v4().simple());
        control
            .execute_unprepared(&format!(r#"CREATE SCHEMA "{schema_name}""#))
            .await?;
        let db = scoped_connection(&database_url, &schema_name).await?;
        let manager = SchemaManager::new(&db);
        for migration in OutboxModule.migrations() {
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
struct CacheState {
    generations: PageCacheGenerationSnapshot,
    requests: Vec<PageCacheInvalidationRequest>,
    receipts: Vec<PageCacheInvalidationReceipt>,
}

struct RecordingInvalidationPort {
    state: Mutex<CacheState>,
}

impl RecordingInvalidationPort {
    fn new() -> Self {
        Self {
            state: Mutex::new(CacheState::default()),
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
impl PageCacheInvalidationPort for RecordingInvalidationPort {
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

struct RestartTarget {
    failures_remaining: AtomicUsize,
    handler: PageCacheInvalidationEventHandler,
    delivered_event_ids: Mutex<Vec<Uuid>>,
}

impl RestartTarget {
    fn new(handler: PageCacheInvalidationEventHandler, failures: usize) -> Self {
        Self {
            failures_remaining: AtomicUsize::new(failures),
            handler,
            delivered_event_ids: Mutex::new(Vec::new()),
        }
    }

    fn delivered_event_ids(&self) -> Vec<Uuid> {
        self.delivered_event_ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }
}

#[async_trait]
impl EventTransport for RestartTarget {
    async fn publish(&self, envelope: EventEnvelope) -> rustok_core::Result<()> {
        if self
            .failures_remaining
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                value.checked_sub(1)
            })
            .is_ok()
        {
            return Err(Error::External(
                "simulated Pages cache target outage".to_string(),
            ));
        }
        self.handler.handle(&envelope).await?;
        self.delivered_event_ids
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(envelope.id);
        Ok(())
    }

    fn reliability_level(&self) -> ReliabilityLevel {
        ReliabilityLevel::Outbox
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[tokio::test]
async fn restarted_relay_dispatches_pending_node_published_before_acknowledging_row()
-> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let tenant_id = Uuid::new_v4();
    let page_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();

    let outbox_transport: Arc<dyn EventTransport> = Arc::new(OutboxTransport::new(db.clone()));
    let event_bus = TransactionalEventBus::new(outbox_transport);
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
    txn.commit().await?;

    let pending = SysEvents::find_by_id(event_id)
        .one(&db)
        .await?
        .expect("durable NodePublished row must exist before relay");
    assert_eq!(pending.status, SysEventStatus::Pending);
    assert_eq!(pending.retry_count, 0);
    assert!(pending.dispatched_at.is_none());

    let cache_port = Arc::new(RecordingInvalidationPort::new());
    let invalidation_port: Arc<dyn PageCacheInvalidationPort> = cache_port.clone();
    let handler = PageCacheInvalidationEventHandler::new(PagesCacheInvalidationRuntime::new(
        invalidation_port,
    ));
    let target = Arc::new(RestartTarget::new(handler, 1));
    let first_target: Arc<dyn EventTransport> = target.clone();
    let first_relay = OutboxRelay::new(db.clone(), first_target)
        .with_config(relay_config("pages-relay-before-restart"));

    assert_eq!(first_relay.process_pending_once(Some(1)).await?, 1);
    let retrying = SysEvents::find_by_id(event_id)
        .one(&db)
        .await?
        .expect("failed delivery must retain the durable row");
    assert_eq!(retrying.status, SysEventStatus::Pending);
    assert_eq!(retrying.retry_count, 1);
    assert!(retrying.last_error.is_some());
    assert!(retrying.next_attempt_at.is_some());
    assert!(retrying.claimed_by.is_none());
    assert!(retrying.claimed_at.is_none());
    assert!(retrying.dispatched_at.is_none());
    assert!(target.delivered_event_ids().is_empty());
    assert_eq!(
        cache_port.recorded().0,
        PageCacheGenerationSnapshot::default()
    );
    let first_metrics = first_relay.metrics();
    assert_eq!(first_metrics.failure_total, 1);
    assert_eq!(first_metrics.success_total, 0);
    assert_eq!(first_metrics.processed_total, 1);

    let restarted_target: Arc<dyn EventTransport> = target.clone();
    let restarted_relay = OutboxRelay::new(db.clone(), restarted_target)
        .with_config(relay_config("pages-relay-after-restart"));
    assert_eq!(restarted_relay.process_pending_once(Some(1)).await?, 1);

    let dispatched = SysEvents::find_by_id(event_id)
        .one(&db)
        .await?
        .expect("successful restarted delivery must retain the acknowledged row");
    assert_eq!(dispatched.status, SysEventStatus::Dispatched);
    assert_eq!(dispatched.retry_count, 1);
    assert!(dispatched.last_error.is_none());
    assert!(dispatched.next_attempt_at.is_none());
    assert!(dispatched.claimed_by.is_none());
    assert!(dispatched.claimed_at.is_none());
    assert!(dispatched.dispatched_at.is_some());
    assert_eq!(target.delivered_event_ids(), vec![event_id]);

    let (generations, requests, receipts) = cache_port.recorded();
    assert_eq!(generations, PageCacheGenerationSnapshot::new(1, 1, 1));
    assert_eq!(requests.len(), 1);
    assert_eq!(receipts.len(), 1);
    assert_eq!(requests[0].tenant_id, tenant_id);
    assert_eq!(requests[0].page_id, page_id);
    assert_eq!(requests[0].event_id, event_id);
    assert_eq!(requests[0].correlation_id, event_id);
    assert_eq!(
        requests[0].scopes(),
        &[
            PageCacheScope::Route,
            PageCacheScope::Page,
            PageCacheScope::Artifact,
        ]
    );
    assert_eq!(receipts[0].event_id, event_id);
    assert_eq!(receipts[0].correlation_id, event_id);
    assert_eq!(receipts[0].route_generation, Some(1));
    assert_eq!(receipts[0].page_generation, Some(1));
    assert_eq!(receipts[0].artifact_generation, Some(1));
    let restarted_metrics = restarted_relay.metrics();
    assert_eq!(restarted_metrics.success_total, 1);
    assert_eq!(restarted_metrics.failure_total, 0);
    assert_eq!(restarted_metrics.processed_total, 1);

    database.cleanup().await
}

fn relay_config(worker_id: &str) -> RelayConfig {
    RelayConfig {
        batch_size: 1,
        max_attempts: 3,
        backoff_base: Duration::ZERO,
        backoff_max: Duration::ZERO,
        max_concurrency: 1,
        claim_ttl: Duration::from_millis(1),
        worker_id: worker_id.to_string(),
    }
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
