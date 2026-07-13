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
                        DROP TRIGGER IF EXISTS order_inventory_reservation_guard ON orders;
                        DROP TRIGGER IF EXISTS order_line_items_inventory_immutability ON order_line_items;
                        DROP FUNCTION IF EXISTS enforce_order_inventory_reservation();
                        DROP FUNCTION IF EXISTS enforce_order_line_item_inventory_immutability();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS order_inventory_reserve_on_confirm;
                        DROP TRIGGER IF EXISTS order_inventory_release_on_cancel;
                        DROP TRIGGER IF EXISTS order_line_items_inventory_immutable_insert;
                        DROP TRIGGER IF EXISTS order_line_items_inventory_immutable_update;
                        DROP TRIGGER IF EXISTS order_line_items_inventory_immutable_delete;
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
            CREATE OR REPLACE FUNCTION enforce_order_inventory_reservation() RETURNS trigger AS $$
            DECLARE
                demand RECORD;
                line RECORD;
                release RECORD;
                inventory_item_id UUID;
                selected_level_id UUID;
                selected_location_id UUID;
                selected_available BIGINT;
                current_reserved BIGINT;
                default_location_id UUID;
            BEGIN
                IF OLD.status = 'pending' AND NEW.status = 'confirmed' THEN
                    IF EXISTS (
                        SELECT 1
                        FROM order_line_items oli
                        LEFT JOIN product_variants pv ON pv.id = oli.variant_id
                        WHERE oli.order_id = NEW.id
                          AND oli.variant_id IS NOT NULL
                          AND (pv.id IS NULL OR pv.tenant_id <> NEW.tenant_id)
                    ) THEN
                        RAISE EXCEPTION 'order contains a missing or cross-tenant variant'
                            USING ERRCODE = '23514';
                    END IF;

                    FOR demand IN
                        SELECT
                            oli.variant_id,
                            SUM(oli.quantity)::BIGINT AS required_quantity,
                            pv.inventory_policy,
                            pv.sku
                        FROM order_line_items oli
                        JOIN product_variants pv ON pv.id = oli.variant_id
                        WHERE oli.order_id = NEW.id
                          AND oli.variant_id IS NOT NULL
                          AND pv.tenant_id = NEW.tenant_id
                        GROUP BY oli.variant_id, pv.inventory_policy, pv.sku
                    LOOP
                        IF demand.required_quantity <= 0
                           OR demand.required_quantity > 2147483647 THEN
                            RAISE EXCEPTION 'order inventory demand is outside the supported integer range'
                                USING ERRCODE = '23514';
                        END IF;

                        SELECT ii.id
                        INTO inventory_item_id
                        FROM inventory_items ii
                        WHERE ii.variant_id = demand.variant_id
                        FOR UPDATE;

                        IF NOT FOUND THEN
                            IF lower(demand.inventory_policy) <> 'continue' THEN
                                RAISE EXCEPTION 'variant % has no inventory state', demand.variant_id
                                    USING ERRCODE = '23514';
                            END IF;

                            INSERT INTO inventory_items (
                                id,
                                variant_id,
                                sku,
                                requires_shipping,
                                metadata,
                                created_at,
                                updated_at
                            )
                            VALUES (
                                demand.variant_id,
                                demand.variant_id,
                                demand.sku,
                                TRUE,
                                '{"source":"order_confirmation"}'::jsonb,
                                CURRENT_TIMESTAMP,
                                CURRENT_TIMESTAMP
                            )
                            ON CONFLICT (variant_id) DO NOTHING;

                            SELECT ii.id
                            INTO inventory_item_id
                            FROM inventory_items ii
                            WHERE ii.variant_id = demand.variant_id
                            FOR UPDATE;
                        END IF;

                        SELECT
                            il.id,
                            il.location_id,
                            (il.stocked_quantity - il.reserved_quantity)::BIGINT
                        INTO selected_level_id, selected_location_id, selected_available
                        FROM inventory_levels il
                        JOIN stock_locations sl ON sl.id = il.location_id
                        WHERE il.inventory_item_id = inventory_item_id
                          AND sl.tenant_id = NEW.tenant_id
                          AND sl.deleted_at IS NULL
                        ORDER BY (il.stocked_quantity - il.reserved_quantity) DESC, il.id
                        LIMIT 1
                        FOR UPDATE OF il;

                        IF NOT FOUND THEN
                            IF lower(demand.inventory_policy) <> 'continue' THEN
                                RAISE EXCEPTION 'variant % has no inventory level', demand.variant_id
                                    USING ERRCODE = '23514';
                            END IF;

                            SELECT sl.id
                            INTO default_location_id
                            FROM stock_locations sl
                            WHERE sl.tenant_id = NEW.tenant_id
                              AND sl.deleted_at IS NULL
                            ORDER BY sl.created_at, sl.id
                            LIMIT 1
                            FOR UPDATE;

                            IF NOT FOUND THEN
                                default_location_id := NEW.tenant_id;
                                INSERT INTO stock_locations (
                                    id,
                                    tenant_id,
                                    code,
                                    metadata,
                                    created_at,
                                    updated_at,
                                    deleted_at
                                )
                                VALUES (
                                    default_location_id,
                                    NEW.tenant_id,
                                    'default',
                                    '{"source":"order_confirmation"}'::jsonb,
                                    CURRENT_TIMESTAMP,
                                    CURRENT_TIMESTAMP,
                                    NULL
                                )
                                ON CONFLICT (id) DO NOTHING;

                                INSERT INTO stock_location_translations (
                                    id,
                                    stock_location_id,
                                    locale,
                                    name
                                )
                                VALUES (
                                    default_location_id,
                                    default_location_id,
                                    'en',
                                    'Default'
                                )
                                ON CONFLICT DO NOTHING;
                            END IF;

                            INSERT INTO inventory_levels (
                                id,
                                inventory_item_id,
                                location_id,
                                stocked_quantity,
                                reserved_quantity,
                                incoming_quantity,
                                low_stock_threshold,
                                updated_at
                            )
                            VALUES (
                                inventory_item_id,
                                inventory_item_id,
                                default_location_id,
                                0,
                                0,
                                0,
                                NULL,
                                CURRENT_TIMESTAMP
                            )
                            ON CONFLICT (inventory_item_id, location_id) DO NOTHING;

                            SELECT
                                il.id,
                                il.location_id,
                                (il.stocked_quantity - il.reserved_quantity)::BIGINT
                            INTO selected_level_id, selected_location_id, selected_available
                            FROM inventory_levels il
                            WHERE il.inventory_item_id = inventory_item_id
                              AND il.location_id = default_location_id
                            FOR UPDATE;
                        END IF;

                        IF lower(demand.inventory_policy) <> 'continue'
                           AND selected_available < demand.required_quantity THEN
                            RAISE EXCEPTION
                                'insufficient inventory for variant %: requested %, available %',
                                demand.variant_id,
                                demand.required_quantity,
                                selected_available
                                USING ERRCODE = '23514';
                        END IF;

                        FOR line IN
                            SELECT oli.id, oli.quantity
                            FROM order_line_items oli
                            WHERE oli.order_id = NEW.id
                              AND oli.variant_id = demand.variant_id
                            ORDER BY oli.id
                        LOOP
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
                            VALUES (
                                line.id,
                                inventory_item_id,
                                selected_location_id,
                                line.quantity,
                                line.id,
                                'Order inventory reservation',
                                'oli:' || line.id::text,
                                jsonb_build_object(
                                    'source', 'order_confirmation',
                                    'order_id', NEW.id,
                                    'order_line_item_id', line.id
                                ),
                                CURRENT_TIMESTAMP,
                                CURRENT_TIMESTAMP,
                                NULL
                            );
                        END LOOP;

                        UPDATE inventory_levels
                        SET reserved_quantity = reserved_quantity + demand.required_quantity::INTEGER,
                            updated_at = CURRENT_TIMESTAMP
                        WHERE id = selected_level_id;
                    END LOOP;
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
                            RAISE EXCEPTION 'inventory reservation ledger is inconsistent for order %', NEW.id
                                USING ERRCODE = '23514';
                        END IF;

                        UPDATE inventory_levels
                        SET reserved_quantity = reserved_quantity - release.release_quantity::INTEGER,
                            updated_at = CURRENT_TIMESTAMP
                        WHERE inventory_item_id = release.inventory_item_id
                          AND location_id = release.location_id;
                    END LOOP;

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

            CREATE TRIGGER order_inventory_reservation_guard
            BEFORE UPDATE OF status ON orders
            FOR EACH ROW
            WHEN (OLD.status IS DISTINCT FROM NEW.status)
            EXECUTE FUNCTION enforce_order_inventory_reservation();

            CREATE OR REPLACE FUNCTION enforce_order_line_item_inventory_immutability()
            RETURNS trigger AS $$
            DECLARE
                parent_order_id UUID;
                parent_status VARCHAR(32);
            BEGIN
                parent_order_id := COALESCE(NEW.order_id, OLD.order_id);
                SELECT status INTO parent_status FROM orders WHERE id = parent_order_id;

                IF FOUND AND parent_status <> 'pending' THEN
                    RAISE EXCEPTION 'order line items are immutable after order confirmation'
                        USING ERRCODE = '23514';
                END IF;

                RETURN COALESCE(NEW, OLD);
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER order_line_items_inventory_immutability
            BEFORE INSERT OR UPDATE OR DELETE ON order_line_items
            FOR EACH ROW
            EXECUTE FUNCTION enforce_order_line_item_inventory_immutability();
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
            CREATE TRIGGER order_inventory_reserve_on_confirm
            BEFORE UPDATE OF status ON orders
            FOR EACH ROW
            WHEN OLD.status = 'pending' AND NEW.status = 'confirmed'
            BEGIN
                SELECT CASE WHEN EXISTS (
                    SELECT 1
                    FROM order_line_items oli
                    LEFT JOIN product_variants pv ON pv.id = oli.variant_id
                    WHERE oli.order_id = NEW.id
                      AND oli.variant_id IS NOT NULL
                      AND (pv.id IS NULL OR pv.tenant_id <> NEW.tenant_id)
                ) THEN RAISE(ABORT, 'order contains a missing or cross-tenant variant') END;

                SELECT CASE WHEN EXISTS (
                    SELECT 1
                    FROM (
                        SELECT
                            oli.variant_id AS variant_id,
                            SUM(oli.quantity) AS required_quantity,
                            lower(pv.inventory_policy) AS inventory_policy
                        FROM order_line_items oli
                        JOIN product_variants pv ON pv.id = oli.variant_id
                        WHERE oli.order_id = NEW.id
                          AND oli.variant_id IS NOT NULL
                          AND pv.tenant_id = NEW.tenant_id
                        GROUP BY oli.variant_id, lower(pv.inventory_policy)
                    ) demand
                    LEFT JOIN inventory_items ii ON ii.variant_id = demand.variant_id
                    WHERE demand.required_quantity <= 0
                       OR demand.required_quantity > 2147483647
                       OR (
                            demand.inventory_policy <> 'continue'
                            AND COALESCE((
                                SELECT MAX(il.stocked_quantity - il.reserved_quantity)
                                FROM inventory_levels il
                                JOIN stock_locations sl ON sl.id = il.location_id
                                WHERE il.inventory_item_id = ii.id
                                  AND sl.tenant_id = NEW.tenant_id
                                  AND sl.deleted_at IS NULL
                            ), 0) < demand.required_quantity
                       )
                ) THEN RAISE(ABORT, 'insufficient or invalid order inventory demand') END;

                INSERT OR IGNORE INTO inventory_items (
                    id,
                    variant_id,
                    sku,
                    requires_shipping,
                    metadata,
                    created_at,
                    updated_at
                )
                SELECT
                    pv.id,
                    pv.id,
                    pv.sku,
                    1,
                    '{"source":"order_confirmation"}',
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP
                FROM product_variants pv
                WHERE pv.tenant_id = NEW.tenant_id
                  AND lower(pv.inventory_policy) = 'continue'
                  AND pv.id IN (
                      SELECT oli.variant_id
                      FROM order_line_items oli
                      WHERE oli.order_id = NEW.id
                        AND oli.variant_id IS NOT NULL
                  );

                INSERT OR IGNORE INTO stock_locations (
                    id,
                    tenant_id,
                    code,
                    metadata,
                    created_at,
                    updated_at,
                    deleted_at
                )
                SELECT
                    NEW.tenant_id,
                    NEW.tenant_id,
                    'default',
                    '{"source":"order_confirmation"}',
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP,
                    NULL
                WHERE EXISTS (
                    SELECT 1
                    FROM order_line_items oli
                    JOIN product_variants pv ON pv.id = oli.variant_id
                    JOIN inventory_items ii ON ii.variant_id = pv.id
                    WHERE oli.order_id = NEW.id
                      AND lower(pv.inventory_policy) = 'continue'
                      AND NOT EXISTS (
                          SELECT 1 FROM inventory_levels il
                          WHERE il.inventory_item_id = ii.id
                      )
                )
                  AND NOT EXISTS (
                      SELECT 1 FROM stock_locations sl
                      WHERE sl.tenant_id = NEW.tenant_id
                        AND sl.deleted_at IS NULL
                  );

                INSERT OR IGNORE INTO stock_location_translations (
                    id,
                    stock_location_id,
                    locale,
                    name
                )
                SELECT NEW.tenant_id, NEW.tenant_id, 'en', 'Default'
                WHERE EXISTS (
                    SELECT 1 FROM stock_locations sl
                    WHERE sl.id = NEW.tenant_id
                      AND sl.tenant_id = NEW.tenant_id
                );

                INSERT OR IGNORE INTO inventory_levels (
                    id,
                    inventory_item_id,
                    location_id,
                    stocked_quantity,
                    reserved_quantity,
                    incoming_quantity,
                    low_stock_threshold,
                    updated_at
                )
                SELECT
                    ii.id,
                    ii.id,
                    (
                        SELECT sl.id
                        FROM stock_locations sl
                        WHERE sl.tenant_id = NEW.tenant_id
                          AND sl.deleted_at IS NULL
                        ORDER BY sl.created_at, sl.id
                        LIMIT 1
                    ),
                    0,
                    0,
                    0,
                    NULL,
                    CURRENT_TIMESTAMP
                FROM order_line_items oli
                JOIN product_variants pv ON pv.id = oli.variant_id
                JOIN inventory_items ii ON ii.variant_id = pv.id
                WHERE oli.order_id = NEW.id
                  AND lower(pv.inventory_policy) = 'continue'
                  AND NOT EXISTS (
                      SELECT 1 FROM inventory_levels il
                      WHERE il.inventory_item_id = ii.id
                  )
                GROUP BY ii.id;

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
                SELECT
                    oli.id,
                    ii.id,
                    (
                        SELECT il.location_id
                        FROM inventory_levels il
                        JOIN stock_locations sl ON sl.id = il.location_id
                        WHERE il.inventory_item_id = ii.id
                          AND sl.tenant_id = NEW.tenant_id
                          AND sl.deleted_at IS NULL
                        ORDER BY (il.stocked_quantity - il.reserved_quantity) DESC, il.id
                        LIMIT 1
                    ),
                    oli.quantity,
                    oli.id,
                    'Order inventory reservation',
                    'oli:' || CAST(oli.id AS TEXT),
                    '{"source":"order_confirmation"}',
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP,
                    NULL
                FROM order_line_items oli
                JOIN inventory_items ii ON ii.variant_id = oli.variant_id
                WHERE oli.order_id = NEW.id
                  AND oli.variant_id IS NOT NULL;

                UPDATE inventory_levels
                SET reserved_quantity = reserved_quantity + (
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
            END;

            CREATE TRIGGER order_inventory_release_on_cancel
            BEFORE UPDATE OF status ON orders
            FOR EACH ROW
            WHEN OLD.status <> 'cancelled' AND NEW.status = 'cancelled'
            BEGIN
                SELECT CASE WHEN EXISTS (
                    SELECT 1
                    FROM inventory_levels il
                    JOIN (
                        SELECT
                            ri.inventory_item_id,
                            ri.location_id,
                            SUM(ri.quantity) AS release_quantity
                        FROM reservation_items ri
                        JOIN order_line_items oli ON oli.id = ri.line_item_id
                        WHERE oli.order_id = NEW.id
                          AND ri.deleted_at IS NULL
                          AND ri.quantity > 0
                        GROUP BY ri.inventory_item_id, ri.location_id
                    ) release
                      ON release.inventory_item_id = il.inventory_item_id
                     AND release.location_id = il.location_id
                    WHERE il.reserved_quantity < release.release_quantity
                ) THEN RAISE(ABORT, 'inventory reservation ledger is inconsistent') END;

                UPDATE inventory_levels
                SET reserved_quantity = reserved_quantity - (
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

            CREATE TRIGGER order_line_items_inventory_immutable_insert
            BEFORE INSERT ON order_line_items
            FOR EACH ROW
            WHEN COALESCE((SELECT status FROM orders WHERE id = NEW.order_id), 'pending') <> 'pending'
            BEGIN
                SELECT RAISE(ABORT, 'order line items are immutable after order confirmation');
            END;

            CREATE TRIGGER order_line_items_inventory_immutable_update
            BEFORE UPDATE ON order_line_items
            FOR EACH ROW
            WHEN COALESCE((SELECT status FROM orders WHERE id = OLD.order_id), 'pending') <> 'pending'
            BEGIN
                SELECT RAISE(ABORT, 'order line items are immutable after order confirmation');
            END;

            CREATE TRIGGER order_line_items_inventory_immutable_delete
            BEFORE DELETE ON order_line_items
            FOR EACH ROW
            WHEN COALESCE((SELECT status FROM orders WHERE id = OLD.order_id), 'pending') <> 'pending'
            BEGIN
                SELECT RAISE(ABORT, 'order line items are immutable after order confirmation');
            END;
            "#,
        )
        .await?;

    Ok(())
}
