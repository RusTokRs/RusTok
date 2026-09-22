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
    IF EXISTS (SELECT 1 FROM product_images WHERE media_id IS NULL) THEN
        RAISE EXCEPTION 'cannot remove transitional product image storage: media_id is missing';
    END IF;
END $$;

DROP INDEX IF EXISTS idx_products_storefront_published;

ALTER TABLE products
    DROP COLUMN IF EXISTS is_gift_card,
    DROP COLUMN IF EXISTS discountable,
    DROP COLUMN IF EXISTS weight,
    DROP COLUMN IF EXISTS length,
    DROP COLUMN IF EXISTS height,
    DROP COLUMN IF EXISTS width,
    DROP COLUMN IF EXISTS hs_code,
    DROP COLUMN IF EXISTS origin_country,
    DROP COLUMN IF EXISTS mid_code,
    DROP COLUMN IF EXISTS external_id,
    DROP COLUMN IF EXISTS deleted_at;

ALTER TABLE product_translations
    DROP COLUMN IF EXISTS subtitle,
    DROP COLUMN IF EXISTS material;

ALTER TABLE product_options
    DROP COLUMN IF EXISTS name,
    DROP COLUMN IF EXISTS values;

ALTER TABLE product_images
    ALTER COLUMN media_id SET NOT NULL,
    DROP COLUMN IF EXISTS url,
    DROP COLUMN IF EXISTS metadata,
    DROP COLUMN IF EXISTS created_at;

ALTER TABLE product_variants
    ALTER COLUMN weight TYPE NUMERIC(20, 6) USING weight::numeric,
    DROP COLUMN IF EXISTS length,
    DROP COLUMN IF EXISTS height,
    DROP COLUMN IF EXISTS width,
    DROP COLUMN IF EXISTS hs_code,
    DROP COLUMN IF EXISTS origin_country,
    DROP COLUMN IF EXISTS mid_code,
    DROP COLUMN IF EXISTS metadata,
    DROP COLUMN IF EXISTS deleted_at;

CREATE INDEX idx_products_storefront_published
    ON products (tenant_id, status, published_at DESC, created_at DESC)
    WHERE published_at IS NOT NULL
      AND COALESCE(
          metadata #> '{channel_visibility,allowed_channel_slugs}',
          '[]'::jsonb
      ) = '[]'::jsonb;
"#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Removed compatibility-only columns are not part of the target Product schema.
        Ok(())
    }
}
