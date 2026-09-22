use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        CREATE OR REPLACE FUNCTION consume_order_inventory_reservations() RETURNS trigger AS $$
                        DECLARE
                            consumption RECORD;
                            current_reserved BIGINT;
                        BEGIN
                            FOR consumption IN
                                SELECT
                                    ri.inventory_item_id,
                                    ri.location_id,
                                    SUM(ri.quantity)::BIGINT AS quantity
                                FROM reservation_items ri
                                JOIN order_line_items oli ON oli.id = ri.line_item_id
                                WHERE oli.order_id = NEW.id
                                  AND ri.deleted_at IS NULL
                                  AND ri.quantity > 0
                                GROUP BY ri.inventory_item_id, ri.location_id
                            LOOP
                                SELECT il.reserved_quantity::BIGINT
                                INTO current_reserved
                                FROM inventory_levels il
                                WHERE il.inventory_item_id = consumption.inventory_item_id
                                  AND il.location_id = consumption.location_id
                                FOR UPDATE;

                                IF NOT FOUND OR current_reserved < consumption.quantity THEN
                                    RAISE EXCEPTION 'inventory reservation ledger is inconsistent for delivered order %', NEW.id
                                        USING ERRCODE = '23514';
                                END IF;

                                UPDATE inventory_levels
                                SET stocked_quantity = stocked_quantity - consumption.quantity::INTEGER,
                                    reserved_quantity = reserved_quantity - consumption.quantity::INTEGER,
                                    updated_at = CURRENT_TIMESTAMP
                                WHERE inventory_item_id = consumption.inventory_item_id
                                  AND location_id = consumption.location_id;
                            END LOOP;

                            UPDATE reservation_items ri
                            SET quantity = 0,
                                updated_at = CURRENT_TIMESTAMP,
                                deleted_at = CURRENT_TIMESTAMP
                            FROM order_line_items oli
                            WHERE oli.id = ri.line_item_id
                              AND oli.order_id = NEW.id
                              AND ri.deleted_at IS NULL;

                            RETURN NEW;
                        END;
                        $$ LANGUAGE plpgsql;

                        CREATE TRIGGER order_inventory_consume_on_delivery
                        BEFORE UPDATE OF status ON orders
                        FOR EACH ROW
                        WHEN (OLD.status = 'shipped' AND NEW.status = 'delivered')
                        EXECUTE FUNCTION consume_order_inventory_reservations();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        CREATE TRIGGER order_inventory_consume_on_delivery
                        BEFORE UPDATE OF status ON orders
                        FOR EACH ROW
                        WHEN OLD.status = 'shipped' AND NEW.status = 'delivered'
                        BEGIN
                            SELECT CASE WHEN EXISTS (
                                SELECT 1
                                FROM inventory_levels il
                                JOIN (
                                    SELECT
                                        ri.inventory_item_id,
                                        ri.location_id,
                                        SUM(ri.quantity) AS quantity
                                    FROM reservation_items ri
                                    JOIN order_line_items oli ON oli.id = ri.line_item_id
                                    WHERE oli.order_id = NEW.id
                                      AND ri.deleted_at IS NULL
                                      AND ri.quantity > 0
                                    GROUP BY ri.inventory_item_id, ri.location_id
                                ) consumption
                                  ON consumption.inventory_item_id = il.inventory_item_id
                                 AND consumption.location_id = il.location_id
                                WHERE il.reserved_quantity < consumption.quantity
                            ) THEN RAISE(ABORT, 'inventory reservation ledger is inconsistent') END;

                            SELECT CASE WHEN EXISTS (
                                SELECT 1
                                FROM (
                                    SELECT DISTINCT ri.inventory_item_id, ri.location_id
                                    FROM reservation_items ri
                                    JOIN order_line_items oli ON oli.id = ri.line_item_id
                                    WHERE oli.order_id = NEW.id
                                      AND ri.deleted_at IS NULL
                                      AND ri.quantity > 0
                                ) consumption
                                WHERE NOT EXISTS (
                                    SELECT 1 FROM inventory_levels il
                                    WHERE il.inventory_item_id = consumption.inventory_item_id
                                      AND il.location_id = consumption.location_id
                                )
                            ) THEN RAISE(ABORT, 'inventory level for reservation is missing') END;

                            UPDATE inventory_levels
                            SET stocked_quantity = stocked_quantity - (
                                    SELECT COALESCE(SUM(ri.quantity), 0)
                                    FROM reservation_items ri
                                    JOIN order_line_items oli ON oli.id = ri.line_item_id
                                    WHERE oli.order_id = NEW.id
                                      AND ri.inventory_item_id = inventory_levels.inventory_item_id
                                      AND ri.location_id = inventory_levels.location_id
                                      AND ri.deleted_at IS NULL
                                ),
                                reserved_quantity = reserved_quantity - (
                                    SELECT COALESCE(SUM(ri.quantity), 0)
                                    FROM reservation_items ri
                                    JOIN order_line_items oli ON oli.id = ri.line_item_id
                                    WHERE oli.order_id = NEW.id
                                      AND ri.inventory_item_id = inventory_levels.inventory_item_id
                                      AND ri.location_id = inventory_levels.location_id
                                      AND ri.deleted_at IS NULL
                                ),
                                updated_at = CURRENT_TIMESTAMP
                            WHERE EXISTS (
                                SELECT 1
                                FROM reservation_items ri
                                JOIN order_line_items oli ON oli.id = ri.line_item_id
                                WHERE oli.order_id = NEW.id
                                  AND ri.inventory_item_id = inventory_levels.inventory_item_id
                                  AND ri.location_id = inventory_levels.location_id
                                  AND ri.deleted_at IS NULL
                            );

                            UPDATE reservation_items
                            SET quantity = 0,
                                updated_at = CURRENT_TIMESTAMP,
                                deleted_at = CURRENT_TIMESTAMP
                            WHERE deleted_at IS NULL
                              AND line_item_id IN (
                                  SELECT oli.id
                                  FROM order_line_items oli
                                  WHERE oli.order_id = NEW.id
                              );
                        END;
                        "#,
                    )
                    .await?;
            }
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
                        DROP TRIGGER IF EXISTS order_inventory_consume_on_delivery ON orders;
                        DROP FUNCTION IF EXISTS consume_order_inventory_reservations();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        "DROP TRIGGER IF EXISTS order_inventory_consume_on_delivery;",
                    )
                    .await?;
            }
            _ => {}
        }

        Ok(())
    }
}
