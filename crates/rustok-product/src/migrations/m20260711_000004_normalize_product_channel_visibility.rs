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
CREATE OR REPLACE FUNCTION rustok_product_normalize_channel_visibility()
RETURNS TRIGGER AS $$
DECLARE
    normalized_slugs JSONB;
BEGIN
    IF jsonb_typeof(NEW.metadata) <> 'object' THEN
        NEW.metadata := '{}'::jsonb;
    END IF;

    IF jsonb_typeof(NEW.metadata #> '{channel_visibility,allowed_channel_slugs}') = 'array' THEN
        SELECT COALESCE(jsonb_agg(value ORDER BY value), '[]'::jsonb)
          INTO normalized_slugs
          FROM (
              SELECT DISTINCT to_jsonb(lower(btrim(item #>> '{}'))) AS value
              FROM jsonb_array_elements(NEW.metadata #> '{channel_visibility,allowed_channel_slugs}') item
              WHERE jsonb_typeof(item) = 'string'
                AND btrim(item #>> '{}') <> ''
          ) normalized;
        NEW.metadata := jsonb_set(
            NEW.metadata,
            '{channel_visibility,allowed_channel_slugs}',
            normalized_slugs,
            TRUE
        );
    ELSIF NEW.metadata ? 'channel_visibility' THEN
        NEW.metadata := jsonb_set(
            NEW.metadata,
            '{channel_visibility,allowed_channel_slugs}',
            '[]'::jsonb,
            TRUE
        );
    END IF;

    RETURN NEW;
END;
$$ LANGUAGE plpgsql;

UPDATE products
SET metadata = CASE
    WHEN jsonb_typeof(metadata) = 'object' THEN metadata
    ELSE '{}'::jsonb
END;

CREATE TRIGGER trg_products_normalize_channel_visibility
BEFORE INSERT OR UPDATE OF metadata ON products
FOR EACH ROW EXECUTE FUNCTION rustok_product_normalize_channel_visibility();

UPDATE products SET metadata = metadata;

ALTER TABLE products
    ADD CONSTRAINT chk_products_metadata_object CHECK (jsonb_typeof(metadata) = 'object'),
    ADD CONSTRAINT chk_products_metadata_size CHECK (pg_column_size(metadata) <= 65536);

CREATE INDEX idx_products_channel_visibility_jsonb
    ON products USING GIN (metadata jsonb_path_ops);
"#,
            )
            .await?;
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Canonical metadata and its lookup index are part of the target schema.
        Ok(())
    }
}
