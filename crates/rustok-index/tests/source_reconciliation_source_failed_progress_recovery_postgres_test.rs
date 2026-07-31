#[path = "reconciliation_source_failed_progress_recovery/connection.rs"]
mod connection;
#[path = "reconciliation_source_failed_progress_recovery/database.rs"]
mod database;
#[path = "reconciliation_source_failed_progress_recovery/evidence.rs"]
mod evidence;
#[path = "reconciliation_source_failed_progress_recovery/runner.rs"]
mod runner;
#[path = "reconciliation_source_failed_progress_recovery/schema.rs"]
mod schema;
#[path = "reconciliation_source_failed_progress_recovery/source.rs"]
mod source;

use rustok_index::{
    IndexReconciliationCancelOutcome, IndexReconciliationRunError,
    IndexReconciliationRunStatus, IndexReconciliationTerminalState, IndexSourceError,
    IndexSourceFailureKind,
};
use serde_json::json;

use connection::TestResult;
use database::TestDatabase;
use evidence::{count, read_job};
use runner::{SOURCE_NAME, request, runner};
use source::{FAILURE_CODE, SourceFailedProgressSource};

const PAGE_FAILURE_CODE: &str = "index.reconciliation_page_failed";
const FAILURE_CONTRACT: &str = "index_reconciliation_run_failure_v1";

#[tokio::test]
async fn retryable_source_failure_preserves_progress_and_recovers_by_duplicate_redelivery(
) -> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };
    let tenant_id = database.tenant_id;
    let failed_runner = runner(
        database.connection().await?,
        SourceFailedProgressSource::FailSecondPage,
    );

    let error = failed_runner
        .run(request(tenant_id, "source-failed-progress-worker-a"))
        .await
        .expect_err("second-page retryable source failure must escape reconciliation");
    match error {
        IndexReconciliationRunError::Source(IndexSourceError::SourceFailure {
            source_name,
            failure,
        }) => {
            assert_eq!(source_name, SOURCE_NAME);
            assert_eq!(failure.code(), FAILURE_CODE);
            assert_eq!(failure.kind(), IndexSourceFailureKind::Retryable);
        }
        other => panic!("unexpected reconciliation source failure: {other:?}"),
    }

    let inspection = database.connection().await?;
    let failed = read_job(&inspection, tenant_id, "failed").await?;
    assert_eq!(failed.state, "failed");
    assert_eq!(failed.attempt_count, 1);
    assert_eq!(failed.completed_passes, 0);
    assert_eq!(failed.pages_processed, 1);
    assert_eq!(failed.source_cursor, json!({ "offset": 1 }));
    assert_eq!(failed.last_error_code.as_deref(), Some(PAGE_FAILURE_CODE));
    assert_eq!(
        failed.last_error_details,
        Some(json!({
            "contract": FAILURE_CONTRACT,
            "dependency_code": FAILURE_CODE,
            "retryable": true,
        }))
    );
    assert_eq!(
        failed
            .last_error_details
            .as_ref()
            .and_then(serde_json::Value::as_object)
            .expect("failure details must be an object")
            .len(),
        3
    );
    assert!(failed.lease_released);
    assert!(failed.completed);
    assert_eq!(count(&inspection, "index_jobs").await?, 1);
    assert_eq!(count(&inspection, "index_entities").await?, 1);
    assert_eq!(count(&inspection, "index_inbox").await?, 1);

    assert_eq!(
        failed_runner
            .request_cancel(tenant_id, failed.job_id)
            .await?,
        IndexReconciliationCancelOutcome::AlreadyTerminal(
            IndexReconciliationTerminalState::Failed
        )
    );

    let recovery = runner(
        database.connection().await?,
        SourceFailedProgressSource::RecoverSecondPage,
    )
    .run(request(tenant_id, "source-failed-progress-worker-b"))
    .await?;
    let recovery_job_id = recovery.job_id().expect("recovery job id");
    assert_ne!(recovery_job_id, failed.job_id);
    assert_eq!(recovery.status(), IndexReconciliationRunStatus::Complete);
    assert_eq!(recovery.attempt_count(), Some(1));
    assert_eq!(recovery.pages_processed(), 2);
    assert_eq!(recovery.passes_completed(), 1);
    assert_eq!(recovery.heartbeat_count(), 1);
    assert_eq!(recovery.mutation_count(), 2);
    assert_eq!(recovery.applied_count(), 1);
    assert_eq!(recovery.duplicate_count(), 1);
    assert_eq!(recovery.stale_count(), 0);

    let succeeded = read_job(&inspection, tenant_id, "succeeded").await?;
    assert_eq!(succeeded.job_id, recovery_job_id);
    assert_eq!(succeeded.state, "succeeded");
    assert_eq!(succeeded.attempt_count, 1);
    assert_eq!(succeeded.completed_passes, 1);
    assert_eq!(succeeded.pages_processed, 2);
    assert_eq!(succeeded.source_cursor, json!(null));
    assert_eq!(succeeded.last_error_code, None);
    assert_eq!(succeeded.last_error_details, None);
    assert!(succeeded.lease_released);
    assert!(succeeded.completed);
    assert_eq!(count(&inspection, "index_jobs").await?, 2);
    assert_eq!(count(&inspection, "index_entities").await?, 2);
    assert_eq!(count(&inspection, "index_inbox").await?, 2);

    let already_complete = runner(
        database.connection().await?,
        SourceFailedProgressSource::RecoverSecondPage,
    )
    .run(request(tenant_id, "source-failed-progress-worker-c"))
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
