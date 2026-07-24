use anyhow::{Context, Result};
use sea_orm::{ConnectOptions, ConnectionTrait, Database, DatabaseConnection};

pub async fn connect(database_url: &str) -> Result<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .min_connections(1)
        .max_connections(1)
        .sqlx_logging(false);

    let db = Database::connect(options)
        .await
        .context("failed to connect to PostgreSQL with a single benchmark session")?;
    db.execute_unprepared("SET standard_conforming_strings = on;")
        .await
        .context("failed to pin PostgreSQL standard-conforming string semantics")?;
    Ok(db)
}
