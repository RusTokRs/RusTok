#[path = "reconciliation_mutation_failure_diagnostics/connection.rs"]
mod connection;
#[path = "reconciliation_mutation_failure_diagnostics/database.rs"]
mod database;
#[path = "reconciliation_mutation_failure_diagnostics/evidence.rs"]
mod evidence;
#[path = "reconciliation_mutation_failure_diagnostics/runner.rs"]
mod runner;
#[path = "reconciliation_mutation_failure_diagnostics/schema.rs"]
mod schema;
#[path = "reconciliation_mutation_failure_diagnostics/source.rs"]
mod source;

use std::sync::Arc;

use rustok_index::{
    IndexReconciliationCancelOutcome, IndexReconciliationRunError,
    IndexReconciliationTerminalState, IndexReplayFailureKind,
};
use serde_json::json;
use tokio::sync::Barrier;

use connection::TestResult;
use database::TestDatabase;
use evidence::{count, read_failure, read_running};
use runner::{request, runner};
use source::MutationFailureSource;

const PAGE_FAILURE_CODE: &str = "index.reconciliation_page_failed";
const FAILURE_CONTRACT: &str = "index_reconciliation_run_failure_v1";
const PERMANENT_CODE: &str = "mutation_rejected";
const RETRYABLE_CODE: &str = "mutation_storage_retryable";

fn assert_mutation_failure(
    error: IndexReconciliationRunError,
    expected_code: &str,
    expected_kind: IndexReplayFailureKind,
) {
    match error {
        IndexReconciliationRunError::MutationFailed { position, failure } => {
            assert_eq!(position, 0);
            assert_eq!(failure.code(), expected_code);
            assert_eq!(failure.kind(), expected_kind);
        }
        other => panic!("unexpected reconciliation mutation failure: {other:?}"),
    }
}

#[tokio::test]
async fn invalid_mutation_terminalizes_with_permanent_bounded_diagnostic() -> TestResult<()> {
    let Some(database) = TestDatabase::setup("permanent").await? else {
        return Ok(());
    };
    let tenant_id = database.tenant_id;
    let reconciliation = runner(
        database.connection().await?,
        MutationFailureSource::InvalidRecord,
    );

    let error = reconciliation
        .run(request(tenant_id, "mutation-failure-worker-permanent"))
        .await
        .expect_err("schema-invalid mutation must fail reconciliation");
    assert_mutation_failure(error, PERMANENT_CODE, IndexReplayFailureKind::Permanent);

    let evidence_db = database.connection().await?;
    let failure = read_failure(&evidence_db, tenant_id).await?;
    assert_eq!(failure.state, "failed");
    assert_eq!(failure.attempt_count, 1);
    assert_eq!(failure.completed_passes, 0);
    assert_eq!(failure.pages_processed, 0);
    assert_eq!(failure.last_error_code, PAGE_FAILURE_CODE);
    assert_eq!(
        failure.last_error_details,
        json!({
            "contract": FAILURE_CONTRACT,
            "dependency_code": PERMANENT_CODE,
            "retryable": false,
        })
    );
    assert_eq!(
        failure
            .last_error_details
            .as_object()
            .expect("failure details must be an object")
            .len(),
        3
    );
    assert!(failure.lease_released);
    assert!(failure.completed);
    assert_eq!(count(&evidence_db, "index_jobs").await?, 1);
    assert_eq!(count(&evidence_db, "index_entities").await?, 0);
    assert_eq!(count(&evidence_db, "index_inbox").await?, 0);

    assert_eq!(
        reconciliation
            .request_cancel(tenant_id, failure.job_id)
            .await?,
        IndexReconciliationCancelOutcome::AlreadyTerminal(
            IndexReconciliationTerminalState::Failed
        )
    );

    database.cleanup().await
}

#[tokio::test]
async fn storage_failure_terminalizes_with_retryable_bounded_diagnostic() -> TestResult<()> {
    let Some(database) = TestDatabase::setup("retryable").await? else {
        return Ok(());
    };
    let tenant_id = database.tenant_id;
    let entered = Arc::new(Barrier::new(2));
    let release = Arc::new(Barrier::new(2));
    let active_runner = runner(
        database.connection().await?,
        MutationFailureSource::BlockingValid {
            entered: entered.clone(),
            release: release.clone(),
        },
    );
    let active_task = tokio::spawn(async move {
        active_runner
            .run(request(tenant_id, "mutation-failure-worker-retryable"))
            .await
    });

    entered.wait().await;
    let inspection = database.connection().await?;
    let running = read_running(&inspection, tenant_id).await?;
    assert_eq!(running.state, "running");
    assert_eq!(running.attempt_count, 1);
    assert_eq!(running.completed_passes, 0);
    assert_eq!(running.pages_processed, 0);
    assert_eq!(
        running.lease_owner.as_deref(),
        Some("mutation-failure-worker-retryable")
    );
    assert!(running.lease_active);

    database.hide_entities_table().await?;
    release.wait().await;
    let result = active_task.await?;
    database.restore_entities_table().await?;
    let error = result.expect_err("temporarily unavailable entity storage must fail reconciliation");
    assert_mutation_failure(error, RETRYABLE_CODE, IndexReplayFailureKind::Retryable);

    let failure = read_failure(&inspection, tenant_id).await?;
    assert_eq!(failure.job_id, running.job_id);
    assert_eq!(failure.state, "failed");
    assert_eq!(failure.attempt_count, 1);
    assert_eq!(failure.completed_passes, 0);
    assert_eq!(failure.pages_processed, 0);
    assert_eq!(failure.last_error_code, PAGE_FAILURE_CODE);
    assert_eq!(
        failure.last_error_details,
        json!({
            "contract": FAILURE_CONTRACT,
            "dependency_code": RETRYABLE_CODE,
            "retryable": true,
        })
    );
    assert_eq!(
        failure
            .last_error_details
            .as_object()
            .expect("failure details must be an object")
            .len(),
        3
    );
    assert!(failure.lease_released);
    assert!(failure.completed);
    assert_eq!(count(&inspection, "index_jobs").await?, 1);
    assert_eq!(count(&inspection, "index_entities").await?, 0);
    assert_eq!(count(&inspection, "index_inbox").await?, 0);

    let canceller = runner(
        database.connection().await?,
        MutationFailureSource::InvalidRecord,
    );
    assert_eq!(
        canceller.request_cancel(tenant_id, failure.job_id).await?,
        IndexReconciliationCancelOutcome::AlreadyTerminal(
            IndexReconciliationTerminalState::Failed
        )
    );

    database.cleanup().await
}
