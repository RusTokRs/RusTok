use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        match manager.get_database_backend() {
            // PostgreSQL already receives the two-column tenant identity keys from
            // m20260701_000002. A Bundle item that selects a Variant also needs a
            // parent key containing that Variant's Product, however, so the Bundle
            // composite FK can reject a same-tenant Variant for a different Product.
            DatabaseBackend::Postgres => {
                db.execute_unprepared(
                    r#"
DO $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_constraint
        WHERE conrelid = 'product_variants'::regclass
          AND conname = 'uq_product_variants_tenant_product_id'
    ) THEN
        ALTER TABLE product_variants
            ADD CONSTRAINT uq_product_variants_tenant_product_id
            UNIQUE (tenant_id, product_id, id);
    END IF;
END $$;
"#,
                )
                .await?;
            }
            // SQLite requires the exact referenced parent columns to have a UNIQUE
            // key before it can enforce a composite foreign key. Its portable
            // Product migration equivalent therefore creates both the tenant keys
            // and the Variant-to-Product parent key used by Bundle items.
            DatabaseBackend::Sqlite => {
                db.execute_unprepared(
                    r#"
CREATE UNIQUE INDEX IF NOT EXISTS uq_products_tenant_id
    ON products (tenant_id, id);
CREATE UNIQUE INDEX IF NOT EXISTS uq_product_variants_tenant_id
    ON product_variants (tenant_id, id);
CREATE UNIQUE INDEX IF NOT EXISTS uq_product_variants_tenant_product_id
    ON product_variants (tenant_id, product_id, id);
"#,
                )
                .await?;
            }
            _ => {}
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        let db = manager.get_connection();
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                db.execute_unprepared(
                    r#"
ALTER TABLE product_variants
    DROP CONSTRAINT IF EXISTS uq_product_variants_tenant_product_id;
"#,
                )
                .await?;
            }
            DatabaseBackend::Sqlite => {
                db.execute_unprepared(
                    r#"
DROP INDEX IF EXISTS uq_product_variants_tenant_product_id;
DROP INDEX IF EXISTS uq_product_variants_tenant_id;
DROP INDEX IF EXISTS uq_products_tenant_id;
"#,
                )
                .await?;
            }
            _ => {}
        }

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use sea_orm::{ConnectionTrait, Database};
    use sea_orm_migration::{MigrationTrait, SchemaManager};

    use super::Migration;

    #[tokio::test]
    async fn sqlite_parent_keys_support_tenant_scoped_bundle_variant_foreign_keys() {
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
    tenant_id TEXT NOT NULL
);
CREATE TABLE product_variants (
    id TEXT PRIMARY KEY,
    product_id TEXT NOT NULL,
    tenant_id TEXT NOT NULL
);
"#,
        )
        .await
        .expect("Product parent tables");

        Migration
            .up(&SchemaManager::new(&db))
            .await
            .expect("SQLite Product identity-key migration should apply");

        db.execute_unprepared(
            r#"
CREATE TABLE product_bundles (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    bundle_product_id TEXT,
    UNIQUE (tenant_id, id),
    FOREIGN KEY (tenant_id, bundle_product_id)
        REFERENCES products (tenant_id, id)
        ON DELETE CASCADE
);
CREATE TABLE product_bundle_items (
    id TEXT PRIMARY KEY,
    tenant_id TEXT NOT NULL,
    bundle_id TEXT NOT NULL,
    product_id TEXT NOT NULL,
    variant_id TEXT,
    FOREIGN KEY (tenant_id, bundle_id)
        REFERENCES product_bundles (tenant_id, id)
        ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, product_id)
        REFERENCES products (tenant_id, id)
        ON DELETE CASCADE,
    FOREIGN KEY (tenant_id, product_id, variant_id)
        REFERENCES product_variants (tenant_id, product_id, id)
        ON DELETE CASCADE
);
INSERT INTO products (id, tenant_id) VALUES
    ('owner', 'tenant-a'),
    ('component', 'tenant-a'),
    ('other-component', 'tenant-a');
INSERT INTO product_variants (id, product_id, tenant_id)
    VALUES ('component-variant', 'component', 'tenant-a');
INSERT INTO product_bundles (id, tenant_id, bundle_product_id)
    VALUES ('bundle', 'tenant-a', 'owner');
INSERT INTO product_bundle_items (id, tenant_id, bundle_id, product_id, variant_id)
    VALUES ('item', 'tenant-a', 'bundle', 'component', 'component-variant');
"#,
        )
        .await
        .expect(
            "valid tenant-scoped Bundle references should not hit a SQLite foreign-key mismatch",
        );

        assert!(
            db.execute_unprepared(concat!(
                "INSERT INTO product_bundles (id, tenant_id, bundle_product_id) ",
                "VALUES ('cross-tenant-owner', 'tenant-b', 'owner')",
            ))
            .await
            .is_err(),
            "the owner Product composite key must reject a cross-tenant reference"
        );
        assert!(
            db.execute_unprepared(concat!(
                "INSERT INTO product_bundle_items ",
                "(id, tenant_id, bundle_id, product_id, variant_id) ",
                "VALUES ('cross-tenant-item', 'tenant-a', 'bundle', 'component', 'missing-variant')",
            ))
            .await
            .is_err(),
            "the component Variant composite key must remain enforced"
        );
        assert!(
            db.execute_unprepared(concat!(
                "INSERT INTO product_bundle_items ",
                "(id, tenant_id, bundle_id, product_id, variant_id) ",
                "VALUES ('wrong-parent-item', 'tenant-a', 'bundle', 'other-component', 'component-variant')",
            ))
            .await
            .is_err(),
            "a Variant must belong to the selected component Product"
        );
    }
}
