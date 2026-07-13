use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres_guard(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite_guard(manager).await?,
            _ => {}
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS checkout_inventory_reservations_quantity_guard
                            ON checkout_inventory_reservations;
                        DROP FUNCTION IF EXISTS enforce_checkout_inventory_reservation_quantity();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS checkout_inventory_reservations_quantity_guard_insert;
                        DROP TRIGGER IF EXISTS checkout_inventory_reservations_quantity_guard_update;
                        "#,
                    )
                    .await?;
            }
            _ => {}
        }
        Ok(())
    }
}

async fn install_postgres_guard(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE OR REPLACE FUNCTION enforce_checkout_inventory_reservation_quantity()
            RETURNS trigger AS $$
            BEGIN
                IF NOT EXISTS (
                    SELECT 1
                    FROM cart_line_items
                    WHERE id = NEW.cart_line_item_id
                      AND quantity = NEW.quantity
                ) THEN
                    RAISE EXCEPTION 'checkout inventory reservation quantity mismatch'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER checkout_inventory_reservations_quantity_guard
            BEFORE INSERT OR UPDATE OF cart_line_item_id, quantity
            ON checkout_inventory_reservations
            FOR EACH ROW
            EXECUTE FUNCTION enforce_checkout_inventory_reservation_quantity();
            "#,
        )
        .await?;
    Ok(())
}

async fn install_sqlite_guard(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE TRIGGER checkout_inventory_reservations_quantity_guard_insert
            BEFORE INSERT ON checkout_inventory_reservations
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1
                    FROM cart_line_items
                    WHERE id = NEW.cart_line_item_id
                      AND quantity = NEW.quantity
                ) THEN RAISE(ABORT, 'checkout inventory reservation quantity mismatch') END;
            END;

            CREATE TRIGGER checkout_inventory_reservations_quantity_guard_update
            BEFORE UPDATE OF cart_line_item_id, quantity ON checkout_inventory_reservations
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1
                    FROM cart_line_items
                    WHERE id = NEW.cart_line_item_id
                      AND quantity = NEW.quantity
                ) THEN RAISE(ABORT, 'checkout inventory reservation quantity mismatch') END;
            END;
            "#,
        )
        .await?;
    Ok(())
}
