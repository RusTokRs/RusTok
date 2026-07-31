use std::error::Error;

use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};

pub type TestResult<T> = Result<T, Box<dyn Error + Send + Sync>>;
const DATABASE_ENV: &str = "RUSTOK_INDEX_TEST_DATABASE_URL";

pub fn database_url() -> Option<String> {
    std::env::var(DATABASE_ENV)
        .or_else(|_| std::env::var("DATABASE_URL"))
        .ok()
        .filter(|url| url.starts_with("postgres://") || url.starts_with("postgresql://"))
}

pub async fn connect(url: &str) -> TestResult<DatabaseConnection> {
    let mut options = ConnectOptions::new(url.to_owned());
    options
        .max_connections(1)
        .min_connections(1)
        .sqlx_logging(false);
    Ok(Database::connect(options).await?)
}

pub async fn scoped_connection(
    url: &str,
    schema_name: &str,
) -> TestResult<DatabaseConnection> {
    let db = connect(url).await?;
    db.execute_unprepared(&format!(r#"SET search_path TO "{schema_name}""#))
        .await?;
    Ok(db)
}

pub fn print_skip() {
    eprintln!(
        "{DATABASE_ENV} is not set to a PostgreSQL URL; skipping running reconciliation cancellation harness"
    );
}
