//! Ordered schema assembly for the single canonical Blog migration.

use super::*;
use sea_orm::DatabaseBackend;

mod postgres;
mod sqlite;
mod tables;

pub(super) async fn up(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    tables::up(manager).await?;
    match manager.get_database_backend() {
        DatabaseBackend::Postgres => postgres::up(manager).await?,
        DatabaseBackend::Sqlite => sqlite::up(manager).await?,
        backend => {
            return Err(DbErr::Custom(format!(
                "Blog schema migration does not support {backend:?}"
            )));
        }
    }
    Ok(())
}

pub(super) async fn down(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    tables::down(manager).await
}
