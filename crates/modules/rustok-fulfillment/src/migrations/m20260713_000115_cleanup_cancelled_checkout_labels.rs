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
                        DROP TRIGGER IF EXISTS cancelled_order_pending_label_cleanup ON orders;
                        DROP FUNCTION IF EXISTS cleanup_cancelled_order_pending_labels();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS cancelled_order_pending_label_cleanup;
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
            DELETE FROM fulfillment_provider_operations operation
            USING fulfillments fulfillment, orders parent_order
            WHERE operation.fulfillment_id = fulfillment.id
              AND fulfillment.order_id = parent_order.id
              AND fulfillment.tenant_id = parent_order.tenant_id
              AND parent_order.status = 'cancelled'
              AND operation.operation = 'create_label'
              AND operation.status = 'pending';

            CREATE OR REPLACE FUNCTION cleanup_cancelled_order_pending_labels()
            RETURNS trigger AS $$
            BEGIN
                IF OLD.status IS DISTINCT FROM NEW.status AND NEW.status = 'cancelled' THEN
                    DELETE FROM fulfillment_provider_operations operation
                    USING fulfillments fulfillment
                    WHERE operation.fulfillment_id = fulfillment.id
                      AND fulfillment.order_id = NEW.id
                      AND fulfillment.tenant_id = NEW.tenant_id
                      AND operation.tenant_id = NEW.tenant_id
                      AND operation.operation = 'create_label'
                      AND operation.status = 'pending';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER cancelled_order_pending_label_cleanup
            AFTER UPDATE OF status ON orders
            FOR EACH ROW
            EXECUTE FUNCTION cleanup_cancelled_order_pending_labels();
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
            DELETE FROM fulfillment_provider_operations
            WHERE operation = 'create_label'
              AND status = 'pending'
              AND EXISTS (
                  SELECT 1
                  FROM fulfillments fulfillment
                  JOIN orders parent_order
                    ON parent_order.id = fulfillment.order_id
                   AND parent_order.tenant_id = fulfillment.tenant_id
                  WHERE fulfillment.id = fulfillment_provider_operations.fulfillment_id
                    AND parent_order.status = 'cancelled'
              );

            CREATE TRIGGER cancelled_order_pending_label_cleanup
            AFTER UPDATE OF status ON orders
            FOR EACH ROW
            WHEN OLD.status <> NEW.status AND NEW.status = 'cancelled'
            BEGIN
                DELETE FROM fulfillment_provider_operations
                WHERE operation = 'create_label'
                  AND status = 'pending'
                  AND tenant_id = NEW.tenant_id
                  AND fulfillment_id IN (
                      SELECT fulfillment.id
                      FROM fulfillments fulfillment
                      WHERE fulfillment.order_id = NEW.id
                        AND fulfillment.tenant_id = NEW.tenant_id
                  );
            END;
            "#,
        )
        .await?;
    Ok(())
}
