use std::any::Any;
use std::env;
use std::error::Error as StdError;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use async_trait::async_trait;
use rustok_core::{Error, EventTransport, MigrationSource, ReliabilityLevel};
use rustok_events::{
    BLOG_COMMENTS_SCHEDULE_AUDIT_EVENT_TYPE, BLOG_COMMENTS_SCHEDULE_AUDIT_SCHEMA_VERSION,
    BLOG_COMMENTS_SCHEDULE_AUDIT_STATE_KEY, BlogCommentsDelegationScheduleAuditEvent,
    ContractEventEnvelope, ContractEventPayload, EventEnvelope,
};
use rustok_outbox::entity::SysEventStatus;
use rustok_outbox::{OutboxModule, OutboxRelay, RelayConfig, SysEvents, TransactionalEventBus};
use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, EntityTrait, TransactionTrait,
};
use sea_orm_migration::SchemaManager;
use uuid::Uuid;

const DATABASE_ENV: &str = "RUSTOK_OUTBOX_BLOG_AUDIT_TEST_DATABASE_URL";

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
                "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping Blog Comments audit relay evidence"
            );
            return Ok(None);
        };
        let control = connect(&database_url).await?;
        let schema_name = format!("rustok_outbox_blog_audit_{}", Uuid::new_v4().simple());
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

#[derive(Clone, Debug, PartialEq, Eq)]
struct DeliveredAudit {
    id: Uuid,
    correlation_id: Uuid,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    request_id: Uuid,
    event_type: String,
    schema_version: u16,
}

struct RecordingContractTransport {
    failures_remaining: AtomicUsize,
    delivered: Mutex<Vec<DeliveredAudit>>,
}

impl RecordingContractTransport {
    fn new(failures: usize) -> Self {
        Self {
            failures_remaining: AtomicUsize::new(failures),
            delivered: Mutex::new(Vec::new()),
        }
    }

    fn delivered(&self) -> Vec<DeliveredAudit> {
        self.delivered
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .clone()
    }

    fn should_fail(&self) -> bool {
        self.failures_remaining
            .fetch_update(Ordering::SeqCst, Ordering::SeqCst, |value| {
                value.checked_sub(1)
            })
            .is_ok()
    }
}

#[async_trait]
impl EventTransport for RecordingContractTransport {
    async fn publish(&self, _envelope: EventEnvelope) -> rustok_core::Result<()> {
        Err(Error::Validation(
            "Blog Comments audit evidence expects a sealed contract envelope".to_string(),
        ))
    }

    async fn publish_contract(&self, envelope: ContractEventEnvelope) -> rustok_core::Result<()> {
        if self.should_fail() {
            return Err(Error::External(
                "simulated Blog Comments audit target outage".to_string(),
            ));
        }

        let request_id = match envelope
            .payload()
            .map_err(|error| Error::Validation(error.to_string()))?
        {
            ContractEventPayload::BlogCommentsDelegationScheduleAudit(event) => event.request_id(),
            _ => {
                return Err(Error::Validation(
                    "unexpected contract event family in Blog audit relay evidence".to_string(),
                ));
            }
        };
        self.delivered
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .push(DeliveredAudit {
                id: envelope.id(),
                correlation_id: envelope.correlation_id(),
                tenant_id: envelope.tenant_id(),
                actor_id: envelope.actor_id(),
                request_id,
                event_type: envelope.event_type().to_string(),
                schema_version: envelope.schema_version(),
            });
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
async fn blog_audit_relay_restarts_and_acknowledges_only_after_delivery() -> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let request_id = Uuid::new_v4();
    seed_blog_audit(&db, tenant_id, actor_id, request_id).await?;

    let pending = SysEvents::find_by_id(request_id)
        .one(&db)
        .await?
        .expect("canonical Blog audit row must exist before relay");
    assert_eq!(pending.status, SysEventStatus::Pending);
    assert_eq!(pending.retry_count, 0);
    assert!(pending.dispatched_at.is_none());
    assert_eq!(pending.event_type, BLOG_COMMENTS_SCHEDULE_AUDIT_EVENT_TYPE);
    assert_eq!(
        pending.schema_version,
        BLOG_COMMENTS_SCHEDULE_AUDIT_SCHEMA_VERSION as i16
    );

    let target = Arc::new(RecordingContractTransport::new(1));
    let first_target: Arc<dyn EventTransport> = target.clone();
    let first_relay = OutboxRelay::new(db.clone(), first_target)
        .with_config(relay_config("blog-audit-relay-before-restart", 3));

    assert_eq!(first_relay.process_pending_once(Some(1)).await?, 1);
    let retrying = SysEvents::find_by_id(request_id)
        .one(&db)
        .await?
        .expect("failed delivery must retain canonical Blog audit row");
    assert_eq!(retrying.status, SysEventStatus::Pending);
    assert_eq!(retrying.retry_count, 1);
    assert!(retrying.last_error.is_some());
    assert!(retrying.next_attempt_at.is_some());
    assert!(retrying.claimed_by.is_none());
    assert!(retrying.claimed_at.is_none());
    assert!(retrying.dispatched_at.is_none());
    assert!(target.delivered().is_empty());
    let first_metrics = first_relay.metrics();
    assert_eq!(first_metrics.failure_total, 1);
    assert_eq!(first_metrics.retry_total, 1);
    assert_eq!(first_metrics.success_total, 0);

    let restarted_target: Arc<dyn EventTransport> = target.clone();
    let restarted_relay = OutboxRelay::new(db.clone(), restarted_target)
        .with_config(relay_config("blog-audit-relay-after-restart", 3));
    assert_eq!(restarted_relay.process_pending_once(Some(1)).await?, 1);

    let dispatched = SysEvents::find_by_id(request_id)
        .one(&db)
        .await?
        .expect("successful restarted relay must retain acknowledged row");
    assert_eq!(dispatched.status, SysEventStatus::Dispatched);
    assert_eq!(dispatched.retry_count, 1);
    assert!(dispatched.last_error.is_none());
    assert!(dispatched.next_attempt_at.is_none());
    assert!(dispatched.claimed_by.is_none());
    assert!(dispatched.claimed_at.is_none());
    assert!(dispatched.dispatched_at.is_some());

    assert_eq!(
        target.delivered(),
        vec![DeliveredAudit {
            id: request_id,
            correlation_id: request_id,
            tenant_id,
            actor_id: Some(actor_id),
            request_id,
            event_type: BLOG_COMMENTS_SCHEDULE_AUDIT_EVENT_TYPE.to_string(),
            schema_version: BLOG_COMMENTS_SCHEDULE_AUDIT_SCHEMA_VERSION,
        }]
    );
    let restarted_metrics = restarted_relay.metrics();
    assert_eq!(restarted_metrics.success_total, 1);
    assert_eq!(restarted_metrics.failure_total, 0);
    assert_eq!(restarted_metrics.processed_total, 1);

    database.cleanup().await
}

#[tokio::test]
async fn blog_audit_relay_retries_then_moves_exact_envelope_to_dlq() -> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let tenant_id = Uuid::new_v4();
    let actor_id = Uuid::new_v4();
    let request_id = Uuid::new_v4();
    seed_blog_audit(&db, tenant_id, actor_id, request_id).await?;

    let target = Arc::new(RecordingContractTransport::new(8));
    let first_target: Arc<dyn EventTransport> = target.clone();
    let first_relay = OutboxRelay::new(db.clone(), first_target)
        .with_config(relay_config("blog-audit-dlq-before-restart", 2));
    assert_eq!(first_relay.process_pending_once(Some(1)).await?, 1);

    let retrying = SysEvents::find_by_id(request_id)
        .one(&db)
        .await?
        .expect("first failed Blog audit relay attempt must remain durable");
    assert_eq!(retrying.status, SysEventStatus::Pending);
    assert_eq!(retrying.retry_count, 1);
    assert!(retrying.next_attempt_at.is_some());
    assert!(retrying.dispatched_at.is_none());
    assert!(retrying.claimed_by.is_none());
    assert!(retrying.claimed_at.is_none());

    let second_target: Arc<dyn EventTransport> = target.clone();
    let restarted_relay = OutboxRelay::new(db.clone(), second_target)
        .with_config(relay_config("blog-audit-dlq-after-restart", 2));
    assert_eq!(restarted_relay.process_pending_once(Some(1)).await?, 1);

    let failed = SysEvents::find_by_id(request_id)
        .one(&db)
        .await?
        .expect("attempt-budget exhaustion must retain Blog audit DLQ row");
    assert_eq!(failed.status, SysEventStatus::Failed);
    assert_eq!(failed.retry_count, 2);
    assert!(failed.next_attempt_at.is_none());
    assert!(failed.last_error.is_some());
    assert!(failed.claimed_by.is_none());
    assert!(failed.claimed_at.is_none());
    assert!(failed.dispatched_at.is_none());
    assert_eq!(failed.id, request_id);
    assert_eq!(failed.event_type, BLOG_COMMENTS_SCHEDULE_AUDIT_EVENT_TYPE);
    assert_eq!(
        failed.schema_version,
        BLOG_COMMENTS_SCHEDULE_AUDIT_SCHEMA_VERSION as i16
    );
    assert!(target.delivered().is_empty());
    let metrics = restarted_relay.metrics();
    assert_eq!(metrics.failure_total, 1);
    assert_eq!(metrics.dlq_total, 1);
    assert_eq!(metrics.success_total, 0);

    database.cleanup().await
}

async fn seed_blog_audit(
    db: &DatabaseConnection,
    tenant_id: Uuid,
    actor_id: Uuid,
    request_id: Uuid,
) -> TestResult<()> {
    let event = BlogCommentsDelegationScheduleAuditEvent::ReplacementSucceeded {
        audit_schema_version: BLOG_COMMENTS_SCHEDULE_AUDIT_SCHEMA_VERSION,
        request_id,
        state_key: BLOG_COMMENTS_SCHEDULE_AUDIT_STATE_KEY.to_string(),
        occurred_at_unix_ms: 1,
        principal_kind: "service".to_string(),
        operation: "replace_host_schedule".to_string(),
        source: "host_provided".to_string(),
        previous_generation: 1,
        candidate_generation: 2,
    };
    let txn = db.begin().await?;
    let written = match TransactionalEventBus::publish_contract_once_direct_in_tx_with_envelope_id(
        &txn,
        request_id,
        tenant_id,
        Some(actor_id),
        event,
    )
    .await
    {
        Ok(written) => written,
        Err(error) => panic!("canonical Blog audit write failed: {error:?}"),
    };
    assert_eq!(written, request_id);
    txn.commit().await?;
    Ok(())
}

fn relay_config(worker_id: &str, max_attempts: i32) -> RelayConfig {
    RelayConfig {
        batch_size: 1,
        max_attempts,
        backoff_base: Duration::ZERO,
        backoff_max: Duration::ZERO,
        max_concurrency: 1,
        claim_ttl: Duration::from_millis(1),
        worker_id: worker_id.to_string(),
    }
}

fn database_url() -> Option<String> {
    env::var(DATABASE_ENV)
        .or_else(|_| env::var("RUSTOK_OUTBOX_TEST_DATABASE_URL"))
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
