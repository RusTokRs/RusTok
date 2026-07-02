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
ALTER TABLE products
    ADD COLUMN IF NOT EXISTS primary_category_id UUID;

CREATE TABLE IF NOT EXISTS product_attributes (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    code VARCHAR(128) NOT NULL,
    value_type VARCHAR(32) NOT NULL,
    scope VARCHAR(32) NOT NULL DEFAULT 'product',
    is_localized BOOLEAN NOT NULL DEFAULT FALSE,
    is_filterable BOOLEAN NOT NULL DEFAULT FALSE,
    is_searchable BOOLEAN NOT NULL DEFAULT FALSE,
    is_sortable BOOLEAN NOT NULL DEFAULT FALSE,
    is_comparable BOOLEAN NOT NULL DEFAULT FALSE,
    show_on_storefront BOOLEAN NOT NULL DEFAULT TRUE,
    show_in_admin_grid BOOLEAN NOT NULL DEFAULT FALSE,
    search_weight INTEGER NOT NULL DEFAULT 1,
    filter_display VARCHAR(32),
    facet_mode VARCHAR(32),
    position INTEGER NOT NULL DEFAULT 0,
    validation JSONB NOT NULL DEFAULT '{}'::jsonb,
    default_value JSONB,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    archived_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT chk_product_attributes_value_type CHECK (
        value_type IN ('text', 'textarea', 'richtext', 'integer', 'decimal', 'boolean', 'date', 'datetime', 'select', 'multiselect', 'json')
    ),
    CONSTRAINT chk_product_attributes_scope CHECK (scope IN ('product', 'variant', 'both')),
    CONSTRAINT uq_product_attributes_tenant_code UNIQUE (tenant_id, code)
);

CREATE TABLE IF NOT EXISTS product_attribute_translations (
    id UUID PRIMARY KEY,
    attribute_id UUID NOT NULL REFERENCES product_attributes(id) ON DELETE CASCADE,
    locale VARCHAR(32) NOT NULL,
    label VARCHAR(255) NOT NULL,
    help_text TEXT,
    facet_label VARCHAR(255),
    seo_label VARCHAR(255),
    CONSTRAINT uq_product_attribute_translations UNIQUE (attribute_id, locale)
);

CREATE TABLE IF NOT EXISTS product_attribute_options (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    attribute_id UUID NOT NULL REFERENCES product_attributes(id) ON DELETE CASCADE,
    code VARCHAR(128) NOT NULL,
    position INTEGER NOT NULL DEFAULT 0,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    archived_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_product_attribute_options_attribute_code UNIQUE (attribute_id, code)
);

CREATE TABLE IF NOT EXISTS product_attribute_option_translations (
    id UUID PRIMARY KEY,
    option_id UUID NOT NULL REFERENCES product_attribute_options(id) ON DELETE CASCADE,
    locale VARCHAR(32) NOT NULL,
    label VARCHAR(255) NOT NULL,
    CONSTRAINT uq_product_attribute_option_translations UNIQUE (option_id, locale)
);

CREATE TABLE IF NOT EXISTS product_attribute_channel_settings (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    attribute_id UUID NOT NULL REFERENCES product_attributes(id) ON DELETE CASCADE,
    channel_id UUID NOT NULL,
    is_filterable BOOLEAN,
    is_searchable BOOLEAN,
    is_sortable BOOLEAN,
    show_on_storefront BOOLEAN,
    show_in_admin_grid BOOLEAN,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_product_attribute_channel_settings UNIQUE (tenant_id, attribute_id, channel_id)
);

CREATE TABLE IF NOT EXISTS catalog_categories (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    parent_id UUID REFERENCES catalog_categories(id) ON DELETE SET NULL,
    code VARCHAR(128) NOT NULL,
    slug VARCHAR(255) NOT NULL,
    kind VARCHAR(32) NOT NULL DEFAULT 'structural',
    path VARCHAR(2048) NOT NULL,
    level INTEGER NOT NULL DEFAULT 0,
    position INTEGER NOT NULL DEFAULT 0,
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    rule_config JSONB NOT NULL DEFAULT '{}'::jsonb,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    deleted_at TIMESTAMPTZ,
    CONSTRAINT chk_catalog_categories_kind CHECK (kind IN ('structural', 'collection', 'virtual')),
    CONSTRAINT uq_catalog_categories_tenant_code UNIQUE (tenant_id, code),
    CONSTRAINT uq_catalog_categories_parent_slug UNIQUE (tenant_id, parent_id, slug)
);

CREATE TABLE IF NOT EXISTS catalog_category_translations (
    id UUID PRIMARY KEY,
    category_id UUID NOT NULL REFERENCES catalog_categories(id) ON DELETE CASCADE,
    locale VARCHAR(32) NOT NULL,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    meta_title VARCHAR(255),
    meta_description VARCHAR(500),
    CONSTRAINT uq_catalog_category_translations UNIQUE (category_id, locale)
);

CREATE TABLE IF NOT EXISTS catalog_category_closure (
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    ancestor_id UUID NOT NULL REFERENCES catalog_categories(id) ON DELETE CASCADE,
    descendant_id UUID NOT NULL REFERENCES catalog_categories(id) ON DELETE CASCADE,
    depth INTEGER NOT NULL,
    PRIMARY KEY (tenant_id, ancestor_id, descendant_id),
    CONSTRAINT chk_catalog_category_closure_depth CHECK (depth >= 0)
);

CREATE TABLE IF NOT EXISTS product_attribute_schemas (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    code VARCHAR(128) NOT NULL,
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    archived_at TIMESTAMPTZ,
    CONSTRAINT uq_product_attribute_schemas_tenant_code UNIQUE (tenant_id, code),
    CONSTRAINT chk_product_attribute_schemas_status CHECK (status IN ('active', 'archived'))
);

CREATE TABLE IF NOT EXISTS product_attribute_schema_translations (
    id UUID PRIMARY KEY,
    schema_id UUID NOT NULL REFERENCES product_attribute_schemas(id) ON DELETE CASCADE,
    locale VARCHAR(32) NOT NULL,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    CONSTRAINT uq_product_attribute_schema_translations UNIQUE (schema_id, locale)
);

CREATE TABLE IF NOT EXISTS product_attribute_schema_groups (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    schema_id UUID NOT NULL REFERENCES product_attribute_schemas(id) ON DELETE CASCADE,
    code VARCHAR(128) NOT NULL,
    position INTEGER NOT NULL DEFAULT 0,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    CONSTRAINT uq_product_attribute_schema_groups_schema_code UNIQUE (schema_id, code)
);

CREATE TABLE IF NOT EXISTS product_attribute_schema_group_translations (
    id UUID PRIMARY KEY,
    group_id UUID NOT NULL REFERENCES product_attribute_schema_groups(id) ON DELETE CASCADE,
    locale VARCHAR(32) NOT NULL,
    label VARCHAR(255) NOT NULL,
    CONSTRAINT uq_product_attribute_schema_group_translations UNIQUE (group_id, locale)
);

CREATE TABLE IF NOT EXISTS product_attribute_schema_attributes (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    schema_id UUID NOT NULL REFERENCES product_attribute_schemas(id) ON DELETE CASCADE,
    attribute_id UUID NOT NULL REFERENCES product_attributes(id) ON DELETE RESTRICT,
    group_id UUID REFERENCES product_attribute_schema_groups(id) ON DELETE SET NULL,
    is_required BOOLEAN NOT NULL DEFAULT FALSE,
    is_disabled BOOLEAN NOT NULL DEFAULT FALSE,
    position INTEGER NOT NULL DEFAULT 0,
    visibility_overrides JSONB NOT NULL DEFAULT '{}'::jsonb,
    validation_overrides JSONB NOT NULL DEFAULT '{}'::jsonb,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    CONSTRAINT uq_product_attribute_schema_attributes UNIQUE (schema_id, attribute_id)
);

CREATE TABLE IF NOT EXISTS category_attribute_schema_assignments (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    category_id UUID NOT NULL REFERENCES catalog_categories(id) ON DELETE CASCADE,
    mode VARCHAR(32) NOT NULL DEFAULT 'inherit',
    schema_id UUID REFERENCES product_attribute_schemas(id) ON DELETE SET NULL,
    cloned_from_category_id UUID REFERENCES catalog_categories(id) ON DELETE SET NULL,
    snapshot JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_category_attribute_schema_assignments_category UNIQUE (tenant_id, category_id),
    CONSTRAINT chk_category_attribute_schema_assignments_mode CHECK (mode IN ('inherit', 'use_schema', 'clone_from_category', 'custom'))
);

CREATE TABLE IF NOT EXISTS category_attribute_groups (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    category_id UUID NOT NULL REFERENCES catalog_categories(id) ON DELETE CASCADE,
    code VARCHAR(128) NOT NULL,
    inherited_from_group_id UUID,
    position INTEGER NOT NULL DEFAULT 0,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    CONSTRAINT uq_category_attribute_groups_category_code UNIQUE (category_id, code)
);

CREATE TABLE IF NOT EXISTS category_attribute_group_translations (
    id UUID PRIMARY KEY,
    group_id UUID NOT NULL REFERENCES category_attribute_groups(id) ON DELETE CASCADE,
    locale VARCHAR(32) NOT NULL,
    label VARCHAR(255) NOT NULL,
    CONSTRAINT uq_category_attribute_group_translations UNIQUE (group_id, locale)
);

CREATE TABLE IF NOT EXISTS category_attributes (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    category_id UUID NOT NULL REFERENCES catalog_categories(id) ON DELETE CASCADE,
    attribute_id UUID NOT NULL REFERENCES product_attributes(id) ON DELETE RESTRICT,
    group_id UUID REFERENCES category_attribute_groups(id) ON DELETE SET NULL,
    binding_kind VARCHAR(32) NOT NULL DEFAULT 'addition',
    is_required BOOLEAN,
    is_disabled BOOLEAN NOT NULL DEFAULT FALSE,
    position INTEGER,
    visibility_overrides JSONB NOT NULL DEFAULT '{}'::jsonb,
    validation_overrides JSONB NOT NULL DEFAULT '{}'::jsonb,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    CONSTRAINT uq_category_attributes_category_attribute UNIQUE (category_id, attribute_id),
    CONSTRAINT chk_category_attributes_binding_kind CHECK (binding_kind IN ('addition', 'override', 'removal'))
);

CREATE TABLE IF NOT EXISTS product_categories (
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    product_id UUID NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    category_id UUID NOT NULL REFERENCES catalog_categories(id) ON DELETE CASCADE,
    assignment_kind VARCHAR(32) NOT NULL DEFAULT 'navigation',
    position INTEGER NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, product_id, category_id),
    CONSTRAINT chk_product_categories_assignment_kind CHECK (assignment_kind IN ('primary', 'navigation', 'collection', 'virtual'))
);

CREATE TABLE IF NOT EXISTS virtual_category_product_assignments (
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    category_id UUID NOT NULL REFERENCES catalog_categories(id) ON DELETE CASCADE,
    product_id UUID NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    matched_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    match_reason JSONB NOT NULL DEFAULT '{}'::jsonb,
    PRIMARY KEY (tenant_id, category_id, product_id)
);

CREATE TABLE IF NOT EXISTS product_attribute_values (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    product_id UUID NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    attribute_id UUID NOT NULL REFERENCES product_attributes(id) ON DELETE RESTRICT,
    value_text TEXT,
    value_integer BIGINT,
    value_decimal NUMERIC(20, 6),
    value_boolean BOOLEAN,
    value_date DATE,
    value_datetime TIMESTAMPTZ,
    value_json JSONB,
    detached_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_product_attribute_values UNIQUE (tenant_id, product_id, attribute_id)
);

CREATE TABLE IF NOT EXISTS product_attribute_value_translations (
    id UUID PRIMARY KEY,
    value_id UUID NOT NULL REFERENCES product_attribute_values(id) ON DELETE CASCADE,
    locale VARCHAR(32) NOT NULL,
    value_text TEXT,
    CONSTRAINT uq_product_attribute_value_translations UNIQUE (value_id, locale)
);

CREATE TABLE IF NOT EXISTS product_attribute_value_options (
    value_id UUID NOT NULL REFERENCES product_attribute_values(id) ON DELETE CASCADE,
    option_id UUID NOT NULL REFERENCES product_attribute_options(id) ON DELETE RESTRICT,
    PRIMARY KEY (value_id, option_id)
);

CREATE TABLE IF NOT EXISTS product_variant_attribute_values (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL REFERENCES tenants(id) ON DELETE CASCADE,
    variant_id UUID NOT NULL REFERENCES product_variants(id) ON DELETE CASCADE,
    attribute_id UUID NOT NULL REFERENCES product_attributes(id) ON DELETE RESTRICT,
    value_text TEXT,
    value_integer BIGINT,
    value_decimal NUMERIC(20, 6),
    value_boolean BOOLEAN,
    value_date DATE,
    value_datetime TIMESTAMPTZ,
    value_json JSONB,
    detached_at TIMESTAMPTZ,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    CONSTRAINT uq_product_variant_attribute_values UNIQUE (tenant_id, variant_id, attribute_id)
);

CREATE TABLE IF NOT EXISTS product_variant_attribute_value_translations (
    id UUID PRIMARY KEY,
    value_id UUID NOT NULL REFERENCES product_variant_attribute_values(id) ON DELETE CASCADE,
    locale VARCHAR(32) NOT NULL,
    value_text TEXT,
    CONSTRAINT uq_product_variant_attribute_value_translations UNIQUE (value_id, locale)
);

CREATE TABLE IF NOT EXISTS product_variant_attribute_value_options (
    value_id UUID NOT NULL REFERENCES product_variant_attribute_values(id) ON DELETE CASCADE,
    option_id UUID NOT NULL REFERENCES product_attribute_options(id) ON DELETE RESTRICT,
    PRIMARY KEY (value_id, option_id)
);

ALTER TABLE products
    ADD CONSTRAINT fk_products_primary_category
    FOREIGN KEY (primary_category_id)
    REFERENCES catalog_categories(id)
    ON DELETE SET NULL;

CREATE INDEX IF NOT EXISTS idx_products_primary_category ON products (tenant_id, primary_category_id);
CREATE INDEX IF NOT EXISTS idx_product_attributes_flags ON product_attributes (tenant_id, is_filterable, is_searchable, is_sortable);
CREATE INDEX IF NOT EXISTS idx_catalog_categories_tree ON catalog_categories (tenant_id, parent_id, position);
CREATE INDEX IF NOT EXISTS idx_catalog_category_closure_descendant ON catalog_category_closure (tenant_id, descendant_id, depth);
CREATE INDEX IF NOT EXISTS idx_product_categories_category ON product_categories (tenant_id, category_id, position);
CREATE INDEX IF NOT EXISTS idx_product_attribute_values_lookup ON product_attribute_values (tenant_id, attribute_id, product_id);
CREATE INDEX IF NOT EXISTS idx_product_variant_attribute_values_lookup ON product_variant_attribute_values (tenant_id, attribute_id, variant_id);
"#,
            )
            .await
            .map(|_| ())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
ALTER TABLE products DROP CONSTRAINT IF EXISTS fk_products_primary_category;
DROP TABLE IF EXISTS product_variant_attribute_value_options;
DROP TABLE IF EXISTS product_variant_attribute_value_translations;
DROP TABLE IF EXISTS product_variant_attribute_values;
DROP TABLE IF EXISTS product_attribute_value_options;
DROP TABLE IF EXISTS product_attribute_value_translations;
DROP TABLE IF EXISTS product_attribute_values;
DROP TABLE IF EXISTS virtual_category_product_assignments;
DROP TABLE IF EXISTS product_categories;
DROP TABLE IF EXISTS category_attributes;
DROP TABLE IF EXISTS category_attribute_group_translations;
DROP TABLE IF EXISTS category_attribute_groups;
DROP TABLE IF EXISTS category_attribute_schema_assignments;
DROP TABLE IF EXISTS product_attribute_schema_attributes;
DROP TABLE IF EXISTS product_attribute_schema_group_translations;
DROP TABLE IF EXISTS product_attribute_schema_groups;
DROP TABLE IF EXISTS product_attribute_schema_translations;
DROP TABLE IF EXISTS product_attribute_schemas;
DROP TABLE IF EXISTS catalog_category_closure;
DROP TABLE IF EXISTS catalog_category_translations;
DROP TABLE IF EXISTS catalog_categories;
DROP TABLE IF EXISTS product_attribute_channel_settings;
DROP TABLE IF EXISTS product_attribute_option_translations;
DROP TABLE IF EXISTS product_attribute_options;
DROP TABLE IF EXISTS product_attribute_translations;
DROP TABLE IF EXISTS product_attributes;
ALTER TABLE products DROP COLUMN IF EXISTS primary_category_id;
"#,
            )
            .await
            .map(|_| ())
    }
}
