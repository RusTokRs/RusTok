use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => repair_postgres(manager).await?,
            DatabaseBackend::Sqlite => repair_sqlite(manager).await?,
            _ => {}
        }
        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        // This migration removes a duplicate reservation path and reconciles
        // quantities that may already have been applied twice. Re-enabling the
        // superseded triggers would restore the defect, so the repair is
        // intentionally irreversible.
        Ok(())
    }
}

async fn repair_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER IF EXISTS checkout_order_line_inventory_reserve ON order_line_items;
            DROP TRIGGER IF EXISTS checkout_order_inventory_dispose ON orders;
            DROP FUNCTION IF EXISTS reserve_checkout_order_line_inventory();
            DROP FUNCTION IF EXISTS dispose_checkout_order_inventory();

            DO $$
            DECLARE
                repair RECORD;
            BEGIN
                FOR repair IN
                    SELECT
                        ri.inventory_item_id,
                        ri.location_id,
                        SUM(oli.quantity)::BIGINT AS quantity
                    FROM reservation_items ri
                    JOIN order_line_items oli ON oli.id = ri.line_item_id
                    WHERE ri.external_id LIKE 'checkout:%'
                      AND ri.deleted_at IS NOT NULL
                      AND ri.metadata ->> 'inventory_disposition' = 'committed'
                    GROUP BY ri.inventory_item_id, ri.location_id
                LOOP
                    IF repair.quantity <= 0 OR repair.quantity > 2147483647 THEN
                        RAISE EXCEPTION 'checkout inventory stock repair is outside the supported integer range'
                            USING ERRCODE = '23514';
                    END IF;

                    UPDATE inventory_levels
                    SET stocked_quantity = stocked_quantity + repair.quantity::INTEGER,
                        updated_at = CURRENT_TIMESTAMP
                    WHERE inventory_item_id = repair.inventory_item_id
                      AND location_id = repair.location_id;

                    IF NOT FOUND THEN
                        RAISE EXCEPTION 'inventory level is missing for committed checkout reservation repair'
                            USING ERRCODE = '23514';
                    END IF;
                END LOOP;
            END;
            $$;

            DO $$
            DECLARE
                release RECORD;
                current_reserved BIGINT;
            BEGIN
                FOR release IN
                    SELECT
                        inventory_item_id,
                        location_id,
                        SUM(quantity)::BIGINT AS quantity
                    FROM reservation_items
                    WHERE external_id LIKE 'checkout:%'
                      AND deleted_at IS NULL
                      AND quantity > 0
                    GROUP BY inventory_item_id, location_id
                LOOP
                    IF release.quantity <= 0 OR release.quantity > 2147483647 THEN
                        RAISE EXCEPTION 'checkout reservation repair is outside the supported integer range'
                            USING ERRCODE = '23514';
                    END IF;

                    SELECT reserved_quantity::BIGINT
                    INTO current_reserved
                    FROM inventory_levels
                    WHERE inventory_item_id = release.inventory_item_id
                      AND location_id = release.location_id
                    FOR UPDATE;

                    IF NOT FOUND OR current_reserved < release.quantity THEN
                        RAISE EXCEPTION 'inventory reservation ledger is inconsistent during checkout reservation repair'
                            USING ERRCODE = '23514';
                    END IF;

                    UPDATE inventory_levels
                    SET reserved_quantity = reserved_quantity - release.quantity::INTEGER,
                        updated_at = CURRENT_TIMESTAMP
                    WHERE inventory_item_id = release.inventory_item_id
                      AND location_id = release.location_id;
                END LOOP;

                UPDATE reservation_items
                SET quantity = 0,
                    metadata = metadata || jsonb_build_object(
                        'inventory_disposition', 'superseded',
                        'superseded_by', 'order_confirmation_reservation',
                        'superseded_at', CURRENT_TIMESTAMP
                    ),
                    updated_at = CURRENT_TIMESTAMP,
                    deleted_at = CURRENT_TIMESTAMP
                WHERE external_id LIKE 'checkout:%'
                  AND deleted_at IS NULL;
            END;
            $$;
            "#,
        )
        .await?;

    Ok(())
}

async fn repair_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER IF EXISTS checkout_order_line_inventory_validate;
            DROP TRIGGER IF EXISTS checkout_order_line_inventory_reserve;
            DROP TRIGGER IF EXISTS checkout_order_inventory_commit;
            DROP TRIGGER IF EXISTS checkout_order_inventory_release;

            UPDATE inventory_levels
            SET stocked_quantity = stocked_quantity + (
                    SELECT COALESCE(SUM(oli.quantity), 0)
                    FROM reservation_items ri
                    JOIN order_line_items oli ON oli.id = ri.line_item_id
                    WHERE ri.inventory_item_id = inventory_levels.inventory_item_id
                      AND ri.location_id = inventory_levels.location_id
                      AND ri.external_id LIKE 'checkout:%'
                      AND ri.deleted_at IS NOT NULL
                      AND json_extract(ri.metadata, '$.inventory_disposition') = 'committed'
                ),
                updated_at = CURRENT_TIMESTAMP
            WHERE EXISTS (
                SELECT 1
                FROM reservation_items ri
                JOIN order_line_items oli ON oli.id = ri.line_item_id
                WHERE ri.inventory_item_id = inventory_levels.inventory_item_id
                  AND ri.location_id = inventory_levels.location_id
                  AND ri.external_id LIKE 'checkout:%'
                  AND ri.deleted_at IS NOT NULL
                  AND json_extract(ri.metadata, '$.inventory_disposition') = 'committed'
            );

            CREATE TEMP TABLE checkout_reservation_repair_guard (
                valid INTEGER NOT NULL CHECK (valid = 1)
            );

            INSERT INTO checkout_reservation_repair_guard (valid)
            SELECT CASE WHEN EXISTS (
                SELECT 1
                FROM (
                    SELECT inventory_item_id, location_id, SUM(quantity) AS quantity
                    FROM reservation_items
                    WHERE external_id LIKE 'checkout:%'
                      AND deleted_at IS NULL
                      AND quantity > 0
                    GROUP BY inventory_item_id, location_id
                ) release
                LEFT JOIN inventory_levels il
                  ON il.inventory_item_id = release.inventory_item_id
                 AND il.location_id = release.location_id
                WHERE il.id IS NULL OR il.reserved_quantity < release.quantity
            ) THEN 0 ELSE 1 END;

            DROP TABLE checkout_reservation_repair_guard;

            UPDATE inventory_levels
            SET reserved_quantity = reserved_quantity - (
                    SELECT COALESCE(SUM(ri.quantity), 0)
                    FROM reservation_items ri
                    WHERE ri.inventory_item_id = inventory_levels.inventory_item_id
                      AND ri.location_id = inventory_levels.location_id
                      AND ri.external_id LIKE 'checkout:%'
                      AND ri.deleted_at IS NULL
                      AND ri.quantity > 0
                ),
                updated_at = CURRENT_TIMESTAMP
            WHERE EXISTS (
                SELECT 1
                FROM reservation_items ri
                WHERE ri.inventory_item_id = inventory_levels.inventory_item_id
                  AND ri.location_id = inventory_levels.location_id
                  AND ri.external_id LIKE 'checkout:%'
                  AND ri.deleted_at IS NULL
                  AND ri.quantity > 0
            );

            UPDATE reservation_items
            SET quantity = 0,
                metadata = json_set(
                    metadata,
                    '$.inventory_disposition', 'superseded',
                    '$.superseded_by', 'order_confirmation_reservation',
                    '$.superseded_at', CURRENT_TIMESTAMP
                ),
                updated_at = CURRENT_TIMESTAMP,
                deleted_at = CURRENT_TIMESTAMP
            WHERE external_id LIKE 'checkout:%'
              AND deleted_at IS NULL;
            "#,
        )
        .await?;

    Ok(())
}
