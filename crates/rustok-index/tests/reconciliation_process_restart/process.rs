use std::{env, io, process::Command};

use super::{
    connection::{
        PROCESS_DATABASE_ENV, PROCESS_PHASE_ENV, PROCESS_SCHEMA_ENV, PROCESS_TENANT_ENV,
        PROCESS_WORKER_ENV, TestResult,
    },
    database::TestDatabase,
};

pub const YIELD_PHASE: &str = "yield";
pub const COMPLETE_PHASE: &str = "complete";
const WORKER_TEST: &str = "process_restart_worker_resumes_reconciliation_from_env";

pub fn spawn_worker(fixture: &TestDatabase, phase: &str) -> TestResult<()> {
    let executable = env::current_exe()?;
    let status = Command::new(executable)
        .arg(WORKER_TEST)
        .arg("--exact")
        .arg("--nocapture")
        .env(PROCESS_WORKER_ENV, "1")
        .env(PROCESS_DATABASE_ENV, &fixture.database_url)
        .env(PROCESS_SCHEMA_ENV, &fixture.schema_name)
        .env(PROCESS_TENANT_ENV, fixture.tenant_id.to_string())
        .env(PROCESS_PHASE_ENV, phase)
        .status()?;
    if status.success() {
        return Ok(());
    }
    Err(io::Error::new(
        io::ErrorKind::Other,
        format!("reconciliation process restart worker {phase} exited with {status}"),
    )
    .into())
}
