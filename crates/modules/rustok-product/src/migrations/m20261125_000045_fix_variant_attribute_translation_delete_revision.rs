use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        Some(true)
    }

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
CREATE OR REPLACE FUNCTION rustok_product_touch_variant_attribute_value_translation_revision()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        UPDATE product_variant_attribute_values
        SET translation_revision = translation_revision + 1,
            updated_at = clock_timestamp()
        WHERE id = NEW.value_id;
        RETURN NEW;
    END IF;

    IF TG_OP = 'DELETE' THEN
        UPDATE product_variant_attribute_values
        SET translation_revision = translation_revision + 1,
            updated_at = clock_timestamp()
        WHERE id = OLD.value_id;
        RETURN OLD;
    END IF;

    IF OLD.value_text IS DISTINCT FROM NEW.value_text THEN
        UPDATE product_variant_attribute_values
        SET translation_revision = translation_revision + 1,
            updated_at = clock_timestamp()
        WHERE id = NEW.value_id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

DROP TRIGGER IF EXISTS trg_product_variant_attribute_value_translation_revision_delete
    ON product_variant_attribute_value_translations;
CREATE TRIGGER trg_product_variant_attribute_value_translation_revision_delete
AFTER DELETE ON product_variant_attribute_value_translations
FOR EACH ROW
EXECUTE FUNCTION rustok_product_touch_variant_attribute_value_translation_revision();
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
DROP TRIGGER IF EXISTS trg_product_variant_attribute_value_translation_revision_delete
    ON product_variant_attribute_value_translations;

CREATE OR REPLACE FUNCTION rustok_product_touch_variant_attribute_value_translation_revision()
RETURNS TRIGGER AS $$
BEGIN
    IF TG_OP = 'INSERT' THEN
        UPDATE product_variant_attribute_values
        SET translation_revision = translation_revision + 1,
            updated_at = clock_timestamp()
        WHERE id = NEW.value_id;
    ELSIF OLD.value_text IS DISTINCT FROM NEW.value_text THEN
        UPDATE product_variant_attribute_values
        SET translation_revision = translation_revision + 1,
            updated_at = clock_timestamp()
        WHERE id = NEW.value_id;
    END IF;
    RETURN NEW;
END;
$$ LANGUAGE plpgsql;
"#,
            )
            .await?;
        Ok(())
    }
}
