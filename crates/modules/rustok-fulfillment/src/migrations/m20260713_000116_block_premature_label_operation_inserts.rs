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
                        DROP TRIGGER IF EXISTS checkout_create_label_insert_payment_guard
                            ON fulfillment_provider_operations;
                        DROP FUNCTION IF EXISTS enforce_checkout_create_label_insert_payment();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS checkout_create_label_insert_payment_guard;
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
            UPDATE fulfillment_provider_operations operation
            SET status = 'reconciliation_required',
                error_message = 'create-label execution started before order payment',
                provider_completed_at = CURRENT_TIMESTAMP,
                updated_at = CURRENT_TIMESTAMP
            FROM fulfillments fulfillment
            JOIN orders parent_order
              ON parent_order.id = fulfillment.order_id
             AND parent_order.tenant_id = fulfillment.tenant_id
            WHERE operation.fulfillment_id = fulfillment.id
              AND operation.tenant_id = fulfillment.tenant_id
              AND operation.operation = 'create_label'
              AND operation.status = 'executing'
              AND parent_order.status <> 'paid';

            CREATE OR REPLACE FUNCTION enforce_checkout_create_label_insert_payment()
            RETURNS trigger AS $$
            BEGIN
                IF NEW.operation = 'create_label' AND NEW.status = 'executing' AND NOT EXISTS (
                    SELECT 1
                    FROM fulfillments fulfillment
                    JOIN orders parent_order
                      ON parent_order.id = fulfillment.order_id
                     AND parent_order.tenant_id = fulfillment.tenant_id
                    WHERE fulfillment.id = NEW.fulfillment_id
                      AND fulfillment.tenant_id = NEW.tenant_id
                      AND parent_order.status = 'paid'
                ) THEN
                    RAISE EXCEPTION 'create-label operation cannot be inserted as executing before order payment'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER checkout_create_label_insert_payment_guard
            BEFORE INSERT ON fulfillment_provider_operations
            FOR EACH ROW
            EXECUTE FUNCTION enforce_checkout_create_label_insert_payment();
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
            UPDATE fulfillment_provider_operations
            SET status = 'reconciliation_required',
                error_message = 'create-label execution started before order payment',
                provider_completed_at = CURRENT_TIMESTAMP,
                updated_at = CURRENT_TIMESTAMP
            WHERE operation = 'create_label'
              AND status = 'executing'
              AND EXISTS (
                  SELECT 1
                  FROM fulfillments fulfillment
                  JOIN orders parent_order
                    ON parent_order.id = fulfillment.order_id
                   AND parent_order.tenant_id = fulfillment.tenant_id
                  WHERE fulfillment.id = fulfillment_provider_operations.fulfillment_id
                    AND fulfillment.tenant_id = fulfillment_provider_operations.tenant_id
                    AND parent_order.status <> 'paid'
              );

            CREATE TRIGGER checkout_create_label_insert_payment_guard
            BEFORE INSERT ON fulfillment_provider_operations
            FOR EACH ROW
            WHEN NEW.operation = 'create_label' AND NEW.status = 'executing'
            BEGIN
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1
                    FROM fulfillments fulfillment
                    JOIN orders parent_order
                      ON parent_order.id = fulfillment.order_id
                     AND parent_order.tenant_id = fulfillment.tenant_id
                    WHERE fulfillment.id = NEW.fulfillment_id
                      AND fulfillment.tenant_id = NEW.tenant_id
                      AND parent_order.status = 'paid'
                ) THEN RAISE(ABORT, 'create-label operation cannot be inserted as executing before order payment') END;
            END;
            "#,
        )
        .await?;
    Ok(())
}
