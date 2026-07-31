#[path = "reconciliation_event_id_contract/connection.rs"]
mod connection;
#[path = "reconciliation_event_id_contract/database.rs"]
mod database;
#[path = "reconciliation_event_id_contract/evidence.rs"]
mod evidence;
#[path = "reconciliation_event_id_contract/runner.rs"]
mod runner;
#[path = "reconciliation_event_id_contract/schema.rs"]
mod schema;
#[path = "reconciliation_event_id_contract/source.rs"]
mod source;

use rustok_index::{
    IndexReconciliationCancelOutcome, IndexReconciliationRunError,
    IndexReconciliationTerminalState,
};
use serde_json::json;
use uuid::Uuid;

use connection::TestResult;
use database::TestDatabase;
use evidence::{count, read_failure};
use runner::{request, runner};
use source::{DUPLICATE_EVENT_ID, EventIdContractSource};

const PAGE_FAILURE_CODE: &str = "index.reconciliation_page_failed";
const FAILURE_CONTRACT: &str = "index_reconciliation_run_failure_v1";
const DEPENDENCY_CODE: &str = "reconciliation_contract_invalid";

#[derive(Debug, Clone, Copy)]
enum ExpectedFailure {
    NilSecond,
    DuplicateSecond,
}

#[tokio::test]
async fn nil_second_event_id_rejects_whole_page_before_mutation_persistence() -> TestResult<()> {
    assert_contract_failure("nil", EventIdContractSource::NilSecond, ExpectedFailure::NilSecond).await
}

#[tokio::test]
async fn duplicate_second_event_id_rejects_whole_page_before_mutation_persistence() -> TestResult<()> {
    assert_contract_failure(
        "duplicate",
        EventIdContractSource::DuplicateSecond,
        ExpectedFailure::DuplicateSecond,
    )
    .await
}

async fn assert_contract_failure(
    case_name: &str,
    source: EventIdContractSource,
    expected: ExpectedFailure,
) -> TestResult<()> {
    let Some(database) = TestDatabase::setup(case_name).await? else {
        return Ok(());
    };
    let tenant_id = database.tenant_id;
    let reconciliation = runner(database.connection().await?, source);

    let error = reconciliation
        .run(request(tenant_id, &format!("event-id-{case_name}-worker")))
        .await
        .expect_err("invalid page event identity must fail reconciliation");
    match (expected, error) {
        (ExpectedFailure::NilSecond, IndexReconciliationRunError::NilEventId { position }) => {
            assert_eq!(position, 1);
        }
        (
            ExpectedFailure::DuplicateSecond,
            IndexReconciliationRunError::DuplicateEventId { position, event_id },
        ) => {
            assert_eq!(position, 1);
            assert_eq!(event_id, DUPLICATE_EVENT_ID);
        }
        (_, other) => panic!("unexpected event-id contract error: {other:?}"),
    }

    let inspection = database.connection().await?;
    let failure = read_failure(&inspection, tenant_id).await?;
    assert!(!failure.job_id.is_nil());
    assert_eq!(failure.state, "failed");
    assert_eq!(failure.attempt_count, 1);
    assert_eq!(failure.completed_passes, 0);
    assert_eq!(failure.pages_processed, 0);
    assert_eq!(failure.last_error_code, PAGE_FAILURE_CODE);
    assert_eq!(
        failure.last_error_details,
        json!({
            "contract": FAILURE_CONTRACT,
            "dependency_code": DEPENDENCY_CODE,
            "retryable": false,
        })
    );
    assert_eq!(
        failure
            .last_error_details
            .as_object()
            .map(serde_json::Map::len),
        Some(3)
    );
    assert!(failure.lease_released);
    assert!(failure.completed);
    assert_eq!(count(&inspection, "index_jobs").await?, 1);
    assert_eq!(count(&inspection, "index_entities").await?, 0);
    assert_eq!(count(&inspection, "index_inbox").await?, 0);

    assert_eq!(
        reconciliation
            .request_cancel(Uuid::new_v4(), failure.job_id)
            .await?,
        IndexReconciliationCancelOutcome::NotFound
    );
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
