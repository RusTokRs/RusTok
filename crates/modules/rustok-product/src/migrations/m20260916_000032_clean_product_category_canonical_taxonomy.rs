use sea_orm::DatabaseBackend;
use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();

        // 1. Drop the intermediate binding table
        manager
            .drop_table(
                Table::drop()
                    .table(ProductCatalogCategoryTaxonomyBindings::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;

        // 2. Schema cleanup for PostgreSQL
        if manager.get_database_backend() == DatabaseBackend::Postgres {
            db.execute_unprepared(
                r#"
DROP TRIGGER IF EXISTS trg_catalog_categories_validate_tree ON catalog_categories;
DROP FUNCTION IF EXISTS rustok_product_validate_category_tree_trigger();
DROP FUNCTION IF EXISTS rustok_product_assert_category_tree();

ALTER TABLE catalog_categories DROP CONSTRAINT IF EXISTS fk_catalog_categories_parent_tenant;
ALTER TABLE catalog_categories DROP CONSTRAINT IF EXISTS uq_catalog_categories_parent_slug;
DROP INDEX IF EXISTS uq_catalog_categories_tenant_root_slug;
DROP INDEX IF EXISTS idx_catalog_categories_tree;

ALTER TABLE catalog_categories DROP COLUMN IF EXISTS parent_id;
ALTER TABLE catalog_categories DROP COLUMN IF EXISTS slug;
ALTER TABLE catalog_categories DROP COLUMN IF EXISTS position;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.tables
        WHERE table_schema = current_schema() AND table_name = 'taxonomy_terms'
    ) AND NOT EXISTS (
        SELECT 1 FROM pg_constraint WHERE conname = 'fk_catalog_categories_taxonomy_term'
    ) THEN
        ALTER TABLE catalog_categories
            ADD CONSTRAINT fk_catalog_categories_taxonomy_term
            FOREIGN KEY (tenant_id, id)
            REFERENCES taxonomy_terms(tenant_id, id)
            ON DELETE CASCADE;
    END IF;
END;
$$;
"#,
            )
            .await?;
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Irreversible clean taxonomy cutover
        Ok(())
    }
}

#[derive(Iden)]
enum ProductCatalogCategoryTaxonomyBindings {
    #[iden = "product_catalog_category_taxonomy_bindings"]
    Table,
}
