use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::{ConnectionTrait, DatabaseBackend, Statement};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        ensure_existing_rows_are_tenant_consistent(manager).await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite(manager).await?,
            DatabaseBackend::MySql => install_mysql(manager).await?,
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
                        DROP TRIGGER IF EXISTS payment_provider_operations_ownership_guard
                            ON payment_provider_operations;
                        DROP FUNCTION IF EXISTS enforce_payment_provider_operation_ownership();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS payment_provider_operations_ownership_guard;
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::MySql => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS payment_provider_operations_ownership_guard
                        "#,
                    )
                    .await?;
            }
        }

        Ok(())
    }
}

async fn ensure_existing_rows_are_tenant_consistent(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let violation = manager
        .get_connection()
        .query_one_raw(Statement::from_string(
            manager.get_database_backend(),
            r#"
            SELECT 1
            FROM payment_provider_operations operation
            JOIN payment_collections collection
              ON collection.id = operation.payment_collection_id
            WHERE operation.tenant_id <> collection.tenant_id
               OR (
                    operation.refund_id IS NOT NULL
                    AND NOT EXISTS (
                        SELECT 1
                        FROM refunds refund
                        WHERE refund.id = operation.refund_id
                          AND refund.tenant_id = operation.tenant_id
                          AND refund.payment_collection_id = operation.payment_collection_id
                    )
               )
            LIMIT 1
            "#
            .to_owned(),
        ))
        .await?;

    if violation.is_some() {
        return Err(DbErr::Custom(
            "cannot install payment provider operation tenant ownership guard while existing rows violate payment collection/refund ownership"
                .to_string(),
        ));
    }

    Ok(())
}

async fn install_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE OR REPLACE FUNCTION enforce_payment_provider_operation_ownership()
            RETURNS trigger AS $$
            BEGIN
                IF NOT EXISTS (
                    SELECT 1
                    FROM payment_collections
                    WHERE id = NEW.payment_collection_id
                      AND tenant_id = NEW.tenant_id
                ) THEN
                    RAISE EXCEPTION 'payment provider operation does not belong to payment collection tenant'
                        USING ERRCODE = '23514';
                END IF;

                IF NEW.refund_id IS NOT NULL
                   AND NOT EXISTS (
                        SELECT 1
                        FROM refunds
                        WHERE id = NEW.refund_id
                          AND tenant_id = NEW.tenant_id
                          AND payment_collection_id = NEW.payment_collection_id
                   ) THEN
                    RAISE EXCEPTION 'payment provider operation refund does not belong to payment collection tenant'
                        USING ERRCODE = '23514';
                END IF;

                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            DROP TRIGGER IF EXISTS payment_provider_operations_ownership_guard
                ON payment_provider_operations;

            CREATE TRIGGER payment_provider_operations_ownership_guard
            BEFORE INSERT ON payment_provider_operations
            FOR EACH ROW
            EXECUTE FUNCTION enforce_payment_provider_operation_ownership();
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
            DROP TRIGGER IF EXISTS payment_provider_operations_ownership_guard;

            CREATE TRIGGER payment_provider_operations_ownership_guard
            BEFORE INSERT ON payment_provider_operations
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1
                    FROM payment_collections
                    WHERE id = NEW.payment_collection_id
                      AND tenant_id = NEW.tenant_id
                ) THEN RAISE(ABORT, 'payment provider operation does not belong to payment collection tenant') END;

                SELECT CASE WHEN NEW.refund_id IS NOT NULL
                    AND NOT EXISTS (
                        SELECT 1
                        FROM refunds
                        WHERE id = NEW.refund_id
                          AND tenant_id = NEW.tenant_id
                          AND payment_collection_id = NEW.payment_collection_id
                    )
                    THEN RAISE(ABORT, 'payment provider operation refund does not belong to payment collection tenant') END;
            END;
            "#,
        )
        .await?;

    Ok(())
}

async fn install_mysql(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER IF EXISTS payment_provider_operations_ownership_guard;

            CREATE TRIGGER payment_provider_operations_ownership_guard
            BEFORE INSERT ON payment_provider_operations
            FOR EACH ROW
            BEGIN
                IF NOT EXISTS (
                    SELECT 1
                    FROM payment_collections
                    WHERE id = NEW.payment_collection_id
                      AND tenant_id = NEW.tenant_id
                ) THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'payment provider operation does not belong to payment collection tenant';
                END IF;

                IF NEW.refund_id IS NOT NULL
                   AND NOT EXISTS (
                        SELECT 1
                        FROM refunds
                        WHERE id = NEW.refund_id
                          AND tenant_id = NEW.tenant_id
                          AND payment_collection_id = NEW.payment_collection_id
                   ) THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'payment provider operation refund does not belong to payment collection tenant';
                END IF;
            END;
            "#,
        )
        .await?;

    Ok(())
}
