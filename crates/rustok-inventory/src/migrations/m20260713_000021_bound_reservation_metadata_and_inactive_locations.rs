use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

const MAX_RESERVATION_METADATA_BYTES: usize = 32 * 1024;

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
                        DROP TRIGGER IF EXISTS inventory_level_inactive_location_guard ON inventory_levels;
                        DROP TRIGGER IF EXISTS stock_location_deactivation_guard ON stock_locations;
                        DROP FUNCTION IF EXISTS enforce_inventory_level_active_location();
                        DROP FUNCTION IF EXISTS enforce_stock_location_deactivation();
                        ALTER TABLE reservation_items
                            DROP CONSTRAINT IF EXISTS ck_reservation_items_metadata_size;
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS reservation_metadata_size_update_guard;
                        DROP TRIGGER IF EXISTS reservation_metadata_size_insert_guard;
                        DROP TRIGGER IF EXISTS inventory_level_inactive_location_update_guard;
                        DROP TRIGGER IF EXISTS inventory_level_inactive_location_insert_guard;
                        DROP TRIGGER IF EXISTS stock_location_deactivation_guard;
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
        .execute_unprepared(&format!(
            r#"
            DO $$
            BEGIN
                IF EXISTS (
                    SELECT 1
                    FROM stock_locations sl
                    JOIN inventory_levels il ON il.location_id = sl.id
                    WHERE sl.deleted_at IS NOT NULL
                      AND (
                          il.stocked_quantity <> 0
                          OR il.reserved_quantity <> 0
                          OR il.incoming_quantity <> 0
                      )
                ) THEN
                    RAISE EXCEPTION 'deleted stock locations contain non-zero inventory levels'
                        USING ERRCODE = '23514';
                END IF;
            END;
            $$;

            ALTER TABLE reservation_items
                ADD CONSTRAINT ck_reservation_items_metadata_size
                CHECK (octet_length(metadata::text) <= {MAX_RESERVATION_METADATA_BYTES}) NOT VALID;

            CREATE OR REPLACE FUNCTION enforce_stock_location_deactivation()
            RETURNS trigger AS $$
            BEGIN
                IF OLD.deleted_at IS NULL AND NEW.deleted_at IS NOT NULL AND EXISTS (
                    SELECT 1
                    FROM inventory_levels il
                    WHERE il.location_id = NEW.id
                      AND (
                          il.stocked_quantity <> 0
                          OR il.reserved_quantity <> 0
                          OR il.incoming_quantity <> 0
                      )
                ) THEN
                    RAISE EXCEPTION 'stock location cannot be deleted while inventory quantities remain'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER stock_location_deactivation_guard
            BEFORE UPDATE OF deleted_at ON stock_locations
            FOR EACH ROW
            EXECUTE FUNCTION enforce_stock_location_deactivation();

            CREATE OR REPLACE FUNCTION enforce_inventory_level_active_location()
            RETURNS trigger AS $$
            DECLARE
                location_deleted_at TIMESTAMPTZ;
            BEGIN
                SELECT deleted_at INTO location_deleted_at
                FROM stock_locations
                WHERE id = NEW.location_id;

                IF NOT FOUND THEN
                    RAISE EXCEPTION 'inventory level references a missing stock location'
                        USING ERRCODE = '23503';
                END IF;
                IF location_deleted_at IS NOT NULL AND (
                    NEW.stocked_quantity <> 0
                    OR NEW.reserved_quantity <> 0
                    OR NEW.incoming_quantity <> 0
                ) THEN
                    RAISE EXCEPTION 'deleted stock locations cannot carry inventory quantities'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER inventory_level_inactive_location_guard
            BEFORE INSERT OR UPDATE ON inventory_levels
            FOR EACH ROW
            EXECUTE FUNCTION enforce_inventory_level_active_location();
            "#,
        ))
        .await?;
    Ok(())
}

async fn install_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(&format!(
            r#"
            DROP TABLE IF EXISTS inactive_inventory_location_validation;
            CREATE TEMP TABLE inactive_inventory_location_validation (
                valid INTEGER NOT NULL CHECK (valid = 1)
            );

            INSERT INTO inactive_inventory_location_validation (valid)
            SELECT CASE WHEN EXISTS (
                SELECT 1
                FROM stock_locations sl
                JOIN inventory_levels il ON il.location_id = sl.id
                WHERE sl.deleted_at IS NOT NULL
                  AND (
                      il.stocked_quantity <> 0
                      OR il.reserved_quantity <> 0
                      OR il.incoming_quantity <> 0
                  )
            ) THEN 0 ELSE 1 END;

            DROP TABLE inactive_inventory_location_validation;

            CREATE TRIGGER stock_location_deactivation_guard
            BEFORE UPDATE OF deleted_at ON stock_locations
            FOR EACH ROW
            WHEN OLD.deleted_at IS NULL AND NEW.deleted_at IS NOT NULL
            BEGIN
                SELECT CASE WHEN EXISTS (
                    SELECT 1
                    FROM inventory_levels il
                    WHERE il.location_id = NEW.id
                      AND (
                          il.stocked_quantity <> 0
                          OR il.reserved_quantity <> 0
                          OR il.incoming_quantity <> 0
                      )
                ) THEN RAISE(ABORT, 'stock location cannot be deleted while inventory quantities remain') END;
            END;

            CREATE TRIGGER inventory_level_inactive_location_insert_guard
            BEFORE INSERT ON inventory_levels
            FOR EACH ROW
            WHEN (
                NEW.stocked_quantity <> 0
                OR NEW.reserved_quantity <> 0
                OR NEW.incoming_quantity <> 0
            )
            BEGIN
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1
                    FROM stock_locations sl
                    WHERE sl.id = NEW.location_id AND sl.deleted_at IS NULL
                ) THEN RAISE(ABORT, 'active inventory quantities require an active stock location') END;
            END;

            CREATE TRIGGER inventory_level_inactive_location_update_guard
            BEFORE UPDATE OF location_id, stocked_quantity, reserved_quantity, incoming_quantity
            ON inventory_levels
            FOR EACH ROW
            WHEN (
                NEW.stocked_quantity <> 0
                OR NEW.reserved_quantity <> 0
                OR NEW.incoming_quantity <> 0
            )
            BEGIN
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1
                    FROM stock_locations sl
                    WHERE sl.id = NEW.location_id AND sl.deleted_at IS NULL
                ) THEN RAISE(ABORT, 'active inventory quantities require an active stock location') END;
            END;

            CREATE TRIGGER reservation_metadata_size_insert_guard
            BEFORE INSERT ON reservation_items
            FOR EACH ROW
            WHEN length(CAST(NEW.metadata AS TEXT)) > {MAX_RESERVATION_METADATA_BYTES}
            BEGIN
                SELECT RAISE(ABORT, 'reservation metadata exceeds the supported size');
            END;

            CREATE TRIGGER reservation_metadata_size_update_guard
            BEFORE UPDATE OF metadata ON reservation_items
            FOR EACH ROW
            WHEN length(CAST(NEW.metadata AS TEXT)) > {MAX_RESERVATION_METADATA_BYTES}
            BEGIN
                SELECT RAISE(ABORT, 'reservation metadata exceeds the supported size');
            END;
            "#,
        ))
        .await?;
    Ok(())
}
