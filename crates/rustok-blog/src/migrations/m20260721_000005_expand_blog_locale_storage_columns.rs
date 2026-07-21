use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            // SQLite does not enforce declared VARCHAR lengths. PostgreSQL owns the
            // production width contract for normalized BCP47-like locale tags.
            return Ok(());
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
ALTER TABLE blog_post_translations
    ALTER COLUMN locale TYPE VARCHAR(32);
ALTER TABLE blog_category_translations
    ALTER COLUMN locale TYPE VARCHAR(32);
"#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Irreversible by design: narrowing locale columns can truncate valid
        // normalized BCP47-like tags already persisted by tenants.
        Ok(())
    }
}
