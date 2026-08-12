#[path = "support/drift_repair.rs"]
mod support;

use sea_orm::{ConnectionTrait, DbBackend, Statement};

use support::{TestDatabase, TestResult};

#[tokio::test]
async fn repair_evidence_environment_reports_postgres_version() -> TestResult<()> {
    let Some(database) = TestDatabase::setup("repair_environment").await? else {
        return Ok(());
    };
    let db = database.connection().await?;
    let row = db
        .query_one(Statement::from_string(
            DbBackend::Postgres,
            "SELECT current_setting('server_version') AS server_version, current_setting('server_version_num') AS server_version_num".to_owned(),
        ))
        .await?
        .ok_or_else(|| std::io::Error::other("PostgreSQL version metadata returned no row"))?;
    let server_version: String = row.try_get("", "server_version")?;
    let server_version_num: String = row.try_get("", "server_version_num")?;

    println!("RUSTOK_INDEX_REPAIR_EVIDENCE postgres_server_version={server_version}");
    println!("RUSTOK_INDEX_REPAIR_EVIDENCE postgres_server_version_num={server_version_num}");

    database.cleanup().await
}
