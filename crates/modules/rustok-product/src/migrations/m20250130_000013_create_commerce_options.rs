use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Zero-legacy policy (AGENTS.md): the legacy product_options subsystem is
        // eliminated and superseded by the canonical variant axis architecture.
        // Drop any legacy option tables if present in existing databases.
        if manager.get_database_backend() == DatabaseBackend::Sqlite {
            manager
                .get_connection()
                .execute_unprepared(
                    r#"
                    DROP TABLE IF EXISTS product_option_value_translations;
                    DROP TABLE IF EXISTS product_option_values;
                    DROP TABLE IF EXISTS product_option_translations;
                    DROP TABLE IF EXISTS product_options;
                    "#,
                )
                .await?;
        } else {
            manager
                .get_connection()
                .execute_unprepared(
                    r#"
                    DROP TABLE IF EXISTS product_option_value_translations CASCADE;
                    DROP TABLE IF EXISTS product_option_values CASCADE;
                    DROP TABLE IF EXISTS product_option_translations CASCADE;
                    DROP TABLE IF EXISTS product_options CASCADE;
                    "#,
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Ok(())
    }
}
