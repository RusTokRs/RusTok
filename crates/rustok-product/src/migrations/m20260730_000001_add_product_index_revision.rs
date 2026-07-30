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
ALTER TABLE products
    ADD COLUMN index_revision BIGINT NOT NULL DEFAULT 1,
    ADD CONSTRAINT chk_products_index_revision_positive CHECK (index_revision > 0);

CREATE OR REPLACE FUNCTION rustok_product_bump_index_revision()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF NEW.index_revision <= OLD.index_revision THEN
        IF OLD.index_revision = 9223372036854775807 THEN
            RAISE EXCEPTION 'product index revision exhausted for product %', OLD.id;
        END IF;
        NEW.index_revision := OLD.index_revision + 1;
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_products_bump_index_revision
BEFORE UPDATE ON products
FOR EACH ROW
EXECUTE FUNCTION rustok_product_bump_index_revision();

CREATE OR REPLACE FUNCTION rustok_product_translation_bump_index_revision()
RETURNS trigger
LANGUAGE plpgsql
AS $$
BEGIN
    IF TG_OP = 'UPDATE' AND OLD.product_id IS DISTINCT FROM NEW.product_id THEN
        UPDATE products
        SET index_revision = index_revision + 1
        WHERE id = OLD.product_id;

        UPDATE products
        SET index_revision = index_revision + 1
        WHERE id = NEW.product_id;
    ELSIF TG_OP = 'DELETE' THEN
        UPDATE products
        SET index_revision = index_revision + 1
        WHERE id = OLD.product_id;
    ELSE
        UPDATE products
        SET index_revision = index_revision + 1
        WHERE id = NEW.product_id;
    END IF;

    IF TG_OP = 'DELETE' THEN
        RETURN OLD;
    END IF;
    RETURN NEW;
END;
$$;

CREATE TRIGGER trg_product_translations_bump_index_revision
AFTER INSERT OR UPDATE OR DELETE ON product_translations
FOR EACH ROW
EXECUTE FUNCTION rustok_product_translation_bump_index_revision();
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
DROP TRIGGER IF EXISTS trg_product_translations_bump_index_revision ON product_translations;
DROP FUNCTION IF EXISTS rustok_product_translation_bump_index_revision();
DROP TRIGGER IF EXISTS trg_products_bump_index_revision ON products;
DROP FUNCTION IF EXISTS rustok_product_bump_index_revision();
ALTER TABLE products
    DROP CONSTRAINT IF EXISTS chk_products_index_revision_positive,
    DROP COLUMN IF EXISTS index_revision;
"#,
            )
            .await?;
        Ok(())
    }
}
