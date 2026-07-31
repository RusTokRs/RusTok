#[path = "reconciliation_running_cancel/cancel.rs"]
mod cancel;
#[path = "reconciliation_running_cancel/connection.rs"]
mod connection;
#[path = "reconciliation_running_cancel/control.rs"]
mod control;
#[path = "reconciliation_running_cancel/database.rs"]
mod database;
#[path = "reconciliation_running_cancel/job.rs"]
mod job;
#[path = "reconciliation_running_cancel/prepare.rs"]
mod prepare;
#[path = "reconciliation_running_cancel/recover.rs"]
mod recover;
#[path = "reconciliation_running_cancel/request.rs"]
mod request;
#[path = "reconciliation_running_cancel/scalar.rs"]
mod scalar;
#[path = "reconciliation_running_cancel/schema.rs"]
mod schema;
#[path = "reconciliation_running_cancel/source.rs"]
mod source;

use connection::TestResult;
use control::SourceControl;
use database::TestDatabase;

#[tokio::test]
async fn running_cancel_preserves_cursor_and_recovers_by_duplicate_redelivery() -> TestResult<()> {
    let Some(database) = TestDatabase::setup().await? else {
        return Ok(());
    };
    let control = SourceControl::new();
    let inspection = database.connection().await?;

    let cancelled_job_id = cancel::run(&database, &control, &inspection).await?;
    recover::run(&database, &control, &inspection, cancelled_job_id).await?;

    database.cleanup().await
}
