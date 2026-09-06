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
            DatabaseBackend::Postgres => restore_postgres(manager).await?,
            DatabaseBackend::Sqlite => restore_sqlite(manager).await?,
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
            ALTER TABLE payment_provider_operations
                DROP CONSTRAINT IF EXISTS ck_payment_provider_operations_status,
                DROP CONSTRAINT IF EXISTS ck_payment_provider_operations_state;

            ALTER TABLE payment_provider_operations
                ADD CONSTRAINT ck_payment_provider_operations_status
                CHECK (status IN (
                    'pending',
                    'executing',
                    'provider_succeeded',
                    'provider_error',
                    'reconciliation_required',
                    'committed'
                )),
                ADD CONSTRAINT ck_payment_provider_operations_state
                CHECK (
                    (status IN ('pending', 'executing')
                        AND provider_completed_at IS NULL
                        AND committed_at IS NULL)
                    OR
                    (status = 'provider_error'
                        AND error_message IS NOT NULL
                        AND committed_at IS NULL)
                    OR
                    (status IN ('provider_succeeded', 'reconciliation_required')
                        AND provider_completed_at IS NOT NULL
                        AND committed_at IS NULL)
                    OR
                    (status = 'committed'
                        AND provider_completed_at IS NOT NULL
                        AND committed_at IS NOT NULL)
                );

            CREATE OR REPLACE FUNCTION enforce_payment_provider_operation_lifecycle()
            RETURNS trigger AS $$
            BEGIN
                IF NEW.id IS DISTINCT FROM OLD.id
                   OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
                   OR NEW.payment_collection_id IS DISTINCT FROM OLD.payment_collection_id
                   OR NEW.operation IS DISTINCT FROM OLD.operation
                   OR NEW.provider_id IS DISTINCT FROM OLD.provider_id
                   OR NEW.idempotency_key IS DISTINCT FROM OLD.idempotency_key
                   OR NEW.request_payload IS DISTINCT FROM OLD.request_payload THEN
                    RAISE EXCEPTION 'payment provider operation identity is immutable'
                        USING ERRCODE = '23514';
                END IF;

                IF NEW.status IS DISTINCT FROM OLD.status
                   AND NOT (
                        (OLD.status IN ('pending', 'provider_error')
                            AND NEW.status = 'executing')
                        OR (OLD.status = 'executing'
                            AND NEW.status IN ('provider_succeeded', 'provider_error'))
                        OR (OLD.status = 'provider_succeeded'
                            AND NEW.status IN ('reconciliation_required', 'committed'))
                        OR (OLD.status = 'reconciliation_required'
                            AND NEW.status = 'committed')
                   ) THEN
                    RAISE EXCEPTION 'invalid payment provider operation transition from % to %',
                        OLD.status, NEW.status
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

async fn install_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER IF EXISTS payment_provider_operations_state_guard_insert;
            DROP TRIGGER IF EXISTS payment_provider_operations_state_guard_update;

            CREATE TRIGGER payment_provider_operations_state_guard_insert
            BEFORE INSERT ON payment_provider_operations
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NEW.operation NOT IN ('authorize', 'capture', 'cancel', 'refund')
                    THEN RAISE(ABORT, 'invalid payment provider operation') END;
                SELECT CASE WHEN NEW.status NOT IN (
                    'pending', 'executing', 'provider_succeeded', 'provider_error',
                    'reconciliation_required', 'committed'
                ) THEN RAISE(ABORT, 'invalid payment provider operation status') END;
                SELECT CASE WHEN trim(NEW.provider_id) = '' OR trim(NEW.idempotency_key) = ''
                    THEN RAISE(ABORT, 'invalid payment provider operation identity') END;
                SELECT CASE WHEN NOT (
                    (NEW.status IN ('pending', 'executing')
                        AND NEW.provider_completed_at IS NULL
                        AND NEW.committed_at IS NULL)
                    OR
                    (NEW.status = 'provider_error'
                        AND NEW.error_message IS NOT NULL
                        AND NEW.committed_at IS NULL)
                    OR
                    (NEW.status IN ('provider_succeeded', 'reconciliation_required')
                        AND NEW.provider_completed_at IS NOT NULL
                        AND NEW.committed_at IS NULL)
                    OR
                    (NEW.status = 'committed'
                        AND NEW.provider_completed_at IS NOT NULL
                        AND NEW.committed_at IS NOT NULL)
                ) THEN RAISE(ABORT, 'invalid payment provider operation state') END;
            END;

            CREATE TRIGGER payment_provider_operations_state_guard_update
            BEFORE UPDATE ON payment_provider_operations
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NEW.id IS NOT OLD.id
                    OR NEW.tenant_id IS NOT OLD.tenant_id
                    OR NEW.payment_collection_id IS NOT OLD.payment_collection_id
                    OR NEW.operation IS NOT OLD.operation
                    OR NEW.provider_id IS NOT OLD.provider_id
                    OR NEW.idempotency_key IS NOT OLD.idempotency_key
                    OR NEW.request_payload IS NOT OLD.request_payload
                    THEN RAISE(ABORT, 'payment provider operation identity is immutable') END;
                SELECT CASE WHEN NEW.status NOT IN (
                    'pending', 'executing', 'provider_succeeded', 'provider_error',
                    'reconciliation_required', 'committed'
                ) THEN RAISE(ABORT, 'invalid payment provider operation status') END;
                SELECT CASE WHEN NOT (
                    (OLD.status = NEW.status)
                    OR (OLD.status IN ('pending', 'provider_error') AND NEW.status = 'executing')
                    OR (OLD.status = 'executing'
                        AND NEW.status IN ('provider_succeeded', 'provider_error'))
                    OR (OLD.status = 'provider_succeeded'
                        AND NEW.status IN ('reconciliation_required', 'committed'))
                    OR (OLD.status = 'reconciliation_required'
                        AND NEW.status = 'committed')
                ) THEN RAISE(ABORT, 'invalid payment provider operation transition') END;
                SELECT CASE WHEN NOT (
                    (NEW.status IN ('pending', 'executing')
                        AND NEW.provider_completed_at IS NULL
                        AND NEW.committed_at IS NULL)
                    OR
                    (NEW.status = 'provider_error'
                        AND NEW.error_message IS NOT NULL
                        AND NEW.committed_at IS NULL)
                    OR
                    (NEW.status IN ('provider_succeeded', 'reconciliation_required')
                        AND NEW.provider_completed_at IS NOT NULL
                        AND NEW.committed_at IS NULL)
                    OR
                    (NEW.status = 'committed'
                        AND NEW.provider_completed_at IS NOT NULL
                        AND NEW.committed_at IS NOT NULL)
                ) THEN RAISE(ABORT, 'invalid payment provider operation state') END;
            END;
            "#,
        )
        .await?;
    Ok(())
}

async fn restore_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            UPDATE payment_provider_operations
            SET status = 'provider_error',
                error_message = COALESCE(error_message, 'execution interrupted by migration rollback'),
                updated_at = CURRENT_TIMESTAMP
            WHERE status = 'executing';

            ALTER TABLE payment_provider_operations
                DROP CONSTRAINT IF EXISTS ck_payment_provider_operations_status,
                DROP CONSTRAINT IF EXISTS ck_payment_provider_operations_state;

            ALTER TABLE payment_provider_operations
                ADD CONSTRAINT ck_payment_provider_operations_status
                CHECK (status IN (
                    'pending',
                    'provider_succeeded',
                    'provider_error',
                    'reconciliation_required',
                    'committed'
                )),
                ADD CONSTRAINT ck_payment_provider_operations_state
                CHECK (
                    (status = 'pending'
                        AND provider_completed_at IS NULL
                        AND committed_at IS NULL)
                    OR
                    (status = 'provider_error'
                        AND error_message IS NOT NULL
                        AND committed_at IS NULL)
                    OR
                    (status IN ('provider_succeeded', 'reconciliation_required')
                        AND provider_completed_at IS NOT NULL
                        AND committed_at IS NULL)
                    OR
                    (status = 'committed'
                        AND provider_completed_at IS NOT NULL
                        AND committed_at IS NOT NULL)
                );

            CREATE OR REPLACE FUNCTION enforce_payment_provider_operation_lifecycle()
            RETURNS trigger AS $$
            BEGIN
                IF NEW.id IS DISTINCT FROM OLD.id
                   OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
                   OR NEW.payment_collection_id IS DISTINCT FROM OLD.payment_collection_id
                   OR NEW.operation IS DISTINCT FROM OLD.operation
                   OR NEW.provider_id IS DISTINCT FROM OLD.provider_id
                   OR NEW.idempotency_key IS DISTINCT FROM OLD.idempotency_key
                   OR NEW.request_payload IS DISTINCT FROM OLD.request_payload THEN
                    RAISE EXCEPTION 'payment provider operation identity is immutable'
                        USING ERRCODE = '23514';
                END IF;

                IF NEW.status IS DISTINCT FROM OLD.status
                   AND NOT (
                        (OLD.status IN ('pending', 'provider_error')
                            AND NEW.status IN ('provider_succeeded', 'provider_error'))
                        OR (OLD.status = 'provider_succeeded'
                            AND NEW.status IN ('reconciliation_required', 'committed'))
                        OR (OLD.status = 'reconciliation_required'
                            AND NEW.status = 'committed')
                   ) THEN
                    RAISE EXCEPTION 'invalid payment provider operation transition from % to %',
                        OLD.status, NEW.status
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

async fn restore_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            UPDATE payment_provider_operations
            SET status = 'provider_error',
                error_message = COALESCE(error_message, 'execution interrupted by migration rollback'),
                updated_at = CURRENT_TIMESTAMP
            WHERE status = 'executing';

            DROP TRIGGER IF EXISTS payment_provider_operations_state_guard_insert;
            DROP TRIGGER IF EXISTS payment_provider_operations_state_guard_update;

            CREATE TRIGGER payment_provider_operations_state_guard_insert
            BEFORE INSERT ON payment_provider_operations
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NEW.operation NOT IN ('authorize', 'capture', 'cancel', 'refund')
                    THEN RAISE(ABORT, 'invalid payment provider operation') END;
                SELECT CASE WHEN NEW.status NOT IN (
                    'pending', 'provider_succeeded', 'provider_error',
                    'reconciliation_required', 'committed'
                ) THEN RAISE(ABORT, 'invalid payment provider operation status') END;
                SELECT CASE WHEN trim(NEW.provider_id) = '' OR trim(NEW.idempotency_key) = ''
                    THEN RAISE(ABORT, 'invalid payment provider operation identity') END;
                SELECT CASE WHEN NOT (
                    (NEW.status = 'pending'
                        AND NEW.provider_completed_at IS NULL
                        AND NEW.committed_at IS NULL)
                    OR
                    (NEW.status = 'provider_error'
                        AND NEW.error_message IS NOT NULL
                        AND NEW.committed_at IS NULL)
                    OR
                    (NEW.status IN ('provider_succeeded', 'reconciliation_required')
                        AND NEW.provider_completed_at IS NOT NULL
                        AND NEW.committed_at IS NULL)
                    OR
                    (NEW.status = 'committed'
                        AND NEW.provider_completed_at IS NOT NULL
                        AND NEW.committed_at IS NOT NULL)
                ) THEN RAISE(ABORT, 'invalid payment provider operation state') END;
            END;

            CREATE TRIGGER payment_provider_operations_state_guard_update
            BEFORE UPDATE ON payment_provider_operations
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NEW.id IS NOT OLD.id
                    OR NEW.tenant_id IS NOT OLD.tenant_id
                    OR NEW.payment_collection_id IS NOT OLD.payment_collection_id
                    OR NEW.operation IS NOT OLD.operation
                    OR NEW.provider_id IS NOT OLD.provider_id
                    OR NEW.idempotency_key IS NOT OLD.idempotency_key
                    OR NEW.request_payload IS NOT OLD.request_payload
                    THEN RAISE(ABORT, 'payment provider operation identity is immutable') END;
                SELECT CASE WHEN NEW.status NOT IN (
                    'pending', 'provider_succeeded', 'provider_error',
                    'reconciliation_required', 'committed'
                ) THEN RAISE(ABORT, 'invalid payment provider operation status') END;
                SELECT CASE WHEN NOT (
                    (OLD.status = NEW.status)
                    OR (OLD.status IN ('pending', 'provider_error')
                        AND NEW.status IN ('provider_succeeded', 'provider_error'))
                    OR (OLD.status = 'provider_succeeded'
                        AND NEW.status IN ('reconciliation_required', 'committed'))
                    OR (OLD.status = 'reconciliation_required'
                        AND NEW.status = 'committed')
                ) THEN RAISE(ABORT, 'invalid payment provider operation transition') END;
                SELECT CASE WHEN NOT (
                    (NEW.status = 'pending'
                        AND NEW.provider_completed_at IS NULL
                        AND NEW.committed_at IS NULL)
                    OR
                    (NEW.status = 'provider_error'
                        AND NEW.error_message IS NOT NULL
                        AND NEW.committed_at IS NULL)
                    OR
                    (NEW.status IN ('provider_succeeded', 'reconciliation_required')
                        AND NEW.provider_completed_at IS NOT NULL
                        AND NEW.committed_at IS NULL)
                    OR
                    (NEW.status = 'committed'
                        AND NEW.provider_completed_at IS NOT NULL
                        AND NEW.committed_at IS NOT NULL)
                ) THEN RAISE(ABORT, 'invalid payment provider operation state') END;
            END;
            "#,
        )
        .await?;
    Ok(())
}
