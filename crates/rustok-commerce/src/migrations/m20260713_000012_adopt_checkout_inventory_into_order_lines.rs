use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .alter_table(
                Table::alter()
                    .table(CheckoutInventoryReservations::Table)
                    .add_column(
                        ColumnDef::new(CheckoutInventoryReservations::OrderLineItemId).uuid(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("ux_checkout_inventory_reservations_order_line")
                    .table(CheckoutInventoryReservations::Table)
                    .col(CheckoutInventoryReservations::TenantId)
                    .col(CheckoutInventoryReservations::OrderLineItemId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres_guard(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite_guards(manager).await?,
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
                        DROP TRIGGER IF EXISTS checkout_inventory_reservation_adoption_guard
                            ON checkout_inventory_reservations;
                        DROP FUNCTION IF EXISTS enforce_checkout_inventory_reservation_adoption();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS checkout_inventory_reservation_adoption_guard_insert;
                        DROP TRIGGER IF EXISTS checkout_inventory_reservation_adoption_guard_update;
                        "#,
                    )
                    .await?;
            }
            _ => {}
        }

        manager
            .drop_index(
                Index::drop()
                    .name("ux_checkout_inventory_reservations_order_line")
                    .table(CheckoutInventoryReservations::Table)
                    .to_owned(),
            )
            .await?;

        manager
            .alter_table(
                Table::alter()
                    .table(CheckoutInventoryReservations::Table)
                    .drop_column(CheckoutInventoryReservations::OrderLineItemId)
                    .to_owned(),
            )
            .await
    }
}

async fn install_postgres_guard(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE OR REPLACE FUNCTION enforce_checkout_inventory_reservation_adoption()
            RETURNS trigger AS $$
            DECLARE
                order_tenant UUID;
                order_variant UUID;
                order_quantity INTEGER;
                source_cart_line TEXT;
                source_operation TEXT;
                reservation_line UUID;
                reservation_quantity INTEGER;
                reservation_deleted TIMESTAMPTZ;
                reservation_variant UUID;
            BEGIN
                IF TG_OP = 'UPDATE'
                   AND OLD.order_line_item_id IS NOT NULL
                   AND NEW.order_line_item_id IS DISTINCT FROM OLD.order_line_item_id THEN
                    RAISE EXCEPTION 'checkout inventory reservation order line is immutable'
                        USING ERRCODE = '23514';
                END IF;

                IF NEW.order_line_item_id IS NULL THEN
                    RETURN NEW;
                END IF;

                IF NEW.status <> 'reserved' THEN
                    RAISE EXCEPTION 'only an active reserved checkout inventory row can be adopted'
                        USING ERRCODE = '23514';
                END IF;

                SELECT
                    o.tenant_id,
                    oli.variant_id,
                    oli.quantity,
                    oli.metadata #>> '{checkout,cart_line_item_id}',
                    o.metadata #>> '{checkout,operation_id}'
                INTO
                    order_tenant,
                    order_variant,
                    order_quantity,
                    source_cart_line,
                    source_operation
                FROM order_line_items oli
                JOIN orders o ON o.id = oli.order_id
                WHERE oli.id = NEW.order_line_item_id;

                IF order_tenant IS NULL OR order_tenant <> NEW.tenant_id THEN
                    RAISE EXCEPTION 'checkout inventory reservation order tenant mismatch'
                        USING ERRCODE = '23514';
                END IF;
                IF order_variant IS NULL OR order_variant <> NEW.variant_id THEN
                    RAISE EXCEPTION 'checkout inventory reservation order variant mismatch'
                        USING ERRCODE = '23514';
                END IF;
                IF order_quantity <> NEW.quantity THEN
                    RAISE EXCEPTION 'checkout inventory reservation order quantity mismatch'
                        USING ERRCODE = '23514';
                END IF;
                IF source_cart_line IS DISTINCT FROM NEW.cart_line_item_id::text THEN
                    RAISE EXCEPTION 'checkout inventory reservation cart line provenance mismatch'
                        USING ERRCODE = '23514';
                END IF;
                IF source_operation IS DISTINCT FROM NEW.checkout_operation_id::text THEN
                    RAISE EXCEPTION 'checkout inventory reservation operation provenance mismatch'
                        USING ERRCODE = '23514';
                END IF;

                SELECT
                    ri.line_item_id,
                    ri.quantity,
                    ri.deleted_at,
                    ii.variant_id
                INTO
                    reservation_line,
                    reservation_quantity,
                    reservation_deleted,
                    reservation_variant
                FROM reservation_items ri
                JOIN inventory_items ii ON ii.id = ri.inventory_item_id
                WHERE ri.id = NEW.reservation_id;

                IF reservation_line IS NULL
                   OR reservation_line <> NEW.order_line_item_id
                   OR reservation_quantity <> NEW.quantity
                   OR reservation_deleted IS NOT NULL
                   OR reservation_variant <> NEW.variant_id THEN
                    RAISE EXCEPTION 'checkout inventory reservation ledger adoption mismatch'
                        USING ERRCODE = '23514';
                END IF;

                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER checkout_inventory_reservation_adoption_guard
            BEFORE INSERT OR UPDATE OF order_line_item_id
            ON checkout_inventory_reservations
            FOR EACH ROW
            EXECUTE FUNCTION enforce_checkout_inventory_reservation_adoption();
            "#,
        )
        .await?;
    Ok(())
}

async fn install_sqlite_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE TRIGGER checkout_inventory_reservation_adoption_guard_insert
            BEFORE INSERT ON checkout_inventory_reservations
            FOR EACH ROW
            WHEN NEW.order_line_item_id IS NOT NULL
            BEGIN
                SELECT CASE WHEN NEW.status <> 'reserved'
                    THEN RAISE(ABORT, 'only an active reserved checkout inventory row can be adopted') END;
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1
                    FROM order_line_items oli
                    JOIN orders o ON o.id = oli.order_id
                    WHERE oli.id = NEW.order_line_item_id
                      AND o.tenant_id = NEW.tenant_id
                      AND oli.variant_id = NEW.variant_id
                      AND oli.quantity = NEW.quantity
                      AND json_extract(oli.metadata, '$.checkout.cart_line_item_id') = CAST(NEW.cart_line_item_id AS TEXT)
                      AND json_extract(o.metadata, '$.checkout.operation_id') = CAST(NEW.checkout_operation_id AS TEXT)
                ) THEN RAISE(ABORT, 'checkout inventory reservation order provenance mismatch') END;
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1
                    FROM reservation_items ri
                    JOIN inventory_items ii ON ii.id = ri.inventory_item_id
                    WHERE ri.id = NEW.reservation_id
                      AND ri.line_item_id = NEW.order_line_item_id
                      AND ri.quantity = NEW.quantity
                      AND ri.deleted_at IS NULL
                      AND ii.variant_id = NEW.variant_id
                ) THEN RAISE(ABORT, 'checkout inventory reservation ledger adoption mismatch') END;
            END;

            CREATE TRIGGER checkout_inventory_reservation_adoption_guard_update
            BEFORE UPDATE OF order_line_item_id ON checkout_inventory_reservations
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN OLD.order_line_item_id IS NOT NULL
                    AND NEW.order_line_item_id IS NOT OLD.order_line_item_id
                    THEN RAISE(ABORT, 'checkout inventory reservation order line is immutable') END;
                SELECT CASE WHEN NEW.order_line_item_id IS NOT NULL AND NEW.status <> 'reserved'
                    THEN RAISE(ABORT, 'only an active reserved checkout inventory row can be adopted') END;
                SELECT CASE WHEN NEW.order_line_item_id IS NOT NULL AND NOT EXISTS (
                    SELECT 1
                    FROM order_line_items oli
                    JOIN orders o ON o.id = oli.order_id
                    WHERE oli.id = NEW.order_line_item_id
                      AND o.tenant_id = NEW.tenant_id
                      AND oli.variant_id = NEW.variant_id
                      AND oli.quantity = NEW.quantity
                      AND json_extract(oli.metadata, '$.checkout.cart_line_item_id') = CAST(NEW.cart_line_item_id AS TEXT)
                      AND json_extract(o.metadata, '$.checkout.operation_id') = CAST(NEW.checkout_operation_id AS TEXT)
                ) THEN RAISE(ABORT, 'checkout inventory reservation order provenance mismatch') END;
                SELECT CASE WHEN NEW.order_line_item_id IS NOT NULL AND NOT EXISTS (
                    SELECT 1
                    FROM reservation_items ri
                    JOIN inventory_items ii ON ii.id = ri.inventory_item_id
                    WHERE ri.id = NEW.reservation_id
                      AND ri.line_item_id = NEW.order_line_item_id
                      AND ri.quantity = NEW.quantity
                      AND ri.deleted_at IS NULL
                      AND ii.variant_id = NEW.variant_id
                ) THEN RAISE(ABORT, 'checkout inventory reservation ledger adoption mismatch') END;
            END;
            "#,
        )
        .await?;
    Ok(())
}

#[derive(DeriveIden)]
enum CheckoutInventoryReservations {
    Table,
    TenantId,
    OrderLineItemId,
}
