use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

mod postgres_down;
mod postgres_up;
mod sqlite_down;
mod sqlite_up;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => postgres_up::up_postgres(manager).await,
            DatabaseBackend::Sqlite => sqlite_up::up_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum domain event migration does not support {backend:?}"
            ))),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => postgres_down::down_postgres(manager).await,
            DatabaseBackend::Sqlite => sqlite_down::down_sqlite(manager).await,
            backend => Err(DbErr::Custom(format!(
                "rustok-forum domain event migration does not support {backend:?}"
            ))),
        }
    }
}
