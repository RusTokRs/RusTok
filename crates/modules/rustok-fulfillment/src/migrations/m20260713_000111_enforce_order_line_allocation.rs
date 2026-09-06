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
                        DROP TRIGGER IF EXISTS fulfillment_reopen_allocation_guard ON fulfillments;
                        DROP TRIGGER IF EXISTS fulfillment_items_allocation_guard ON fulfillment_items;
                        DROP FUNCTION IF EXISTS enforce_fulfillment_reopen_allocation();
                        DROP FUNCTION IF EXISTS enforce_fulfillment_item_allocation();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS fulfillment_reopen_allocation_guard;
                        DROP TRIGGER IF EXISTS fulfillment_items_allocation_guard_delete;
                        DROP TRIGGER IF EXISTS fulfillment_items_allocation_guard_update;
                        DROP TRIGGER IF EXISTS fulfillment_items_allocation_guard_insert;
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
            CREATE OR REPLACE FUNCTION enforce_fulfillment_item_allocation() RETURNS trigger AS $$
            DECLARE
                order_line_quantity BIGINT;
                parent_status VARCHAR(32);
                committed_quantity BIGINT;
                current_commitment BIGINT;
            BEGIN
                IF TG_OP = 'DELETE' THEN
                    IF OLD.shipped_quantity > 0 THEN
                        RAISE EXCEPTION 'shipped fulfillment items cannot be deleted'
                            USING ERRCODE = '23514';
                    END IF;
                    RETURN OLD;
                END IF;

                SELECT quantity::BIGINT
                INTO order_line_quantity
                FROM order_line_items
                WHERE id = NEW.order_line_item_id;
                IF NOT FOUND THEN
                    RAISE EXCEPTION 'order line % does not exist', NEW.order_line_item_id
                        USING ERRCODE = '23503';
                END IF;

                SELECT status
                INTO parent_status
                FROM fulfillments
                WHERE id = NEW.fulfillment_id;
                IF NOT FOUND THEN
                    RAISE EXCEPTION 'fulfillment % does not exist', NEW.fulfillment_id
                        USING ERRCODE = '23503';
                END IF;

                SELECT COALESCE(SUM(
                    CASE
                        WHEN f.status = 'cancelled' THEN fi.shipped_quantity
                        ELSE fi.quantity
                    END
                ), 0)::BIGINT
                INTO committed_quantity
                FROM fulfillment_items fi
                JOIN fulfillments f ON f.id = fi.fulfillment_id
                WHERE fi.order_line_item_id = NEW.order_line_item_id
                  AND fi.id <> NEW.id;

                current_commitment := CASE
                    WHEN parent_status = 'cancelled' THEN NEW.shipped_quantity
                    ELSE NEW.quantity
                END;

                IF committed_quantity + current_commitment > order_line_quantity THEN
                    RAISE EXCEPTION
                        'fulfillment allocation % exceeds order line quantity % for line %',
                        committed_quantity + current_commitment,
                        order_line_quantity,
                        NEW.order_line_item_id
                        USING ERRCODE = '23514';
                END IF;

                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER fulfillment_items_allocation_guard
            BEFORE INSERT OR UPDATE OR DELETE ON fulfillment_items
            FOR EACH ROW
            EXECUTE FUNCTION enforce_fulfillment_item_allocation();

            CREATE OR REPLACE FUNCTION enforce_fulfillment_reopen_allocation() RETURNS trigger AS $$
            DECLARE
                item RECORD;
                other_commitment BIGINT;
                order_line_quantity BIGINT;
            BEGIN
                IF OLD.status = 'cancelled' AND NEW.status IN ('pending', 'shipped') THEN
                    FOR item IN
                        SELECT id, order_line_item_id, quantity
                        FROM fulfillment_items
                        WHERE fulfillment_id = NEW.id
                    LOOP
                        SELECT quantity::BIGINT
                        INTO order_line_quantity
                        FROM order_line_items
                        WHERE id = item.order_line_item_id;
                        IF NOT FOUND THEN
                            RAISE EXCEPTION 'order line % does not exist', item.order_line_item_id
                                USING ERRCODE = '23503';
                        END IF;

                        SELECT COALESCE(SUM(
                            CASE
                                WHEN f.status = 'cancelled' THEN fi.shipped_quantity
                                ELSE fi.quantity
                            END
                        ), 0)::BIGINT
                        INTO other_commitment
                        FROM fulfillment_items fi
                        JOIN fulfillments f ON f.id = fi.fulfillment_id
                        WHERE fi.order_line_item_id = item.order_line_item_id
                          AND fi.fulfillment_id <> NEW.id;

                        IF other_commitment + item.quantity > order_line_quantity THEN
                            RAISE EXCEPTION
                                'reopened fulfillment allocation exceeds order line quantity for line %',
                                item.order_line_item_id
                                USING ERRCODE = '23514';
                        END IF;
                    END LOOP;
                END IF;

                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER fulfillment_reopen_allocation_guard
            BEFORE UPDATE OF status ON fulfillments
            FOR EACH ROW
            EXECUTE FUNCTION enforce_fulfillment_reopen_allocation();
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
            CREATE TRIGGER fulfillment_items_allocation_guard_insert
            BEFORE INSERT ON fulfillment_items
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1 FROM order_line_items WHERE id = NEW.order_line_item_id
                ) THEN RAISE(ABORT, 'order line does not exist') END;
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1 FROM fulfillments WHERE id = NEW.fulfillment_id
                ) THEN RAISE(ABORT, 'fulfillment does not exist') END;
                SELECT CASE WHEN (
                    SELECT COALESCE(SUM(
                        CASE WHEN f.status = 'cancelled' THEN fi.shipped_quantity ELSE fi.quantity END
                    ), 0)
                    FROM fulfillment_items fi
                    JOIN fulfillments f ON f.id = fi.fulfillment_id
                    WHERE fi.order_line_item_id = NEW.order_line_item_id
                ) + (
                    SELECT CASE WHEN f.status = 'cancelled' THEN NEW.shipped_quantity ELSE NEW.quantity END
                    FROM fulfillments f
                    WHERE f.id = NEW.fulfillment_id
                ) > (
                    SELECT quantity FROM order_line_items WHERE id = NEW.order_line_item_id
                ) THEN RAISE(ABORT, 'fulfillment allocation exceeds order line quantity') END;
            END;

            CREATE TRIGGER fulfillment_items_allocation_guard_update
            BEFORE UPDATE ON fulfillment_items
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1 FROM order_line_items WHERE id = NEW.order_line_item_id
                ) THEN RAISE(ABORT, 'order line does not exist') END;
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1 FROM fulfillments WHERE id = NEW.fulfillment_id
                ) THEN RAISE(ABORT, 'fulfillment does not exist') END;
                SELECT CASE WHEN (
                    SELECT COALESCE(SUM(
                        CASE WHEN f.status = 'cancelled' THEN fi.shipped_quantity ELSE fi.quantity END
                    ), 0)
                    FROM fulfillment_items fi
                    JOIN fulfillments f ON f.id = fi.fulfillment_id
                    WHERE fi.order_line_item_id = NEW.order_line_item_id
                      AND fi.id <> OLD.id
                ) + (
                    SELECT CASE WHEN f.status = 'cancelled' THEN NEW.shipped_quantity ELSE NEW.quantity END
                    FROM fulfillments f
                    WHERE f.id = NEW.fulfillment_id
                ) > (
                    SELECT quantity FROM order_line_items WHERE id = NEW.order_line_item_id
                ) THEN RAISE(ABORT, 'fulfillment allocation exceeds order line quantity') END;
            END;

            CREATE TRIGGER fulfillment_items_allocation_guard_delete
            BEFORE DELETE ON fulfillment_items
            FOR EACH ROW
            WHEN OLD.shipped_quantity > 0
            BEGIN
                SELECT RAISE(ABORT, 'shipped fulfillment items cannot be deleted');
            END;

            CREATE TRIGGER fulfillment_reopen_allocation_guard
            BEFORE UPDATE OF status ON fulfillments
            FOR EACH ROW
            WHEN OLD.status = 'cancelled' AND NEW.status IN ('pending', 'shipped')
            BEGIN
                SELECT CASE WHEN EXISTS (
                    SELECT 1
                    FROM fulfillment_items current_item
                    JOIN order_line_items oli ON oli.id = current_item.order_line_item_id
                    WHERE current_item.fulfillment_id = NEW.id
                      AND (
                          SELECT COALESCE(SUM(
                              CASE WHEN f.status = 'cancelled' THEN fi.shipped_quantity ELSE fi.quantity END
                          ), 0)
                          FROM fulfillment_items fi
                          JOIN fulfillments f ON f.id = fi.fulfillment_id
                          WHERE fi.order_line_item_id = current_item.order_line_item_id
                            AND fi.fulfillment_id <> NEW.id
                      ) + current_item.quantity > oli.quantity
                ) THEN RAISE(ABORT, 'reopened fulfillment allocation exceeds order line quantity') END;
            END;
            "#,
        )
        .await?;
    Ok(())
}
