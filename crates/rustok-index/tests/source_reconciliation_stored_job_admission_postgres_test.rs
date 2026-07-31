#[path = "reconciliation_stored_job_admission/connection.rs"]
mod connection;
#[path = "reconciliation_stored_job_admission/database.rs"]
mod database;
#[path = "reconciliation_stored_job_admission/evidence.rs"]
mod evidence;
#[path = "reconciliation_stored_job_admission/runner.rs"]
mod runner;
#[path = "reconciliation_stored_job_admission/schema.rs"]
mod schema;
#[path = "reconciliation_stored_job_admission/source.rs"]
mod source;

use std::sync::{
    Arc,
    atomic::{AtomicUsize, Ordering},
};

use rustok_index::{IndexReconciliationRunError, IndexReconciliationRunStatus};
use serde_json::json;
use uuid::Uuid;

use connection::TestResult;
use database::TestDatabase;
use evidence::{JobEvidence, count, read_job};
use runner::{request, runner};

const CURSOR_CONTRACT: &str = "index_reconciliation_cursor_v1";
const CORRUPT_CURSOR_CONTRACT: &str = "index_reconciliation_cursor_corrupt";
const REQUEST_MISMATCH_REASON: &str =
    "stored reconciliation request does not match the source/pass contract";
const CURSOR_CONTRACT_REASON: &str = "cursor contract is invalid";

async fn create_pending_job(
    database: &TestDatabase,
    calls: &Arc<AtomicUsize>,
    worker_id: &str,
) -> TestResult<Uuid> {
    let outcome = runner(database.connection().await?, calls.clone())
        .run(request(database.tenant_id, worker_id, 1))
        .await?;
    let job_id = outcome.job_id().expect("yielded job id");
    assert_eq!(outcome.status(), IndexReconciliationRunStatus::Yielded);
    assert_eq!(outcome.attempt_count(), Some(1));
    assert_eq!(outcome.pages_processed(), 1);
    assert_eq!(outcome.passes_completed(), 0);
    assert_eq!(outcome.heartbeat_count(), 0);
    assert_eq!(outcome.mutation_count(), 1);
    assert_eq!(outcome.applied_count(), 1);
    assert_eq!(outcome.duplicate_count(), 0);
    assert_eq!(outcome.stale_count(), 0);
    assert_eq!(calls.load(Ordering::SeqCst), 1);

    let inspection = database.connection().await?;
    let pending = read_job(&inspection, database.tenant_id, "pending").await?;
    assert_pending_boundary(&pending, job_id, 1, CURSOR_CONTRACT);
    assert_eq!(count(&inspection, "index_jobs").await?, 1);
    assert_eq!(count(&inspection, "index_entities").await?, 1);
    assert_eq!(count(&inspection, "index_inbox").await?, 1);
    Ok(job_id)
}

fn assert_pending_boundary(
    pending: &JobEvidence,
    job_id: Uuid,
    request_pass_count: i64,
    cursor_contract: &str,
) {
    assert_eq!(pending.job_id, job_id);
    assert_eq!(pending.state, "pending");
    assert_eq!(pending.attempt_count, 1);
    assert_eq!(pending.request_pass_count, request_pass_count);
    assert_eq!(pending.cursor_contract, cursor_contract);
    assert_eq!(pending.completed_passes, 0);
    assert_eq!(pending.pages_processed, 1);
    assert_eq!(pending.source_cursor, json!({ "offset": 1 }));
    assert_eq!(pending.last_error_code, None);
    assert_eq!(pending.last_error_details, None);
    assert!(pending.lease_released);
    assert!(!pending.completed);
}

async fn assert_recovery(
    database: &TestDatabase,
    calls: &Arc<AtomicUsize>,
    job_id: Uuid,
    worker_id: &str,
) -> TestResult<()> {
    let recovery_runner = runner(database.connection().await?, calls.clone());
    let recovery = recovery_runner
        .run(request(database.tenant_id, worker_id, 4))
        .await?;
    assert_eq!(recovery.status(), IndexReconciliationRunStatus::Complete);
    assert_eq!(recovery.job_id(), Some(job_id));
    assert_eq!(recovery.attempt_count(), Some(2));
    assert_eq!(recovery.pages_processed(), 1);
    assert_eq!(recovery.passes_completed(), 1);
    assert_eq!(recovery.heartbeat_count(), 0);
    assert_eq!(recovery.mutation_count(), 1);
    assert_eq!(recovery.applied_count(), 1);
    assert_eq!(recovery.duplicate_count(), 0);
    assert_eq!(recovery.stale_count(), 0);
    assert_eq!(calls.load(Ordering::SeqCst), 2);

    let inspection = database.connection().await?;
    let succeeded = read_job(&inspection, database.tenant_id, "succeeded").await?;
    assert_eq!(succeeded.job_id, job_id);
    assert_eq!(succeeded.state, "succeeded");
    assert_eq!(succeeded.attempt_count, 2);
    assert_eq!(succeeded.request_pass_count, 1);
    assert_eq!(succeeded.cursor_contract, CURSOR_CONTRACT);
    assert_eq!(succeeded.completed_passes, 1);
    assert_eq!(succeeded.pages_processed, 2);
    assert_eq!(succeeded.source_cursor, json!(null));
    assert_eq!(succeeded.last_error_code, None);
    assert_eq!(succeeded.last_error_details, None);
    assert!(succeeded.lease_released);
    assert!(succeeded.completed);
    assert_eq!(count(&inspection, "index_jobs").await?, 1);
    assert_eq!(count(&inspection, "index_entities").await?, 2);
    assert_eq!(count(&inspection, "index_inbox").await?, 2);

    let already_complete = recovery_runner
        .run(request(
            database.tenant_id,
            "stored-job-admission-worker-complete",
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
    assert_eq!(calls.load(Ordering::SeqCst), 2);
    Ok(())
}

#[tokio::test]
async fn stored_request_mismatch_blocks_claim_and_recovers_after_repair() -> TestResult<()> {
    let Some(database) = TestDatabase::setup("request").await? else {
        return Ok(());
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let job_id = create_pending_job(&database, &calls, "stored-job-request-worker-a").await?;

    database.corrupt_request_pass_count(job_id).await?;
    let inspection = database.connection().await?;
    let corrupted = read_job(&inspection, database.tenant_id, "pending").await?;
    assert_pending_boundary(&corrupted, job_id, 2, CURSOR_CONTRACT);

    let error = runner(database.connection().await?, calls.clone())
        .run(request(database.tenant_id, "stored-job-request-worker-b", 4))
        .await
        .expect_err("stored request mismatch must fail before claim");
    match error {
        IndexReconciliationRunError::InvalidStoredJob(reason) => {
            assert_eq!(reason, REQUEST_MISMATCH_REASON);
        }
        other => panic!("unexpected stored request admission error: {other:?}"),
    }

    let blocked = read_job(&inspection, database.tenant_id, "pending").await?;
    assert_pending_boundary(&blocked, job_id, 2, CURSOR_CONTRACT);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(count(&inspection, "index_jobs").await?, 1);
    assert_eq!(count(&inspection, "index_entities").await?, 1);
    assert_eq!(count(&inspection, "index_inbox").await?, 1);

    database.restore_request_pass_count(job_id).await?;
    assert_recovery(&database, &calls, job_id, "stored-job-request-worker-c").await?;
    database.cleanup().await
}

#[tokio::test]
async fn stored_cursor_contract_blocks_claim_and_recovers_after_repair() -> TestResult<()> {
    let Some(database) = TestDatabase::setup("cursor").await? else {
        return Ok(());
    };
    let calls = Arc::new(AtomicUsize::new(0));
    let job_id = create_pending_job(&database, &calls, "stored-job-cursor-worker-a").await?;

    database.corrupt_cursor_contract(job_id).await?;
    let inspection = database.connection().await?;
    let corrupted = read_job(&inspection, database.tenant_id, "pending").await?;
    assert_pending_boundary(&corrupted, job_id, 1, CORRUPT_CURSOR_CONTRACT);

    let error = runner(database.connection().await?, calls.clone())
        .run(request(database.tenant_id, "stored-job-cursor-worker-b", 4))
        .await
        .expect_err("stored cursor contract mismatch must fail before claim");
    match error {
        IndexReconciliationRunError::InvalidStoredJob(reason) => {
            assert_eq!(reason, CURSOR_CONTRACT_REASON);
        }
        other => panic!("unexpected stored cursor admission error: {other:?}"),
    }

    let blocked = read_job(&inspection, database.tenant_id, "pending").await?;
    assert_pending_boundary(&blocked, job_id, 1, CORRUPT_CURSOR_CONTRACT);
    assert_eq!(calls.load(Ordering::SeqCst), 1);
    assert_eq!(count(&inspection, "index_jobs").await?, 1);
    assert_eq!(count(&inspection, "index_entities").await?, 1);
    assert_eq!(count(&inspection, "index_inbox").await?, 1);

    database.restore_cursor_contract(job_id).await?;
    assert_recovery(&database, &calls, job_id, "stored-job-cursor-worker-c").await?;
    database.cleanup().await
}
