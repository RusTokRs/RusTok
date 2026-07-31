#[path = "reconciliation_heartbeat_takeover/connection.rs"]
mod connection;
#[path = "reconciliation_heartbeat_takeover/database.rs"]
mod database;
#[path = "reconciliation_heartbeat_takeover/job.rs"]
mod job;
#[path = "reconciliation_heartbeat_takeover/runner.rs"]
mod runner;
#[path = "reconciliation_heartbeat_takeover/schema.rs"]
mod schema;
#[path = "reconciliation_heartbeat_takeover/source.rs"]
mod source;

use std::sync::Arc;

use rustok_index::{
    IndexReconciliationRunError, IndexReconciliationRunStatus,
};
use tokio::sync::Barrier;

use connection::TestResult;
use database::TestDatabase;
use job::{count, expire_attempt_one, read_job, shorten_attempt_one};
use runner::{request, runner};
use source::HeartbeatSource;

#[tokio::test]
async fn heartbeat_blocks_takeover_until_exact_lease_expiry() -> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };
    let tenant_id = database.tenant_id;
    let first_entered = Arc::new(Barrier::new(2));
    let first_release = Arc::new(Barrier::new(2));
    let second_entered = Arc::new(Barrier::new(2));
    let second_release = Arc::new(Barrier::new(2));

    let first_runner = runner(
        database.connection().await?,
        HeartbeatSource::Blocking {
            first_entered: first_entered.clone(),
            first_release: first_release.clone(),
            second_entered: second_entered.clone(),
            second_release: second_release.clone(),
        },
    );
    let first_task = tokio::spawn(async move {
        first_runner
            .run(request(tenant_id, "heartbeat-worker-a"))
            .await
    });

    first_entered.wait().await;
    let inspection = database.connection().await?;
    let initial = read_job(&inspection, tenant_id).await?;
    assert_eq!(initial.state, "running");
    assert_eq!(initial.attempt_count, 1);
    assert_eq!(initial.completed_passes, 0);
    assert_eq!(initial.pages_processed, 0);
    assert_eq!(initial.lease_owner.as_deref(), Some("heartbeat-worker-a"));
    assert!(!initial.lease_released);

    shorten_attempt_one(&inspection, tenant_id, initial.job_id).await?;
    let shortened = read_job(&inspection, tenant_id).await?;
    assert!(!shortened.lease_extended);

    first_release.wait().await;
    second_entered.wait().await;

    let after_heartbeat = read_job(&inspection, tenant_id).await?;
    assert_eq!(after_heartbeat.job_id, initial.job_id);
    assert_eq!(after_heartbeat.state, "running");
    assert_eq!(after_heartbeat.attempt_count, 1);
    assert_eq!(after_heartbeat.completed_passes, 0);
    assert_eq!(after_heartbeat.pages_processed, 1);
    assert_eq!(
        after_heartbeat.lease_owner.as_deref(),
        Some("heartbeat-worker-a")
    );
    assert!(after_heartbeat.lease_extended);

    let contender = runner(database.connection().await?, HeartbeatSource::Immediate)
        .run(request(tenant_id, "heartbeat-worker-b"))
        .await?;
    assert_eq!(contender.status(), IndexReconciliationRunStatus::Busy);
    assert_eq!(contender.job_id(), None);
    assert_eq!(contender.attempt_count(), None);
    assert_eq!(contender.pages_processed(), 0);

    expire_attempt_one(&inspection, tenant_id, initial.job_id).await?;
    let takeover = runner(database.connection().await?, HeartbeatSource::Immediate)
        .run(request(tenant_id, "heartbeat-worker-b"))
        .await?;

    second_release.wait().await;
    let stale_result = first_task.await?;

    assert_eq!(takeover.status(), IndexReconciliationRunStatus::Complete);
    assert_eq!(takeover.job_id(), Some(initial.job_id));
    assert_eq!(takeover.attempt_count(), Some(2));
    assert_eq!(takeover.pages_processed(), 1);
    assert_eq!(takeover.passes_completed(), 1);
    assert_eq!(takeover.applied_count(), 1);
    assert_eq!(takeover.duplicate_count(), 0);

    match stale_result {
        Err(IndexReconciliationRunError::LeaseLost {
            job_id,
            attempt_count,
        }) => {
            assert_eq!(job_id, initial.job_id);
            assert_eq!(attempt_count, 1);
        }
        Ok(outcome) => panic!("stale heartbeat worker unexpectedly completed: {outcome:?}"),
        Err(error) => panic!("stale heartbeat worker returned the wrong error: {error:?}"),
    }

    let final_job = read_job(&inspection, tenant_id).await?;
    assert_eq!(final_job.job_id, initial.job_id);
    assert_eq!(final_job.state, "succeeded");
    assert_eq!(final_job.attempt_count, 2);
    assert_eq!(final_job.completed_passes, 1);
    assert_eq!(final_job.pages_processed, 2);
    assert_eq!(final_job.lease_owner, None);
    assert!(final_job.lease_released);
    assert_eq!(count(&inspection, "index_jobs").await?, 1);
    assert_eq!(count(&inspection, "index_entities").await?, 2);
    assert_eq!(count(&inspection, "index_inbox").await?, 2);

    database.cleanup().await
}
