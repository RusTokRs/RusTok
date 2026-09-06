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
ALTER TABLE product_variants
    ADD COLUMN index_revision BIGINT NOT NULL DEFAULT 1,
    ADD CONSTRAINT chk_product_variants_index_revision_positive CHECK (index_revision > 0);

CREATE OR REPLACE FUNCTION rustok_product_variant_bump_index_revision()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF OLD.index_revision = 9223372036854775807 THEN
        RAISE EXCEPTION 'product variant index revision exhausted for variant %', OLD.id;
    END IF;
    NEW.index_revision := OLD.index_revision + 1;
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_product_variants_bump_index_revision
BEFORE UPDATE ON product_variants
FOR EACH ROW
EXECUTE FUNCTION rustok_product_variant_bump_index_revision();
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
DROP TRIGGER IF EXISTS trg_product_variants_bump_index_revision ON product_variants;
DROP FUNCTION IF EXISTS rustok_product_variant_bump_index_revision();
ALTER TABLE product_variants
    DROP CONSTRAINT IF EXISTS chk_product_variants_index_revision_positive,
    DROP COLUMN IF EXISTS index_revision;
"#,
            )
            .await?;
        Ok(())
    }
}
