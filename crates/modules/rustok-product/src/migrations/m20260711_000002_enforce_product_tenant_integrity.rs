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
ALTER TABLE product_translations ADD COLUMN IF NOT EXISTS tenant_id UUID;
UPDATE product_translations translation
SET tenant_id = product.tenant_id
FROM products product
WHERE translation.product_id = product.id
  AND translation.tenant_id IS NULL;
ALTER TABLE product_translations ALTER COLUMN tenant_id SET NOT NULL;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'uq_product_translations_tenant_id') THEN
        ALTER TABLE product_translations
            ADD CONSTRAINT uq_product_translations_tenant_id UNIQUE (tenant_id, id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_product_translations_product_tenant') THEN
        ALTER TABLE product_translations
            ADD CONSTRAINT fk_product_translations_product_tenant
            FOREIGN KEY (tenant_id, product_id)
            REFERENCES products(tenant_id, id)
            ON DELETE CASCADE;
    END IF;
END $$;

CREATE UNIQUE INDEX IF NOT EXISTS uq_product_translations_tenant_locale_handle
    ON product_translations (tenant_id, locale, handle);
CREATE UNIQUE INDEX IF NOT EXISTS uq_product_variants_tenant_sku
    ON product_variants (tenant_id, sku)
    WHERE sku IS NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS uq_catalog_categories_tenant_root_slug
    ON catalog_categories (tenant_id, slug)
    WHERE parent_id IS NULL;
CREATE INDEX IF NOT EXISTS idx_products_storefront_published
    ON products (tenant_id, status, published_at DESC, created_at DESC)
    WHERE deleted_at IS NULL;

UPDATE product_variants
SET inventory_management = CASE WHEN manage_inventory THEN 'manual' ELSE 'none' END,
    inventory_policy = CASE WHEN allow_backorder THEN 'continue' ELSE 'deny' END,
    position = variant_rank
WHERE manage_inventory IS NOT NULL
   OR allow_backorder IS NOT NULL
   OR variant_rank IS NOT NULL;
ALTER TABLE product_variants DROP COLUMN IF EXISTS manage_inventory;
ALTER TABLE product_variants DROP COLUMN IF EXISTS allow_backorder;
ALTER TABLE product_variants DROP COLUMN IF EXISTS variant_rank;

UPDATE product_tags tag
SET tenant_id = product.tenant_id
FROM products product
WHERE tag.product_id = product.id
  AND tag.tenant_id IS DISTINCT FROM product.tenant_id;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_product_tags_product_tenant') THEN
        ALTER TABLE product_tags
            ADD CONSTRAINT fk_product_tags_product_tenant
            FOREIGN KEY (tenant_id, product_id)
            REFERENCES products(tenant_id, id)
            ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_product_tags_term_tenant') THEN
        ALTER TABLE product_tags
            ADD CONSTRAINT fk_product_tags_term_tenant
            FOREIGN KEY (tenant_id, term_id)
            REFERENCES taxonomy_terms(tenant_id, id)
            ON DELETE CASCADE;
    END IF;
END $$;
CREATE INDEX IF NOT EXISTS idx_product_tags_tenant_product
    ON product_tags (tenant_id, product_id);
"#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Tenant invariants and the replacement inventory columns are the target schema.
        Ok(())
    }
}
