use rustok_index::{IndexReconciliationCancelOutcome, IndexReconciliationRunStatus};
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use super::{
    connection::TestResult,
    control::SourceControl,
    database::TestDatabase,
    job::running_job_id,
    request::request,
    scalar::{bool_value, i64_value, string_value},
    schema::runner,
};

pub async fn run(
    database: &TestDatabase,
    control: &SourceControl,
    inspection: &DatabaseConnection,
) -> TestResult<Uuid> {
    let active_runner = runner(database.connection().await?, control.source());
    let active_request = request(database.tenant_id, "running-cancel-worker-a");
    let task = tokio::spawn(async move { active_runner.run(active_request).await });

    control.entered.wait().await;
    let job_id = running_job_id(inspection).await?;
    assert_eq!(
        string_value(
            inspection,
            "SELECT state AS value FROM index_jobs WHERE kind = 'reconcile' AND state = 'running'",
        )
        .await?,
        "running"
    );

    let canceller = runner(database.connection().await?, control.source());
    assert_eq!(
        canceller.request_cancel(Uuid::new_v4(), job_id).await?,
        IndexReconciliationCancelOutcome::NotFound
    );
    assert_eq!(
        canceller
            .request_cancel(database.tenant_id, job_id)
            .await?,
        IndexReconciliationCancelOutcome::Requested
    );
    assert!(
        bool_value(
            inspection,
            "SELECT cancel_requested AS value FROM index_jobs WHERE kind = 'reconcile' AND state = 'running'",
        )
        .await?
    );

    control.release.wait().await;
    let outcome = task.await??;
    assert_eq!(outcome.status(), IndexReconciliationRunStatus::Cancelled);
    assert_eq!(outcome.job_id(), Some(job_id));
    assert_eq!(outcome.attempt_count(), Some(1));
    assert_eq!(outcome.pages_processed(), 1);
    assert_eq!(outcome.passes_completed(), 1);
    assert_eq!(outcome.applied_count(), 1);

    assert_eq!(
        i64_value(
            inspection,
            "SELECT (cursor->>'completed_passes')::bigint AS value FROM index_jobs WHERE kind = 'reconcile' AND state = 'cancelled'",
        )
        .await?,
        0
    );
    assert_eq!(
        i64_value(
            inspection,
            "SELECT (cursor->>'pages_processed')::bigint AS value FROM index_jobs WHERE kind = 'reconcile' AND state = 'cancelled'",
        )
        .await?,
        0
    );
    assert_eq!(
        i64_value(inspection, "SELECT COUNT(*)::bigint AS value FROM index_entities").await?,
        1
    );
    assert_eq!(
        i64_value(inspection, "SELECT COUNT(*)::bigint AS value FROM index_inbox").await?,
        1
    );
    Ok(job_id)
}
