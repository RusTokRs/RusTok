#![cfg(feature = "migrations")]

use std::error::Error;
use std::io::{Error as IoError, ErrorKind};

use sea_orm::{
    ConnectOptions, ConnectionTrait, Database, DatabaseConnection, DbBackend, Statement,
};

const TEST_DATABASE_ENV: &str = "RUSTOK_IGGY_CONNECTOR_TEST_DATABASE_URL";

type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;

#[tokio::test]
async fn reports_postgres_server_version_for_retained_evidence() -> TestResult<()> {
    let Some(database_url) = postgres_database_url() else {
        eprintln!(
            "{TEST_DATABASE_ENV} is not set to a PostgreSQL URL; skipping retained evidence metadata"
        );
        return Ok(());
    };

    let db = connect(&database_url).await?;
    let row = db
        .query_one_raw(Statement::from_string(
            DbBackend::Postgres,
            "SELECT current_setting('server_version') AS server_version, current_setting('server_version_num') AS server_version_num".to_string(),
        ))
        .await?
        .expect("PostgreSQL version query must return one row");
    let server_version: String = row.try_get("", "server_version")?;
    let server_version_num: String = row.try_get("", "server_version_num")?;
    let server_version = bounded_metadata("server_version", &server_version)?;
    let server_version_num = bounded_metadata("server_version_num", &server_version_num)?;

    println!("RUSTOK_IGGY_POISON_EVIDENCE postgres_server_version={server_version}");
    println!("RUSTOK_IGGY_POISON_EVIDENCE postgres_server_version_num={server_version_num}");
    Ok(())
}

fn postgres_database_url() -> Option<String> {
    std::env::var(TEST_DATABASE_ENV)
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

async fn connect(database_url: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    Ok(Database::connect(options).await?)
}

fn bounded_metadata<'a>(field: &'static str, value: &'a str) -> Result<&'a str, IoError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(IoError::new(
            ErrorKind::InvalidData,
            format!("{field} must not be empty"),
        ));
    }
    if value.len() > 128 {
        return Err(IoError::new(
            ErrorKind::InvalidData,
            format!("{field} exceeds retained evidence limit"),
        ));
    }
    if value.chars().any(char::is_control) {
        return Err(IoError::new(
            ErrorKind::InvalidData,
            format!("{field} contains control characters"),
        ));
    }
    Ok(value)
}
