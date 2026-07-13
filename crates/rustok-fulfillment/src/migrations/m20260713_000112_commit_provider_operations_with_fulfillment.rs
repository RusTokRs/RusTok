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
                        DROP TRIGGER IF EXISTS fulfillments_commit_provider_operation ON fulfillments;
                        DROP FUNCTION IF EXISTS commit_fulfillment_provider_operation();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS fulfillments_provider_operation_guard;
                        DROP TRIGGER IF EXISTS fulfillments_commit_provider_operation;
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
            CREATE OR REPLACE FUNCTION commit_fulfillment_provider_operation()
            RETURNS trigger AS $$
            DECLARE
                old_operation_id_text text;
                new_operation_id_text text;
                operation_id uuid;
            BEGIN
                old_operation_id_text := OLD.metadata #>> '{provider_operation,id}';
                new_operation_id_text := NEW.metadata #>> '{provider_operation,id}';

                IF new_operation_id_text IS NOT NULL
                   AND new_operation_id_text IS DISTINCT FROM old_operation_id_text THEN
                    BEGIN
                        operation_id := new_operation_id_text::uuid;
                    EXCEPTION WHEN invalid_text_representation THEN
                        RAISE EXCEPTION 'invalid fulfillment provider operation id in metadata'
                            USING ERRCODE = '23514';
                    END;

                    UPDATE fulfillment_provider_operations
                    SET status = 'committed',
                        error_message = NULL,
                        updated_at = CURRENT_TIMESTAMP,
                        committed_at = COALESCE(committed_at, CURRENT_TIMESTAMP)
                    WHERE id = operation_id
                      AND tenant_id = NEW.tenant_id
                      AND fulfillment_id = NEW.id
                      AND status IN ('provider_succeeded', 'reconciliation_required');

                    IF NOT FOUND AND NOT EXISTS (
                        SELECT 1
                        FROM fulfillment_provider_operations
                        WHERE id = operation_id
                          AND tenant_id = NEW.tenant_id
                          AND fulfillment_id = NEW.id
                          AND status = 'committed'
                    ) THEN
                        RAISE EXCEPTION 'fulfillment provider operation is missing or not ready to commit'
                            USING ERRCODE = '23514';
                    END IF;
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER fulfillments_commit_provider_operation
            BEFORE UPDATE OF metadata ON fulfillments
            FOR EACH ROW
            EXECUTE FUNCTION commit_fulfillment_provider_operation();
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
            CREATE TRIGGER fulfillments_provider_operation_guard
            BEFORE UPDATE OF metadata ON fulfillments
            FOR EACH ROW
            WHEN json_extract(NEW.metadata, '$.provider_operation.id') IS NOT NULL
             AND json_extract(NEW.metadata, '$.provider_operation.id')
                 IS NOT json_extract(OLD.metadata, '$.provider_operation.id')
            BEGIN
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1
                    FROM fulfillment_provider_operations
                    WHERE id = json_extract(NEW.metadata, '$.provider_operation.id')
                      AND tenant_id = NEW.tenant_id
                      AND fulfillment_id = NEW.id
                      AND status IN ('provider_succeeded', 'reconciliation_required', 'committed')
                ) THEN RAISE(ABORT, 'fulfillment provider operation is missing or not ready to commit') END;
            END;

            CREATE TRIGGER fulfillments_commit_provider_operation
            AFTER UPDATE OF metadata ON fulfillments
            FOR EACH ROW
            WHEN json_extract(NEW.metadata, '$.provider_operation.id') IS NOT NULL
             AND json_extract(NEW.metadata, '$.provider_operation.id')
                 IS NOT json_extract(OLD.metadata, '$.provider_operation.id')
            BEGIN
                UPDATE fulfillment_provider_operations
                SET status = 'committed',
                    error_message = NULL,
                    updated_at = CURRENT_TIMESTAMP,
                    committed_at = COALESCE(committed_at, CURRENT_TIMESTAMP)
                WHERE id = json_extract(NEW.metadata, '$.provider_operation.id')
                  AND tenant_id = NEW.tenant_id
                  AND fulfillment_id = NEW.id
                  AND status IN ('provider_succeeded', 'reconciliation_required');
            END;
            "#,
        )
        .await?;
    Ok(())
}
