use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
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

                CREATE TRIGGER payment_provider_operations_lifecycle_guard
                BEFORE UPDATE ON payment_provider_operations
                FOR EACH ROW
                EXECUTE FUNCTION enforce_payment_provider_operation_lifecycle();
                "#,
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        if manager.get_database_backend() != DatabaseBackend::Postgres {
            return Ok(());
        }

        manager
            .get_connection()
            .execute_unprepared(
                r#"
                DROP TRIGGER IF EXISTS payment_provider_operations_lifecycle_guard
                    ON payment_provider_operations;
                DROP FUNCTION IF EXISTS enforce_payment_provider_operation_lifecycle();
                "#,
            )
            .await?;

        Ok(())
    }
}
