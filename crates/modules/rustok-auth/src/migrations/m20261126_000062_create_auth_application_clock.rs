use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let sql = match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                "CREATE TABLE auth_application_clock (id INTEGER PRIMARY KEY CHECK (id = 1), host_ms BIGINT NOT NULL CHECK (host_ms >= 0), database_ms BIGINT NOT NULL CHECK (database_ms >= 0))"
            }
            DatabaseBackend::Sqlite => {
                "CREATE TABLE auth_application_clock (id INTEGER PRIMARY KEY CHECK (id = 1), host_ms BIGINT NOT NULL CHECK (typeof(host_ms) = 'integer' AND host_ms >= 0), database_ms BIGINT NOT NULL CHECK (typeof(database_ms) = 'integer' AND database_ms >= 0))"
            }
            _ => {
                return Err(DbErr::Custom(
                    "unsupported Auth application clock backend".into(),
                ));
            }
        };
        db.execute_unprepared(sql).await?;
        db.execute_unprepared(
            "INSERT INTO auth_application_clock (id, host_ms, database_ms) VALUES (1, 0, 0)",
        )
        .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .get_connection()
            .execute_unprepared("DROP TABLE auth_application_clock")
            .await?;
        Ok(())
    }
}
