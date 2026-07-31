#[path = "reconciliation_failure_diagnostics/connection.rs"]
mod connection;
#[path = "reconciliation_failure_diagnostics/database.rs"]
mod database;
#[path = "reconciliation_failure_diagnostics/evidence.rs"]
mod evidence;
#[path = "reconciliation_failure_diagnostics/runner.rs"]
mod runner;
#[path = "reconciliation_failure_diagnostics/schema.rs"]
mod schema;
#[path = "reconciliation_failure_diagnostics/source.rs"]
mod source;

use rustok_index::{
    IndexReconciliationCancelOutcome, IndexReconciliationRunError,
    IndexReconciliationTerminalState, IndexSourceError, IndexSourceFailureKind,
};
use serde_json::json;

use connection::TestResult;
use database::TestDatabase;
use evidence::{count, read_failure};
use runner::{SOURCE_NAME, request, runner};
use source::{FailingSource, FailureMode};

const PAGE_FAILURE_CODE: &str = "index.reconciliation_page_failed";
const FAILURE_CONTRACT: &str = "index_reconciliation_run_failure_v1";

async fn assert_failure_case(
    case: &str,
    worker_id: &str,
    dependency_code: &'static str,
    mode: FailureMode,
    expected_retryable: bool,
) -> TestResult<()> {
    let Some(database) = TestDatabase::setup(case).await? else {
        return Ok(());
    };
    let tenant_id = database.tenant_id;
    let source = FailingSource::new(dependency_code, mode);
    assert_eq!(source.code(), dependency_code);
    let reconciliation = runner(database.connection().await?, source);

    let error = reconciliation
        .run(request(tenant_id, worker_id))
        .await
        .expect_err("fixture source failure must escape the run");
    match error {
        IndexReconciliationRunError::Source(IndexSourceError::SourceFailure {
            source_name,
            failure,
        }) => {
            assert_eq!(source_name, SOURCE_NAME);
            assert_eq!(failure.code(), dependency_code);
            assert_eq!(
                failure.kind(),
                if expected_retryable {
                    IndexSourceFailureKind::Retryable
                } else {
                    IndexSourceFailureKind::Permanent
                }
            );
        }
        other => panic!("unexpected reconciliation failure: {other:?}"),
    }

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
            "dependency_code": dependency_code,
            "retryable": expected_retryable,
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
async fn permanent_source_failure_terminalizes_with_bounded_diagnostics() -> TestResult<()> {
    assert_failure_case(
        "permanent",
        "failure-worker-permanent",
        "owner_source_permanent",
        FailureMode::Permanent,
        false,
    )
    .await
}

#[tokio::test]
async fn retryable_source_failure_terminalizes_with_retryable_diagnostic() -> TestResult<()> {
    assert_failure_case(
        "retryable",
        "failure-worker-retryable",
        "owner_source_retryable",
        FailureMode::Retryable,
        true,
    )
    .await
}
