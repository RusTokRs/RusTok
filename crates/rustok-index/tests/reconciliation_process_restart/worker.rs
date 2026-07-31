use std::{env, io};

use rustok_index::IndexReconciliationRunStatus;
use uuid::Uuid;

use super::{
    connection::{
        PROCESS_DATABASE_ENV, PROCESS_PHASE_ENV, PROCESS_SCHEMA_ENV, PROCESS_TENANT_ENV,
        PROCESS_WORKER_ENV, TestResult, required_env, scoped_connection,
    },
    process::{COMPLETE_PHASE, YIELD_PHASE},
    runner::{request, runner},
};

pub async fn run() -> TestResult<()> {
    if env::var(PROCESS_WORKER_ENV).as_deref() != Ok("1") {
        return Ok(());
    }
    let database_url = required_env(PROCESS_DATABASE_ENV)?;
    let schema_name = required_env(PROCESS_SCHEMA_ENV)?;
    let tenant_id = Uuid::parse_str(&required_env(PROCESS_TENANT_ENV)?)?;
    let phase = required_env(PROCESS_PHASE_ENV)?;
    let db = scoped_connection(&database_url, &schema_name).await?;
    let runner = runner(db);
    let worker_id = match phase.as_str() {
        YIELD_PHASE => "process-restart-worker-a",
        COMPLETE_PHASE => "process-restart-worker-b",
        _ => {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                format!("unknown reconciliation process phase {phase}"),
            )
            .into());
        }
    };
    let outcome = runner.run(request(tenant_id, worker_id)).await?;
    match phase.as_str() {
        YIELD_PHASE => {
            assert_eq!(outcome.status(), IndexReconciliationRunStatus::Yielded);
            assert_eq!(outcome.attempt_count(), Some(1));
            assert_eq!(outcome.pages_processed(), 1);
            assert_eq!(outcome.passes_completed(), 0);
            assert_eq!(outcome.applied_count(), 1);
        }
        COMPLETE_PHASE => {
            assert_eq!(outcome.status(), IndexReconciliationRunStatus::Complete);
            assert_eq!(outcome.attempt_count(), Some(2));
            assert_eq!(outcome.pages_processed(), 1);
            assert_eq!(outcome.passes_completed(), 1);
            assert_eq!(outcome.applied_count(), 1);
        }
        _ => unreachable!("phase was validated before the run"),
    }
    assert!(outcome.job_id().is_some());
    Ok(())
}
