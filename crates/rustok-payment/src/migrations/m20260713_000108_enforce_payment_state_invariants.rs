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
                        ALTER TABLE payment_collections
                            ADD CONSTRAINT ck_payment_collections_status
                            CHECK (status IN ('pending', 'authorized', 'captured', 'cancelled')) NOT VALID,
                            ADD CONSTRAINT ck_payment_collections_currency
                            CHECK (currency_code ~ '^[A-Z]{3}$') NOT VALID,
                            ADD CONSTRAINT ck_payment_collections_amounts
                            CHECK (
                                amount > 0
                                AND authorized_amount >= 0
                                AND authorized_amount <= amount
                                AND captured_amount >= 0
                                AND captured_amount <= authorized_amount
                            ) NOT VALID,
                            ADD CONSTRAINT ck_payment_collections_state
                            CHECK (
                                (status = 'pending'
                                    AND authorized_amount = 0
                                    AND captured_amount = 0
                                    AND authorized_at IS NULL
                                    AND captured_at IS NULL
                                    AND cancelled_at IS NULL)
                                OR
                                (status = 'authorized'
                                    AND authorized_amount > 0
                                    AND captured_amount = 0
                                    AND provider_id IS NOT NULL
                                    AND btrim(provider_id) <> ''
                                    AND authorized_at IS NOT NULL
                                    AND captured_at IS NULL
                                    AND cancelled_at IS NULL)
                                OR
                                (status = 'captured'
                                    AND authorized_amount > 0
                                    AND captured_amount > 0
                                    AND provider_id IS NOT NULL
                                    AND btrim(provider_id) <> ''
                                    AND authorized_at IS NOT NULL
                                    AND captured_at IS NOT NULL
                                    AND cancelled_at IS NULL)
                                OR
                                (status = 'cancelled'
                                    AND captured_amount = 0
                                    AND captured_at IS NULL
                                    AND cancelled_at IS NOT NULL)
                            ) NOT VALID;

                        ALTER TABLE payments
                            ADD CONSTRAINT ck_payments_status
                            CHECK (status IN ('authorized', 'captured', 'cancelled')) NOT VALID,
                            ADD CONSTRAINT ck_payments_currency
                            CHECK (currency_code ~ '^[A-Z]{3}$') NOT VALID,
                            ADD CONSTRAINT ck_payments_identity
                            CHECK (
                                btrim(provider_id) <> ''
                                AND btrim(provider_payment_id) <> ''
                            ) NOT VALID,
                            ADD CONSTRAINT ck_payments_amounts
                            CHECK (
                                amount > 0
                                AND captured_amount >= 0
                                AND captured_amount <= amount
                            ) NOT VALID,
                            ADD CONSTRAINT ck_payments_state
                            CHECK (
                                (status = 'authorized'
                                    AND captured_amount = 0
                                    AND authorized_at IS NOT NULL
                                    AND captured_at IS NULL
                                    AND cancelled_at IS NULL)
                                OR
                                (status = 'captured'
                                    AND captured_amount > 0
                                    AND authorized_at IS NOT NULL
                                    AND captured_at IS NOT NULL
                                    AND cancelled_at IS NULL)
                                OR
                                (status = 'cancelled'
                                    AND captured_amount = 0
                                    AND authorized_at IS NOT NULL
                                    AND captured_at IS NULL
                                    AND cancelled_at IS NOT NULL)
                            ) NOT VALID;

                        ALTER TABLE refunds
                            ADD CONSTRAINT ck_refunds_state_timestamps
                            CHECK (
                                (status = 'pending'
                                    AND refunded_at IS NULL
                                    AND cancelled_at IS NULL)
                                OR
                                (status = 'refunded'
                                    AND refunded_at IS NOT NULL
                                    AND cancelled_at IS NULL)
                                OR
                                (status = 'cancelled'
                                    AND refunded_at IS NULL
                                    AND cancelled_at IS NOT NULL)
                            ) NOT VALID;
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        CREATE TRIGGER payment_collections_state_guard_insert
                        BEFORE INSERT ON payment_collections
                        FOR EACH ROW
                        BEGIN
                            SELECT CASE WHEN NEW.status NOT IN ('pending', 'authorized', 'captured', 'cancelled')
                                THEN RAISE(ABORT, 'invalid payment collection status') END;
                            SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid payment collection currency') END;
                            SELECT CASE WHEN NEW.amount <= 0
                                OR NEW.authorized_amount < 0
                                OR NEW.authorized_amount > NEW.amount
                                OR NEW.captured_amount < 0
                                OR NEW.captured_amount > NEW.authorized_amount
                                THEN RAISE(ABORT, 'invalid payment collection amounts') END;
                            SELECT CASE WHEN NOT (
                                (NEW.status = 'pending'
                                    AND NEW.authorized_amount = 0
                                    AND NEW.captured_amount = 0
                                    AND NEW.authorized_at IS NULL
                                    AND NEW.captured_at IS NULL
                                    AND NEW.cancelled_at IS NULL)
                                OR
                                (NEW.status = 'authorized'
                                    AND NEW.authorized_amount > 0
                                    AND NEW.captured_amount = 0
                                    AND NEW.provider_id IS NOT NULL
                                    AND trim(NEW.provider_id) <> ''
                                    AND NEW.authorized_at IS NOT NULL
                                    AND NEW.captured_at IS NULL
                                    AND NEW.cancelled_at IS NULL)
                                OR
                                (NEW.status = 'captured'
                                    AND NEW.authorized_amount > 0
                                    AND NEW.captured_amount > 0
                                    AND NEW.provider_id IS NOT NULL
                                    AND trim(NEW.provider_id) <> ''
                                    AND NEW.authorized_at IS NOT NULL
                                    AND NEW.captured_at IS NOT NULL
                                    AND NEW.cancelled_at IS NULL)
                                OR
                                (NEW.status = 'cancelled'
                                    AND NEW.captured_amount = 0
                                    AND NEW.captured_at IS NULL
                                    AND NEW.cancelled_at IS NOT NULL)
                            ) THEN RAISE(ABORT, 'invalid payment collection state') END;
                        END;

                        CREATE TRIGGER payment_collections_state_guard_update
                        BEFORE UPDATE OF status, currency_code, amount, authorized_amount, captured_amount,
                            provider_id, authorized_at, captured_at, cancelled_at
                        ON payment_collections
                        FOR EACH ROW
                        BEGIN
                            SELECT CASE WHEN NEW.status NOT IN ('pending', 'authorized', 'captured', 'cancelled')
                                THEN RAISE(ABORT, 'invalid payment collection status') END;
                            SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid payment collection currency') END;
                            SELECT CASE WHEN NEW.amount <= 0
                                OR NEW.authorized_amount < 0
                                OR NEW.authorized_amount > NEW.amount
                                OR NEW.captured_amount < 0
                                OR NEW.captured_amount > NEW.authorized_amount
                                THEN RAISE(ABORT, 'invalid payment collection amounts') END;
                            SELECT CASE WHEN NOT (
                                (NEW.status = 'pending'
                                    AND NEW.authorized_amount = 0
                                    AND NEW.captured_amount = 0
                                    AND NEW.authorized_at IS NULL
                                    AND NEW.captured_at IS NULL
                                    AND NEW.cancelled_at IS NULL)
                                OR
                                (NEW.status = 'authorized'
                                    AND NEW.authorized_amount > 0
                                    AND NEW.captured_amount = 0
                                    AND NEW.provider_id IS NOT NULL
                                    AND trim(NEW.provider_id) <> ''
                                    AND NEW.authorized_at IS NOT NULL
                                    AND NEW.captured_at IS NULL
                                    AND NEW.cancelled_at IS NULL)
                                OR
                                (NEW.status = 'captured'
                                    AND NEW.authorized_amount > 0
                                    AND NEW.captured_amount > 0
                                    AND NEW.provider_id IS NOT NULL
                                    AND trim(NEW.provider_id) <> ''
                                    AND NEW.authorized_at IS NOT NULL
                                    AND NEW.captured_at IS NOT NULL
                                    AND NEW.cancelled_at IS NULL)
                                OR
                                (NEW.status = 'cancelled'
                                    AND NEW.captured_amount = 0
                                    AND NEW.captured_at IS NULL
                                    AND NEW.cancelled_at IS NOT NULL)
                            ) THEN RAISE(ABORT, 'invalid payment collection state') END;
                        END;

                        CREATE TRIGGER payments_state_guard_insert
                        BEFORE INSERT ON payments
                        FOR EACH ROW
                        BEGIN
                            SELECT CASE WHEN NEW.status NOT IN ('authorized', 'captured', 'cancelled')
                                THEN RAISE(ABORT, 'invalid payment status') END;
                            SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid payment currency') END;
                            SELECT CASE WHEN trim(NEW.provider_id) = '' OR trim(NEW.provider_payment_id) = ''
                                THEN RAISE(ABORT, 'invalid payment provider identity') END;
                            SELECT CASE WHEN NEW.amount <= 0 OR NEW.captured_amount < 0 OR NEW.captured_amount > NEW.amount
                                THEN RAISE(ABORT, 'invalid payment amounts') END;
                            SELECT CASE WHEN NOT (
                                (NEW.status = 'authorized'
                                    AND NEW.captured_amount = 0
                                    AND NEW.authorized_at IS NOT NULL
                                    AND NEW.captured_at IS NULL
                                    AND NEW.cancelled_at IS NULL)
                                OR
                                (NEW.status = 'captured'
                                    AND NEW.captured_amount > 0
                                    AND NEW.authorized_at IS NOT NULL
                                    AND NEW.captured_at IS NOT NULL
                                    AND NEW.cancelled_at IS NULL)
                                OR
                                (NEW.status = 'cancelled'
                                    AND NEW.captured_amount = 0
                                    AND NEW.authorized_at IS NOT NULL
                                    AND NEW.captured_at IS NULL
                                    AND NEW.cancelled_at IS NOT NULL)
                            ) THEN RAISE(ABORT, 'invalid payment state') END;
                        END;

                        CREATE TRIGGER payments_state_guard_update
                        BEFORE UPDATE OF status, currency_code, amount, captured_amount, provider_id,
                            provider_payment_id, authorized_at, captured_at, cancelled_at
                        ON payments
                        FOR EACH ROW
                        BEGIN
                            SELECT CASE WHEN NEW.status NOT IN ('authorized', 'captured', 'cancelled')
                                THEN RAISE(ABORT, 'invalid payment status') END;
                            SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid payment currency') END;
                            SELECT CASE WHEN trim(NEW.provider_id) = '' OR trim(NEW.provider_payment_id) = ''
                                THEN RAISE(ABORT, 'invalid payment provider identity') END;
                            SELECT CASE WHEN NEW.amount <= 0 OR NEW.captured_amount < 0 OR NEW.captured_amount > NEW.amount
                                THEN RAISE(ABORT, 'invalid payment amounts') END;
                            SELECT CASE WHEN NOT (
                                (NEW.status = 'authorized'
                                    AND NEW.captured_amount = 0
                                    AND NEW.authorized_at IS NOT NULL
                                    AND NEW.captured_at IS NULL
                                    AND NEW.cancelled_at IS NULL)
                                OR
                                (NEW.status = 'captured'
                                    AND NEW.captured_amount > 0
                                    AND NEW.authorized_at IS NOT NULL
                                    AND NEW.captured_at IS NOT NULL
                                    AND NEW.cancelled_at IS NULL)
                                OR
                                (NEW.status = 'cancelled'
                                    AND NEW.captured_amount = 0
                                    AND NEW.authorized_at IS NOT NULL
                                    AND NEW.captured_at IS NULL
                                    AND NEW.cancelled_at IS NOT NULL)
                            ) THEN RAISE(ABORT, 'invalid payment state') END;
                        END;

                        CREATE TRIGGER refunds_state_timestamps_guard_insert
                        BEFORE INSERT ON refunds
                        FOR EACH ROW
                        BEGIN
                            SELECT CASE WHEN NOT (
                                (NEW.status = 'pending' AND NEW.refunded_at IS NULL AND NEW.cancelled_at IS NULL)
                                OR (NEW.status = 'refunded' AND NEW.refunded_at IS NOT NULL AND NEW.cancelled_at IS NULL)
                                OR (NEW.status = 'cancelled' AND NEW.refunded_at IS NULL AND NEW.cancelled_at IS NOT NULL)
                            ) THEN RAISE(ABORT, 'invalid refund state timestamps') END;
                        END;

                        CREATE TRIGGER refunds_state_timestamps_guard_update
                        BEFORE UPDATE OF status, refunded_at, cancelled_at ON refunds
                        FOR EACH ROW
                        BEGIN
                            SELECT CASE WHEN NOT (
                                (NEW.status = 'pending' AND NEW.refunded_at IS NULL AND NEW.cancelled_at IS NULL)
                                OR (NEW.status = 'refunded' AND NEW.refunded_at IS NOT NULL AND NEW.cancelled_at IS NULL)
                                OR (NEW.status = 'cancelled' AND NEW.refunded_at IS NULL AND NEW.cancelled_at IS NOT NULL)
                            ) THEN RAISE(ABORT, 'invalid refund state timestamps') END;
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
                        ALTER TABLE refunds DROP CONSTRAINT IF EXISTS ck_refunds_state_timestamps;
                        ALTER TABLE payments
                            DROP CONSTRAINT IF EXISTS ck_payments_state,
                            DROP CONSTRAINT IF EXISTS ck_payments_amounts,
                            DROP CONSTRAINT IF EXISTS ck_payments_identity,
                            DROP CONSTRAINT IF EXISTS ck_payments_currency,
                            DROP CONSTRAINT IF EXISTS ck_payments_status;
                        ALTER TABLE payment_collections
                            DROP CONSTRAINT IF EXISTS ck_payment_collections_state,
                            DROP CONSTRAINT IF EXISTS ck_payment_collections_amounts,
                            DROP CONSTRAINT IF EXISTS ck_payment_collections_currency,
                            DROP CONSTRAINT IF EXISTS ck_payment_collections_status;
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS refunds_state_timestamps_guard_update;
                        DROP TRIGGER IF EXISTS refunds_state_timestamps_guard_insert;
                        DROP TRIGGER IF EXISTS payments_state_guard_update;
                        DROP TRIGGER IF EXISTS payments_state_guard_insert;
                        DROP TRIGGER IF EXISTS payment_collections_state_guard_update;
                        DROP TRIGGER IF EXISTS payment_collections_state_guard_insert;
                        "#,
                    )
                    .await?;
            }
            _ => {}
        }

        Ok(())
    }
}
