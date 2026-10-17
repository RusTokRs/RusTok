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
    CONSTRAINT uq_product_bundles_tenant_slug UNIQUE (tenant_id, slug),
    CONSTRAINT uq_product_bundles_tenant_id UNIQUE (tenant_id, id),
    CONSTRAINT fk_product_bundles_owner_product
        FOREIGN KEY (tenant_id, bundle_product_id)
        REFERENCES products (tenant_id, id)
        ON DELETE CASCADE,
    CONSTRAINT chk_product_bundles_type
        CHECK (bundle_type IN ('fixed', 'flexible')),
    CONSTRAINT chk_product_bundles_status
        CHECK (status IN ('draft', 'active', 'archived')),
    CONSTRAINT chk_product_bundles_discount_type
        CHECK (discount_type IN ('none', 'percentage', 'fixed_amount')),
    CONSTRAINT chk_product_bundles_discount_value
        CHECK (
            discount_value >= 0
            AND (discount_type <> 'none' OR discount_value = 0)
            AND (discount_type <> 'percentage' OR discount_value <= 100)
        )
);

-- A Product editor addresses the one bundle owned by its bundle_product_id. Component membership
-- is stored exclusively in product_bundle_items and deliberately has no uniqueness constraint.
CREATE UNIQUE INDEX IF NOT EXISTS uq_product_bundles_owner_product
    ON product_bundles (tenant_id, bundle_product_id)
    WHERE bundle_product_id IS NOT NULL;

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
    tenant_id UUID NOT NULL,
    bundle_id UUID NOT NULL,
    product_id UUID NOT NULL,
    variant_id UUID,
    quantity INT NOT NULL DEFAULT 1,
    is_optional BOOLEAN NOT NULL DEFAULT FALSE,
    discount_rate DECIMAL(5, 4),
    position INT NOT NULL DEFAULT 0,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    CONSTRAINT fk_product_bundle_items_bundle_tenant
        FOREIGN KEY (tenant_id, bundle_id)
        REFERENCES product_bundles (tenant_id, id)
        ON DELETE CASCADE,
    CONSTRAINT fk_product_bundle_items_product_tenant
        FOREIGN KEY (tenant_id, product_id)
        REFERENCES products (tenant_id, id)
        ON DELETE CASCADE,
    CONSTRAINT fk_product_bundle_items_variant_tenant
        FOREIGN KEY (tenant_id, variant_id)
        REFERENCES product_variants (tenant_id, id)
        ON DELETE CASCADE,
    CONSTRAINT uq_product_bundle_items_position
        UNIQUE (tenant_id, bundle_id, position),
    CONSTRAINT chk_product_bundle_items_quantity
        CHECK (quantity >= 1),
    CONSTRAINT chk_product_bundle_items_position
        CHECK (position >= 0),
    CONSTRAINT chk_product_bundle_items_discount_rate
        CHECK (discount_rate IS NULL OR (discount_rate >= 0 AND discount_rate <= 1))
);

CREATE INDEX IF NOT EXISTS idx_product_bundle_items_lookup
    ON product_bundle_items (tenant_id, bundle_id, position);

CREATE INDEX IF NOT EXISTS idx_product_bundle_items_product
    ON product_bundle_items (tenant_id, product_id);
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
    UNIQUE (tenant_id, slug),
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, bundle_product_id)
        REFERENCES products (tenant_id, id)
        ON DELETE CASCADE,
    CHECK (bundle_type IN ('fixed', 'flexible')),
    CHECK (status IN ('draft', 'active', 'archived')),
    CHECK (discount_type IN ('none', 'percentage', 'fixed_amount')),
    CHECK (
        discount_value >= 0
        AND (discount_type <> 'none' OR discount_value = 0)
        AND (discount_type <> 'percentage' OR discount_value <= 100)
    )
);

CREATE UNIQUE INDEX IF NOT EXISTS uq_product_bundles_owner_product
    ON product_bundles (tenant_id, bundle_product_id)
    WHERE bundle_product_id IS NOT NULL;

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
    tenant_id TEXT NOT NULL,
    bundle_id TEXT NOT NULL,
    product_id TEXT NOT NULL,
    variant_id TEXT,
    quantity INTEGER NOT NULL DEFAULT 1,
    is_optional INTEGER NOT NULL DEFAULT 0,
    discount_rate REAL,
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    FOREIGN KEY (tenant_id, bundle_id)
        REFERENCES product_bundles (tenant_id, id)
        ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, product_id)
        REFERENCES products (tenant_id, id)
        ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, variant_id)
        REFERENCES product_variants (tenant_id, id)
        ON DELETE CASCADE,
    UNIQUE (tenant_id, bundle_id, position),
    CHECK (quantity >= 1),
    CHECK (position >= 0),
    CHECK (discount_rate IS NULL OR (discount_rate >= 0 AND discount_rate <= 1))
);

CREATE INDEX IF NOT EXISTS idx_product_bundle_items_lookup
    ON product_bundle_items (tenant_id, bundle_id, position);

CREATE INDEX IF NOT EXISTS idx_product_bundle_items_product
    ON product_bundle_items (tenant_id, product_id);
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
