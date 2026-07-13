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
            CREATE OR REPLACE FUNCTION enforce_fulfillment_provider_operation_lifecycle()
            RETURNS trigger AS $$
            BEGIN
                IF NEW.id IS DISTINCT FROM OLD.id
                   OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
                   OR NEW.fulfillment_id IS DISTINCT FROM OLD.fulfillment_id
                   OR NEW.operation IS DISTINCT FROM OLD.operation
                   OR NEW.provider_id IS DISTINCT FROM OLD.provider_id
                   OR NEW.idempotency_key IS DISTINCT FROM OLD.idempotency_key
                   OR NEW.request_payload IS DISTINCT FROM OLD.request_payload THEN
                    RAISE EXCEPTION 'fulfillment provider operation identity is immutable'
                        USING ERRCODE = '23514';
                END IF;

                IF NEW.status IS DISTINCT FROM OLD.status
                   AND NOT (
                        (OLD.status IN ('pending', 'provider_error') AND NEW.status = 'executing')
                        OR (OLD.status = 'executing'
                            AND NEW.status IN (
                                'provider_succeeded', 'provider_error', 'reconciliation_required'
                            ))
                        OR (OLD.status = 'provider_succeeded'
                            AND NEW.status IN ('reconciliation_required', 'committed'))
                        OR (OLD.status = 'reconciliation_required'
                            AND NEW.status = 'committed')
                        OR (OLD.status = 'reconciliation_required'
                            AND OLD.provider_result IS NULL
                            AND NEW.status IN ('provider_succeeded', 'provider_error'))
                   ) THEN
                    RAISE EXCEPTION 'invalid fulfillment provider operation transition from % to %',
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
            DROP TRIGGER IF EXISTS fulfillment_provider_operations_state_guard_update;

            CREATE TRIGGER fulfillment_provider_operations_state_guard_update
            BEFORE UPDATE ON fulfillment_provider_operations
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NEW.id IS NOT OLD.id
                    OR NEW.tenant_id IS NOT OLD.tenant_id
                    OR NEW.fulfillment_id IS NOT OLD.fulfillment_id
                    OR NEW.operation IS NOT OLD.operation
                    OR NEW.provider_id IS NOT OLD.provider_id
                    OR NEW.idempotency_key IS NOT OLD.idempotency_key
                    OR NEW.request_payload IS NOT OLD.request_payload
                    THEN RAISE(ABORT, 'fulfillment provider operation identity is immutable') END;
                SELECT CASE WHEN NEW.status NOT IN (
                    'pending', 'executing', 'provider_succeeded', 'provider_error',
                    'reconciliation_required', 'committed'
                ) THEN RAISE(ABORT, 'invalid fulfillment provider operation status') END;
                SELECT CASE WHEN NOT (
                    OLD.status = NEW.status
                    OR (OLD.status IN ('pending', 'provider_error') AND NEW.status = 'executing')
                    OR (OLD.status = 'executing'
                        AND NEW.status IN (
                            'provider_succeeded', 'provider_error', 'reconciliation_required'
                        ))
                    OR (OLD.status = 'provider_succeeded'
                        AND NEW.status IN ('reconciliation_required', 'committed'))
                    OR (OLD.status = 'reconciliation_required' AND NEW.status = 'committed')
                    OR (OLD.status = 'reconciliation_required'
                        AND OLD.provider_result IS NULL
                        AND NEW.status IN ('provider_succeeded', 'provider_error'))
                ) THEN RAISE(ABORT, 'invalid fulfillment provider operation transition') END;
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
                ) THEN RAISE(ABORT, 'invalid fulfillment provider operation state') END;
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
            UPDATE fulfillment_provider_operations
            SET status = 'provider_error',
                provider_completed_at = NULL,
                error_message = COALESCE(error_message, 'unresolved execution during migration rollback'),
                updated_at = CURRENT_TIMESTAMP
            WHERE status = 'reconciliation_required'
              AND provider_result IS NULL;

            CREATE OR REPLACE FUNCTION enforce_fulfillment_provider_operation_lifecycle()
            RETURNS trigger AS $$
            BEGIN
                IF NEW.id IS DISTINCT FROM OLD.id
                   OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
                   OR NEW.fulfillment_id IS DISTINCT FROM OLD.fulfillment_id
                   OR NEW.operation IS DISTINCT FROM OLD.operation
                   OR NEW.provider_id IS DISTINCT FROM OLD.provider_id
                   OR NEW.idempotency_key IS DISTINCT FROM OLD.idempotency_key
                   OR NEW.request_payload IS DISTINCT FROM OLD.request_payload THEN
                    RAISE EXCEPTION 'fulfillment provider operation identity is immutable'
                        USING ERRCODE = '23514';
                END IF;

                IF NEW.status IS DISTINCT FROM OLD.status
                   AND NOT (
                        (OLD.status IN ('pending', 'provider_error') AND NEW.status = 'executing')
                        OR (OLD.status = 'executing'
                            AND NEW.status IN ('provider_succeeded', 'provider_error'))
                        OR (OLD.status = 'provider_succeeded'
                            AND NEW.status IN ('reconciliation_required', 'committed'))
                        OR (OLD.status = 'reconciliation_required' AND NEW.status = 'committed')
                   ) THEN
                    RAISE EXCEPTION 'invalid fulfillment provider operation transition from % to %',
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
            UPDATE fulfillment_provider_operations
            SET status = 'provider_error',
                provider_completed_at = NULL,
                error_message = COALESCE(error_message, 'unresolved execution during migration rollback'),
                updated_at = CURRENT_TIMESTAMP
            WHERE status = 'reconciliation_required'
              AND provider_result IS NULL;

            DROP TRIGGER IF EXISTS fulfillment_provider_operations_state_guard_update;

            CREATE TRIGGER fulfillment_provider_operations_state_guard_update
            BEFORE UPDATE ON fulfillment_provider_operations
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NEW.id IS NOT OLD.id
                    OR NEW.tenant_id IS NOT OLD.tenant_id
                    OR NEW.fulfillment_id IS NOT OLD.fulfillment_id
                    OR NEW.operation IS NOT OLD.operation
                    OR NEW.provider_id IS NOT OLD.provider_id
                    OR NEW.idempotency_key IS NOT OLD.idempotency_key
                    OR NEW.request_payload IS NOT OLD.request_payload
                    THEN RAISE(ABORT, 'fulfillment provider operation identity is immutable') END;
                SELECT CASE WHEN NEW.status NOT IN (
                    'pending', 'executing', 'provider_succeeded', 'provider_error',
                    'reconciliation_required', 'committed'
                ) THEN RAISE(ABORT, 'invalid fulfillment provider operation status') END;
                SELECT CASE WHEN NOT (
                    OLD.status = NEW.status
                    OR (OLD.status IN ('pending', 'provider_error') AND NEW.status = 'executing')
                    OR (OLD.status = 'executing'
                        AND NEW.status IN ('provider_succeeded', 'provider_error'))
                    OR (OLD.status = 'provider_succeeded'
                        AND NEW.status IN ('reconciliation_required', 'committed'))
                    OR (OLD.status = 'reconciliation_required' AND NEW.status = 'committed')
                ) THEN RAISE(ABORT, 'invalid fulfillment provider operation transition') END;
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
                ) THEN RAISE(ABORT, 'invalid fulfillment provider operation state') END;
            END;
            "#,
        )
        .await?;
    Ok(())
}
