use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CheckoutInventoryReservations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CheckoutInventoryReservations::ReservationId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CheckoutInventoryReservations::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutInventoryReservations::CheckoutOperationId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutInventoryReservations::CartLineItemId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutInventoryReservations::ExternalId)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutInventoryReservations::VariantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutInventoryReservations::Quantity)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(CheckoutInventoryReservations::LocationId).uuid())
                    .col(
                        ColumnDef::new(CheckoutInventoryReservations::Status)
                            .string_len(32)
                            .not_null()
                            .default("planned"),
                    )
                    .col(
                        ColumnDef::new(CheckoutInventoryReservations::LastErrorCode)
                            .string_len(100),
                    )
                    .col(
                        ColumnDef::new(CheckoutInventoryReservations::LastErrorMessage)
                            .string_len(2000),
                    )
                    .col(
                        ColumnDef::new(CheckoutInventoryReservations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CheckoutInventoryReservations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CheckoutInventoryReservations::ReleasedAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(
                        ColumnDef::new(CheckoutInventoryReservations::ConsumedAt)
                            .timestamp_with_time_zone(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                CheckoutInventoryReservations::Table,
                                CheckoutInventoryReservations::CheckoutOperationId,
                            )
                            .to(CheckoutOperations::Table, CheckoutOperations::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                CheckoutInventoryReservations::Table,
                                CheckoutInventoryReservations::CartLineItemId,
                            )
                            .to(CartLineItems::Table, CartLineItems::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                CheckoutInventoryReservations::Table,
                                CheckoutInventoryReservations::VariantId,
                            )
                            .to(ProductVariants::Table, ProductVariants::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                CheckoutInventoryReservations::Table,
                                CheckoutInventoryReservations::LocationId,
                            )
                            .to(StockLocations::Table, StockLocations::Id)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("ux_checkout_inventory_reservations_operation_line")
                    .table(CheckoutInventoryReservations::Table)
                    .col(CheckoutInventoryReservations::TenantId)
                    .col(CheckoutInventoryReservations::CheckoutOperationId)
                    .col(CheckoutInventoryReservations::CartLineItemId)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("ux_checkout_inventory_reservations_external_id")
                    .table(CheckoutInventoryReservations::Table)
                    .col(CheckoutInventoryReservations::TenantId)
                    .col(CheckoutInventoryReservations::ExternalId)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_checkout_inventory_reservations_recovery")
                    .table(CheckoutInventoryReservations::Table)
                    .col(CheckoutInventoryReservations::TenantId)
                    .col(CheckoutInventoryReservations::CheckoutOperationId)
                    .col(CheckoutInventoryReservations::Status)
                    .col(CheckoutInventoryReservations::UpdatedAt)
                    .to_owned(),
            )
            .await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres_guards(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite_guards(manager).await?,
            _ => {}
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() == DatabaseBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared(
                    r#"
                    DROP TRIGGER IF EXISTS checkout_inventory_reservations_integrity_guard
                        ON checkout_inventory_reservations;
                    DROP FUNCTION IF EXISTS enforce_checkout_inventory_reservation_integrity();
                    "#,
                )
                .await?;
        }

        manager
            .drop_table(
                Table::drop()
                    .table(CheckoutInventoryReservations::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

async fn install_postgres_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            ALTER TABLE checkout_inventory_reservations
                ADD CONSTRAINT ck_checkout_inventory_reservations_identity
                CHECK (
                    btrim(external_id) <> ''
                    AND quantity > 0
                    AND (
                        (last_error_code IS NULL AND last_error_message IS NULL)
                        OR
                        (last_error_code IS NOT NULL AND btrim(last_error_code) <> ''
                            AND last_error_message IS NOT NULL
                            AND btrim(last_error_message) <> '')
                    )
                ),
                ADD CONSTRAINT ck_checkout_inventory_reservations_lifecycle
                CHECK (
                    (status = 'planned'
                        AND location_id IS NULL
                        AND released_at IS NULL
                        AND consumed_at IS NULL)
                    OR
                    (status = 'reserved'
                        AND location_id IS NOT NULL
                        AND released_at IS NULL
                        AND consumed_at IS NULL)
                    OR
                    (status = 'released'
                        AND location_id IS NOT NULL
                        AND released_at IS NOT NULL
                        AND consumed_at IS NULL)
                    OR
                    (status = 'consumed'
                        AND location_id IS NOT NULL
                        AND released_at IS NULL
                        AND consumed_at IS NOT NULL)
                );

            CREATE OR REPLACE FUNCTION enforce_checkout_inventory_reservation_integrity()
            RETURNS trigger AS $$
            DECLARE
                operation_tenant UUID;
                operation_cart UUID;
                operation_stage VARCHAR(32);
                line_cart UUID;
                line_variant UUID;
                location_tenant UUID;
            BEGIN
                IF TG_OP = 'UPDATE' AND (
                    NEW.reservation_id IS DISTINCT FROM OLD.reservation_id
                    OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
                    OR NEW.checkout_operation_id IS DISTINCT FROM OLD.checkout_operation_id
                    OR NEW.cart_line_item_id IS DISTINCT FROM OLD.cart_line_item_id
                    OR NEW.external_id IS DISTINCT FROM OLD.external_id
                    OR NEW.variant_id IS DISTINCT FROM OLD.variant_id
                    OR NEW.quantity IS DISTINCT FROM OLD.quantity
                    OR NEW.created_at IS DISTINCT FROM OLD.created_at
                ) THEN
                    RAISE EXCEPTION 'checkout inventory reservation identity is immutable'
                        USING ERRCODE = '23514';
                END IF;

                SELECT tenant_id, cart_id, stage
                INTO operation_tenant, operation_cart, operation_stage
                FROM checkout_operations
                WHERE id = NEW.checkout_operation_id;
                IF operation_tenant IS NULL OR operation_tenant <> NEW.tenant_id THEN
                    RAISE EXCEPTION 'checkout inventory reservation operation tenant mismatch'
                        USING ERRCODE = '23514';
                END IF;
                IF TG_OP = 'INSERT' AND operation_stage <> 'cart_locked' THEN
                    RAISE EXCEPTION 'checkout inventory reservation must be planned from cart_locked stage'
                        USING ERRCODE = '23514';
                END IF;

                SELECT cart_id, variant_id
                INTO line_cart, line_variant
                FROM cart_line_items
                WHERE id = NEW.cart_line_item_id;
                IF line_cart IS NULL OR line_cart <> operation_cart THEN
                    RAISE EXCEPTION 'checkout inventory reservation cart line mismatch'
                        USING ERRCODE = '23514';
                END IF;
                IF line_variant IS NULL OR line_variant <> NEW.variant_id THEN
                    RAISE EXCEPTION 'checkout inventory reservation variant mismatch'
                        USING ERRCODE = '23514';
                END IF;

                IF NEW.location_id IS NOT NULL THEN
                    SELECT tenant_id INTO location_tenant
                    FROM stock_locations
                    WHERE id = NEW.location_id AND deleted_at IS NULL;
                    IF location_tenant IS NULL OR location_tenant <> NEW.tenant_id THEN
                        RAISE EXCEPTION 'checkout inventory reservation location tenant mismatch'
                            USING ERRCODE = '23514';
                    END IF;
                END IF;

                IF TG_OP = 'UPDATE' AND NOT (
                    OLD.status = NEW.status
                    OR (OLD.status = 'planned' AND NEW.status = 'reserved')
                    OR (OLD.status = 'reserved' AND NEW.status IN ('released', 'consumed'))
                ) THEN
                    RAISE EXCEPTION
                        'invalid checkout inventory reservation transition from % to %',
                        OLD.status,
                        NEW.status
                        USING ERRCODE = '23514';
                END IF;

                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER checkout_inventory_reservations_integrity_guard
            BEFORE INSERT OR UPDATE ON checkout_inventory_reservations
            FOR EACH ROW
            EXECUTE FUNCTION enforce_checkout_inventory_reservation_integrity();
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
            CREATE TRIGGER checkout_inventory_reservations_guard_insert
            BEFORE INSERT ON checkout_inventory_reservations
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN trim(NEW.external_id) = '' OR NEW.quantity <= 0
                    THEN RAISE(ABORT, 'invalid checkout inventory reservation identity') END;
                SELECT CASE WHEN NEW.status <> 'planned'
                    OR NEW.location_id IS NOT NULL
                    OR NEW.released_at IS NOT NULL
                    OR NEW.consumed_at IS NOT NULL
                    THEN RAISE(ABORT, 'invalid planned checkout inventory reservation') END;
                SELECT CASE WHEN NOT (
                    (NEW.last_error_code IS NULL AND NEW.last_error_message IS NULL)
                    OR
                    (NEW.last_error_code IS NOT NULL AND trim(NEW.last_error_code) <> ''
                        AND NEW.last_error_message IS NOT NULL
                        AND trim(NEW.last_error_message) <> '')
                ) THEN RAISE(ABORT, 'invalid checkout inventory reservation error state') END;
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1
                    FROM checkout_operations co
                    WHERE co.id = NEW.checkout_operation_id
                      AND co.tenant_id = NEW.tenant_id
                      AND co.stage = 'cart_locked'
                ) THEN RAISE(ABORT, 'checkout inventory reservation operation mismatch') END;
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1
                    FROM cart_line_items cli
                    JOIN checkout_operations co ON co.id = NEW.checkout_operation_id
                    WHERE cli.id = NEW.cart_line_item_id
                      AND cli.cart_id = co.cart_id
                      AND cli.variant_id = NEW.variant_id
                ) THEN RAISE(ABORT, 'checkout inventory reservation cart line mismatch') END;
            END;

            CREATE TRIGGER checkout_inventory_reservations_guard_update
            BEFORE UPDATE ON checkout_inventory_reservations
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NEW.reservation_id IS NOT OLD.reservation_id
                    OR NEW.tenant_id IS NOT OLD.tenant_id
                    OR NEW.checkout_operation_id IS NOT OLD.checkout_operation_id
                    OR NEW.cart_line_item_id IS NOT OLD.cart_line_item_id
                    OR NEW.external_id IS NOT OLD.external_id
                    OR NEW.variant_id IS NOT OLD.variant_id
                    OR NEW.quantity IS NOT OLD.quantity
                    OR NEW.created_at IS NOT OLD.created_at
                    THEN RAISE(ABORT, 'checkout inventory reservation identity is immutable') END;
                SELECT CASE WHEN NOT (
                    OLD.status = NEW.status
                    OR (OLD.status = 'planned' AND NEW.status = 'reserved')
                    OR (OLD.status = 'reserved' AND NEW.status IN ('released', 'consumed'))
                ) THEN RAISE(ABORT, 'invalid checkout inventory reservation transition') END;
                SELECT CASE WHEN NOT (
                    (NEW.status = 'planned'
                        AND NEW.location_id IS NULL
                        AND NEW.released_at IS NULL
                        AND NEW.consumed_at IS NULL)
                    OR
                    (NEW.status = 'reserved'
                        AND NEW.location_id IS NOT NULL
                        AND NEW.released_at IS NULL
                        AND NEW.consumed_at IS NULL)
                    OR
                    (NEW.status = 'released'
                        AND NEW.location_id IS NOT NULL
                        AND NEW.released_at IS NOT NULL
                        AND NEW.consumed_at IS NULL)
                    OR
                    (NEW.status = 'consumed'
                        AND NEW.location_id IS NOT NULL
                        AND NEW.released_at IS NULL
                        AND NEW.consumed_at IS NOT NULL)
                ) THEN RAISE(ABORT, 'invalid checkout inventory reservation lifecycle') END;
                SELECT CASE WHEN NOT (
                    (NEW.last_error_code IS NULL AND NEW.last_error_message IS NULL)
                    OR
                    (NEW.last_error_code IS NOT NULL AND trim(NEW.last_error_code) <> ''
                        AND NEW.last_error_message IS NOT NULL
                        AND trim(NEW.last_error_message) <> '')
                ) THEN RAISE(ABORT, 'invalid checkout inventory reservation error state') END;
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1
                    FROM checkout_operations co
                    WHERE co.id = NEW.checkout_operation_id
                      AND co.tenant_id = NEW.tenant_id
                ) THEN RAISE(ABORT, 'checkout inventory reservation operation tenant mismatch') END;
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1
                    FROM cart_line_items cli
                    JOIN checkout_operations co ON co.id = NEW.checkout_operation_id
                    WHERE cli.id = NEW.cart_line_item_id
                      AND cli.cart_id = co.cart_id
                      AND cli.variant_id = NEW.variant_id
                ) THEN RAISE(ABORT, 'checkout inventory reservation cart line mismatch') END;
                SELECT CASE WHEN NEW.location_id IS NOT NULL AND NOT EXISTS (
                    SELECT 1
                    FROM stock_locations sl
                    WHERE sl.id = NEW.location_id
                      AND sl.tenant_id = NEW.tenant_id
                      AND sl.deleted_at IS NULL
                ) THEN RAISE(ABORT, 'checkout inventory reservation location tenant mismatch') END;
            END;
            "#,
        )
        .await?;
    Ok(())
}

#[derive(DeriveIden)]
enum CheckoutInventoryReservations {
    Table,
    ReservationId,
    TenantId,
    CheckoutOperationId,
    CartLineItemId,
    ExternalId,
    VariantId,
    Quantity,
    LocationId,
    Status,
    LastErrorCode,
    LastErrorMessage,
    CreatedAt,
    UpdatedAt,
    ReleasedAt,
    ConsumedAt,
}

#[derive(DeriveIden)]
enum CheckoutOperations {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum CartLineItems {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum ProductVariants {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum StockLocations {
    Table,
    Id,
}
