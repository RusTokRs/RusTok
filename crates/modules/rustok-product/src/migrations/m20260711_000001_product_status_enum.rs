use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Err(DbErr::Custom(
                "rustok-product migrations require PostgreSQL".to_owned(),
            ));
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_type WHERE typname = 'product_status_enum') THEN
        CREATE TYPE product_status_enum AS ENUM ('draft', 'active', 'archived');
    END IF;
END $$;

ALTER TABLE products
    ALTER COLUMN status DROP DEFAULT,
    ALTER COLUMN status TYPE product_status_enum USING status::product_status_enum,
    ALTER COLUMN status SET DEFAULT 'draft'::product_status_enum;
"#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Err(DbErr::Custom(
                "rustok-product migrations require PostgreSQL".to_owned(),
            ));
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
ALTER TABLE products
    ALTER COLUMN status DROP DEFAULT,
    ALTER COLUMN status TYPE varchar(32) USING status::varchar,
    ALTER COLUMN status SET DEFAULT 'draft';
DROP TYPE IF EXISTS product_status_enum;
"#,
            )
            .await?;

        Ok(())
    }
}
