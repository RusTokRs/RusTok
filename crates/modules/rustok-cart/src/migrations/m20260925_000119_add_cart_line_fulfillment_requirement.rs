use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres(manager).await?,
            DatabaseBackend::MySql => install_mysql(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite(manager).await?,
            backend => return Err(DbErr::Custom(format!("unsupported database backend: {backend:?}"))),
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => manager.get_connection().execute_unprepared(
                "ALTER TABLE cart_line_items DROP CONSTRAINT IF EXISTS ck_cart_line_items_fulfillment_shipping, DROP CONSTRAINT IF EXISTS ck_cart_line_items_fulfillment_requirement, DROP COLUMN IF EXISTS fulfillment_requirement;",
            ).await?,
            DatabaseBackend::MySql => manager.get_connection().execute_unprepared(
                "ALTER TABLE cart_line_items DROP CONSTRAINT ck_cart_line_items_fulfillment_shipping, DROP CONSTRAINT ck_cart_line_items_fulfillment_requirement, DROP COLUMN fulfillment_requirement;",
            ).await?,
            DatabaseBackend::Sqlite => manager.get_connection().execute_unprepared(
                "DROP TRIGGER IF EXISTS cart_line_items_fulfillment_update_guard; DROP TRIGGER IF EXISTS cart_line_items_fulfillment_insert_guard;",
            ).await?,
            backend => return Err(DbErr::Custom(format!("unsupported database backend: {backend:?}"))),
        }
        Ok(())
    }
}

async fn install_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(r#"
        ALTER TABLE cart_line_items
            ADD COLUMN IF NOT EXISTS fulfillment_requirement VARCHAR(16) NOT NULL DEFAULT 'physical';
        ALTER TABLE cart_line_items
            ADD CONSTRAINT ck_cart_line_items_fulfillment_requirement
                CHECK (fulfillment_requirement IN ('digital', 'physical')),
            ADD CONSTRAINT ck_cart_line_items_fulfillment_shipping
                CHECK (
                    (fulfillment_requirement = 'digital' AND shipping_profile_slug = '')
                    OR
                    (fulfillment_requirement = 'physical' AND btrim(shipping_profile_slug) <> '')
                );
    "#).await
}

async fn install_mysql(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(r#"
        ALTER TABLE cart_line_items
            ADD COLUMN fulfillment_requirement VARCHAR(16) NOT NULL DEFAULT 'physical';
        ALTER TABLE cart_line_items
            ADD CONSTRAINT ck_cart_line_items_fulfillment_requirement
                CHECK (fulfillment_requirement IN ('digital', 'physical')),
            ADD CONSTRAINT ck_cart_line_items_fulfillment_shipping
                CHECK (
                    (fulfillment_requirement = 'digital' AND shipping_profile_slug = '')
                    OR
                    (fulfillment_requirement = 'physical' AND TRIM(shipping_profile_slug) <> '')
                );
    "#).await
}

async fn install_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager.get_connection().execute_unprepared(r#"
        ALTER TABLE cart_line_items ADD COLUMN fulfillment_requirement TEXT NOT NULL DEFAULT 'physical';
        CREATE TRIGGER cart_line_items_fulfillment_insert_guard
        BEFORE INSERT ON cart_line_items FOR EACH ROW BEGIN
            SELECT CASE WHEN NEW.fulfillment_requirement NOT IN ('digital', 'physical')
                THEN RAISE(ABORT, 'invalid cart fulfillment requirement') END;
            SELECT CASE WHEN NOT (
                (NEW.fulfillment_requirement = 'digital' AND NEW.shipping_profile_slug = '')
                OR (NEW.fulfillment_requirement = 'physical' AND trim(NEW.shipping_profile_slug) <> '')
            ) THEN RAISE(ABORT, 'invalid cart fulfillment shipping profile state') END;
        END;
        CREATE TRIGGER cart_line_items_fulfillment_update_guard
        BEFORE UPDATE OF fulfillment_requirement, shipping_profile_slug
        ON cart_line_items FOR EACH ROW BEGIN
            SELECT CASE WHEN NEW.fulfillment_requirement NOT IN ('digital', 'physical')
                THEN RAISE(ABORT, 'invalid cart fulfillment requirement') END;
            SELECT CASE WHEN NOT (
                (NEW.fulfillment_requirement = 'digital' AND NEW.shipping_profile_slug = '')
                OR (NEW.fulfillment_requirement = 'physical' AND trim(NEW.shipping_profile_slug) <> '')
            ) THEN RAISE(ABORT, 'invalid cart fulfillment shipping profile state') END;
        END;
    "#).await
}
