use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

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
            DatabaseBackend::Postgres => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS checkout_order_line_inventory_reserve ON order_line_items;
                        DROP TRIGGER IF EXISTS checkout_order_inventory_dispose ON orders;
                        DROP FUNCTION IF EXISTS reserve_checkout_order_line_inventory();
                        DROP FUNCTION IF EXISTS dispose_checkout_order_inventory();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS checkout_order_line_inventory_validate;
                        DROP TRIGGER IF EXISTS checkout_order_line_inventory_reserve;
                        DROP TRIGGER IF EXISTS checkout_order_inventory_commit;
                        DROP TRIGGER IF EXISTS checkout_order_inventory_release;
                        "#,
                    )
                    .await?;
            }
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
            CREATE OR REPLACE FUNCTION reserve_checkout_order_line_inventory()
            RETURNS trigger AS $$
            DECLARE
                cart_line_item_id TEXT;
                order_tenant_id UUID;
                inventory_policy VARCHAR(32);
                inventory_item_id UUID;
                selected_level_id UUID;
                selected_location_id UUID;
                selected_available INTEGER;
                reservation_external_id TEXT;
            BEGIN
                cart_line_item_id := NEW.metadata #>> '{checkout,cart_line_item_id}';
                IF cart_line_item_id IS NULL OR btrim(cart_line_item_id) = '' OR NEW.variant_id IS NULL THEN
                    RETURN NEW;
                END IF;

                IF NEW.quantity <= 0 THEN
                    RAISE EXCEPTION 'checkout inventory reservation quantity must be positive'
                        USING ERRCODE = '23514';
                END IF;

                SELECT o.tenant_id, pv.inventory_policy, ii.id
                INTO order_tenant_id, inventory_policy, inventory_item_id
                FROM orders o
                JOIN product_variants pv
                  ON pv.id = NEW.variant_id
                 AND pv.tenant_id = o.tenant_id
                LEFT JOIN inventory_items ii ON ii.variant_id = pv.id
                WHERE o.id = NEW.order_id;

                IF NOT FOUND THEN
                    RAISE EXCEPTION 'checkout order line references a variant outside the order tenant'
                        USING ERRCODE = '23514';
                END IF;

                IF inventory_item_id IS NULL THEN
                    IF lower(inventory_policy) = 'continue' THEN
                        RETURN NEW;
                    END IF;
                    RAISE EXCEPTION 'variant % has no inventory item', NEW.variant_id
                        USING ERRCODE = '23514';
                END IF;

                SELECT il.id,
                       il.location_id,
                       il.stocked_quantity - il.reserved_quantity
                INTO selected_level_id, selected_location_id, selected_available
                FROM inventory_levels il
                JOIN stock_locations sl
                  ON sl.id = il.location_id
                 AND sl.tenant_id = order_tenant_id
                 AND sl.deleted_at IS NULL
                WHERE il.inventory_item_id = inventory_item_id
                ORDER BY (il.stocked_quantity - il.reserved_quantity) DESC, il.id
                LIMIT 1
                FOR UPDATE OF il;

                IF selected_level_id IS NULL THEN
                    IF lower(inventory_policy) = 'continue' THEN
                        RETURN NEW;
                    END IF;
                    RAISE EXCEPTION 'variant % has no active inventory level', NEW.variant_id
                        USING ERRCODE = '23514';
                END IF;

                IF lower(inventory_policy) <> 'continue' AND selected_available < NEW.quantity THEN
                    RAISE EXCEPTION 'insufficient inventory for variant %: requested %, available %',
                        NEW.variant_id, NEW.quantity, selected_available
                        USING ERRCODE = '23514';
                END IF;

                reservation_external_id := 'checkout:' || cart_line_item_id;
                IF EXISTS (
                    SELECT 1
                    FROM reservation_items ri
                    WHERE ri.inventory_item_id = inventory_item_id
                      AND ri.external_id = reservation_external_id
                      AND ri.deleted_at IS NULL
                ) THEN
                    RAISE EXCEPTION 'checkout cart line % already has an active reservation', cart_line_item_id
                        USING ERRCODE = '23505';
                END IF;

                UPDATE inventory_levels
                SET reserved_quantity = reserved_quantity + NEW.quantity,
                    updated_at = CURRENT_TIMESTAMP
                WHERE id = selected_level_id;

                INSERT INTO reservation_items (
                    id,
                    inventory_item_id,
                    location_id,
                    quantity,
                    line_item_id,
                    description,
                    external_id,
                    metadata,
                    created_at,
                    updated_at,
                    deleted_at
                ) VALUES (
                    NEW.id,
                    inventory_item_id,
                    selected_location_id,
                    NEW.quantity,
                    NEW.id,
                    'Checkout order reservation',
                    reservation_external_id,
                    jsonb_build_object(
                        'source', 'checkout_order_line',
                        'order_id', NEW.order_id,
                        'cart_line_item_id', cart_line_item_id
                    ),
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP,
                    NULL
                );

                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER checkout_order_line_inventory_reserve
            AFTER INSERT ON order_line_items
            FOR EACH ROW
            EXECUTE FUNCTION reserve_checkout_order_line_inventory();

            CREATE OR REPLACE FUNCTION dispose_checkout_order_inventory()
            RETURNS trigger AS $$
            DECLARE
                reservation RECORD;
                disposition TEXT;
            BEGIN
                IF NEW.status IS NOT DISTINCT FROM OLD.status
                   OR NEW.status NOT IN ('paid', 'cancelled') THEN
                    RETURN NEW;
                END IF;

                disposition := CASE WHEN NEW.status = 'paid' THEN 'committed' ELSE 'released' END;

                FOR reservation IN
                    SELECT ri.id,
                           ri.inventory_item_id,
                           ri.location_id,
                           ri.quantity
                    FROM reservation_items ri
                    JOIN order_line_items oli ON oli.id = ri.line_item_id
                    WHERE oli.order_id = NEW.id
                      AND ri.deleted_at IS NULL
                      AND ri.external_id LIKE 'checkout:%'
                    ORDER BY ri.created_at, ri.id
                    FOR UPDATE OF ri
                LOOP
                    UPDATE inventory_levels
                    SET stocked_quantity = stocked_quantity
                            - CASE WHEN disposition = 'committed' THEN reservation.quantity ELSE 0 END,
                        reserved_quantity = reserved_quantity - reservation.quantity,
                        updated_at = CURRENT_TIMESTAMP
                    WHERE inventory_item_id = reservation.inventory_item_id
                      AND location_id = reservation.location_id;

                    UPDATE reservation_items
                    SET quantity = 0,
                        metadata = metadata || jsonb_build_object(
                            'inventory_disposition', disposition,
                            'disposed_at', CURRENT_TIMESTAMP
                        ),
                        updated_at = CURRENT_TIMESTAMP,
                        deleted_at = CURRENT_TIMESTAMP
                    WHERE id = reservation.id;
                END LOOP;

                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER checkout_order_inventory_dispose
            AFTER UPDATE OF status ON orders
            FOR EACH ROW
            WHEN (NEW.status IN ('paid', 'cancelled') AND NEW.status IS DISTINCT FROM OLD.status)
            EXECUTE FUNCTION dispose_checkout_order_inventory();
            "#,
        )
        .await?;
    Ok(())
}

async fn install_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE TRIGGER checkout_order_line_inventory_validate
            BEFORE INSERT ON order_line_items
            FOR EACH ROW
            WHEN json_extract(NEW.metadata, '$.checkout.cart_line_item_id') IS NOT NULL
              AND NEW.variant_id IS NOT NULL
            BEGIN
                SELECT CASE WHEN NEW.quantity <= 0
                    THEN RAISE(ABORT, 'checkout inventory reservation quantity must be positive') END;

                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1
                    FROM orders o
                    JOIN product_variants pv
                      ON pv.id = NEW.variant_id
                     AND pv.tenant_id = o.tenant_id
                    WHERE o.id = NEW.order_id
                ) THEN RAISE(ABORT, 'checkout variant tenant mismatch') END;

                SELECT CASE WHEN lower((
                    SELECT pv.inventory_policy
                    FROM product_variants pv
                    JOIN orders o ON o.tenant_id = pv.tenant_id
                    WHERE pv.id = NEW.variant_id AND o.id = NEW.order_id
                )) <> 'continue'
                AND NOT EXISTS (
                    SELECT 1
                    FROM inventory_items ii
                    JOIN inventory_levels il ON il.inventory_item_id = ii.id
                    JOIN stock_locations sl
                      ON sl.id = il.location_id
                     AND sl.deleted_at IS NULL
                    JOIN orders o ON o.id = NEW.order_id AND o.tenant_id = sl.tenant_id
                    WHERE ii.variant_id = NEW.variant_id
                      AND il.stocked_quantity - il.reserved_quantity >= NEW.quantity
                ) THEN RAISE(ABORT, 'insufficient checkout inventory') END;

                SELECT CASE WHEN EXISTS (
                    SELECT 1
                    FROM reservation_items ri
                    JOIN inventory_items ii ON ii.id = ri.inventory_item_id
                    WHERE ii.variant_id = NEW.variant_id
                      AND ri.external_id = 'checkout:' || json_extract(NEW.metadata, '$.checkout.cart_line_item_id')
                      AND ri.deleted_at IS NULL
                ) THEN RAISE(ABORT, 'checkout cart line already reserved') END;
            END;

            CREATE TRIGGER checkout_order_line_inventory_reserve
            AFTER INSERT ON order_line_items
            FOR EACH ROW
            WHEN json_extract(NEW.metadata, '$.checkout.cart_line_item_id') IS NOT NULL
              AND NEW.variant_id IS NOT NULL
              AND EXISTS (
                  SELECT 1 FROM inventory_items ii
                  JOIN inventory_levels il ON il.inventory_item_id = ii.id
                  JOIN stock_locations sl ON sl.id = il.location_id AND sl.deleted_at IS NULL
                  JOIN orders o ON o.id = NEW.order_id AND o.tenant_id = sl.tenant_id
                  WHERE ii.variant_id = NEW.variant_id
              )
            BEGIN
                INSERT INTO reservation_items (
                    id,
                    inventory_item_id,
                    location_id,
                    quantity,
                    line_item_id,
                    description,
                    external_id,
                    metadata,
                    created_at,
                    updated_at,
                    deleted_at
                )
                SELECT NEW.id,
                       ii.id,
                       il.location_id,
                       NEW.quantity,
                       NEW.id,
                       'Checkout order reservation',
                       'checkout:' || json_extract(NEW.metadata, '$.checkout.cart_line_item_id'),
                       json_object(
                           'source', 'checkout_order_line',
                           'order_id', NEW.order_id,
                           'cart_line_item_id', json_extract(NEW.metadata, '$.checkout.cart_line_item_id')
                       ),
                       CURRENT_TIMESTAMP,
                       CURRENT_TIMESTAMP,
                       NULL
                FROM inventory_items ii
                JOIN inventory_levels il ON il.inventory_item_id = ii.id
                JOIN stock_locations sl ON sl.id = il.location_id AND sl.deleted_at IS NULL
                JOIN orders o ON o.id = NEW.order_id AND o.tenant_id = sl.tenant_id
                WHERE ii.variant_id = NEW.variant_id
                ORDER BY (il.stocked_quantity - il.reserved_quantity) DESC, il.id
                LIMIT 1;

                UPDATE inventory_levels
                SET reserved_quantity = reserved_quantity + NEW.quantity,
                    updated_at = CURRENT_TIMESTAMP
                WHERE inventory_item_id = (
                    SELECT inventory_item_id FROM reservation_items WHERE id = NEW.id
                )
                  AND location_id = (
                    SELECT location_id FROM reservation_items WHERE id = NEW.id
                );
            END;

            CREATE TRIGGER checkout_order_inventory_commit
            AFTER UPDATE OF status ON orders
            FOR EACH ROW
            WHEN NEW.status = 'paid' AND OLD.status <> 'paid'
            BEGIN
                UPDATE inventory_levels
                SET stocked_quantity = stocked_quantity - COALESCE((
                        SELECT SUM(ri.quantity)
                        FROM reservation_items ri
                        JOIN order_line_items oli ON oli.id = ri.line_item_id
                        WHERE oli.order_id = NEW.id
                          AND ri.inventory_item_id = inventory_levels.inventory_item_id
                          AND ri.location_id = inventory_levels.location_id
                          AND ri.deleted_at IS NULL
                          AND ri.external_id LIKE 'checkout:%'
                    ), 0),
                    reserved_quantity = reserved_quantity - COALESCE((
                        SELECT SUM(ri.quantity)
                        FROM reservation_items ri
                        JOIN order_line_items oli ON oli.id = ri.line_item_id
                        WHERE oli.order_id = NEW.id
                          AND ri.inventory_item_id = inventory_levels.inventory_item_id
                          AND ri.location_id = inventory_levels.location_id
                          AND ri.deleted_at IS NULL
                          AND ri.external_id LIKE 'checkout:%'
                    ), 0),
                    updated_at = CURRENT_TIMESTAMP
                WHERE EXISTS (
                    SELECT 1
                    FROM reservation_items ri
                    JOIN order_line_items oli ON oli.id = ri.line_item_id
                    WHERE oli.order_id = NEW.id
                      AND ri.inventory_item_id = inventory_levels.inventory_item_id
                      AND ri.location_id = inventory_levels.location_id
                      AND ri.deleted_at IS NULL
                      AND ri.external_id LIKE 'checkout:%'
                );

                UPDATE reservation_items
                SET quantity = 0,
                    metadata = json_set(metadata, '$.inventory_disposition', 'committed'),
                    updated_at = CURRENT_TIMESTAMP,
                    deleted_at = CURRENT_TIMESTAMP
                WHERE deleted_at IS NULL
                  AND external_id LIKE 'checkout:%'
                  AND line_item_id IN (
                      SELECT id FROM order_line_items WHERE order_id = NEW.id
                  );
            END;

            CREATE TRIGGER checkout_order_inventory_release
            AFTER UPDATE OF status ON orders
            FOR EACH ROW
            WHEN NEW.status = 'cancelled' AND OLD.status <> 'cancelled'
            BEGIN
                UPDATE inventory_levels
                SET reserved_quantity = reserved_quantity - COALESCE((
                        SELECT SUM(ri.quantity)
                        FROM reservation_items ri
                        JOIN order_line_items oli ON oli.id = ri.line_item_id
                        WHERE oli.order_id = NEW.id
                          AND ri.inventory_item_id = inventory_levels.inventory_item_id
                          AND ri.location_id = inventory_levels.location_id
                          AND ri.deleted_at IS NULL
                          AND ri.external_id LIKE 'checkout:%'
                    ), 0),
                    updated_at = CURRENT_TIMESTAMP
                WHERE EXISTS (
                    SELECT 1
                    FROM reservation_items ri
                    JOIN order_line_items oli ON oli.id = ri.line_item_id
                    WHERE oli.order_id = NEW.id
                      AND ri.inventory_item_id = inventory_levels.inventory_item_id
                      AND ri.location_id = inventory_levels.location_id
                      AND ri.deleted_at IS NULL
                      AND ri.external_id LIKE 'checkout:%'
                );

                UPDATE reservation_items
                SET quantity = 0,
                    metadata = json_set(metadata, '$.inventory_disposition', 'released'),
                    updated_at = CURRENT_TIMESTAMP,
                    deleted_at = CURRENT_TIMESTAMP
                WHERE deleted_at IS NULL
                  AND external_id LIKE 'checkout:%'
                  AND line_item_id IN (
                      SELECT id FROM order_line_items WHERE order_id = NEW.id
                  );
            END;
            "#,
        )
        .await?;
    Ok(())
}
