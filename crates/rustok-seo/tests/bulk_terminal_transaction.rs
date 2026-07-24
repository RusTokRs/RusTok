use std::any::Any;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_core::{Error, EventEnvelope, EventTransport, ReliabilityLevel};
use rustok_outbox::TransactionalEventBus;
use rustok_seo::entities::{seo_bulk_job, seo_event_delivery};
use rustok_seo::{SeoApplicationServices, SeoTargetRegistry};
use sea_orm::ActiveValue::Set;
use sea_orm::{
    ActiveModelTrait, ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend,
    EntityTrait, PaginatorTrait, Statement,
};
use serde_json::json;
use uuid::Uuid;

struct FailingTransport;

#[async_trait]
impl EventTransport for FailingTransport {
    async fn publish(&self, _envelope: EventEnvelope) -> rustok_core::Result<()> {
        Err(Error::Validation(
            "forced SEO bulk terminal event failure".to_string(),
        ))
    }

    fn reliability_level(&self) -> ReliabilityLevel {
        ReliabilityLevel::InMemory
    }

    fn as_any(&self) -> &dyn Any {
        self
    }
}

#[tokio::test]
async fn bulk_terminal_state_rolls_back_when_transactional_event_fails() {
    let db = test_db().await;
    create_tables(&db).await;

    let tenant_id = Uuid::new_v4();
    let job_id = Uuid::new_v4();
    let now = chrono::Utc::now().fixed_offset();
    seo_bulk_job::ActiveModel {
        id: Set(job_id),
        tenant_id: Set(tenant_id),
        operation_kind: Set("unknown".to_string()),
        status: Set("queued".to_string()),
        target_kind: Set("page".to_string()),
        locale: Set("en".to_string()),
        filter_payload: Set(json!({})),
        input_payload: Set(json!({})),
        publish_after_write: Set(false),
        matched_count: Set(0),
        processed_count: Set(0),
        succeeded_count: Set(0),
        failed_count: Set(0),
        artifact_count: Set(0),
        last_error: Set(None),
        created_by: Set(None),
        started_at: Set(None),
        completed_at: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(&db)
    .await
    .expect("failed to insert queued bulk job");

    let service = SeoApplicationServices::new(
        db.clone(),
        TransactionalEventBus::new(Arc::new(FailingTransport)),
        Arc::new(SeoTargetRegistry::default()),
    );

    let error = service
        .bulk()
        .execute_next_bulk_job()
        .await
        .expect_err("event failure must abort the bulk terminal transaction");
    assert!(
        error
            .to_string()
            .contains("failed to enqueue bulk terminal event transactionally")
    );

    let persisted = seo_bulk_job::Entity::find_by_id(job_id)
        .one(&db)
        .await
        .expect("bulk job should load")
        .expect("bulk job should remain present");
    assert_eq!(persisted.status, "running");
    assert!(persisted.completed_at.is_none());
    assert!(persisted.last_error.is_none());
    assert_eq!(persisted.processed_count, 0);
    assert_eq!(persisted.succeeded_count, 0);
    assert_eq!(persisted.failed_count, 0);
    assert_eq!(
        seo_event_delivery::Entity::find()
            .count(&db)
            .await
            .expect("delivery count should load"),
        0
    );
}

async fn test_db() -> DatabaseConnection {
    let mut options = ConnectOptions::new("sqlite::memory:");
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    Database::connect(options)
        .await
        .expect("failed to connect SEO test database")
}

async fn create_tables(db: &DatabaseConnection) {
    for sql in [
        "CREATE TABLE seo_bulk_jobs (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            operation_kind TEXT NOT NULL,
            status TEXT NOT NULL,
            target_kind TEXT NOT NULL,
            locale TEXT NOT NULL,
            filter_payload TEXT NOT NULL,
            input_payload TEXT NOT NULL,
            publish_after_write INTEGER NOT NULL,
            matched_count INTEGER NOT NULL,
            processed_count INTEGER NOT NULL,
            succeeded_count INTEGER NOT NULL,
            failed_count INTEGER NOT NULL,
            artifact_count INTEGER NOT NULL,
            last_error TEXT NULL,
            created_by TEXT NULL,
            started_at TEXT NULL,
            completed_at TEXT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL
        )",
        "CREATE TABLE seo_event_deliveries (
            id TEXT PRIMARY KEY,
            tenant_id TEXT NOT NULL,
            event_type TEXT NOT NULL,
            idempotency_key TEXT NOT NULL,
            source_kind TEXT NULL,
            source_id TEXT NULL,
            status TEXT NOT NULL,
            outbox_event_id TEXT NULL,
            last_error TEXT NULL,
            created_at TEXT NOT NULL,
            updated_at TEXT NOT NULL,
            dispatched_at TEXT NULL
        )",
        "CREATE UNIQUE INDEX idx_seo_event_deliveries_idempotency
            ON seo_event_deliveries (tenant_id, idempotency_key)",
    ] {
        db.execute(Statement::from_string(DbBackend::Sqlite, sql.to_string()))
            .await
            .expect("failed to create SEO bulk transaction test table");
    }
}
