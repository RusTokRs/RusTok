use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        let is_postgres = manager.get_database_backend() == DatabaseBackend::Postgres;

        if is_postgres {
            db.execute_unprepared(
                r#"
CREATE TABLE IF NOT EXISTS brands (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    slug VARCHAR(255) NOT NULL,
    logo_media_id UUID,
    banner_media_id UUID,
    website_url VARCHAR(512),
    is_active BOOLEAN NOT NULL DEFAULT TRUE,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_brands_tenant_slug UNIQUE (tenant_id, slug)
);

CREATE INDEX IF NOT EXISTS idx_brands_tenant_active
    ON brands (tenant_id, is_active, slug);

CREATE TABLE IF NOT EXISTS brand_translations (
    id UUID PRIMARY KEY,
    brand_id UUID NOT NULL REFERENCES brands(id) ON DELETE CASCADE,
    locale VARCHAR(16) NOT NULL,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_brand_translations_locale UNIQUE (brand_id, locale)
);

CREATE INDEX IF NOT EXISTS idx_brand_translations_name
    ON brand_translations (locale, name);

CREATE TABLE IF NOT EXISTS brand_products (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    brand_id UUID NOT NULL REFERENCES brands(id) ON DELETE CASCADE,
    product_id UUID NOT NULL,
    is_primary BOOLEAN NOT NULL DEFAULT TRUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_brand_products_unique UNIQUE (tenant_id, brand_id, product_id)
);

CREATE INDEX IF NOT EXISTS idx_brand_products_lookup
    ON brand_products (tenant_id, product_id);

CREATE INDEX IF NOT EXISTS idx_brand_products_brand
    ON brand_products (tenant_id, brand_id);
"#,
            )
            .await?;
        } else {
            db.execute_unprepared(
                r#"
CREATE TABLE IF NOT EXISTS brands (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    slug TEXT NOT NULL,
    logo_media_id TEXT,
    banner_media_id TEXT,
    website_url TEXT,
    is_active INTEGER NOT NULL DEFAULT 1,
    metadata TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (tenant_id, slug)
);

CREATE INDEX IF NOT EXISTS idx_brands_tenant_active
    ON brands (tenant_id, is_active, slug);

CREATE TABLE IF NOT EXISTS brand_translations (
    id TEXT PRIMARY KEY,
    brand_id TEXT NOT NULL REFERENCES brands(id) ON DELETE CASCADE,
    locale TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (brand_id, locale)
);

CREATE INDEX IF NOT EXISTS idx_brand_translations_name
    ON brand_translations (locale, name);

CREATE TABLE IF NOT EXISTS brand_products (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    brand_id TEXT NOT NULL REFERENCES brands(id) ON DELETE CASCADE,
    product_id TEXT NOT NULL,
    is_primary INTEGER NOT NULL DEFAULT 1,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (tenant_id, brand_id, product_id)
);

CREATE INDEX IF NOT EXISTS idx_brand_products_lookup
    ON brand_products (tenant_id, product_id);

CREATE INDEX IF NOT EXISTS idx_brand_products_brand
    ON brand_products (tenant_id, brand_id);
"#,
            )
            .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared("DROP TABLE IF EXISTS brand_products;").await?;
        db.execute_unprepared("DROP TABLE IF EXISTS brand_translations;").await?;
        db.execute_unprepared("DROP TABLE IF EXISTS brands;").await?;
        Ok(())
    }
}
