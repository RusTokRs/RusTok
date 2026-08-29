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
CREATE TABLE IF NOT EXISTS catalog_category_seo_translations (
    tenant_id UUID NOT NULL,
    category_id UUID NOT NULL,
    locale VARCHAR(32) NOT NULL,
    meta_title VARCHAR(255),
    meta_description VARCHAR(500),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, category_id, locale),
    CONSTRAINT fk_catalog_category_seo_translations_category_tenant
        FOREIGN KEY (tenant_id, category_id)
        REFERENCES catalog_categories(tenant_id, id)
        ON DELETE CASCADE,
    CONSTRAINT chk_catalog_category_seo_translations_has_value
        CHECK (meta_title IS NOT NULL OR meta_description IS NOT NULL)
);

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM catalog_category_seo_translations seo
        JOIN catalog_category_translations translation
          ON translation.category_id = seo.category_id
         AND translation.locale = seo.locale
        JOIN catalog_categories category
          ON category.id = translation.category_id
         AND category.tenant_id = seo.tenant_id
        WHERE (translation.meta_title IS NOT NULL OR translation.meta_description IS NOT NULL)
          AND (
              seo.meta_title IS DISTINCT FROM translation.meta_title
              OR seo.meta_description IS DISTINCT FROM translation.meta_description
          )
    ) THEN
        RAISE EXCEPTION 'Product Category SEO backfill blocked by incompatible existing SEO ownership';
    END IF;
END $$;

INSERT INTO catalog_category_seo_translations (
    tenant_id,
    category_id,
    locale,
    meta_title,
    meta_description
)
SELECT
    category.tenant_id,
    translation.category_id,
    translation.locale,
    translation.meta_title,
    translation.meta_description
FROM catalog_category_translations translation
JOIN catalog_categories category
  ON category.id = translation.category_id
WHERE translation.meta_title IS NOT NULL
   OR translation.meta_description IS NOT NULL
ON CONFLICT (tenant_id, category_id, locale) DO NOTHING;
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
            .drop_table(
                Table::drop()
                    .table(CatalogCategorySeoTranslations::Table)
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum CatalogCategorySeoTranslations {
    #[iden = "catalog_category_seo_translations"]
    Table,
}
