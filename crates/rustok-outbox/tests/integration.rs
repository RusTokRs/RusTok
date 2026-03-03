use std::any::Any;
use std::collections::HashMap;
use std::sync::OnceLock;
use std::sync::Arc;

use async_trait::async_trait;
use chrono::Utc;
use rustok_core::events::{EventEnvelope, EventTransport, ReliabilityLevel};
use rustok_core::{DomainEvent, Error, Result};
use rustok_outbox::entity::{self, SysEventStatus};
use rustok_outbox::{OutboxRelay, RelayConfig, SysEventsMigration};
use sea_orm::{ActiveModelTrait, Database, DatabaseConnection, EntityTrait, Set};
use sea_orm_migration::prelude::{MigrationTrait, SchemaManager};
use tokio::sync::Mutex;
use uuid::Uuid;

#[derive(Default)]
struct MockTransport {
    delivered: Mutex<Vec<Uuid>>,
    remaining_failures: Mutex<HashMap<Uuid, usize>>,
}

impl MockTransport {
    async fn fail_n_times_for(&self, event_id: Uuid, n: usize) {
        self.remaining_failures.lock().await.insert(event_id, n);
    }

    async fn delivered(&self) -> Vec<Uuid> {
        self.delivered.lock().await.clone()
    }
}

#[async_trait]
impl EventTransport for MockTransport {
    async fn publish(&self, envelope: EventEnvelope) -> Result<()> {
        let mut remaining_failures = self.remaining_failures.lock().await;
        if let Some(remaining) = remaining_failures.get_mut(&envelope.id) {
            if *remaining > 0 {
                *remaining -= 1;
                return Err(Error::External("temporary transport error".to_string()));
            }
        }

        self.delivered.lock().await.push(envelope.id);
        Ok(())
    }

    fn reliability_level(&self) -> ReliabilityLevel {
        ReliabilityLevel::Outbox
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

type TestResult<T> = std::result::Result<T, Box<dyn std::error::Error + Send + Sync>>;

#[tokio::test]
async fn relay_delivers_successfully() -> TestResult<()> {
    let _guard = test_lock().lock().await;

    let Some(db) = setup_db().await? else {
        return Ok(());
    };
    let envelope = seed_event(&db).await?;
    let transport = Arc::new(MockTransport::default());

    let relay = OutboxRelay::new(db.clone(), transport.clone()).with_config(RelayConfig {
        batch_size: 10,
        max_attempts: 3,
        ..Default::default()
    });

    let processed = relay.process_pending_once().await?;
    assert_eq!(processed, 1);

    let record = entity::Entity::find_by_id(envelope.id)
        .one(&db)
        .await?
        .expect("event record");
    assert_eq!(record.status, SysEventStatus::Dispatched);
    assert_eq!(record.retry_count, 0);

    let delivered = transport.delivered().await;
    assert_eq!(delivered, vec![envelope.id]);
    Ok(())
}

#[tokio::test]
async fn relay_retries_then_succeeds() -> TestResult<()> {
    let _guard = test_lock().lock().await;

    let Some(db) = setup_db().await? else {
        return Ok(());
    };
    let envelope = seed_event(&db).await?;
    let transport = Arc::new(MockTransport::default());
    transport.fail_n_times_for(envelope.id, 1).await;

    let relay = OutboxRelay::new(db.clone(), transport.clone()).with_config(RelayConfig {
        batch_size: 10,
        max_attempts: 3,
        backoff_base: std::time::Duration::from_millis(1),
        backoff_max: std::time::Duration::from_millis(2),
        ..Default::default()
    });

    let processed_first = relay.process_pending_once().await?;
    assert_eq!(processed_first, 1);

    let mut failed_once: entity::ActiveModel = entity::Entity::find_by_id(envelope.id)
        .one(&db)
        .await?
        .expect("event record")
        .into();
    failed_once.next_attempt_at = Set(Some(Utc::now() - chrono::Duration::milliseconds(1)));
    failed_once.update(&db).await?;

    let processed_second = relay.process_pending_once().await?;
    assert_eq!(processed_second, 1);

    let record = entity::Entity::find_by_id(envelope.id)
        .one(&db)
        .await?
        .expect("event record");
    assert_eq!(record.status, SysEventStatus::Dispatched);
    assert_eq!(record.retry_count, 1);

    let metrics = relay.metrics();
    assert_eq!(metrics.retry_total, 1);
    assert_eq!(metrics.success_total, 1);
    Ok(())
}

#[tokio::test]
async fn relay_moves_to_dlq_on_max_retry() -> TestResult<()> {
    let _guard = test_lock().lock().await;

    let Some(db) = setup_db().await? else {
        return Ok(());
    };
    let envelope = seed_event(&db).await?;
    let transport = Arc::new(MockTransport::default());
    transport.fail_n_times_for(envelope.id, 10).await;

    let relay = OutboxRelay::new(db.clone(), transport).with_config(RelayConfig {
        batch_size: 10,
        max_attempts: 2,
        backoff_base: std::time::Duration::from_millis(1),
        backoff_max: std::time::Duration::from_millis(1),
        ..Default::default()
    });

    let _ = relay.process_pending_once().await?;

    let mut first_failed: entity::ActiveModel = entity::Entity::find_by_id(envelope.id)
        .one(&db)
        .await?
        .expect("event record")
        .into();
    first_failed.next_attempt_at = Set(Some(Utc::now() - chrono::Duration::milliseconds(1)));
    first_failed.update(&db).await?;

    let _ = relay.process_pending_once().await?;

    let record = entity::Entity::find_by_id(envelope.id)
        .one(&db)
        .await?
        .expect("event record");
    assert_eq!(record.status, SysEventStatus::Failed);
    assert_eq!(record.retry_count, 2);
    assert!(record.next_attempt_at.is_none());
    assert!(record.last_error.is_some());

    let metrics = relay.metrics();
    assert_eq!(metrics.dlq_total, 1);
    assert_eq!(metrics.failure_total, 2);
    Ok(())
}

async fn setup_db() -> TestResult<Option<DatabaseConnection>> {
    let database_url = match std::env::var("RUSTOK_OUTBOX_TEST_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
    {
        Ok(url) => url,
        Err(_) => return Ok(None),
    };

    let db = Database::connect(&database_url).await?;
    let schema_manager = SchemaManager::new(&db);

    let _ = SysEventsMigration.down(&schema_manager).await;
    SysEventsMigration.up(&schema_manager).await?;

    Ok(Some(db))
}

async fn seed_event(db: &DatabaseConnection) -> TestResult<EventEnvelope> {
    let envelope = EventEnvelope::new(
        Uuid::nil(),
        None,
        DomainEvent::UserRegistered {
            user_id: Uuid::nil(),
            email: "test@example.com".to_string(),
        },
    );

    let payload = serde_json::to_value(&envelope)?;
    let model = entity::ActiveModel {
        id: Set(envelope.id),
        event_type: Set(envelope.event_type.clone()),
        schema_version: Set(envelope.schema_version as i16),
        payload: Set(payload),
        status: Set(SysEventStatus::Pending),
        retry_count: Set(0),
        next_attempt_at: Set(None),
        last_error: Set(None),
        claimed_by: Set(None),
        claimed_at: Set(None),
        created_at: Set(Utc::now()),
        dispatched_at: Set(None),
    };
    model.insert(db).await?;

    Ok(envelope)
}

fn test_lock() -> &'static Mutex<()> {
    static TEST_LOCK: OnceLock<Mutex<()>> = OnceLock::new();
    TEST_LOCK.get_or_init(|| Mutex::new(()))
}
