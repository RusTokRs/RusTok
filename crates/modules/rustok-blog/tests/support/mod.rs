//! Migration-backed fixtures shared by the configured PostgreSQL projection tests.
//!
//! These scenarios apply complete owner schemas and actual prerequisites.

mod schema;

use std::{error::Error, io};

use rustok_blog::BlogModule;
use rustok_core::MigrationSource;
use sea_orm::{DatabaseConnection, DbErr};

pub fn postgres_database_url() -> Result<String, Box<dyn Error + Send + Sync>> {
    let name = "RUSTOK_BLOG_TEST_DATABASE_URL";
    let url = std::env::var(name).map_err(|_| {
        io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("set {name} explicitly when selecting ignored PostgreSQL tests"),
        )
    })?;
    // Do not fall back to DATABASE_URL: an operator's runtime database must not
    // become a test target merely because the explicit fixture URL is missing.
    if !url.starts_with("postgres://") && !url.starts_with("postgresql://") {
        return Err(io::Error::new(
            io::ErrorKind::InvalidInput,
            format!("{name} must identify a PostgreSQL test database"),
        )
        .into());
    }
    Ok(url)
}

pub async fn apply_blog_schema(db: &DatabaseConnection) -> Result<(), DbErr> {
    schema::apply_blog_schema(db, BlogModule.migrations()).await
}
