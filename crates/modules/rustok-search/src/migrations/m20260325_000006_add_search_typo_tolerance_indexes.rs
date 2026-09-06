use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }

        manager
            .get_connection()
            .execute_unprepared("CREATE EXTENSION IF NOT EXISTS pg_trgm")
            .await?;

        manager
            .get_connection()
            .execute_unprepared(
                r#"
                CREATE INDEX IF NOT EXISTS idx_search_documents_title_trgm
                    ON search_documents USING GIN (lower(title) gin_trgm_ops);

                CREATE INDEX IF NOT EXISTS idx_search_documents_slug_trgm
                    ON search_documents USING GIN (lower(coalesce(slug, '')) gin_trgm_ops);

                CREATE INDEX IF NOT EXISTS idx_search_documents_handle_trgm
                    ON search_documents USING GIN (lower(coalesce(handle, '')) gin_trgm_ops);

                CREATE INDEX IF NOT EXISTS idx_search_documents_keywords_trgm
                    ON search_documents USING GIN (lower(keywords_text) gin_trgm_ops);
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
                DROP INDEX IF EXISTS idx_search_documents_title_trgm;
                DROP INDEX IF EXISTS idx_search_documents_slug_trgm;
                DROP INDEX IF EXISTS idx_search_documents_handle_trgm;
                DROP INDEX IF EXISTS idx_search_documents_keywords_trgm;
                "#,
            )
            .await?;

        Ok(())
    }
}
