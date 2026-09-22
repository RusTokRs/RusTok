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
                        CREATE OR REPLACE FUNCTION consume_inventory_on_fulfillment_shipping()
                        RETURNS trigger AS $$
                        DECLARE
                            shipped_delta INTEGER;
                            line_variant_id UUID;
                            reservation RECORD;
                            level_reserved INTEGER;
                            remaining_quantity INTEGER;
                        BEGIN
                            shipped_delta := NEW.shipped_quantity - OLD.shipped_quantity;
                            IF shipped_delta <= 0 THEN
                                RETURN NEW;
                            END IF;

                            SELECT variant_id
                            INTO line_variant_id
                            FROM order_line_items
                            WHERE id = NEW.order_line_item_id;

                            IF NOT FOUND THEN
                                RAISE EXCEPTION 'fulfillment item references missing order line %', NEW.order_line_item_id
                                    USING ERRCODE = '23503';
                            END IF;

                            IF line_variant_id IS NULL THEN
                                RETURN NEW;
                            END IF;

                            SELECT ri.id, ri.inventory_item_id, ri.location_id, ri.quantity
                            INTO reservation
                            FROM reservation_items ri
                            WHERE ri.line_item_id = NEW.order_line_item_id
                              AND ri.deleted_at IS NULL
                              AND ri.quantity > 0
                            ORDER BY ri.created_at, ri.id
                            LIMIT 1
                            FOR UPDATE;

                            IF NOT FOUND THEN
                                RAISE EXCEPTION 'active inventory reservation is missing for order line %', NEW.order_line_item_id
                                    USING ERRCODE = '23514';
                            END IF;

                            IF reservation.quantity < shipped_delta THEN
                                RAISE EXCEPTION
                                    'shipped quantity % exceeds remaining reservation % for order line %',
                                    shipped_delta,
                                    reservation.quantity,
                                    NEW.order_line_item_id
                                    USING ERRCODE = '23514';
                            END IF;

                            SELECT reserved_quantity
                            INTO level_reserved
                            FROM inventory_levels
                            WHERE inventory_item_id = reservation.inventory_item_id
                              AND location_id = reservation.location_id
                            FOR UPDATE;

                            IF NOT FOUND OR level_reserved < shipped_delta THEN
                                RAISE EXCEPTION 'inventory level reservation is inconsistent for order line %', NEW.order_line_item_id
                                    USING ERRCODE = '23514';
                            END IF;

                            UPDATE inventory_levels
                            SET stocked_quantity = stocked_quantity - shipped_delta,
                                reserved_quantity = reserved_quantity - shipped_delta,
                                updated_at = CURRENT_TIMESTAMP
                            WHERE inventory_item_id = reservation.inventory_item_id
                              AND location_id = reservation.location_id;

                            remaining_quantity := reservation.quantity - shipped_delta;
                            UPDATE reservation_items
                            SET quantity = remaining_quantity,
                                updated_at = CURRENT_TIMESTAMP,
                                deleted_at = CASE
                                    WHEN remaining_quantity = 0 THEN CURRENT_TIMESTAMP
                                    ELSE NULL
                                END
                            WHERE id = reservation.id;

                            RETURN NEW;
                        END;
                        $$ LANGUAGE plpgsql;

                        CREATE TRIGGER fulfillment_inventory_consume_on_ship
                        BEFORE UPDATE OF shipped_quantity ON fulfillment_items
                        FOR EACH ROW
                        WHEN (NEW.shipped_quantity > OLD.shipped_quantity)
                        EXECUTE FUNCTION consume_inventory_on_fulfillment_shipping();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        CREATE TRIGGER fulfillment_inventory_consume_on_ship
                        BEFORE UPDATE OF shipped_quantity ON fulfillment_items
                        FOR EACH ROW
                        WHEN NEW.shipped_quantity > OLD.shipped_quantity
                        BEGIN
                            SELECT CASE WHEN NOT EXISTS (
                                SELECT 1 FROM order_line_items
                                WHERE id = NEW.order_line_item_id
                            ) THEN RAISE(ABORT, 'fulfillment item references missing order line') END;

                            SELECT CASE WHEN EXISTS (
                                SELECT 1 FROM order_line_items
                                WHERE id = NEW.order_line_item_id
                                  AND variant_id IS NOT NULL
                            ) AND NOT EXISTS (
                                SELECT 1 FROM reservation_items ri
                                WHERE ri.line_item_id = NEW.order_line_item_id
                                  AND ri.deleted_at IS NULL
                                  AND ri.quantity > 0
                            ) THEN RAISE(ABORT, 'active inventory reservation is missing') END;

                            SELECT CASE WHEN EXISTS (
                                SELECT 1 FROM order_line_items
                                WHERE id = NEW.order_line_item_id
                                  AND variant_id IS NOT NULL
                            ) AND (
                                SELECT ri.quantity
                                FROM reservation_items ri
                                WHERE ri.line_item_id = NEW.order_line_item_id
                                  AND ri.deleted_at IS NULL
                                  AND ri.quantity > 0
                                ORDER BY ri.created_at, ri.id
                                LIMIT 1
                            ) < (NEW.shipped_quantity - OLD.shipped_quantity)
                            THEN RAISE(ABORT, 'shipped quantity exceeds remaining reservation') END;

                            SELECT CASE WHEN EXISTS (
                                SELECT 1 FROM order_line_items
                                WHERE id = NEW.order_line_item_id
                                  AND variant_id IS NOT NULL
                            ) AND NOT EXISTS (
                                SELECT 1
                                FROM reservation_items ri
                                JOIN inventory_levels il
                                  ON il.inventory_item_id = ri.inventory_item_id
                                 AND il.location_id = ri.location_id
                                WHERE ri.line_item_id = NEW.order_line_item_id
                                  AND ri.deleted_at IS NULL
                                  AND ri.quantity > 0
                                  AND il.reserved_quantity >= (NEW.shipped_quantity - OLD.shipped_quantity)
                            ) THEN RAISE(ABORT, 'inventory level reservation is inconsistent') END;

                            UPDATE inventory_levels
                            SET stocked_quantity = stocked_quantity - (NEW.shipped_quantity - OLD.shipped_quantity),
                                reserved_quantity = reserved_quantity - (NEW.shipped_quantity - OLD.shipped_quantity),
                                updated_at = CURRENT_TIMESTAMP
                            WHERE EXISTS (
                                SELECT 1
                                FROM reservation_items ri
                                WHERE ri.line_item_id = NEW.order_line_item_id
                                  AND ri.deleted_at IS NULL
                                  AND ri.quantity > 0
                                  AND ri.inventory_item_id = inventory_levels.inventory_item_id
                                  AND ri.location_id = inventory_levels.location_id
                            )
                              AND EXISTS (
                                  SELECT 1 FROM order_line_items
                                  WHERE id = NEW.order_line_item_id
                                    AND variant_id IS NOT NULL
                              );

                            UPDATE reservation_items
                            SET quantity = quantity - (NEW.shipped_quantity - OLD.shipped_quantity),
                                updated_at = CURRENT_TIMESTAMP,
                                deleted_at = CASE
                                    WHEN quantity - (NEW.shipped_quantity - OLD.shipped_quantity) = 0
                                    THEN CURRENT_TIMESTAMP
                                    ELSE NULL
                                END
                            WHERE id = (
                                SELECT ri.id
                                FROM reservation_items ri
                                WHERE ri.line_item_id = NEW.order_line_item_id
                                  AND ri.deleted_at IS NULL
                                  AND ri.quantity > 0
                                ORDER BY ri.created_at, ri.id
                                LIMIT 1
                            )
                              AND EXISTS (
                                  SELECT 1 FROM order_line_items
                                  WHERE id = NEW.order_line_item_id
                                    AND variant_id IS NOT NULL
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
                        DROP TRIGGER IF EXISTS fulfillment_inventory_consume_on_ship ON fulfillment_items;
                        DROP FUNCTION IF EXISTS consume_inventory_on_fulfillment_shipping();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        "DROP TRIGGER IF EXISTS fulfillment_inventory_consume_on_ship;",
                    )
                    .await?;
            }
            _ => {}
        }

        Ok(())
    }
}
