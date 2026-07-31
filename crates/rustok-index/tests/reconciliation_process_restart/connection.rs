use std::{env, error::Error};

use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};

pub type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

pub const DATABASE_ENV: &str = "RUSTOK_INDEX_TEST_DATABASE_URL";
pub const PROCESS_DATABASE_ENV: &str = "RUSTOK_INDEX_RECONCILIATION_PROCESS_DATABASE_URL";
pub const PROCESS_SCHEMA_ENV: &str = "RUSTOK_INDEX_RECONCILIATION_PROCESS_SCHEMA";
pub const PROCESS_TENANT_ENV: &str = "RUSTOK_INDEX_RECONCILIATION_PROCESS_TENANT_ID";
pub const PROCESS_PHASE_ENV: &str = "RUSTOK_INDEX_RECONCILIATION_PROCESS_PHASE";
pub const PROCESS_WORKER_ENV: &str = "RUSTOK_INDEX_RECONCILIATION_PROCESS_WORKER";

pub fn database_url() -> Option<String> {
    env::var(DATABASE_ENV)
        .or_else(|_| env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

pub async fn connect(database_url: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    Ok(Database::connect(options).await?)
}

pub async fn scoped_connection(
    database_url: &str,
    schema_name: &str,
) -> TestResult<DatabaseConnection> {
    let db = connect(database_url).await?;
    db.execute_unprepared(&format!(r#"SET search_path TO "{schema_name}""#))
        .await?;
    Ok(db)
}

pub fn required_env(name: &str) -> TestResult<String> {
    Ok(env::var(name).map_err(|_| format!("missing required process fixture variable {name}"))?)
}
