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
                        ALTER TABLE inventory_levels
                            ADD CONSTRAINT ck_inventory_levels_quantities
                            CHECK (reserved_quantity >= 0 AND incoming_quantity >= 0) NOT VALID;

                        ALTER TABLE reservation_items
                            ADD CONSTRAINT ck_reservation_items_quantity
                            CHECK (quantity >= 0) NOT VALID,
                            ADD CONSTRAINT ck_reservation_items_lifecycle
                            CHECK (
                                (deleted_at IS NULL AND quantity > 0)
                                OR (deleted_at IS NOT NULL AND quantity = 0)
                            ) NOT VALID,
                            ADD CONSTRAINT ck_reservation_items_external_id
                            CHECK (external_id IS NULL OR btrim(external_id) <> '') NOT VALID;

                        CREATE OR REPLACE FUNCTION enforce_inventory_level_tenant_integrity()
                        RETURNS trigger AS $$
                        DECLARE
                            item_tenant UUID;
                            location_tenant UUID;
                        BEGIN
                            SELECT pv.tenant_id
                            INTO item_tenant
                            FROM inventory_items ii
                            JOIN product_variants pv ON pv.id = ii.variant_id
                            WHERE ii.id = NEW.inventory_item_id;

                            SELECT tenant_id
                            INTO location_tenant
                            FROM stock_locations
                            WHERE id = NEW.location_id AND deleted_at IS NULL;

                            IF item_tenant IS NULL THEN
                                RAISE EXCEPTION 'inventory item % has no tenant-scoped variant', NEW.inventory_item_id
                                    USING ERRCODE = '23503';
                            END IF;
                            IF location_tenant IS NULL THEN
                                RAISE EXCEPTION 'active stock location % does not exist', NEW.location_id
                                    USING ERRCODE = '23503';
                            END IF;
                            IF item_tenant <> location_tenant THEN
                                RAISE EXCEPTION 'inventory item and stock location belong to different tenants'
                                    USING ERRCODE = '23514';
                            END IF;

                            RETURN NEW;
                        END;
                        $$ LANGUAGE plpgsql;

                        CREATE TRIGGER inventory_levels_tenant_guard
                        BEFORE INSERT OR UPDATE OF inventory_item_id, location_id
                        ON inventory_levels
                        FOR EACH ROW
                        EXECUTE FUNCTION enforce_inventory_level_tenant_integrity();

                        CREATE OR REPLACE FUNCTION enforce_reservation_location_integrity()
                        RETURNS trigger AS $$
                        BEGIN
                            IF NOT EXISTS (
                                SELECT 1
                                FROM inventory_levels il
                                WHERE il.inventory_item_id = NEW.inventory_item_id
                                  AND il.location_id = NEW.location_id
                            ) THEN
                                RAISE EXCEPTION 'reservation item location has no inventory level'
                                    USING ERRCODE = '23514';
                            END IF;

                            RETURN NEW;
                        END;
                        $$ LANGUAGE plpgsql;

                        CREATE TRIGGER reservation_items_location_guard
                        BEFORE INSERT OR UPDATE OF inventory_item_id, location_id
                        ON reservation_items
                        FOR EACH ROW
                        EXECUTE FUNCTION enforce_reservation_location_integrity();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        CREATE TRIGGER inventory_levels_state_guard_insert
                        BEFORE INSERT ON inventory_levels
                        FOR EACH ROW
                        BEGIN
                            SELECT CASE WHEN NEW.reserved_quantity < 0 OR NEW.incoming_quantity < 0
                                THEN RAISE(ABORT, 'invalid inventory level quantities') END;
                            SELECT CASE WHEN NOT EXISTS (
                                SELECT 1
                                FROM inventory_items ii
                                JOIN product_variants pv ON pv.id = ii.variant_id
                                JOIN stock_locations sl ON sl.id = NEW.location_id
                                WHERE ii.id = NEW.inventory_item_id
                                  AND sl.deleted_at IS NULL
                                  AND pv.tenant_id = sl.tenant_id
                            ) THEN RAISE(ABORT, 'inventory item and location tenant mismatch') END;
                        END;

                        CREATE TRIGGER inventory_levels_state_guard_update
                        BEFORE UPDATE OF inventory_item_id, location_id, reserved_quantity, incoming_quantity
                        ON inventory_levels
                        FOR EACH ROW
                        BEGIN
                            SELECT CASE WHEN NEW.reserved_quantity < 0 OR NEW.incoming_quantity < 0
                                THEN RAISE(ABORT, 'invalid inventory level quantities') END;
                            SELECT CASE WHEN NOT EXISTS (
                                SELECT 1
                                FROM inventory_items ii
                                JOIN product_variants pv ON pv.id = ii.variant_id
                                JOIN stock_locations sl ON sl.id = NEW.location_id
                                WHERE ii.id = NEW.inventory_item_id
                                  AND sl.deleted_at IS NULL
                                  AND pv.tenant_id = sl.tenant_id
                            ) THEN RAISE(ABORT, 'inventory item and location tenant mismatch') END;
                        END;

                        CREATE TRIGGER reservation_items_state_guard_insert
                        BEFORE INSERT ON reservation_items
                        FOR EACH ROW
                        BEGIN
                            SELECT CASE WHEN NEW.quantity < 0
                                THEN RAISE(ABORT, 'reservation quantity must be non-negative') END;
                            SELECT CASE WHEN NOT (
                                (NEW.deleted_at IS NULL AND NEW.quantity > 0)
                                OR (NEW.deleted_at IS NOT NULL AND NEW.quantity = 0)
                            ) THEN RAISE(ABORT, 'invalid reservation lifecycle state') END;
                            SELECT CASE WHEN NEW.external_id IS NOT NULL AND trim(NEW.external_id) = ''
                                THEN RAISE(ABORT, 'reservation external_id must not be blank') END;
                            SELECT CASE WHEN NOT EXISTS (
                                SELECT 1 FROM inventory_levels il
                                WHERE il.inventory_item_id = NEW.inventory_item_id
                                  AND il.location_id = NEW.location_id
                            ) THEN RAISE(ABORT, 'reservation location has no inventory level') END;
                        END;

                        CREATE TRIGGER reservation_items_state_guard_update
                        BEFORE UPDATE OF inventory_item_id, location_id, quantity, external_id, deleted_at
                        ON reservation_items
                        FOR EACH ROW
                        BEGIN
                            SELECT CASE WHEN NEW.quantity < 0
                                THEN RAISE(ABORT, 'reservation quantity must be non-negative') END;
                            SELECT CASE WHEN NOT (
                                (NEW.deleted_at IS NULL AND NEW.quantity > 0)
                                OR (NEW.deleted_at IS NOT NULL AND NEW.quantity = 0)
                            ) THEN RAISE(ABORT, 'invalid reservation lifecycle state') END;
                            SELECT CASE WHEN NEW.external_id IS NOT NULL AND trim(NEW.external_id) = ''
                                THEN RAISE(ABORT, 'reservation external_id must not be blank') END;
                            SELECT CASE WHEN NOT EXISTS (
                                SELECT 1 FROM inventory_levels il
                                WHERE il.inventory_item_id = NEW.inventory_item_id
                                  AND il.location_id = NEW.location_id
                            ) THEN RAISE(ABORT, 'reservation location has no inventory level') END;
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
                        DROP TRIGGER IF EXISTS reservation_items_location_guard ON reservation_items;
                        DROP FUNCTION IF EXISTS enforce_reservation_location_integrity();
                        DROP TRIGGER IF EXISTS inventory_levels_tenant_guard ON inventory_levels;
                        DROP FUNCTION IF EXISTS enforce_inventory_level_tenant_integrity();
                        ALTER TABLE reservation_items
                            DROP CONSTRAINT IF EXISTS ck_reservation_items_external_id,
                            DROP CONSTRAINT IF EXISTS ck_reservation_items_lifecycle,
                            DROP CONSTRAINT IF EXISTS ck_reservation_items_quantity;
                        ALTER TABLE inventory_levels
                            DROP CONSTRAINT IF EXISTS ck_inventory_levels_quantities;
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS reservation_items_state_guard_update;
                        DROP TRIGGER IF EXISTS reservation_items_state_guard_insert;
                        DROP TRIGGER IF EXISTS inventory_levels_state_guard_update;
                        DROP TRIGGER IF EXISTS inventory_levels_state_guard_insert;
                        "#,
                    )
                    .await?;
            }
            _ => {}
        }

        Ok(())
    }
}
