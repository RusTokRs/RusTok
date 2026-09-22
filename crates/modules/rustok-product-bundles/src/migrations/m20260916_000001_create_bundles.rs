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
CREATE TABLE IF NOT EXISTS product_bundles (
    id UUID PRIMARY KEY,
    tenant_id UUID NOT NULL,
    bundle_product_id UUID,
    slug VARCHAR(255) NOT NULL,
    bundle_type VARCHAR(32) NOT NULL DEFAULT 'fixed',
    status VARCHAR(32) NOT NULL DEFAULT 'active',
    discount_type VARCHAR(32) NOT NULL DEFAULT 'none',
    discount_value DECIMAL(12, 4) NOT NULL DEFAULT 0,
    metadata JSONB NOT NULL DEFAULT '{}'::jsonb,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_product_bundles_tenant_slug UNIQUE (tenant_id, slug)
);

CREATE INDEX IF NOT EXISTS idx_product_bundles_lookup
    ON product_bundles (tenant_id, status, slug);

CREATE TABLE IF NOT EXISTS product_bundle_translations (
    id UUID PRIMARY KEY,
    bundle_id UUID NOT NULL REFERENCES product_bundles(id) ON DELETE CASCADE,
    locale VARCHAR(16) NOT NULL,
    name VARCHAR(255) NOT NULL,
    description TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT uq_product_bundle_translations_locale UNIQUE (bundle_id, locale)
);

CREATE INDEX IF NOT EXISTS idx_product_bundle_translations_name
    ON product_bundle_translations (locale, name);

CREATE TABLE IF NOT EXISTS product_bundle_items (
    id UUID PRIMARY KEY,
    bundle_id UUID NOT NULL REFERENCES product_bundles(id) ON DELETE CASCADE,
    product_id UUID NOT NULL,
    variant_id UUID,
    quantity INT NOT NULL DEFAULT 1,
    is_optional BOOLEAN NOT NULL DEFAULT FALSE,
    discount_rate DECIMAL(5, 4),
    position INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_product_bundle_items_lookup
    ON product_bundle_items (bundle_id, position);

CREATE INDEX IF NOT EXISTS idx_product_bundle_items_product
    ON product_bundle_items (product_id);
"#,
            )
            .await?;
        } else {
            db.execute_unprepared(
                r#"
CREATE TABLE IF NOT EXISTS product_bundles (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    bundle_product_id TEXT,
    slug TEXT NOT NULL,
    bundle_type TEXT NOT NULL DEFAULT 'fixed',
    status TEXT NOT NULL DEFAULT 'active',
    discount_type TEXT NOT NULL DEFAULT 'none',
    discount_value REAL NOT NULL DEFAULT 0,
    metadata TEXT NOT NULL DEFAULT '{}',
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (tenant_id, slug)
);

CREATE INDEX IF NOT EXISTS idx_product_bundles_lookup
    ON product_bundles (tenant_id, status, slug);

CREATE TABLE IF NOT EXISTS product_bundle_translations (
    id TEXT PRIMARY KEY,
    bundle_id TEXT NOT NULL REFERENCES product_bundles(id) ON DELETE CASCADE,
    locale TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (bundle_id, locale)
);

CREATE INDEX IF NOT EXISTS idx_product_bundle_translations_name
    ON product_bundle_translations (locale, name);

CREATE TABLE IF NOT EXISTS product_bundle_items (
    id TEXT PRIMARY KEY,
    bundle_id TEXT NOT NULL REFERENCES product_bundles(id) ON DELETE CASCADE,
    product_id TEXT NOT NULL,
    variant_id TEXT,
    quantity INTEGER NOT NULL DEFAULT 1,
    is_optional INTEGER NOT NULL DEFAULT 0,
    discount_rate REAL,
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);

CREATE INDEX IF NOT EXISTS idx_product_bundle_items_lookup
    ON product_bundle_items (bundle_id, position);

CREATE INDEX IF NOT EXISTS idx_product_bundle_items_product
    ON product_bundle_items (product_id);
"#,
            )
            .await?;
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        db.execute_unprepared(
            r#"
DROP TABLE IF EXISTS product_bundle_items;
DROP TABLE IF EXISTS product_bundle_translations;
DROP TABLE IF EXISTS product_bundles;
"#,
        )
        .await?;
        Ok(())
    }
}
