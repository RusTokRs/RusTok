#[path = "reconciliation_lease_fencing/connection.rs"]
mod connection;
#[path = "reconciliation_lease_fencing/database.rs"]
mod database;
#[path = "reconciliation_lease_fencing/job.rs"]
mod job;
#[path = "reconciliation_lease_fencing/runner.rs"]
mod runner;
#[path = "reconciliation_lease_fencing/schema.rs"]
mod schema;
#[path = "reconciliation_lease_fencing/source.rs"]
mod source;

use std::sync::Arc;

use rustok_index::{IndexReconciliationRunError, IndexReconciliationRunStatus};
use tokio::sync::Barrier;

use connection::TestResult;
use database::TestDatabase;
use job::{count, expire_attempt_one, read_job};
use runner::{request, runner};
use source::LeaseSource;

#[tokio::test]
async fn expired_lease_takeover_fences_stale_reconciliation_worker() -> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };
    let tenant_id = database.tenant_id;
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));

    let first_runner = runner(
        database.connection().await?,
        LeaseSource::Blocking {
            entered: entered.clone(),
            release: release.clone(),
        },
    );
    let first_task = tokio::spawn(async move {
        first_runner
            .run(request(tenant_id, "lease-worker-a"))
            .await
    });

    entered.wait().await;
    let inspection = database.connection().await?;
    let initial = read_job(&inspection, tenant_id).await?;
    assert_eq!(initial.state, "running");
    assert_eq!(initial.attempt_count, 1);
    assert_eq!(initial.completed_passes, 0);
    assert_eq!(initial.pages_processed, 0);
    assert!(!initial.lease_released);

    expire_attempt_one(&inspection, tenant_id, initial.job_id).await?;

    let second_runner = runner(database.connection().await?, LeaseSource::Immediate);
    let takeover = second_runner
        .run(request(tenant_id, "lease-worker-b"))
        .await;

    release.wait().await;
    let stale_result = first_task.await?;
    let takeover = takeover?;

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
        Ok(outcome) => panic!("stale reconciliation worker unexpectedly completed: {outcome:?}"),
        Err(error) => panic!("stale reconciliation worker returned the wrong error: {error:?}"),
    }

    let final_job = read_job(&inspection, tenant_id).await?;
    assert_eq!(final_job.job_id, initial.job_id);
    assert_eq!(final_job.state, "succeeded");
    assert_eq!(final_job.attempt_count, 2);
    assert_eq!(final_job.completed_passes, 1);
    assert_eq!(final_job.pages_processed, 1);
    assert!(final_job.lease_released);
    assert_eq!(count(&inspection, "index_jobs").await?, 1);
    assert_eq!(count(&inspection, "index_entities").await?, 1);
    assert_eq!(count(&inspection, "index_inbox").await?, 1);

    database.cleanup().await
}
