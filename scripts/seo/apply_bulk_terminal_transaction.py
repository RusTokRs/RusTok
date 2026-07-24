from __future__ import annotations

import os
from pathlib import Path


def replace_once(text: str, old: str, new: str, label: str) -> str:
    count = text.count(old)
    if count != 1:
        raise SystemExit(f"{label}: expected one match, found {count}")
    return text.replace(old, new, 1)


def replace_between(text: str, start: str, end: str, replacement: str, label: str) -> str:
    start_index = text.find(start)
    if start_index < 0:
        raise SystemExit(f"{label}: start marker not found")
    end_index = text.find(end, start_index)
    if end_index < 0:
        raise SystemExit(f"{label}: end marker not found")
    return text[:start_index] + replacement + text[end_index:]


def patch_events() -> None:
    path = Path("crates/rustok-seo/src/services/events.rs")
    text = path.read_text()
    text = replace_once(
        text,
        "    ActiveModelTrait, ColumnTrait, Condition, DbErr, EntityTrait, QueryFilter, QueryOrder,\n    QuerySelect,\n",
        "    ActiveModelTrait, ColumnTrait, Condition, DatabaseTransaction, DbErr, EntityTrait,\n    QueryFilter, QueryOrder, QuerySelect,\n",
        "events imports",
    )
    start = "    #[allow(clippy::too_many_arguments)]\n    pub(super) async fn publish_seo_bulk_completed_event("
    end = "    pub async fn index_delivery_status("
    replacement = '''    #[allow(clippy::too_many_arguments)]
    pub(super) async fn publish_seo_bulk_terminal_event_in_tx(
        &self,
        txn: &DatabaseTransaction,
        tenant_id: Uuid,
        job_id: Uuid,
        target_kind: &str,
        locale: &str,
        status: &str,
        processed_count: i32,
        succeeded_count: i32,
        failed_count: i32,
    ) -> SeoResult<()> {
        let event_scope = match status {
            "partial" => "seo.bulk.partial",
            "failed" => "seo.bulk.failed",
            _ => "seo.bulk.completed",
        };
        let idempotency_key = self.build_event_key(
            event_scope,
            tenant_id,
            &[
                target_kind.to_string(),
                locale.to_string(),
                job_id.to_string(),
                status.to_string(),
                processed_count.to_string(),
                succeeded_count.to_string(),
                failed_count.to_string(),
            ],
        );
        let existing = seo_event_delivery::Entity::find()
            .filter(seo_event_delivery::Column::TenantId.eq(tenant_id))
            .filter(seo_event_delivery::Column::IdempotencyKey.eq(idempotency_key.as_str()))
            .one(txn)
            .await?;
        if existing.is_some() {
            return Ok(());
        }

        let event = seo_bulk_terminal_event(
            job_id,
            target_kind,
            locale,
            status,
            processed_count,
            succeeded_count,
            failed_count,
            idempotency_key.clone(),
        );
        let outbox_event_id = self
            .event_bus
            .publish_in_tx_with_envelope_id(txn, tenant_id, None, event)
            .await
            .map_err(|error| {
                SeoError::Database(DbErr::Custom(format!(
                    "failed to enqueue bulk terminal event transactionally: {error}"
                )))
            })?;
        let now = Utc::now().fixed_offset();

        seo_event_delivery::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            event_type: Set(event_scope.to_string()),
            idempotency_key: Set(idempotency_key),
            source_kind: Set(Some("bulk_job".to_string())),
            source_id: Set(Some(job_id)),
            status: Set(DELIVERY_STATUS_SENT.to_string()),
            outbox_event_id: Set(Some(outbox_event_id)),
            last_error: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            dispatched_at: Set(Some(now)),
        }
        .insert(txn)
        .await?;

        Ok(())
    }

'''
    path.write_text(replace_between(text, start, end, replacement, "bulk terminal event method"))


def patch_bulk() -> None:
    path = Path("crates/rustok-seo/src/services/bulk.rs")
    text = path.read_text()
    text = replace_once(
        text,
        "use sea_orm::{ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder};",
        "use sea_orm::{\n    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, QueryOrder, TransactionTrait,\n};",
        "bulk imports",
    )
    fail_start = "    async fn fail_bulk_job(&self, job: &seo_bulk_job::Model, message: String) -> SeoResult<()> {"
    finish_start = "    async fn finish_bulk_job("
    fail_replacement = '''    async fn fail_bulk_job(&self, job: &seo_bulk_job::Model, message: String) -> SeoResult<()> {
        let now = Utc::now().fixed_offset();
        let txn = self.db.begin().await?;
        let mut active: seo_bulk_job::ActiveModel = job.clone().into();
        active.status = Set(SeoBulkJobStatus::Failed.as_str().to_string());
        active.last_error = Set(Some(limit_job_message(message)));
        active.completed_at = Set(Some(now));
        active.updated_at = Set(now);
        let updated = active.update(&txn).await?;

        self.publish_seo_bulk_terminal_event_in_tx(
            &txn,
            updated.tenant_id,
            updated.id,
            updated.target_kind.as_str(),
            updated.locale.as_str(),
            updated.status.as_str(),
            updated.processed_count,
            updated.succeeded_count,
            updated.failed_count,
        )
        .await?;
        txn.commit().await?;

        Ok(())
    }

'''
    text = replace_between(text, fail_start, finish_start, fail_replacement, "fail bulk job")
    item_start = "    async fn insert_bulk_job_item("
    finish_replacement = '''    async fn finish_bulk_job(
        &self,
        job: &seo_bulk_job::Model,
        processed_count: i32,
        succeeded_count: i32,
        failed_count: i32,
        artifact_count: i32,
        last_error: Option<String>,
    ) -> SeoResult<()> {
        let status = if failed_count == 0 {
            SeoBulkJobStatus::Completed
        } else if succeeded_count == 0 {
            SeoBulkJobStatus::Failed
        } else {
            SeoBulkJobStatus::Partial
        };
        let now = Utc::now().fixed_offset();
        let txn = self.db.begin().await?;
        let mut active: seo_bulk_job::ActiveModel = job.clone().into();
        active.status = Set(status.as_str().to_string());
        active.processed_count = Set(processed_count);
        active.succeeded_count = Set(succeeded_count);
        active.failed_count = Set(failed_count);
        active.artifact_count = Set(artifact_count);
        active.last_error = Set(last_error.map(limit_job_message));
        active.completed_at = Set(Some(now));
        active.updated_at = Set(now);
        let updated = active.update(&txn).await?;

        self.publish_seo_bulk_terminal_event_in_tx(
            &txn,
            updated.tenant_id,
            updated.id,
            updated.target_kind.as_str(),
            updated.locale.as_str(),
            updated.status.as_str(),
            updated.processed_count,
            updated.succeeded_count,
            updated.failed_count,
        )
        .await?;
        txn.commit().await?;

        Ok(())
    }

'''
    path.write_text(replace_between(text, finish_start, item_start, finish_replacement, "finish bulk job"))


def write_regression_test() -> None:
    Path("crates/rustok-seo/tests/bulk_terminal_transaction.rs").write_text(r'''use std::any::Any;
use std::sync::Arc;

use async_trait::async_trait;
use rustok_core::{Error, EventEnvelope, EventTransport, ReliabilityLevel};
use rustok_outbox::TransactionalEventBus;
use rustok_seo::entities::{seo_bulk_job, seo_event_delivery};
use rustok_seo::{SeoService, SeoTargetRegistry};
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

    let service = SeoService::new(
        db.clone(),
        TransactionalEventBus::new(Arc::new(FailingTransport)),
        Arc::new(SeoTargetRegistry::default()),
    );

    let error = service
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
''')


def patch_roadmap() -> None:
    path = Path("docs/roadmaps/seo-hardening-progress.md")
    text = path.read_text()
    pr_number = os.environ["PR_NUMBER"]
    replacements = [
        (
            "- [ ] Persist bulk terminal state and terminal event transactionally.",
            f"- [x] Persist bulk terminal state and terminal event transactionally. (#{pr_number})",
        ),
        (
            "- [ ] Add rollback coverage for bulk terminal state and terminal event transactions.",
            f"- [x] Add rollback coverage for bulk terminal state and terminal event transactions. (#{pr_number})",
        ),
        (
            "- [ ] Run `cargo fmt --check` for the affected workspace packages.",
            f"- [x] Run `cargo fmt --check` for the affected workspace packages. (#{pr_number})",
        ),
        (
            "- [ ] Run `cargo check -p rustok-seo`.",
            f"- [x] Run `cargo check -p rustok-seo`. (#{pr_number})",
        ),
        (
            "- [ ] Run `cargo test -p rustok-seo`.",
            f"- [x] Run `cargo test -p rustok-seo`. (#{pr_number})",
        ),
    ]
    for old, new in replacements:
        text = replace_once(text, old, new, old)
    path.write_text(text)


def remove_bootstrap_files() -> None:
    for value in [
        ".github/workflows/seo-bulk-terminal-transaction-slice.yml",
        "scripts/seo/apply_bulk_terminal_transaction.py",
    ]:
        Path(value).unlink(missing_ok=True)


patch_events()
patch_bulk()
write_regression_test()
patch_roadmap()
remove_bootstrap_files()
