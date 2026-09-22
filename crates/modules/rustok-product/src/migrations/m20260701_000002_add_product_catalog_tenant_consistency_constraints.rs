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
UPDATE product_variants pv
SET tenant_id = p.tenant_id
FROM products p
WHERE pv.product_id = p.id
  AND pv.tenant_id IS NULL;

ALTER TABLE product_variants
    ALTER COLUMN tenant_id SET NOT NULL;

ALTER TABLE product_attribute_value_options
    ADD COLUMN IF NOT EXISTS tenant_id UUID;

UPDATE product_attribute_value_options pavo
SET tenant_id = pav.tenant_id
FROM product_attribute_values pav
WHERE pavo.value_id = pav.id
  AND pavo.tenant_id IS NULL;

ALTER TABLE product_attribute_value_options
    ALTER COLUMN tenant_id SET NOT NULL;

ALTER TABLE product_variant_attribute_value_options
    ADD COLUMN IF NOT EXISTS tenant_id UUID;

UPDATE product_variant_attribute_value_options pvavo
SET tenant_id = pvav.tenant_id
FROM product_variant_attribute_values pvav
WHERE pvavo.value_id = pvav.id
  AND pvavo.tenant_id IS NULL;

ALTER TABLE product_variant_attribute_value_options
    ALTER COLUMN tenant_id SET NOT NULL;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'uq_products_tenant_id') THEN
        ALTER TABLE products ADD CONSTRAINT uq_products_tenant_id UNIQUE (tenant_id, id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'uq_product_variants_tenant_id') THEN
        ALTER TABLE product_variants ADD CONSTRAINT uq_product_variants_tenant_id UNIQUE (tenant_id, id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'uq_product_attributes_tenant_id') THEN
        ALTER TABLE product_attributes ADD CONSTRAINT uq_product_attributes_tenant_id UNIQUE (tenant_id, id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'uq_product_attribute_options_tenant_id') THEN
        ALTER TABLE product_attribute_options ADD CONSTRAINT uq_product_attribute_options_tenant_id UNIQUE (tenant_id, id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'uq_catalog_categories_tenant_id') THEN
        ALTER TABLE catalog_categories ADD CONSTRAINT uq_catalog_categories_tenant_id UNIQUE (tenant_id, id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'uq_product_attribute_schemas_tenant_id') THEN
        ALTER TABLE product_attribute_schemas ADD CONSTRAINT uq_product_attribute_schemas_tenant_id UNIQUE (tenant_id, id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'uq_product_attribute_schema_groups_tenant_id') THEN
        ALTER TABLE product_attribute_schema_groups ADD CONSTRAINT uq_product_attribute_schema_groups_tenant_id UNIQUE (tenant_id, id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'uq_category_attribute_groups_tenant_id') THEN
        ALTER TABLE category_attribute_groups ADD CONSTRAINT uq_category_attribute_groups_tenant_id UNIQUE (tenant_id, id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'uq_product_attribute_values_tenant_id') THEN
        ALTER TABLE product_attribute_values ADD CONSTRAINT uq_product_attribute_values_tenant_id UNIQUE (tenant_id, id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'uq_product_variant_attribute_values_tenant_id') THEN
        ALTER TABLE product_variant_attribute_values ADD CONSTRAINT uq_product_variant_attribute_values_tenant_id UNIQUE (tenant_id, id);
    END IF;
END $$;

DO $$
BEGIN
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_products_primary_category_tenant') THEN
        ALTER TABLE products
            ADD CONSTRAINT fk_products_primary_category_tenant
            FOREIGN KEY (tenant_id, primary_category_id)
            REFERENCES catalog_categories(tenant_id, id)
            ON DELETE SET NULL (primary_category_id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_product_variants_product_tenant') THEN
        ALTER TABLE product_variants
            ADD CONSTRAINT fk_product_variants_product_tenant
            FOREIGN KEY (tenant_id, product_id)
            REFERENCES products(tenant_id, id)
            ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_product_attribute_options_attribute_tenant') THEN
        ALTER TABLE product_attribute_options
            ADD CONSTRAINT fk_product_attribute_options_attribute_tenant
            FOREIGN KEY (tenant_id, attribute_id)
            REFERENCES product_attributes(tenant_id, id)
            ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_product_attribute_channel_settings_attribute_tenant') THEN
        ALTER TABLE product_attribute_channel_settings
            ADD CONSTRAINT fk_product_attribute_channel_settings_attribute_tenant
            FOREIGN KEY (tenant_id, attribute_id)
            REFERENCES product_attributes(tenant_id, id)
            ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_catalog_categories_parent_tenant') THEN
        ALTER TABLE catalog_categories
            ADD CONSTRAINT fk_catalog_categories_parent_tenant
            FOREIGN KEY (tenant_id, parent_id)
            REFERENCES catalog_categories(tenant_id, id)
            ON DELETE SET NULL (parent_id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_catalog_category_closure_ancestor_tenant') THEN
        ALTER TABLE catalog_category_closure
            ADD CONSTRAINT fk_catalog_category_closure_ancestor_tenant
            FOREIGN KEY (tenant_id, ancestor_id)
            REFERENCES catalog_categories(tenant_id, id)
            ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_catalog_category_closure_descendant_tenant') THEN
        ALTER TABLE catalog_category_closure
            ADD CONSTRAINT fk_catalog_category_closure_descendant_tenant
            FOREIGN KEY (tenant_id, descendant_id)
            REFERENCES catalog_categories(tenant_id, id)
            ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_product_attribute_schema_groups_schema_tenant') THEN
        ALTER TABLE product_attribute_schema_groups
            ADD CONSTRAINT fk_product_attribute_schema_groups_schema_tenant
            FOREIGN KEY (tenant_id, schema_id)
            REFERENCES product_attribute_schemas(tenant_id, id)
            ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_product_attribute_schema_attributes_schema_tenant') THEN
        ALTER TABLE product_attribute_schema_attributes
            ADD CONSTRAINT fk_product_attribute_schema_attributes_schema_tenant
            FOREIGN KEY (tenant_id, schema_id)
            REFERENCES product_attribute_schemas(tenant_id, id)
            ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_product_attribute_schema_attributes_attribute_tenant') THEN
        ALTER TABLE product_attribute_schema_attributes
            ADD CONSTRAINT fk_product_attribute_schema_attributes_attribute_tenant
            FOREIGN KEY (tenant_id, attribute_id)
            REFERENCES product_attributes(tenant_id, id)
            ON DELETE RESTRICT;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_product_attribute_schema_attributes_group_tenant') THEN
        ALTER TABLE product_attribute_schema_attributes
            ADD CONSTRAINT fk_product_attribute_schema_attributes_group_tenant
            FOREIGN KEY (tenant_id, group_id)
            REFERENCES product_attribute_schema_groups(tenant_id, id)
            ON DELETE SET NULL (group_id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_category_attribute_schema_assignments_category_tenant') THEN
        ALTER TABLE category_attribute_schema_assignments
            ADD CONSTRAINT fk_category_attribute_schema_assignments_category_tenant
            FOREIGN KEY (tenant_id, category_id)
            REFERENCES catalog_categories(tenant_id, id)
            ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_category_attribute_schema_assignments_schema_tenant') THEN
        ALTER TABLE category_attribute_schema_assignments
            ADD CONSTRAINT fk_category_attribute_schema_assignments_schema_tenant
            FOREIGN KEY (tenant_id, schema_id)
            REFERENCES product_attribute_schemas(tenant_id, id)
            ON DELETE SET NULL (schema_id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_category_attribute_schema_assignments_cloned_category_tenant') THEN
        ALTER TABLE category_attribute_schema_assignments
            ADD CONSTRAINT fk_category_attribute_schema_assignments_cloned_category_tenant
            FOREIGN KEY (tenant_id, cloned_from_category_id)
            REFERENCES catalog_categories(tenant_id, id)
            ON DELETE SET NULL (cloned_from_category_id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_category_attribute_groups_category_tenant') THEN
        ALTER TABLE category_attribute_groups
            ADD CONSTRAINT fk_category_attribute_groups_category_tenant
            FOREIGN KEY (tenant_id, category_id)
            REFERENCES catalog_categories(tenant_id, id)
            ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_category_attribute_groups_inherited_group_tenant') THEN
        ALTER TABLE category_attribute_groups
            ADD CONSTRAINT fk_category_attribute_groups_inherited_group_tenant
            FOREIGN KEY (tenant_id, inherited_from_group_id)
            REFERENCES category_attribute_groups(tenant_id, id)
            ON DELETE SET NULL (inherited_from_group_id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_category_attributes_category_tenant') THEN
        ALTER TABLE category_attributes
            ADD CONSTRAINT fk_category_attributes_category_tenant
            FOREIGN KEY (tenant_id, category_id)
            REFERENCES catalog_categories(tenant_id, id)
            ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_category_attributes_attribute_tenant') THEN
        ALTER TABLE category_attributes
            ADD CONSTRAINT fk_category_attributes_attribute_tenant
            FOREIGN KEY (tenant_id, attribute_id)
            REFERENCES product_attributes(tenant_id, id)
            ON DELETE RESTRICT;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_category_attributes_group_tenant') THEN
        ALTER TABLE category_attributes
            ADD CONSTRAINT fk_category_attributes_group_tenant
            FOREIGN KEY (tenant_id, group_id)
            REFERENCES category_attribute_groups(tenant_id, id)
            ON DELETE SET NULL (group_id);
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_product_categories_product_tenant') THEN
        ALTER TABLE product_categories
            ADD CONSTRAINT fk_product_categories_product_tenant
            FOREIGN KEY (tenant_id, product_id)
            REFERENCES products(tenant_id, id)
            ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_product_categories_category_tenant') THEN
        ALTER TABLE product_categories
            ADD CONSTRAINT fk_product_categories_category_tenant
            FOREIGN KEY (tenant_id, category_id)
            REFERENCES catalog_categories(tenant_id, id)
            ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_virtual_category_product_assignments_category_tenant') THEN
        ALTER TABLE virtual_category_product_assignments
            ADD CONSTRAINT fk_virtual_category_product_assignments_category_tenant
            FOREIGN KEY (tenant_id, category_id)
            REFERENCES catalog_categories(tenant_id, id)
            ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_virtual_category_product_assignments_product_tenant') THEN
        ALTER TABLE virtual_category_product_assignments
            ADD CONSTRAINT fk_virtual_category_product_assignments_product_tenant
            FOREIGN KEY (tenant_id, product_id)
            REFERENCES products(tenant_id, id)
            ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_product_attribute_values_product_tenant') THEN
        ALTER TABLE product_attribute_values
            ADD CONSTRAINT fk_product_attribute_values_product_tenant
            FOREIGN KEY (tenant_id, product_id)
            REFERENCES products(tenant_id, id)
            ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_product_attribute_values_attribute_tenant') THEN
        ALTER TABLE product_attribute_values
            ADD CONSTRAINT fk_product_attribute_values_attribute_tenant
            FOREIGN KEY (tenant_id, attribute_id)
            REFERENCES product_attributes(tenant_id, id)
            ON DELETE RESTRICT;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_product_attribute_value_options_value_tenant') THEN
        ALTER TABLE product_attribute_value_options
            ADD CONSTRAINT fk_product_attribute_value_options_value_tenant
            FOREIGN KEY (tenant_id, value_id)
            REFERENCES product_attribute_values(tenant_id, id)
            ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_product_attribute_value_options_option_tenant') THEN
        ALTER TABLE product_attribute_value_options
            ADD CONSTRAINT fk_product_attribute_value_options_option_tenant
            FOREIGN KEY (tenant_id, option_id)
            REFERENCES product_attribute_options(tenant_id, id)
            ON DELETE RESTRICT;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_product_variant_attribute_values_variant_tenant') THEN
        ALTER TABLE product_variant_attribute_values
            ADD CONSTRAINT fk_product_variant_attribute_values_variant_tenant
            FOREIGN KEY (tenant_id, variant_id)
            REFERENCES product_variants(tenant_id, id)
            ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_product_variant_attribute_values_attribute_tenant') THEN
        ALTER TABLE product_variant_attribute_values
            ADD CONSTRAINT fk_product_variant_attribute_values_attribute_tenant
            FOREIGN KEY (tenant_id, attribute_id)
            REFERENCES product_attributes(tenant_id, id)
            ON DELETE RESTRICT;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_product_variant_attribute_value_options_value_tenant') THEN
        ALTER TABLE product_variant_attribute_value_options
            ADD CONSTRAINT fk_product_variant_attribute_value_options_value_tenant
            FOREIGN KEY (tenant_id, value_id)
            REFERENCES product_variant_attribute_values(tenant_id, id)
            ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (SELECT 1 FROM pg_constraint WHERE conname = 'fk_product_variant_attribute_value_options_option_tenant') THEN
        ALTER TABLE product_variant_attribute_value_options
            ADD CONSTRAINT fk_product_variant_attribute_value_options_option_tenant
            FOREIGN KEY (tenant_id, option_id)
            REFERENCES product_attribute_options(tenant_id, id)
            ON DELETE RESTRICT;
    END IF;
END $$;
"#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // Tenant consistency constraints are part of the target storage contract.
        Ok(())
    }
}
