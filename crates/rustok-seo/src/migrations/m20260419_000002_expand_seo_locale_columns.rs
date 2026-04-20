use sea_orm::{ConnectionTrait, DbBackend, Statement};
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == DbBackend::Postgres {
            manager
                .get_connection()
                .execute(Statement::from_string(
                    DbBackend::Postgres,
                    r#"
ALTER TABLE meta_translations
    ALTER COLUMN locale TYPE VARCHAR(32);
ALTER TABLE content_canonical_urls
    ALTER COLUMN locale TYPE VARCHAR(32);
ALTER TABLE content_url_aliases
    ALTER COLUMN locale TYPE VARCHAR(32);
"#
                    .to_string(),
                ))
                .await?;
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Locale widening to VARCHAR(32) follows the platform multilingual storage contract.
        // Rolling back by narrowing locale columns would risk truncating valid BCP47-like tags
        // such as `pt-BR` or `zh-Hant`, so the safe rollback path stays forward-only.
        Ok(())
    }
}
