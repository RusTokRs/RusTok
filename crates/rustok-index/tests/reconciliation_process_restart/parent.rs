use super::{
    connection::TestResult,
    database::TestDatabase,
    process::{COMPLETE_PHASE, YIELD_PHASE, spawn_worker},
    query::{count, job_evidence},
};

pub async fn run() -> TestResult<()> {
    let Some(fixture) = TestDatabase::setup().await? else {
        return Ok(());
    };

    spawn_worker(&fixture, YIELD_PHASE)?;
    let inspection = fixture.connection().await?;
    let yielded = job_evidence(&inspection).await?;
    assert_eq!(yielded.state, "pending");
    assert_eq!(yielded.attempt_count, 1);
    assert_eq!(yielded.completed_passes, 0);
    assert_eq!(yielded.pages_processed, 1);
    assert_eq!(count(&inspection, "index_entities").await?, 1);
    assert_eq!(count(&inspection, "index_inbox").await?, 1);
    assert_eq!(
        count(&inspection, "index_jobs WHERE kind = 'reconcile'").await?,
        1
    );

    spawn_worker(&fixture, COMPLETE_PHASE)?;
    let completed = job_evidence(&inspection).await?;
    assert_eq!(completed.job_id, yielded.job_id);
    assert_eq!(completed.state, "succeeded");
    assert_eq!(completed.attempt_count, 2);
    assert_eq!(completed.completed_passes, 1);
    assert_eq!(completed.pages_processed, 2);
    assert_eq!(count(&inspection, "index_entities").await?, 2);
    assert_eq!(count(&inspection, "index_inbox").await?, 2);
    assert_eq!(
        count(&inspection, "index_jobs WHERE kind = 'reconcile'").await?,
        1
    );

    drop(inspection);
    fixture.cleanup().await
}
