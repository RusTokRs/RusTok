use std::sync::atomic::Ordering;

use rustok_index::IndexReconciliationRunStatus;
use sea_orm::DatabaseConnection;
use uuid::Uuid;

use super::{
    connection::TestResult,
    control::SourceControl,
    database::TestDatabase,
    request::request,
    scalar::i64_value,
    schema::runner,
};

pub async fn run(
    database: &TestDatabase,
    control: &SourceControl,
    inspection: &DatabaseConnection,
    cancelled_job_id: Uuid,
) -> TestResult<()> {
    let recovery_runner = runner(database.connection().await?, control.source());
    let recovered = recovery_runner
        .run(request(database.tenant_id, "running-cancel-worker-b"))
        .await?;
    assert_eq!(recovered.status(), IndexReconciliationRunStatus::Complete);
    assert_ne!(recovered.job_id(), Some(cancelled_job_id));
    assert_eq!(recovered.attempt_count(), Some(1));
    assert_eq!(recovered.pages_processed(), 1);
    assert_eq!(recovered.passes_completed(), 1);
    assert_eq!(recovered.mutation_count(), 1);
    assert_eq!(recovered.applied_count(), 0);
    assert_eq!(recovered.duplicate_count(), 1);
    assert_eq!(control.calls.load(Ordering::SeqCst), 2);

    assert_eq!(
        i64_value(
            inspection,
            "SELECT COUNT(*)::bigint AS value FROM index_jobs WHERE kind = 'reconcile'",
        )
        .await?,
        2
    );
    assert_eq!(
        i64_value(
            inspection,
            "SELECT COUNT(*)::bigint AS value FROM index_jobs WHERE kind = 'reconcile' AND state = 'cancelled'",
        )
        .await?,
        1
    );
    assert_eq!(
        i64_value(
            inspection,
            "SELECT COUNT(*)::bigint AS value FROM index_jobs WHERE kind = 'reconcile' AND state = 'succeeded'",
        )
        .await?,
        1
    );
    assert_eq!(
        i64_value(inspection, "SELECT COUNT(*)::bigint AS value FROM index_entities").await?,
        1
    );
    assert_eq!(
        i64_value(inspection, "SELECT COUNT(*)::bigint AS value FROM index_inbox").await?,
        1
    );
    Ok(())
}
