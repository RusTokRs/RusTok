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
                        DROP TRIGGER IF EXISTS payment_collections_lifecycle_guard ON payment_collections;
                        DROP TRIGGER IF EXISTS payments_lifecycle_guard ON payments;
                        DROP TRIGGER IF EXISTS refunds_lifecycle_guard ON refunds;
                        DROP FUNCTION IF EXISTS enforce_payment_collection_lifecycle();
                        DROP FUNCTION IF EXISTS enforce_payment_lifecycle();
                        DROP FUNCTION IF EXISTS enforce_refund_lifecycle();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS payment_collections_identity_guard;
                        DROP TRIGGER IF EXISTS payment_collections_transition_guard;
                        DROP TRIGGER IF EXISTS payments_identity_guard;
                        DROP TRIGGER IF EXISTS payments_transition_guard;
                        DROP TRIGGER IF EXISTS refunds_identity_guard;
                        DROP TRIGGER IF EXISTS refunds_transition_guard;
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
            CREATE OR REPLACE FUNCTION enforce_payment_collection_lifecycle() RETURNS trigger AS $$
            BEGIN
                IF NEW.id IS DISTINCT FROM OLD.id
                   OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
                   OR NEW.cart_id IS DISTINCT FROM OLD.cart_id
                   OR NEW.customer_id IS DISTINCT FROM OLD.customer_id
                   OR NEW.currency_code IS DISTINCT FROM OLD.currency_code
                   OR NEW.amount IS DISTINCT FROM OLD.amount THEN
                    RAISE EXCEPTION 'payment collection identity and monetary basis are immutable'
                        USING ERRCODE = '23514';
                END IF;

                IF OLD.order_id IS NOT NULL AND NEW.order_id IS DISTINCT FROM OLD.order_id THEN
                    RAISE EXCEPTION 'payment collection order_id is immutable once attached'
                        USING ERRCODE = '23514';
                END IF;

                IF OLD.status IS DISTINCT FROM NEW.status THEN
                    IF NOT (
                        (OLD.status = 'pending' AND NEW.status IN ('authorized', 'cancelled'))
                        OR (OLD.status = 'authorized' AND NEW.status IN ('captured', 'cancelled'))
                    ) THEN
                        RAISE EXCEPTION 'invalid payment collection transition from % to %', OLD.status, NEW.status
                            USING ERRCODE = '23514';
                    END IF;

                    IF NEW.status = 'cancelled' THEN
                        IF EXISTS (
                            SELECT 1 FROM payments
                            WHERE payment_collection_id = NEW.id
                              AND status = 'captured'
                        ) THEN
                            RAISE EXCEPTION 'captured payment collection cannot be cancelled'
                                USING ERRCODE = '23514';
                        END IF;

                        IF EXISTS (
                            SELECT 1 FROM payments
                            WHERE payment_collection_id = NEW.id
                              AND status NOT IN ('authorized', 'cancelled')
                        ) THEN
                            RAISE EXCEPTION 'payment row has an invalid state for collection cancellation'
                                USING ERRCODE = '23514';
                        END IF;

                        UPDATE payments
                        SET status = 'cancelled',
                            error_message = COALESCE(NULLIF(BTRIM(NEW.cancellation_reason), ''), 'cancelled'),
                            cancelled_at = COALESCE(cancelled_at, CURRENT_TIMESTAMP),
                            updated_at = CURRENT_TIMESTAMP
                        WHERE payment_collection_id = NEW.id
                          AND status = 'authorized';
                    END IF;
                END IF;

                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER payment_collections_lifecycle_guard
            BEFORE UPDATE ON payment_collections
            FOR EACH ROW
            EXECUTE FUNCTION enforce_payment_collection_lifecycle();

            CREATE OR REPLACE FUNCTION enforce_payment_lifecycle() RETURNS trigger AS $$
            BEGIN
                IF NEW.id IS DISTINCT FROM OLD.id
                   OR NEW.payment_collection_id IS DISTINCT FROM OLD.payment_collection_id
                   OR NEW.provider_id IS DISTINCT FROM OLD.provider_id
                   OR NEW.provider_payment_id IS DISTINCT FROM OLD.provider_payment_id
                   OR NEW.currency_code IS DISTINCT FROM OLD.currency_code
                   OR NEW.amount IS DISTINCT FROM OLD.amount THEN
                    RAISE EXCEPTION 'payment identity and authorization basis are immutable'
                        USING ERRCODE = '23514';
                END IF;

                IF OLD.status IS DISTINCT FROM NEW.status
                   AND NOT (
                       OLD.status = 'authorized'
                       AND NEW.status IN ('captured', 'cancelled')
                   ) THEN
                    RAISE EXCEPTION 'invalid payment transition from % to %', OLD.status, NEW.status
                        USING ERRCODE = '23514';
                END IF;

                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER payments_lifecycle_guard
            BEFORE UPDATE ON payments
            FOR EACH ROW
            EXECUTE FUNCTION enforce_payment_lifecycle();

            CREATE OR REPLACE FUNCTION enforce_refund_lifecycle() RETURNS trigger AS $$
            BEGIN
                IF NEW.id IS DISTINCT FROM OLD.id
                   OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
                   OR NEW.payment_collection_id IS DISTINCT FROM OLD.payment_collection_id
                   OR NEW.currency_code IS DISTINCT FROM OLD.currency_code
                   OR NEW.amount IS DISTINCT FROM OLD.amount THEN
                    RAISE EXCEPTION 'refund identity and amount are immutable'
                        USING ERRCODE = '23514';
                END IF;

                IF OLD.status IS DISTINCT FROM NEW.status
                   AND NOT (
                       OLD.status = 'pending'
                       AND NEW.status IN ('refunded', 'cancelled')
                   ) THEN
                    RAISE EXCEPTION 'invalid refund transition from % to %', OLD.status, NEW.status
                        USING ERRCODE = '23514';
                END IF;

                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER refunds_lifecycle_guard
            BEFORE UPDATE ON refunds
            FOR EACH ROW
            EXECUTE FUNCTION enforce_refund_lifecycle();
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
            CREATE TRIGGER payment_collections_identity_guard
            BEFORE UPDATE ON payment_collections
            FOR EACH ROW
            WHEN NEW.id IS NOT OLD.id
              OR NEW.tenant_id IS NOT OLD.tenant_id
              OR NEW.cart_id IS NOT OLD.cart_id
              OR NEW.customer_id IS NOT OLD.customer_id
              OR NEW.currency_code IS NOT OLD.currency_code
              OR NEW.amount IS NOT OLD.amount
              OR (OLD.order_id IS NOT NULL AND NEW.order_id IS NOT OLD.order_id)
            BEGIN
                SELECT RAISE(ABORT, 'payment collection identity and monetary basis are immutable');
            END;

            CREATE TRIGGER payment_collections_transition_guard
            BEFORE UPDATE OF status ON payment_collections
            FOR EACH ROW
            WHEN NEW.status <> OLD.status
            BEGIN
                SELECT CASE WHEN NOT (
                    (OLD.status = 'pending' AND NEW.status IN ('authorized', 'cancelled'))
                    OR (OLD.status = 'authorized' AND NEW.status IN ('captured', 'cancelled'))
                ) THEN RAISE(ABORT, 'invalid payment collection transition') END;

                SELECT CASE WHEN NEW.status = 'cancelled' AND EXISTS (
                    SELECT 1 FROM payments
                    WHERE payment_collection_id = NEW.id
                      AND status = 'captured'
                ) THEN RAISE(ABORT, 'captured payment collection cannot be cancelled') END;

                SELECT CASE WHEN NEW.status = 'cancelled' AND EXISTS (
                    SELECT 1 FROM payments
                    WHERE payment_collection_id = NEW.id
                      AND status NOT IN ('authorized', 'cancelled')
                ) THEN RAISE(ABORT, 'payment row has an invalid state for collection cancellation') END;

                UPDATE payments
                SET status = 'cancelled',
                    error_message = COALESCE(NULLIF(TRIM(NEW.cancellation_reason), ''), 'cancelled'),
                    cancelled_at = COALESCE(cancelled_at, CURRENT_TIMESTAMP),
                    updated_at = CURRENT_TIMESTAMP
                WHERE NEW.status = 'cancelled'
                  AND payment_collection_id = NEW.id
                  AND status = 'authorized';
            END;

            CREATE TRIGGER payments_identity_guard
            BEFORE UPDATE ON payments
            FOR EACH ROW
            WHEN NEW.id IS NOT OLD.id
              OR NEW.payment_collection_id IS NOT OLD.payment_collection_id
              OR NEW.provider_id IS NOT OLD.provider_id
              OR NEW.provider_payment_id IS NOT OLD.provider_payment_id
              OR NEW.currency_code IS NOT OLD.currency_code
              OR NEW.amount IS NOT OLD.amount
            BEGIN
                SELECT RAISE(ABORT, 'payment identity and authorization basis are immutable');
            END;

            CREATE TRIGGER payments_transition_guard
            BEFORE UPDATE OF status ON payments
            FOR EACH ROW
            WHEN NEW.status <> OLD.status
              AND NOT (OLD.status = 'authorized' AND NEW.status IN ('captured', 'cancelled'))
            BEGIN
                SELECT RAISE(ABORT, 'invalid payment transition');
            END;

            CREATE TRIGGER refunds_identity_guard
            BEFORE UPDATE ON refunds
            FOR EACH ROW
            WHEN NEW.id IS NOT OLD.id
              OR NEW.tenant_id IS NOT OLD.tenant_id
              OR NEW.payment_collection_id IS NOT OLD.payment_collection_id
              OR NEW.currency_code IS NOT OLD.currency_code
              OR NEW.amount IS NOT OLD.amount
            BEGIN
                SELECT RAISE(ABORT, 'refund identity and amount are immutable');
            END;

            CREATE TRIGGER refunds_transition_guard
            BEFORE UPDATE OF status ON refunds
            FOR EACH ROW
            WHEN NEW.status <> OLD.status
              AND NOT (OLD.status = 'pending' AND NEW.status IN ('refunded', 'cancelled'))
            BEGIN
                SELECT RAISE(ABORT, 'invalid refund transition');
            END;
            "#,
        )
        .await?;
    Ok(())
}
