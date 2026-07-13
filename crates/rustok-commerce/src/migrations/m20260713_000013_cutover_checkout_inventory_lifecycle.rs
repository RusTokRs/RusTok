use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

const SQLITE_LEGACY_WHEN: &str =
    "WHEN OLD.status = 'pending' AND NEW.status = 'confirmed'";
const SQLITE_SCOPED_WHEN: &str = "WHEN OLD.status = 'pending' AND NEW.status = 'confirmed' AND json_extract(NEW.metadata, '$.checkout.operation_id') IS NULL";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite(manager).await?,
            _ => {}
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => uninstall_postgres(manager).await?,
            DatabaseBackend::Sqlite => uninstall_sqlite(manager).await?,
            _ => {}
        }
        Ok(())
    }
}

async fn install_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER IF EXISTS order_inventory_reservation_guard ON orders;

            CREATE TRIGGER order_inventory_reservation_guard
            BEFORE UPDATE OF status ON orders
            FOR EACH ROW
            WHEN (
                OLD.status IS DISTINCT FROM NEW.status
                AND NEW.metadata #>> '{checkout,operation_id}' IS NULL
            )
            EXECUTE FUNCTION enforce_order_inventory_reservation();

            CREATE OR REPLACE FUNCTION enforce_checkout_order_inventory_lifecycle()
            RETURNS trigger AS $$
            DECLARE
                operation_id UUID;
                release RECORD;
                current_reserved BIGINT;
            BEGIN
                operation_id := (NEW.metadata #>> '{checkout,operation_id}')::UUID;

                IF OLD.status = 'pending' AND NEW.status = 'confirmed' THEN
                    IF NOT EXISTS (
                        SELECT 1
                        FROM checkout_operations co
                        WHERE co.id = operation_id
                          AND co.tenant_id = NEW.tenant_id
                          AND co.order_id = NEW.id
                          AND co.status = 'executing'
                          AND co.stage = 'order_created'
                    ) THEN
                        RAISE EXCEPTION
                            'checkout operation is not ready to confirm order %', NEW.id
                            USING ERRCODE = '23514';
                    END IF;

                    IF EXISTS (
                        SELECT 1
                        FROM order_line_items oli
                        WHERE oli.order_id = NEW.id
                          AND oli.variant_id IS NOT NULL
                          AND NOT EXISTS (
                              SELECT 1
                              FROM checkout_inventory_reservations cir
                              JOIN reservation_items ri
                                ON ri.id = cir.reservation_id
                              JOIN inventory_items ii
                                ON ii.id = ri.inventory_item_id
                              WHERE cir.tenant_id = NEW.tenant_id
                                AND cir.checkout_operation_id = operation_id
                                AND cir.order_line_item_id = oli.id
                                AND cir.status = 'reserved'
                                AND cir.variant_id = oli.variant_id
                                AND cir.quantity = oli.quantity
                                AND ri.line_item_id = oli.id
                                AND ri.quantity = oli.quantity
                                AND ri.deleted_at IS NULL
                                AND ii.variant_id = oli.variant_id
                          )
                    ) THEN
                        RAISE EXCEPTION
                            'checkout order % has an incomplete adopted inventory ledger', NEW.id
                            USING ERRCODE = '23514';
                    END IF;

                    IF EXISTS (
                        SELECT 1
                        FROM checkout_inventory_reservations cir
                        WHERE cir.tenant_id = NEW.tenant_id
                          AND cir.checkout_operation_id = operation_id
                          AND (
                              cir.status <> 'reserved'
                              OR cir.order_line_item_id IS NULL
                              OR NOT EXISTS (
                                  SELECT 1
                                  FROM order_line_items oli
                                  WHERE oli.id = cir.order_line_item_id
                                    AND oli.order_id = NEW.id
                                    AND oli.variant_id = cir.variant_id
                                    AND oli.quantity = cir.quantity
                              )
                          )
                    ) THEN
                        RAISE EXCEPTION
                            'checkout order % has foreign or inactive inventory reservations', NEW.id
                            USING ERRCODE = '23514';
                    END IF;
                END IF;

                IF OLD.status <> 'cancelled' AND NEW.status = 'cancelled' THEN
                    FOR release IN
                        SELECT
                            ri.inventory_item_id,
                            ri.location_id,
                            SUM(ri.quantity)::BIGINT AS release_quantity
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
                        WHERE il.inventory_item_id = release.inventory_item_id
                          AND il.location_id = release.location_id
                        FOR UPDATE;

                        IF NOT FOUND OR current_reserved < release.release_quantity THEN
                            RAISE EXCEPTION
                                'inventory reservation ledger is inconsistent for order %', NEW.id
                                USING ERRCODE = '23514';
                        END IF;

                        UPDATE inventory_levels
                        SET reserved_quantity = reserved_quantity - release.release_quantity::INTEGER,
                            updated_at = CURRENT_TIMESTAMP
                        WHERE inventory_item_id = release.inventory_item_id
                          AND location_id = release.location_id;
                    END LOOP;

                    UPDATE checkout_inventory_reservations cir
                    SET status = 'released',
                        released_at = CURRENT_TIMESTAMP,
                        updated_at = CURRENT_TIMESTAMP,
                        last_error_code = NULL,
                        last_error_message = NULL
                    WHERE cir.tenant_id = NEW.tenant_id
                      AND cir.checkout_operation_id = operation_id
                      AND cir.status = 'reserved'
                      AND cir.order_line_item_id IN (
                          SELECT oli.id
                          FROM order_line_items oli
                          WHERE oli.order_id = NEW.id
                      );

                    UPDATE reservation_items ri
                    SET quantity = 0,
                        updated_at = CURRENT_TIMESTAMP,
                        deleted_at = CURRENT_TIMESTAMP
                    FROM order_line_items oli
                    WHERE oli.id = ri.line_item_id
                      AND oli.order_id = NEW.id
                      AND ri.deleted_at IS NULL;
                END IF;

                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER checkout_order_inventory_lifecycle_guard
            BEFORE UPDATE OF status ON orders
            FOR EACH ROW
            WHEN (
                OLD.status IS DISTINCT FROM NEW.status
                AND NEW.metadata #>> '{checkout,operation_id}' IS NOT NULL
            )
            EXECUTE FUNCTION enforce_checkout_order_inventory_lifecycle();

            CREATE OR REPLACE FUNCTION sync_checkout_inventory_consumed_on_fulfillment()
            RETURNS trigger AS $$
            BEGIN
                UPDATE checkout_inventory_reservations cir
                SET status = 'consumed',
                    consumed_at = CURRENT_TIMESTAMP,
                    updated_at = CURRENT_TIMESTAMP,
                    last_error_code = NULL,
                    last_error_message = NULL
                WHERE cir.order_line_item_id = NEW.order_line_item_id
                  AND cir.status = 'reserved'
                  AND NOT EXISTS (
                      SELECT 1
                      FROM reservation_items ri
                      WHERE ri.id = cir.reservation_id
                        AND ri.deleted_at IS NULL
                        AND ri.quantity > 0
                  );
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER checkout_inventory_consumed_on_fulfillment
            AFTER UPDATE OF shipped_quantity ON fulfillment_items
            FOR EACH ROW
            WHEN (NEW.shipped_quantity > OLD.shipped_quantity)
            EXECUTE FUNCTION sync_checkout_inventory_consumed_on_fulfillment();

            CREATE OR REPLACE FUNCTION sync_checkout_inventory_consumed_on_delivery()
            RETURNS trigger AS $$
            BEGIN
                UPDATE checkout_inventory_reservations cir
                SET status = 'consumed',
                    consumed_at = CURRENT_TIMESTAMP,
                    updated_at = CURRENT_TIMESTAMP,
                    last_error_code = NULL,
                    last_error_message = NULL
                WHERE cir.status = 'reserved'
                  AND cir.order_line_item_id IN (
                      SELECT oli.id
                      FROM order_line_items oli
                      WHERE oli.order_id = NEW.id
                  )
                  AND NOT EXISTS (
                      SELECT 1
                      FROM reservation_items ri
                      WHERE ri.id = cir.reservation_id
                        AND ri.deleted_at IS NULL
                        AND ri.quantity > 0
                  );
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER checkout_inventory_consumed_on_delivery
            AFTER UPDATE OF status ON orders
            FOR EACH ROW
            WHEN (OLD.status = 'shipped' AND NEW.status = 'delivered')
            EXECUTE FUNCTION sync_checkout_inventory_consumed_on_delivery();
            "#,
        )
        .await?;
    Ok(())
}

async fn uninstall_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER IF EXISTS checkout_inventory_consumed_on_delivery ON orders;
            DROP FUNCTION IF EXISTS sync_checkout_inventory_consumed_on_delivery();
            DROP TRIGGER IF EXISTS checkout_inventory_consumed_on_fulfillment ON fulfillment_items;
            DROP FUNCTION IF EXISTS sync_checkout_inventory_consumed_on_fulfillment();
            DROP TRIGGER IF EXISTS checkout_order_inventory_lifecycle_guard ON orders;
            DROP FUNCTION IF EXISTS enforce_checkout_order_inventory_lifecycle();
            DROP TRIGGER IF EXISTS order_inventory_reservation_guard ON orders;

            CREATE TRIGGER order_inventory_reservation_guard
            BEFORE UPDATE OF status ON orders
            FOR EACH ROW
            WHEN (OLD.status IS DISTINCT FROM NEW.status)
            EXECUTE FUNCTION enforce_order_inventory_reservation();
            "#,
        )
        .await?;
    Ok(())
}

async fn install_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    scope_sqlite_legacy_reservation(manager, true).await?;
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER IF EXISTS checkout_order_inventory_confirm_guard;
            DROP TRIGGER IF EXISTS checkout_inventory_released_on_cancel;
            DROP TRIGGER IF EXISTS checkout_inventory_consumed_on_fulfillment;
            DROP TRIGGER IF EXISTS checkout_inventory_consumed_on_delivery;

            CREATE TRIGGER checkout_order_inventory_confirm_guard
            BEFORE UPDATE OF status ON orders
            FOR EACH ROW
            WHEN OLD.status = 'pending'
             AND NEW.status = 'confirmed'
             AND json_extract(NEW.metadata, '$.checkout.operation_id') IS NOT NULL
            BEGIN
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1
                    FROM checkout_operations co
                    WHERE CAST(co.id AS TEXT) = json_extract(NEW.metadata, '$.checkout.operation_id')
                      AND co.tenant_id = NEW.tenant_id
                      AND co.order_id = NEW.id
                      AND co.status = 'executing'
                      AND co.stage = 'order_created'
                ) THEN RAISE(ABORT, 'checkout operation is not ready to confirm order') END;

                SELECT CASE WHEN EXISTS (
                    SELECT 1
                    FROM order_line_items oli
                    WHERE oli.order_id = NEW.id
                      AND oli.variant_id IS NOT NULL
                      AND NOT EXISTS (
                          SELECT 1
                          FROM checkout_inventory_reservations cir
                          JOIN checkout_operations co
                            ON co.id = cir.checkout_operation_id
                          JOIN reservation_items ri
                            ON ri.id = cir.reservation_id
                          JOIN inventory_items ii
                            ON ii.id = ri.inventory_item_id
                          WHERE CAST(co.id AS TEXT) = json_extract(NEW.metadata, '$.checkout.operation_id')
                            AND cir.tenant_id = NEW.tenant_id
                            AND cir.order_line_item_id = oli.id
                            AND cir.status = 'reserved'
                            AND cir.variant_id = oli.variant_id
                            AND cir.quantity = oli.quantity
                            AND ri.line_item_id = oli.id
                            AND ri.quantity = oli.quantity
                            AND ri.deleted_at IS NULL
                            AND ii.variant_id = oli.variant_id
                      )
                ) THEN RAISE(ABORT, 'checkout order has an incomplete adopted inventory ledger') END;

                SELECT CASE WHEN EXISTS (
                    SELECT 1
                    FROM checkout_inventory_reservations cir
                    JOIN checkout_operations co
                      ON co.id = cir.checkout_operation_id
                    WHERE CAST(co.id AS TEXT) = json_extract(NEW.metadata, '$.checkout.operation_id')
                      AND cir.tenant_id = NEW.tenant_id
                      AND (
                          cir.status <> 'reserved'
                          OR cir.order_line_item_id IS NULL
                          OR NOT EXISTS (
                              SELECT 1
                              FROM order_line_items oli
                              WHERE oli.id = cir.order_line_item_id
                                AND oli.order_id = NEW.id
                                AND oli.variant_id = cir.variant_id
                                AND oli.quantity = cir.quantity
                          )
                      )
                ) THEN RAISE(ABORT, 'checkout order has foreign or inactive inventory reservations') END;
            END;

            CREATE TRIGGER checkout_inventory_released_on_cancel
            AFTER UPDATE OF status ON orders
            FOR EACH ROW
            WHEN OLD.status <> 'cancelled'
             AND NEW.status = 'cancelled'
             AND json_extract(NEW.metadata, '$.checkout.operation_id') IS NOT NULL
            BEGIN
                UPDATE checkout_inventory_reservations
                SET status = 'released',
                    released_at = CURRENT_TIMESTAMP,
                    updated_at = CURRENT_TIMESTAMP,
                    last_error_code = NULL,
                    last_error_message = NULL
                WHERE tenant_id = NEW.tenant_id
                  AND status = 'reserved'
                  AND order_line_item_id IN (
                      SELECT oli.id
                      FROM order_line_items oli
                      WHERE oli.order_id = NEW.id
                  );
            END;

            CREATE TRIGGER checkout_inventory_consumed_on_fulfillment
            AFTER UPDATE OF shipped_quantity ON fulfillment_items
            FOR EACH ROW
            WHEN NEW.shipped_quantity > OLD.shipped_quantity
            BEGIN
                UPDATE checkout_inventory_reservations
                SET status = 'consumed',
                    consumed_at = CURRENT_TIMESTAMP,
                    updated_at = CURRENT_TIMESTAMP,
                    last_error_code = NULL,
                    last_error_message = NULL
                WHERE order_line_item_id = NEW.order_line_item_id
                  AND status = 'reserved'
                  AND NOT EXISTS (
                      SELECT 1
                      FROM reservation_items ri
                      WHERE ri.id = checkout_inventory_reservations.reservation_id
                        AND ri.deleted_at IS NULL
                        AND ri.quantity > 0
                  );
            END;

            CREATE TRIGGER checkout_inventory_consumed_on_delivery
            AFTER UPDATE OF status ON orders
            FOR EACH ROW
            WHEN OLD.status = 'shipped' AND NEW.status = 'delivered'
            BEGIN
                UPDATE checkout_inventory_reservations
                SET status = 'consumed',
                    consumed_at = CURRENT_TIMESTAMP,
                    updated_at = CURRENT_TIMESTAMP,
                    last_error_code = NULL,
                    last_error_message = NULL
                WHERE status = 'reserved'
                  AND order_line_item_id IN (
                      SELECT oli.id
                      FROM order_line_items oli
                      WHERE oli.order_id = NEW.id
                  )
                  AND NOT EXISTS (
                      SELECT 1
                      FROM reservation_items ri
                      WHERE ri.id = checkout_inventory_reservations.reservation_id
                        AND ri.deleted_at IS NULL
                        AND ri.quantity > 0
                  );
            END;
            "#,
        )
        .await?;
    Ok(())
}

async fn uninstall_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER IF EXISTS checkout_order_inventory_confirm_guard;
            DROP TRIGGER IF EXISTS checkout_inventory_released_on_cancel;
            DROP TRIGGER IF EXISTS checkout_inventory_consumed_on_fulfillment;
            DROP TRIGGER IF EXISTS checkout_inventory_consumed_on_delivery;
            "#,
        )
        .await?;
    scope_sqlite_legacy_reservation(manager, false).await
}

async fn scope_sqlite_legacy_reservation(
    manager: &SchemaManager<'_>,
    checkout_aware: bool,
) -> Result<(), DbErr> {
    let row = manager
        .get_connection()
        .query_one(Statement::from_string(
            DatabaseBackend::Sqlite,
            "SELECT sql FROM sqlite_master WHERE type = 'trigger' AND name = 'order_inventory_reserve_on_confirm'"
                .to_string(),
        ))
        .await?
        .ok_or_else(|| DbErr::Custom("legacy order inventory reservation trigger is missing".to_string()))?;
    let sql: String = row.try_get("", "sql")?;
    let (from, to) = if checkout_aware {
        (SQLITE_LEGACY_WHEN, SQLITE_SCOPED_WHEN)
    } else {
        (SQLITE_SCOPED_WHEN, SQLITE_LEGACY_WHEN)
    };
    if sql.contains(to) {
        return Ok(());
    }
    if !sql.contains(from) {
        return Err(DbErr::Custom(
            "legacy order inventory reservation trigger shape changed".to_string(),
        ));
    }
    let sql = sql.replacen(from, to, 1);
    manager
        .get_connection()
        .execute_unprepared("DROP TRIGGER order_inventory_reserve_on_confirm")
        .await?;
    manager
        .get_connection()
        .execute_unprepared(sql.as_str())
        .await?;
    Ok(())
}
