#[path = "reconciliation_stale_version_guard/connection.rs"]
mod connection;
#[path = "reconciliation_stale_version_guard/database.rs"]
mod database;
#[path = "reconciliation_stale_version_guard/evidence.rs"]
mod evidence;
#[path = "reconciliation_stale_version_guard/runner.rs"]
mod runner;
#[path = "reconciliation_stale_version_guard/schema.rs"]
mod schema;
#[path = "reconciliation_stale_version_guard/source.rs"]
mod source;

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use rustok_index::IndexReconciliationRunStatus;
use serde_json::json;

use connection::TestResult;
use database::TestDatabase;
use evidence::{assert_inbox_applied, count, read_entity, read_job};
use runner::{request, runner};
use source::{
    ENTITY_ID, FRESH_DELETE_EVENT_ID, FRESH_UPSERT_EVENT_ID, STALE_DELETE_EVENT_ID,
    STALE_UPSERT_EVENT_ID, fresh_fields,
};

#[tokio::test]
async fn reconciliation_retains_stale_delete_and_resurrection_guards() -> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };
    let calls = Arc::new(AtomicUsize::new(0));

    let first_runner = runner(database.connection().await?, calls.clone());
    let first = first_runner
        .run(request(
            database.tenant_id,
            "stale-version-guard-worker-a",
            2,
        ))
        .await?;
    let job_id = first.job_id().expect("yielded reconciliation job id");
    assert_eq!(first.status(), IndexReconciliationRunStatus::Yielded);
    assert_eq!(first.attempt_count(), Some(1));
    assert_eq!(first.pages_processed(), 2);
    assert_eq!(first.passes_completed(), 0);
    assert_eq!(first.heartbeat_count(), 1);
    assert_eq!(first.mutation_count(), 2);
    assert_eq!(first.applied_count(), 1);
    assert_eq!(first.duplicate_count(), 0);
    assert_eq!(first.stale_count(), 1);
    assert_eq!(calls.load(Ordering::SeqCst), 2);

    let inspection = database.connection().await?;
    let pending = read_job(&inspection, database.tenant_id, "pending").await?;
    assert_eq!(pending.job_id, job_id);
    assert_eq!(pending.state, "pending");
    assert_eq!(pending.attempt_count, 1);
    assert_eq!(pending.completed_passes, 0);
    assert_eq!(pending.pages_processed, 2);
    assert_eq!(pending.source_cursor, json!({ "offset": 2 }));
    assert_eq!(pending.mutation_count, 2);
    assert_eq!(pending.applied_count, 1);
    assert_eq!(pending.duplicate_count, 0);
    assert_eq!(pending.stale_count, 1);
    assert_eq!(pending.last_error_code, None);
    assert_eq!(pending.last_error_details, None);
    assert!(pending.lease_released);
    assert!(!pending.completed);

    let live = read_entity(&inspection, database.tenant_id, ENTITY_ID).await?;
    assert_eq!(live.source_version, 3);
    assert!(!live.is_deleted);
    assert_eq!(live.payload, Some(serde_json::to_value(fresh_fields())?));
    assert_eq!(count(&inspection, "index_jobs").await?, 1);
    assert_eq!(count(&inspection, "index_entities").await?, 1);
    assert_eq!(count(&inspection, "index_inbox").await?, 2);
    assert_eq!(count(&inspection, "index_links").await?, 0);
    assert_inbox_applied(&inspection, database.tenant_id, FRESH_UPSERT_EVENT_ID).await?;
    assert_inbox_applied(&inspection, database.tenant_id, STALE_DELETE_EVENT_ID).await?;

    let second_runner = runner(database.connection().await?, calls.clone());
    let second = second_runner
        .run(request(
            database.tenant_id,
            "stale-version-guard-worker-b",
            4,
        ))
        .await?;
    assert_eq!(second.status(), IndexReconciliationRunStatus::Complete);
    assert_eq!(second.job_id(), Some(job_id));
    assert_eq!(second.attempt_count(), Some(2));
    assert_eq!(second.pages_processed(), 2);
    assert_eq!(second.passes_completed(), 1);
    assert_eq!(second.heartbeat_count(), 1);
    assert_eq!(second.mutation_count(), 2);
    assert_eq!(second.applied_count(), 1);
    assert_eq!(second.duplicate_count(), 0);
    assert_eq!(second.stale_count(), 1);
    assert_eq!(calls.load(Ordering::SeqCst), 4);

    let succeeded = read_job(&inspection, database.tenant_id, "succeeded").await?;
    assert_eq!(succeeded.job_id, job_id);
    assert_eq!(succeeded.state, "succeeded");
    assert_eq!(succeeded.attempt_count, 2);
    assert_eq!(succeeded.completed_passes, 1);
    assert_eq!(succeeded.pages_processed, 4);
    assert_eq!(succeeded.source_cursor, json!(null));
    assert_eq!(succeeded.mutation_count, 4);
    assert_eq!(succeeded.applied_count, 2);
    assert_eq!(succeeded.duplicate_count, 0);
    assert_eq!(succeeded.stale_count, 2);
    assert_eq!(succeeded.last_error_code, None);
    assert_eq!(succeeded.last_error_details, None);
    assert!(succeeded.lease_released);
    assert!(succeeded.completed);

    let tombstone = read_entity(&inspection, database.tenant_id, ENTITY_ID).await?;
    assert_eq!(tombstone.source_version, 4);
    assert!(tombstone.is_deleted);
    assert_eq!(tombstone.payload, None);
    assert_eq!(count(&inspection, "index_jobs").await?, 1);
    assert_eq!(count(&inspection, "index_entities").await?, 1);
    assert_eq!(count(&inspection, "index_inbox").await?, 4);
    assert_eq!(count(&inspection, "index_links").await?, 0);
    assert_inbox_applied(&inspection, database.tenant_id, FRESH_UPSERT_EVENT_ID).await?;
    assert_inbox_applied(&inspection, database.tenant_id, STALE_DELETE_EVENT_ID).await?;
    assert_inbox_applied(&inspection, database.tenant_id, FRESH_DELETE_EVENT_ID).await?;
    assert_inbox_applied(&inspection, database.tenant_id, STALE_UPSERT_EVENT_ID).await?;

    let already_complete = second_runner
        .run(request(
            database.tenant_id,
            "stale-version-guard-worker-complete",
            4,
        ))
        .await?;
    assert_eq!(
        already_complete.status(),
        IndexReconciliationRunStatus::AlreadyComplete
    );
    assert_eq!(already_complete.job_id(), Some(job_id));
    assert_eq!(already_complete.attempt_count(), None);
    assert_eq!(already_complete.pages_processed(), 0);
    assert_eq!(already_complete.passes_completed(), 1);
    assert_eq!(already_complete.heartbeat_count(), 0);
    assert_eq!(already_complete.mutation_count(), 0);
    assert_eq!(already_complete.applied_count(), 0);
    assert_eq!(already_complete.duplicate_count(), 0);
    assert_eq!(already_complete.stale_count(), 0);
    assert_eq!(calls.load(Ordering::SeqCst), 4);

    database.cleanup().await
}
