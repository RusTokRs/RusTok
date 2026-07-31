#[path = "reconciliation_schema_admission/connection.rs"]
mod connection;
#[path = "reconciliation_schema_admission/database.rs"]
mod database;
#[path = "reconciliation_schema_admission/evidence.rs"]
mod evidence;
#[path = "reconciliation_schema_admission/runner.rs"]
mod runner;
#[path = "reconciliation_schema_admission/schema.rs"]
mod schema;
#[path = "reconciliation_schema_admission/source.rs"]
mod source;

use rustok_index::{IndexReconciliationRunError, IndexReconciliationRunStatus};
use serde_json::json;

use connection::TestResult;
use database::TestDatabase;
use evidence::{count, read_job};
use runner::{request, runner};
use schema::{persist_schema, schema_ref, set_schema_status};
use source::SchemaAdmissionSource;

#[tokio::test]
async fn schema_admission_blocks_jobs_and_preserves_pending_resume_identity() -> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };
    let tenant_id = database.tenant_id;
    let source = SchemaAdmissionSource::default();

    let missing_error = runner(database.connection().await?, source.clone())
        .run(request(tenant_id, "schema-missing-worker", 1))
        .await
        .expect_err("unregistered schema must fail before job acquisition");
    match missing_error {
        IndexReconciliationRunError::SchemaNotRegistered(reference) => {
            assert_eq!(reference, schema_ref());
        }
        other => panic!("unexpected unregistered-schema error: {other:?}"),
    }

    let inspection = database.connection().await?;
    assert_eq!(source.scan_count(), 0);
    assert_eq!(count(&inspection, "index_jobs").await?, 0);
    assert_eq!(count(&inspection, "index_entities").await?, 0);
    assert_eq!(count(&inspection, "index_inbox").await?, 0);

    persist_schema(&inspection, tenant_id).await?;
    let yielded = runner(database.connection().await?, source.clone())
        .run(request(tenant_id, "schema-active-worker-a", 1))
        .await?;
    let job_id = yielded.job_id().expect("yielded job id");
    assert_eq!(yielded.status(), IndexReconciliationRunStatus::Yielded);
    assert_eq!(yielded.attempt_count(), Some(1));
    assert_eq!(yielded.pages_processed(), 1);
    assert_eq!(yielded.passes_completed(), 0);
    assert_eq!(yielded.heartbeat_count(), 0);
    assert_eq!(yielded.mutation_count(), 1);
    assert_eq!(yielded.applied_count(), 1);
    assert_eq!(yielded.duplicate_count(), 0);
    assert_eq!(yielded.stale_count(), 0);
    assert_eq!(source.scan_count(), 1);

    let pending = read_job(&inspection, tenant_id).await?;
    assert_eq!(pending.job_id, job_id);
    assert_eq!(pending.state, "pending");
    assert_eq!(pending.attempt_count, 1);
    assert_eq!(pending.completed_passes, 0);
    assert_eq!(pending.pages_processed, 1);
    assert_eq!(pending.source_cursor, json!({ "offset": 1 }));
    assert!(pending.lease_released);
    assert!(!pending.completed);
    assert!(!pending.cancel_requested);
    assert_eq!(pending.last_error_code, None);
    assert_eq!(pending.last_error_details, None);
    assert_eq!(count(&inspection, "index_jobs").await?, 1);
    assert_eq!(count(&inspection, "index_entities").await?, 1);
    assert_eq!(count(&inspection, "index_inbox").await?, 1);

    set_schema_status(&inspection, tenant_id, "retired").await?;
    let retired_error = runner(database.connection().await?, source.clone())
        .run(request(tenant_id, "schema-retired-worker", 2))
        .await
        .expect_err("retired schema must block pending job claim");
    match retired_error {
        IndexReconciliationRunError::SchemaRetired(reference) => {
            assert_eq!(reference, schema_ref());
        }
        other => panic!("unexpected retired-schema error: {other:?}"),
    }

    assert_eq!(source.scan_count(), 1);
    let still_pending = read_job(&inspection, tenant_id).await?;
    assert_eq!(still_pending.job_id, job_id);
    assert_eq!(still_pending.state, "pending");
    assert_eq!(still_pending.attempt_count, 1);
    assert_eq!(still_pending.completed_passes, 0);
    assert_eq!(still_pending.pages_processed, 1);
    assert_eq!(still_pending.source_cursor, json!({ "offset": 1 }));
    assert!(still_pending.lease_released);
    assert!(!still_pending.completed);
    assert_eq!(count(&inspection, "index_jobs").await?, 1);
    assert_eq!(count(&inspection, "index_entities").await?, 1);
    assert_eq!(count(&inspection, "index_inbox").await?, 1);

    set_schema_status(&inspection, tenant_id, "active").await?;
    let resumed = runner(database.connection().await?, source.clone())
        .run(request(tenant_id, "schema-active-worker-b", 2))
        .await?;
    assert_eq!(resumed.status(), IndexReconciliationRunStatus::Complete);
    assert_eq!(resumed.job_id(), Some(job_id));
    assert_eq!(resumed.attempt_count(), Some(2));
    assert_eq!(resumed.pages_processed(), 1);
    assert_eq!(resumed.passes_completed(), 1);
    assert_eq!(resumed.heartbeat_count(), 0);
    assert_eq!(resumed.mutation_count(), 1);
    assert_eq!(resumed.applied_count(), 1);
    assert_eq!(resumed.duplicate_count(), 0);
    assert_eq!(resumed.stale_count(), 0);
    assert_eq!(source.scan_count(), 2);

    let succeeded = read_job(&inspection, tenant_id).await?;
    assert_eq!(succeeded.job_id, job_id);
    assert_eq!(succeeded.state, "succeeded");
    assert_eq!(succeeded.attempt_count, 2);
    assert_eq!(succeeded.completed_passes, 1);
    assert_eq!(succeeded.pages_processed, 2);
    assert_eq!(succeeded.source_cursor, json!(null));
    assert!(succeeded.lease_released);
    assert!(succeeded.completed);
    assert!(!succeeded.cancel_requested);
    assert_eq!(succeeded.last_error_code, None);
    assert_eq!(succeeded.last_error_details, None);
    assert_eq!(count(&inspection, "index_jobs").await?, 1);
    assert_eq!(count(&inspection, "index_entities").await?, 2);
    assert_eq!(count(&inspection, "index_inbox").await?, 2);

    set_schema_status(&inspection, tenant_id, "retired").await?;
    let retired_terminal_error = runner(database.connection().await?, source.clone())
        .run(request(tenant_id, "schema-retired-terminal-worker", 2))
        .await
        .expect_err("retired schema must block terminal completion lookup");
    match retired_terminal_error {
        IndexReconciliationRunError::SchemaRetired(reference) => {
            assert_eq!(reference, schema_ref());
        }
        other => panic!("unexpected retired terminal-schema error: {other:?}"),
    }
    assert_eq!(source.scan_count(), 2);
    assert_eq!(count(&inspection, "index_jobs").await?, 1);
    assert_eq!(count(&inspection, "index_entities").await?, 2);
    assert_eq!(count(&inspection, "index_inbox").await?, 2);

    set_schema_status(&inspection, tenant_id, "active").await?;
    let already_complete = runner(database.connection().await?, source.clone())
        .run(request(tenant_id, "schema-active-worker-c", 2))
        .await?;
    assert_eq!(
        already_complete.status(),
        IndexReconciliationRunStatus::AlreadyComplete
    );
    assert_eq!(already_complete.job_id(), Some(job_id));
    assert_eq!(already_complete.attempt_count(), None);
    assert_eq!(already_complete.pages_processed(), 0);
    assert_eq!(already_complete.passes_completed(), 1);
    assert_eq!(source.scan_count(), 2);

    database.cleanup().await
}
