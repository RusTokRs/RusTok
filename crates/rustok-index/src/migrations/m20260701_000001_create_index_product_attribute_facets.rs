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
CREATE TABLE IF NOT EXISTS index_product_categories (
    tenant_id UUID NOT NULL,
    product_id UUID NOT NULL,
    category_id UUID NOT NULL,
    locale VARCHAR(32) NOT NULL,
    category_kind VARCHAR(32) NOT NULL,
    assignment_kind VARCHAR(32) NOT NULL,
    path TEXT,
    name VARCHAR(255),
    position INTEGER NOT NULL DEFAULT 0,
    indexed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (tenant_id, product_id, category_id, locale)
);

CREATE TABLE IF NOT EXISTS index_product_attribute_values (
    id UUID NOT NULL,
    tenant_id UUID NOT NULL,
    product_id UUID NOT NULL,
    locale VARCHAR(32) NOT NULL,
    channel_id UUID,
    attribute_id UUID NOT NULL,
    attribute_code VARCHAR(128) NOT NULL,
    value_key VARCHAR(255) NOT NULL DEFAULT '',
    value_label TEXT,
    value_number NUMERIC(20, 6),
    value_bool BOOLEAN,
    value_datetime TIMESTAMPTZ,
    sort_value TEXT,
    search_text TEXT,
    facet_bucket_key VARCHAR(255),
    is_filterable BOOLEAN NOT NULL DEFAULT FALSE,
    is_searchable BOOLEAN NOT NULL DEFAULT FALSE,
    is_sortable BOOLEAN NOT NULL DEFAULT FALSE,
    is_comparable BOOLEAN NOT NULL DEFAULT FALSE,
    show_on_storefront BOOLEAN NOT NULL DEFAULT TRUE,
    show_in_admin_grid BOOLEAN NOT NULL DEFAULT FALSE,
    is_detached BOOLEAN NOT NULL DEFAULT FALSE,
    indexed_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (id),
    CONSTRAINT uq_index_product_attribute_values UNIQUE (
        tenant_id, product_id, locale, channel_id, attribute_id, value_key
    )
);

CREATE INDEX IF NOT EXISTS idx_index_product_categories_category
    ON index_product_categories (tenant_id, category_id, locale, assignment_kind);

CREATE INDEX IF NOT EXISTS idx_index_product_attribute_facets
    ON index_product_attribute_values (tenant_id, locale, channel_id, attribute_code, facet_bucket_key)
    WHERE is_filterable = TRUE AND is_detached = FALSE;

CREATE INDEX IF NOT EXISTS idx_index_product_attribute_sort
    ON index_product_attribute_values (tenant_id, locale, channel_id, attribute_code, sort_value)
    WHERE is_sortable = TRUE AND is_detached = FALSE;

CREATE INDEX IF NOT EXISTS idx_index_product_attribute_search
    ON index_product_attribute_values (tenant_id, locale, channel_id, attribute_code)
    WHERE is_searchable = TRUE AND is_detached = FALSE;
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
            .get_connection()
            .execute_unprepared(
                r#"
DROP TABLE IF EXISTS index_product_attribute_values;
DROP TABLE IF EXISTS index_product_categories;
"#,
            )
            .await?;

        Ok(())
    }
}
