use anyhow::{Context, Result};
use sea_orm::{ConnectOptions, Database, DatabaseConnection};

pub async fn connect(database_url: &str) -> Result<DatabaseConnection> {
    let mut options = ConnectOptions::new(database_url.to_owned());
    options
        .min_connections(1)
        .max_connections(1)
        .sqlx_logging(false);

    Database::connect(options)
        .await
        .context("failed to connect to PostgreSQL with a single benchmark session")
}
