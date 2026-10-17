use sea_orm::ConnectionTrait;
use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

/// Upgrades the originally unscoped Bundle tables without rewriting the historical create
/// migration. Production installations can already have recorded that migration, so the tenant
/// identity, ownership, and item-reference invariants must be introduced append-only.
#[async_trait::async_trait]
impl MigrationTrait for Migration {
    fn use_transaction(&self) -> Option<bool> {
        // SQLite table rebuilds and PostgreSQL's multi-step backfill must both be all-or-nothing.
        Some(true)
    }

    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => upgrade_postgres(manager).await,
            DatabaseBackend::Sqlite => upgrade_sqlite(manager).await,
            _ => Ok(()),
        }
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => downgrade_postgres(manager).await,
            DatabaseBackend::Sqlite => downgrade_sqlite(manager).await,
            _ => Ok(()),
        }
    }
}

async fn upgrade_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
ALTER TABLE product_bundle_items
    ADD COLUMN IF NOT EXISTS tenant_id UUID;

UPDATE product_bundle_items item
SET tenant_id = bundle.tenant_id
FROM product_bundles bundle
WHERE item.bundle_id = bundle.id
  AND item.tenant_id IS NULL;

DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM product_bundle_items item
        LEFT JOIN product_bundles bundle ON bundle.id = item.bundle_id
        WHERE item.tenant_id IS NULL
           OR bundle.id IS NULL
           OR item.tenant_id <> bundle.tenant_id
    ) THEN
        RAISE EXCEPTION
            'Product Bundle item tenant backfill found an orphan or cross-tenant Bundle reference';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM product_bundles bundle
        LEFT JOIN products owner
          ON owner.tenant_id = bundle.tenant_id
         AND owner.id = bundle.bundle_product_id
        WHERE bundle.bundle_product_id IS NOT NULL
          AND owner.id IS NULL
    ) THEN
        RAISE EXCEPTION
            'Product Bundle owner Product references must belong to the same tenant';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM product_bundle_items item
        LEFT JOIN products product
          ON product.tenant_id = item.tenant_id
         AND product.id = item.product_id
        LEFT JOIN product_variants variant
          ON variant.tenant_id = item.tenant_id
         AND variant.id = item.variant_id
        WHERE product.id IS NULL
           OR (
                item.variant_id IS NOT NULL
                AND (
                    variant.id IS NULL
                    OR variant.product_id IS DISTINCT FROM item.product_id
                )
           )
    ) THEN
        RAISE EXCEPTION
            'Product Bundle item Product and Variant references must belong to the same tenant and Product';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM product_bundles
        WHERE bundle_product_id IS NOT NULL
        GROUP BY tenant_id, bundle_product_id
        HAVING COUNT(*) > 1
    ) THEN
        RAISE EXCEPTION
            'A Product may own at most one Product Bundle per tenant';
    END IF;

    IF EXISTS (
        SELECT 1
        FROM product_bundle_items
        GROUP BY tenant_id, bundle_id, position
        HAVING COUNT(*) > 1
    ) THEN
        RAISE EXCEPTION
            'Product Bundle item positions must be unique within a tenant Bundle';
    END IF;
END $$;

ALTER TABLE product_bundle_items
    ALTER COLUMN tenant_id SET NOT NULL;

DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'product_bundles'::regclass
          AND conname = 'uq_product_bundles_tenant_id'
    ) THEN
        ALTER TABLE product_bundles
            ADD CONSTRAINT uq_product_bundles_tenant_id UNIQUE (tenant_id, id);
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'product_bundles'::regclass
          AND conname = 'fk_product_bundles_owner_product'
    ) THEN
        ALTER TABLE product_bundles
            ADD CONSTRAINT fk_product_bundles_owner_product
            FOREIGN KEY (tenant_id, bundle_product_id)
            REFERENCES products (tenant_id, id)
            ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'product_bundles'::regclass
          AND conname = 'chk_product_bundles_type'
    ) THEN
        ALTER TABLE product_bundles
            ADD CONSTRAINT chk_product_bundles_type
            CHECK (bundle_type IN ('fixed', 'flexible'));
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'product_bundles'::regclass
          AND conname = 'chk_product_bundles_status'
    ) THEN
        ALTER TABLE product_bundles
            ADD CONSTRAINT chk_product_bundles_status
            CHECK (status IN ('draft', 'active', 'archived'));
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'product_bundles'::regclass
          AND conname = 'chk_product_bundles_discount_type'
    ) THEN
        ALTER TABLE product_bundles
            ADD CONSTRAINT chk_product_bundles_discount_type
            CHECK (discount_type IN ('none', 'percentage', 'fixed_amount'));
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'product_bundles'::regclass
          AND conname = 'chk_product_bundles_discount_value'
    ) THEN
        ALTER TABLE product_bundles
            ADD CONSTRAINT chk_product_bundles_discount_value
            CHECK (
                discount_value >= 0
                AND (discount_type <> 'none' OR discount_value = 0)
                AND (discount_type <> 'percentage' OR discount_value <= 100)
            );
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'product_bundle_items'::regclass
          AND conname = 'fk_product_bundle_items_bundle_tenant'
    ) THEN
        ALTER TABLE product_bundle_items
            ADD CONSTRAINT fk_product_bundle_items_bundle_tenant
            FOREIGN KEY (tenant_id, bundle_id)
            REFERENCES product_bundles (tenant_id, id)
            ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'product_bundle_items'::regclass
          AND conname = 'fk_product_bundle_items_product_tenant'
    ) THEN
        ALTER TABLE product_bundle_items
            ADD CONSTRAINT fk_product_bundle_items_product_tenant
            FOREIGN KEY (tenant_id, product_id)
            REFERENCES products (tenant_id, id)
            ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'product_bundle_items'::regclass
          AND conname = 'fk_product_bundle_items_variant_product_tenant'
    ) THEN
        ALTER TABLE product_bundle_items
            ADD CONSTRAINT fk_product_bundle_items_variant_product_tenant
            FOREIGN KEY (tenant_id, product_id, variant_id)
            REFERENCES product_variants (tenant_id, product_id, id)
            ON DELETE CASCADE;
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'product_bundle_items'::regclass
          AND conname = 'uq_product_bundle_items_position'
    ) THEN
        ALTER TABLE product_bundle_items
            ADD CONSTRAINT uq_product_bundle_items_position
            UNIQUE (tenant_id, bundle_id, position);
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'product_bundle_items'::regclass
          AND conname = 'chk_product_bundle_items_quantity'
    ) THEN
        ALTER TABLE product_bundle_items
            ADD CONSTRAINT chk_product_bundle_items_quantity
            CHECK (quantity >= 1);
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'product_bundle_items'::regclass
          AND conname = 'chk_product_bundle_items_position'
    ) THEN
        ALTER TABLE product_bundle_items
            ADD CONSTRAINT chk_product_bundle_items_position
            CHECK (position >= 0);
    END IF;
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'product_bundle_items'::regclass
          AND conname = 'chk_product_bundle_items_discount_rate'
    ) THEN
        ALTER TABLE product_bundle_items
            ADD CONSTRAINT chk_product_bundle_items_discount_rate
            CHECK (discount_rate IS NULL OR (discount_rate >= 0 AND discount_rate <= 1));
    END IF;
END $$;

CREATE UNIQUE INDEX IF NOT EXISTS uq_product_bundles_owner_product
    ON product_bundles (tenant_id, bundle_product_id)
    WHERE bundle_product_id IS NOT NULL;

DROP INDEX IF EXISTS idx_product_bundle_items_lookup;
CREATE INDEX idx_product_bundle_items_lookup
    ON product_bundle_items (tenant_id, bundle_id, position);
DROP INDEX IF EXISTS idx_product_bundle_items_product;
CREATE INDEX idx_product_bundle_items_product
    ON product_bundle_items (tenant_id, product_id);
"#,
        )
        .await?;
    Ok(())
}

async fn upgrade_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    // SQLite cannot add the composite foreign keys and checks in place. Rebuild the Bundle tables
    // transactionally; all temporary tables are populated before any original table is dropped.
    manager
        .get_connection()
        .execute_unprepared(
            r#"
CREATE TABLE product_bundles_tenant_integrity (
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

-- A missing same-tenant owner is made NULL for the NOT NULL destination tenant_id, which fails
-- the copy instead of silently retaining an invalid cross-tenant reference when FK enforcement is
-- disabled by an embedding SQLite connection.
INSERT INTO product_bundles_tenant_integrity (
    id, tenant_id, bundle_product_id, slug, bundle_type, status,
    discount_type, discount_value, metadata, created_at, updated_at
)
SELECT
    bundle.id,
    CASE
        WHEN bundle.bundle_product_id IS NULL OR owner.id IS NOT NULL THEN bundle.tenant_id
        ELSE NULL
    END,
    bundle.bundle_product_id,
    bundle.slug,
    bundle.bundle_type,
    bundle.status,
    bundle.discount_type,
    bundle.discount_value,
    bundle.metadata,
    bundle.created_at,
    bundle.updated_at
FROM product_bundles bundle
LEFT JOIN products owner
  ON owner.tenant_id = bundle.tenant_id
 AND owner.id = bundle.bundle_product_id;

-- Validate owner uniqueness before destructive replacement. The temporary name lets this coexist
-- with a legacy index of the canonical name until the old table is dropped.
CREATE UNIQUE INDEX uq_product_bundles_owner_product_tenant_integrity
    ON product_bundles_tenant_integrity (tenant_id, bundle_product_id)
    WHERE bundle_product_id IS NOT NULL;

CREATE TABLE product_bundle_translations_tenant_integrity (
    id TEXT PRIMARY KEY,
    bundle_id TEXT NOT NULL
        REFERENCES product_bundles_tenant_integrity(id)
        ON DELETE CASCADE,
    locale TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (bundle_id, locale)
);

-- Preserve orphan detection even if a host has disabled SQLite foreign-key enforcement.
INSERT INTO product_bundle_translations_tenant_integrity (
    id, bundle_id, locale, name, description, created_at, updated_at
)
SELECT
    translation.id,
    CASE WHEN bundle.id IS NULL THEN NULL ELSE translation.bundle_id END,
    translation.locale,
    translation.name,
    translation.description,
    translation.created_at,
    translation.updated_at
FROM product_bundle_translations translation
LEFT JOIN product_bundles_tenant_integrity bundle ON bundle.id = translation.bundle_id;

CREATE TABLE product_bundle_items_tenant_integrity (
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
        REFERENCES product_bundles_tenant_integrity (tenant_id, id)
        ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, product_id)
        REFERENCES products (tenant_id, id)
        ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, product_id, variant_id)
        REFERENCES product_variants (tenant_id, product_id, id)
        ON DELETE CASCADE,
    UNIQUE (tenant_id, bundle_id, position),
    CHECK (quantity >= 1),
    CHECK (position >= 0),
    CHECK (discount_rate IS NULL OR (discount_rate >= 0 AND discount_rate <= 1))
);

-- The tenant is derived from the Bundle parent. Any orphan, cross-tenant Product or Variant, or
-- Variant selected for the wrong Product turns that mandatory value NULL and fails the copy before
-- old data is touched.
INSERT INTO product_bundle_items_tenant_integrity (
    id, tenant_id, bundle_id, product_id, variant_id,
    quantity, is_optional, discount_rate, position, created_at
)
SELECT
    item.id,
    CASE
        WHEN bundle.id IS NULL
          OR product.id IS NULL
          OR (item.variant_id IS NOT NULL AND variant.id IS NULL)
        THEN NULL
        ELSE bundle.tenant_id
    END,
    item.bundle_id,
    item.product_id,
    item.variant_id,
    item.quantity,
    item.is_optional,
    item.discount_rate,
    item.position,
    item.created_at
FROM product_bundle_items item
LEFT JOIN product_bundles_tenant_integrity bundle ON bundle.id = item.bundle_id
LEFT JOIN products product
  ON product.tenant_id = bundle.tenant_id
 AND product.id = item.product_id
LEFT JOIN product_variants variant
  ON variant.tenant_id = bundle.tenant_id
 AND variant.product_id = item.product_id
 AND variant.id = item.variant_id;

DROP TABLE product_bundle_items;
DROP TABLE product_bundle_translations;
DROP TABLE product_bundles;

ALTER TABLE product_bundles_tenant_integrity RENAME TO product_bundles;
ALTER TABLE product_bundle_translations_tenant_integrity RENAME TO product_bundle_translations;
ALTER TABLE product_bundle_items_tenant_integrity RENAME TO product_bundle_items;

DROP INDEX uq_product_bundles_owner_product_tenant_integrity;
CREATE UNIQUE INDEX uq_product_bundles_owner_product
    ON product_bundles (tenant_id, bundle_product_id)
    WHERE bundle_product_id IS NOT NULL;
CREATE INDEX idx_product_bundles_lookup
    ON product_bundles (tenant_id, status, slug);
CREATE INDEX idx_product_bundle_translations_name
    ON product_bundle_translations (locale, name);
CREATE INDEX idx_product_bundle_items_lookup
    ON product_bundle_items (tenant_id, bundle_id, position);
CREATE INDEX idx_product_bundle_items_product
    ON product_bundle_items (tenant_id, product_id);
"#,
        )
        .await?;
    Ok(())
}

async fn downgrade_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
ALTER TABLE product_bundle_items
    DROP CONSTRAINT IF EXISTS fk_product_bundle_items_bundle_tenant,
    DROP CONSTRAINT IF EXISTS fk_product_bundle_items_product_tenant,
    DROP CONSTRAINT IF EXISTS fk_product_bundle_items_variant_product_tenant,
    DROP CONSTRAINT IF EXISTS uq_product_bundle_items_position,
    DROP CONSTRAINT IF EXISTS chk_product_bundle_items_quantity,
    DROP CONSTRAINT IF EXISTS chk_product_bundle_items_position,
    DROP CONSTRAINT IF EXISTS chk_product_bundle_items_discount_rate;

ALTER TABLE product_bundles
    DROP CONSTRAINT IF EXISTS fk_product_bundles_owner_product,
    DROP CONSTRAINT IF EXISTS uq_product_bundles_tenant_id,
    DROP CONSTRAINT IF EXISTS chk_product_bundles_type,
    DROP CONSTRAINT IF EXISTS chk_product_bundles_status,
    DROP CONSTRAINT IF EXISTS chk_product_bundles_discount_type,
    DROP CONSTRAINT IF EXISTS chk_product_bundles_discount_value;

DROP INDEX IF EXISTS uq_product_bundles_owner_product;
DROP INDEX IF EXISTS idx_product_bundle_items_lookup;
DROP INDEX IF EXISTS idx_product_bundle_items_product;

ALTER TABLE product_bundle_items
    DROP COLUMN IF EXISTS tenant_id;

CREATE INDEX IF NOT EXISTS idx_product_bundle_items_lookup
    ON product_bundle_items (bundle_id, position);
CREATE INDEX IF NOT EXISTS idx_product_bundle_items_product
    ON product_bundle_items (product_id);
"#,
        )
        .await?;
    Ok(())
}

async fn downgrade_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
CREATE TABLE product_bundles_legacy_schema (
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
INSERT INTO product_bundles_legacy_schema (
    id, tenant_id, bundle_product_id, slug, bundle_type, status,
    discount_type, discount_value, metadata, created_at, updated_at
)
SELECT
    id, tenant_id, bundle_product_id, slug, bundle_type, status,
    discount_type, discount_value, metadata, created_at, updated_at
FROM product_bundles;

CREATE TABLE product_bundle_translations_legacy_schema (
    id TEXT PRIMARY KEY,
    bundle_id TEXT NOT NULL
        REFERENCES product_bundles_legacy_schema(id)
        ON DELETE CASCADE,
    locale TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (bundle_id, locale)
);
INSERT INTO product_bundle_translations_legacy_schema (
    id, bundle_id, locale, name, description, created_at, updated_at
)
SELECT id, bundle_id, locale, name, description, created_at, updated_at
FROM product_bundle_translations;

CREATE TABLE product_bundle_items_legacy_schema (
    id TEXT PRIMARY KEY,
    bundle_id TEXT NOT NULL
        REFERENCES product_bundles_legacy_schema(id)
        ON DELETE CASCADE,
    product_id TEXT NOT NULL,
    variant_id TEXT,
    quantity INTEGER NOT NULL DEFAULT 1,
    is_optional INTEGER NOT NULL DEFAULT 0,
    discount_rate REAL,
    position INTEGER NOT NULL DEFAULT 0,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
INSERT INTO product_bundle_items_legacy_schema (
    id, bundle_id, product_id, variant_id,
    quantity, is_optional, discount_rate, position, created_at
)
SELECT
    id, bundle_id, product_id, variant_id,
    quantity, is_optional, discount_rate, position, created_at
FROM product_bundle_items;

DROP TABLE product_bundle_items;
DROP TABLE product_bundle_translations;
DROP TABLE product_bundles;

ALTER TABLE product_bundles_legacy_schema RENAME TO product_bundles;
ALTER TABLE product_bundle_translations_legacy_schema RENAME TO product_bundle_translations;
ALTER TABLE product_bundle_items_legacy_schema RENAME TO product_bundle_items;

CREATE INDEX idx_product_bundles_lookup
    ON product_bundles (tenant_id, status, slug);
CREATE INDEX idx_product_bundle_translations_name
    ON product_bundle_translations (locale, name);
CREATE INDEX idx_product_bundle_items_lookup
    ON product_bundle_items (bundle_id, position);
CREATE INDEX idx_product_bundle_items_product
    ON product_bundle_items (product_id);
"#,
        )
        .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, Database, TryGetable};
    use sea_orm_migration::{MigrationTrait, SchemaManager};

    use super::Migration;

    #[tokio::test]
    async fn sqlite_upgrade_backfills_item_tenant_and_enforces_composite_references() {
        let db = Database::connect("sqlite::memory:")
            .await
            .expect("SQLite database");
        db.execute_unprepared("PRAGMA foreign_keys = ON;")
            .await
            .expect("SQLite foreign-key enforcement");
        db.execute_unprepared(
            r#"
CREATE TABLE products (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    UNIQUE (tenant_id, id)
);
CREATE TABLE product_variants (
    id TEXT PRIMARY KEY,
    product_id TEXT NOT NULL,
    tenant_id TEXT NOT NULL,
    UNIQUE (tenant_id, id),
    UNIQUE (tenant_id, product_id, id),
    FOREIGN KEY (tenant_id, product_id)
        REFERENCES products (tenant_id, id)
        ON DELETE CASCADE
);
CREATE TABLE product_bundles (
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
CREATE TABLE product_bundle_translations (
    id TEXT PRIMARY KEY,
    bundle_id TEXT NOT NULL REFERENCES product_bundles(id) ON DELETE CASCADE,
    locale TEXT NOT NULL,
    name TEXT NOT NULL,
    description TEXT,
    created_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
    UNIQUE (bundle_id, locale)
);
CREATE TABLE product_bundle_items (
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
INSERT INTO products (id, tenant_id) VALUES
    ('owner', 'tenant-a'),
    ('component', 'tenant-a'),
    ('other-component', 'tenant-a');
INSERT INTO product_variants (id, product_id, tenant_id)
    VALUES ('variant', 'component', 'tenant-a');
INSERT INTO product_bundles (
    id, tenant_id, bundle_product_id, slug, bundle_type, status, discount_type,
    discount_value, metadata, created_at, updated_at
) VALUES ('bundle', 'tenant-a', 'owner', 'starter', 'fixed', 'active', 'none', 0, '{}', 'now', 'now');
INSERT INTO product_bundle_translations (id, bundle_id, locale, name, created_at, updated_at)
    VALUES ('translation', 'bundle', 'en', 'Starter', 'now', 'now');
INSERT INTO product_bundle_items (
    id, bundle_id, product_id, variant_id, quantity, is_optional, position, created_at
) VALUES ('item', 'bundle', 'component', 'variant', 1, 0, 0, 'now');
"#,
        )
        .await
        .expect("legacy Bundle schema");

        Migration
            .up(&SchemaManager::new(&db))
            .await
            .expect("SQLite Bundle upgrade should apply");

        let tenant_id: String = db
            .query_one(sea_orm::Statement::from_string(
                sea_orm::DatabaseBackend::Sqlite,
                "SELECT tenant_id FROM product_bundle_items WHERE id = 'item'".to_owned(),
            ))
            .await
            .expect("query upgraded item")
            .expect("upgraded item")
            .try_get("", "tenant_id")
            .expect("upgraded item tenant");
        assert_eq!(tenant_id, "tenant-a");

        assert!(
            db.execute_unprepared(
                "INSERT INTO product_bundle_items (id, tenant_id, bundle_id, product_id, quantity, is_optional, position, created_at) VALUES ('cross-tenant', 'tenant-b', 'bundle', 'component', 1, 0, 1, 'now')",
            )
            .await
            .is_err(),
            "the upgraded composite Bundle key must reject cross-tenant items"
        );
        assert!(
            db.execute_unprepared(
                "INSERT INTO product_bundles (id, tenant_id, bundle_product_id, slug, bundle_type, status, discount_type, discount_value, metadata, created_at, updated_at) VALUES ('second', 'tenant-a', 'owner', 'second', 'fixed', 'active', 'none', 0, '{}', 'now', 'now')",
            )
            .await
            .is_err(),
            "the upgraded owner uniqueness invariant must reject a second owner Bundle"
        );
        assert!(
            db.execute_unprepared(
                "INSERT INTO product_bundle_items (id, tenant_id, bundle_id, product_id, variant_id, quantity, is_optional, position, created_at) VALUES ('wrong-product-variant', 'tenant-a', 'bundle', 'other-component', 'variant', 1, 0, 1, 'now')",
            )
            .await
            .is_err(),
            "the upgraded Variant key must reject a Variant selected for another Product"
        );

        Migration
            .down(&SchemaManager::new(&db))
            .await
            .expect("SQLite Bundle upgrade should support one-step rollback");
        assert!(
            !SchemaManager::new(&db)
                .has_column("product_bundle_items", "tenant_id")
                .await
                .expect("legacy item schema inspection"),
            "rollback should restore the legacy item shape"
        );
        Migration
            .up(&SchemaManager::new(&db))
            .await
            .expect("SQLite Bundle upgrade should reapply after rollback");
    }
}
