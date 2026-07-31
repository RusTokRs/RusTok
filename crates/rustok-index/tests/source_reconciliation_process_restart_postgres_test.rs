#[path = "reconciliation_process_restart/connection.rs"]
mod connection;
#[path = "reconciliation_process_restart/database.rs"]
mod database;
#[path = "reconciliation_process_restart/parent.rs"]
mod parent;
#[path = "reconciliation_process_restart/process.rs"]
mod process;
#[path = "reconciliation_process_restart/query.rs"]
mod query;
#[path = "reconciliation_process_restart/runner.rs"]
mod runner;
#[path = "reconciliation_process_restart/schema.rs"]
mod schema;
#[path = "reconciliation_process_restart/source.rs"]
mod source;
#[path = "reconciliation_process_restart/worker.rs"]
mod worker;

use connection::TestResult;

#[tokio::test]
async fn reconciliation_yield_resumes_across_two_test_processes() -> TestResult<()> {
    parent::run().await
}

#[tokio::test]
async fn process_restart_worker_resumes_reconciliation_from_env() -> TestResult<()> {
    worker::run().await
}
