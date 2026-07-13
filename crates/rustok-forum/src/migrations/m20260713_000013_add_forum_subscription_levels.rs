mod down;
mod postgres_up;
mod sqlite_up;

use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => postgres_up::up(manager).await,
            DatabaseBackend::Sqlite => sqlite_up::up(manager).await,
            backend => Err(DbErr::Custom(format!(
                "Unsupported forum subscription migration backend: {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => down::postgres(manager).await,
            DatabaseBackend::Sqlite => down::sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "Unsupported forum subscription migration backend: {backend:?}"
            ))),
        }
    }
}
