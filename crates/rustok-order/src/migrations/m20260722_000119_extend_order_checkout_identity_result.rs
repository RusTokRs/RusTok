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
                    .table(OrderCheckoutIdentities::Table)
                    .add_column(ColumnDef::new(OrderCheckoutIdentities::PaymentCollectionId).uuid())
                    .add_column(ColumnDef::new(OrderCheckoutIdentities::ShippingOptionId).uuid())
                    .to_owned(),
            )
            .await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres_monotonic_guard(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite_monotonic_guard(manager).await?,
            DatabaseBackend::MySql => install_mysql_monotonic_guard(manager).await?,
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => restore_postgres_immutable_guard(manager).await?,
            DatabaseBackend::Sqlite => restore_sqlite_immutable_guard(manager).await?,
            DatabaseBackend::MySql => restore_mysql_immutable_guard(manager).await?,
        }

        manager
            .alter_table(
                Table::alter()
                    .table(OrderCheckoutIdentities::Table)
                    .drop_column(OrderCheckoutIdentities::ShippingOptionId)
                    .drop_column(OrderCheckoutIdentities::PaymentCollectionId)
                    .to_owned(),
            )
            .await
    }
}

async fn install_postgres_monotonic_guard(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            ALTER TABLE order_checkout_identities
                ADD CONSTRAINT ck_order_checkout_identity_result_ids
                CHECK (
                    (payment_collection_id IS NULL OR payment_collection_id <> '00000000-0000-0000-0000-000000000000'::uuid)
                    AND (shipping_option_id IS NULL OR shipping_option_id <> '00000000-0000-0000-0000-000000000000'::uuid)
                );

            CREATE OR REPLACE FUNCTION enforce_order_checkout_identity_integrity()
            RETURNS trigger AS $$
            DECLARE
                order_tenant UUID;
            BEGIN
                IF TG_OP = 'UPDATE' AND (
                    NEW.checkout_operation_id IS DISTINCT FROM OLD.checkout_operation_id
                    OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
                    OR NEW.order_id IS DISTINCT FROM OLD.order_id
                    OR NEW.created_at IS DISTINCT FROM OLD.created_at
                    OR (OLD.source_cart_id IS NOT NULL AND NEW.source_cart_id IS DISTINCT FROM OLD.source_cart_id)
                    OR (OLD.payment_collection_id IS NOT NULL AND NEW.payment_collection_id IS DISTINCT FROM OLD.payment_collection_id)
                    OR (OLD.shipping_option_id IS NOT NULL AND NEW.shipping_option_id IS DISTINCT FROM OLD.shipping_option_id)
                    OR (OLD.snapshot_hash IS NOT NULL AND NEW.snapshot_hash IS DISTINCT FROM OLD.snapshot_hash)
                    OR (OLD.request_hash IS NOT NULL AND NEW.request_hash IS DISTINCT FROM OLD.request_hash)
                ) THEN
                    RAISE EXCEPTION 'order checkout identity facts are immutable once recorded'
                        USING ERRCODE = '23514';
                END IF;

                SELECT tenant_id INTO order_tenant
                FROM orders
                WHERE id = NEW.order_id;

                IF order_tenant IS NULL OR order_tenant <> NEW.tenant_id THEN
                    RAISE EXCEPTION 'order checkout identity tenant/order mismatch'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;
            "#,
        )
        .await?;
    Ok(())
}

async fn restore_postgres_immutable_guard(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            ALTER TABLE order_checkout_identities
                DROP CONSTRAINT IF EXISTS ck_order_checkout_identity_result_ids;

            CREATE OR REPLACE FUNCTION enforce_order_checkout_identity_integrity()
            RETURNS trigger AS $$
            DECLARE
                order_tenant UUID;
            BEGIN
                IF TG_OP = 'UPDATE' THEN
                    RAISE EXCEPTION 'order checkout identity is immutable'
                        USING ERRCODE = '23514';
                END IF;

                SELECT tenant_id INTO order_tenant
                FROM orders
                WHERE id = NEW.order_id;

                IF order_tenant IS NULL OR order_tenant <> NEW.tenant_id THEN
                    RAISE EXCEPTION 'order checkout identity tenant/order mismatch'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;
            "#,
        )
        .await?;
    Ok(())
}

async fn install_sqlite_monotonic_guard(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER IF EXISTS order_checkout_identity_guard_update;

            CREATE TRIGGER order_checkout_identity_result_guard_insert
            BEFORE INSERT ON order_checkout_identities
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NEW.payment_collection_id = '00000000-0000-0000-0000-000000000000'
                    OR NEW.shipping_option_id = '00000000-0000-0000-0000-000000000000'
                    THEN RAISE(ABORT, 'invalid order checkout result identity') END;
            END;

            CREATE TRIGGER order_checkout_identity_guard_update
            BEFORE UPDATE ON order_checkout_identities
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NEW.checkout_operation_id IS NOT OLD.checkout_operation_id
                    OR NEW.tenant_id IS NOT OLD.tenant_id
                    OR NEW.order_id IS NOT OLD.order_id
                    OR NEW.created_at IS NOT OLD.created_at
                    OR (OLD.source_cart_id IS NOT NULL AND NEW.source_cart_id IS NOT OLD.source_cart_id)
                    OR (OLD.payment_collection_id IS NOT NULL AND NEW.payment_collection_id IS NOT OLD.payment_collection_id)
                    OR (OLD.shipping_option_id IS NOT NULL AND NEW.shipping_option_id IS NOT OLD.shipping_option_id)
                    OR (OLD.snapshot_hash IS NOT NULL AND NEW.snapshot_hash IS NOT OLD.snapshot_hash)
                    OR (OLD.request_hash IS NOT NULL AND NEW.request_hash IS NOT OLD.request_hash)
                    THEN RAISE(ABORT, 'order checkout identity facts are immutable once recorded') END;
                SELECT CASE WHEN NEW.payment_collection_id = '00000000-0000-0000-0000-000000000000'
                    OR NEW.shipping_option_id = '00000000-0000-0000-0000-000000000000'
                    THEN RAISE(ABORT, 'invalid order checkout result identity') END;
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1 FROM orders
                    WHERE id = NEW.order_id
                      AND tenant_id = NEW.tenant_id
                ) THEN RAISE(ABORT, 'order checkout identity tenant/order mismatch') END;
            END;
            "#,
        )
        .await?;
    Ok(())
}

async fn restore_sqlite_immutable_guard(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER IF EXISTS order_checkout_identity_result_guard_insert;
            DROP TRIGGER IF EXISTS order_checkout_identity_guard_update;

            CREATE TRIGGER order_checkout_identity_guard_update
            BEFORE UPDATE ON order_checkout_identities
            FOR EACH ROW
            BEGIN
                SELECT RAISE(ABORT, 'order checkout identity is immutable');
            END;
            "#,
        )
        .await?;
    Ok(())
}

async fn install_mysql_monotonic_guard(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared("DROP TRIGGER IF EXISTS order_checkout_identity_guard_insert;")
        .await?;
    manager
        .get_connection()
        .execute_unprepared("DROP TRIGGER IF EXISTS order_checkout_identity_guard_update;")
        .await?;
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE TRIGGER order_checkout_identity_guard_insert
            BEFORE INSERT ON order_checkout_identities
            FOR EACH ROW
            BEGIN
                IF NEW.checkout_operation_id = '00000000-0000-0000-0000-000000000000'
                    OR NEW.source_cart_id = '00000000-0000-0000-0000-000000000000'
                    OR NEW.payment_collection_id = '00000000-0000-0000-0000-000000000000'
                    OR NEW.shipping_option_id = '00000000-0000-0000-0000-000000000000'
                THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'invalid order checkout identity';
                END IF;
                IF (
                    SELECT COUNT(*) FROM orders
                    WHERE id = NEW.order_id
                      AND tenant_id = NEW.tenant_id
                ) <> 1 THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'order checkout identity tenant/order mismatch';
                END IF;
                IF NOT (
                    (NEW.snapshot_hash IS NULL AND NEW.request_hash IS NULL)
                    OR (
                        NEW.snapshot_hash REGEXP '^[0-9a-f]{1,128}$'
                        AND NEW.request_hash REGEXP '^[0-9a-f]{64}$'
                    )
                ) THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'invalid order checkout identity hashes';
                END IF;
            END;
            "#,
        )
        .await?;
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE TRIGGER order_checkout_identity_guard_update
            BEFORE UPDATE ON order_checkout_identities
            FOR EACH ROW
            BEGIN
                IF NOT (NEW.checkout_operation_id <=> OLD.checkout_operation_id)
                    OR NOT (NEW.tenant_id <=> OLD.tenant_id)
                    OR NOT (NEW.order_id <=> OLD.order_id)
                    OR NOT (NEW.created_at <=> OLD.created_at)
                    OR (OLD.source_cart_id IS NOT NULL AND NOT (NEW.source_cart_id <=> OLD.source_cart_id))
                    OR (OLD.payment_collection_id IS NOT NULL AND NOT (NEW.payment_collection_id <=> OLD.payment_collection_id))
                    OR (OLD.shipping_option_id IS NOT NULL AND NOT (NEW.shipping_option_id <=> OLD.shipping_option_id))
                    OR (OLD.snapshot_hash IS NOT NULL AND NOT (NEW.snapshot_hash <=> OLD.snapshot_hash))
                    OR (OLD.request_hash IS NOT NULL AND NOT (NEW.request_hash <=> OLD.request_hash))
                THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'order checkout identity facts are immutable once recorded';
                END IF;
                IF NEW.payment_collection_id = '00000000-0000-0000-0000-000000000000'
                    OR NEW.shipping_option_id = '00000000-0000-0000-0000-000000000000'
                THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'invalid order checkout result identity';
                END IF;
                IF (
                    SELECT COUNT(*) FROM orders
                    WHERE id = NEW.order_id
                      AND tenant_id = NEW.tenant_id
                ) <> 1 THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'order checkout identity tenant/order mismatch';
                END IF;
            END;
            "#,
        )
        .await?;
    Ok(())
}

async fn restore_mysql_immutable_guard(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared("DROP TRIGGER IF EXISTS order_checkout_identity_guard_insert;")
        .await?;
    manager
        .get_connection()
        .execute_unprepared("DROP TRIGGER IF EXISTS order_checkout_identity_guard_update;")
        .await?;
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE TRIGGER order_checkout_identity_guard_insert
            BEFORE INSERT ON order_checkout_identities
            FOR EACH ROW
            BEGIN
                IF NEW.checkout_operation_id = '00000000-0000-0000-0000-000000000000'
                    OR NEW.source_cart_id = '00000000-0000-0000-0000-000000000000'
                THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'invalid order checkout identity';
                END IF;
                IF (
                    SELECT COUNT(*) FROM orders
                    WHERE id = NEW.order_id
                      AND tenant_id = NEW.tenant_id
                ) <> 1 THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'order checkout identity tenant/order mismatch';
                END IF;
                IF NOT (
                    (NEW.snapshot_hash IS NULL AND NEW.request_hash IS NULL)
                    OR (
                        NEW.snapshot_hash REGEXP '^[0-9a-f]{1,128}$'
                        AND NEW.request_hash REGEXP '^[0-9a-f]{64}$'
                    )
                ) THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'invalid order checkout identity hashes';
                END IF;
            END;
            "#,
        )
        .await?;
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE TRIGGER order_checkout_identity_guard_update
            BEFORE UPDATE ON order_checkout_identities
            FOR EACH ROW
            BEGIN
                SIGNAL SQLSTATE '45000'
                    SET MESSAGE_TEXT = 'order checkout identity is immutable';
            END;
            "#,
        )
        .await?;
    Ok(())
}

#[derive(Iden)]
enum OrderCheckoutIdentities {
    Table,
    PaymentCollectionId,
    ShippingOptionId,
}
