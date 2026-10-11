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
            .execute_unprepared(
                r#"
ALTER TABLE product_translations
    ALTER COLUMN locale TYPE VARCHAR(32);

ALTER TABLE product_image_translations
    ALTER COLUMN locale TYPE VARCHAR(32);

ALTER TABLE product_variant_translations
    ALTER COLUMN locale TYPE VARCHAR(32);

DO $$
BEGIN
    IF EXISTS (SELECT FROM pg_tables WHERE schemaname = 'public' AND tablename = 'product_option_translations') THEN
        ALTER TABLE product_option_translations ALTER COLUMN locale TYPE VARCHAR(32);
    END IF;
    IF EXISTS (SELECT FROM pg_tables WHERE schemaname = 'public' AND tablename = 'product_option_value_translations') THEN
        ALTER TABLE product_option_value_translations ALTER COLUMN locale TYPE VARCHAR(32);
    END IF;
END $$;
"#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Irreversible by design: shrinking locale columns can truncate valid BCP47-like tags.
        Ok(())
    }
}
