#[path = "reconciliation_heartbeat_cancel/connection.rs"]
mod connection;
#[path = "reconciliation_heartbeat_cancel/database.rs"]
mod database;
#[path = "reconciliation_heartbeat_cancel/job.rs"]
mod job;
#[path = "reconciliation_heartbeat_cancel/runner.rs"]
mod runner;
#[path = "reconciliation_heartbeat_cancel/schema.rs"]
mod schema;
#[path = "reconciliation_heartbeat_cancel/source.rs"]
mod source;

use std::sync::Arc;

use rustok_index::{
    IndexReconciliationCancelOutcome, IndexReconciliationRunStatus,
    IndexReconciliationTerminalState,
};
use tokio::sync::Barrier;
use uuid::Uuid;

use connection::TestResult;
use database::TestDatabase;
use job::{count, read_job, shorten_attempt_one};
use runner::{request, runner};
use source::HeartbeatCancelSource;

#[tokio::test]
async fn cancellation_after_heartbeat_preserves_cursor_and_recovers_duplicates() -> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };
    let tenant_id = database.tenant_id;
    let first_entered = Arc::new(Barrier::new(2));
    let first_release = Arc::new(Barrier::new(2));
    let second_entered = Arc::new(Barrier::new(2));
    let second_release = Arc::new(Barrier::new(2));

    let active_runner = runner(
        database.connection().await?,
        HeartbeatCancelSource::Blocking {
            first_entered: first_entered.clone(),
            first_release: first_release.clone(),
            second_entered: second_entered.clone(),
            second_release: second_release.clone(),
        },
    );
    let active_task = tokio::spawn(async move {
        active_runner
            .run(request(tenant_id, "heartbeat-cancel-worker-a"))
            .await
    });

    first_entered.wait().await;
    let inspection = database.connection().await?;
    let initial = read_job(&inspection, tenant_id, "running").await?;
    assert_eq!(initial.state, "running");
    assert_eq!(initial.attempt_count, 1);
    assert_eq!(initial.completed_passes, 0);
    assert_eq!(initial.pages_processed, 0);
    assert_eq!(
        initial.lease_owner.as_deref(),
        Some("heartbeat-cancel-worker-a")
    );
    assert!(!initial.lease_released);
    assert!(!initial.cancel_requested);

    shorten_attempt_one(&inspection, tenant_id, initial.job_id).await?;
    let shortened = read_job(&inspection, tenant_id, "running").await?;
    assert!(!shortened.lease_extended);

    first_release.wait().await;
    second_entered.wait().await;

    let after_heartbeat = read_job(&inspection, tenant_id, "running").await?;
    assert_eq!(after_heartbeat.job_id, initial.job_id);
    assert_eq!(after_heartbeat.attempt_count, 1);
    assert_eq!(after_heartbeat.completed_passes, 0);
    assert_eq!(after_heartbeat.pages_processed, 1);
    assert_eq!(
        after_heartbeat.lease_owner.as_deref(),
        Some("heartbeat-cancel-worker-a")
    );
    assert!(after_heartbeat.lease_extended);
    assert!(!after_heartbeat.cancel_requested);

    let canceller = runner(
        database.connection().await?,
        HeartbeatCancelSource::Immediate,
    );
    assert_eq!(
        canceller
            .request_cancel(Uuid::new_v4(), initial.job_id)
            .await?,
        IndexReconciliationCancelOutcome::NotFound
    );
    assert_eq!(
        canceller.request_cancel(tenant_id, initial.job_id).await?,
        IndexReconciliationCancelOutcome::Requested
    );

    let cancel_requested = read_job(&inspection, tenant_id, "running").await?;
    assert_eq!(cancel_requested.job_id, initial.job_id);
    assert_eq!(cancel_requested.pages_processed, 1);
    assert!(cancel_requested.lease_extended);
    assert!(cancel_requested.cancel_requested);

    second_release.wait().await;
    let cancelled_outcome = active_task.await??;
    assert_eq!(
        cancelled_outcome.status(),
        IndexReconciliationRunStatus::Cancelled
    );
    assert_eq!(cancelled_outcome.job_id(), Some(initial.job_id));
    assert_eq!(cancelled_outcome.attempt_count(), Some(1));
    assert_eq!(cancelled_outcome.pages_processed(), 2);
    assert_eq!(cancelled_outcome.passes_completed(), 1);
    assert_eq!(cancelled_outcome.heartbeat_count(), 1);
    assert_eq!(cancelled_outcome.applied_count(), 2);
    assert_eq!(cancelled_outcome.duplicate_count(), 0);

    let cancelled_job = read_job(&inspection, tenant_id, "cancelled").await?;
    assert_eq!(cancelled_job.job_id, initial.job_id);
    assert_eq!(cancelled_job.attempt_count, 1);
    assert_eq!(cancelled_job.completed_passes, 0);
    assert_eq!(cancelled_job.pages_processed, 1);
    assert_eq!(cancelled_job.lease_owner, None);
    assert!(cancelled_job.lease_released);
    assert!(cancelled_job.cancel_requested);
    assert_eq!(count(&inspection, "index_jobs").await?, 1);
    assert_eq!(count(&inspection, "index_entities").await?, 2);
    assert_eq!(count(&inspection, "index_inbox").await?, 2);

    assert_eq!(
        canceller.request_cancel(tenant_id, initial.job_id).await?,
        IndexReconciliationCancelOutcome::AlreadyTerminal(
            IndexReconciliationTerminalState::Cancelled
        )
    );

    let recovery = runner(
        database.connection().await?,
        HeartbeatCancelSource::Immediate,
    )
    .run(request(tenant_id, "heartbeat-cancel-worker-b"))
    .await?;
    let recovery_job_id = recovery.job_id().expect("recovery job id");
    assert_ne!(recovery_job_id, initial.job_id);
    assert_eq!(recovery.status(), IndexReconciliationRunStatus::Complete);
    assert_eq!(recovery.attempt_count(), Some(1));
    assert_eq!(recovery.pages_processed(), 2);
    assert_eq!(recovery.passes_completed(), 1);
    assert_eq!(recovery.heartbeat_count(), 1);
    assert_eq!(recovery.applied_count(), 0);
    assert_eq!(recovery.duplicate_count(), 2);

    let succeeded_job = read_job(&inspection, tenant_id, "succeeded").await?;
    assert_eq!(succeeded_job.job_id, recovery_job_id);
    assert_eq!(succeeded_job.attempt_count, 1);
    assert_eq!(succeeded_job.completed_passes, 1);
    assert_eq!(succeeded_job.pages_processed, 2);
    assert_eq!(succeeded_job.lease_owner, None);
    assert!(succeeded_job.lease_released);
    assert!(!succeeded_job.cancel_requested);
    assert_eq!(count(&inspection, "index_jobs").await?, 2);
    assert_eq!(count(&inspection, "index_entities").await?, 2);
    assert_eq!(count(&inspection, "index_inbox").await?, 2);

    let already_complete = runner(
        database.connection().await?,
        HeartbeatCancelSource::Immediate,
    )
    .run(request(tenant_id, "heartbeat-cancel-worker-c"))
    .await?;
    assert_eq!(
        already_complete.status(),
        IndexReconciliationRunStatus::AlreadyComplete
    );
    assert_eq!(already_complete.job_id(), Some(recovery_job_id));
    assert_eq!(already_complete.attempt_count(), None);
    assert_eq!(already_complete.pages_processed(), 0);
    assert_eq!(already_complete.passes_completed(), 1);

    database.cleanup().await
}
