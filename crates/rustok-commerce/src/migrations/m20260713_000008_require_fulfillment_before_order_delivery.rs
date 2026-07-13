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
                        CREATE OR REPLACE FUNCTION require_fulfillment_before_order_delivery()
                        RETURNS trigger AS $$
                        BEGIN
                            IF EXISTS (
                                SELECT 1 FROM fulfillments f WHERE f.order_id = NEW.id
                            ) THEN
                                IF NOT EXISTS (
                                    SELECT 1
                                    FROM fulfillments f
                                    WHERE f.order_id = NEW.id
                                      AND f.status = 'delivered'
                                ) THEN
                                    RAISE EXCEPTION 'order % has no delivered fulfillment', NEW.id
                                        USING ERRCODE = '23514';
                                END IF;

                                IF EXISTS (
                                    SELECT 1
                                    FROM fulfillments f
                                    WHERE f.order_id = NEW.id
                                      AND f.status NOT IN ('delivered', 'cancelled')
                                ) THEN
                                    RAISE EXCEPTION 'order % has incomplete fulfillments', NEW.id
                                        USING ERRCODE = '23514';
                                END IF;

                                IF EXISTS (
                                    SELECT 1
                                    FROM fulfillment_items fi
                                    JOIN fulfillments f ON f.id = fi.fulfillment_id
                                    WHERE f.order_id = NEW.id
                                      AND f.status <> 'cancelled'
                                      AND fi.delivered_quantity <> fi.quantity
                                ) THEN
                                    RAISE EXCEPTION 'order % has undelivered fulfillment items', NEW.id
                                        USING ERRCODE = '23514';
                                END IF;
                            END IF;

                            RETURN NEW;
                        END;
                        $$ LANGUAGE plpgsql;

                        CREATE TRIGGER order_delivery_fulfillment_guard
                        BEFORE UPDATE OF status ON orders
                        FOR EACH ROW
                        WHEN (OLD.status = 'shipped' AND NEW.status = 'delivered')
                        EXECUTE FUNCTION require_fulfillment_before_order_delivery();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        CREATE TRIGGER order_delivery_fulfillment_guard
                        BEFORE UPDATE OF status ON orders
                        FOR EACH ROW
                        WHEN OLD.status = 'shipped' AND NEW.status = 'delivered'
                        BEGIN
                            SELECT CASE WHEN EXISTS (
                                SELECT 1 FROM fulfillments f WHERE f.order_id = NEW.id
                            ) AND NOT EXISTS (
                                SELECT 1
                                FROM fulfillments f
                                WHERE f.order_id = NEW.id
                                  AND f.status = 'delivered'
                            ) THEN RAISE(ABORT, 'order has no delivered fulfillment') END;

                            SELECT CASE WHEN EXISTS (
                                SELECT 1
                                FROM fulfillments f
                                WHERE f.order_id = NEW.id
                                  AND f.status NOT IN ('delivered', 'cancelled')
                            ) THEN RAISE(ABORT, 'order has incomplete fulfillments') END;

                            SELECT CASE WHEN EXISTS (
                                SELECT 1
                                FROM fulfillment_items fi
                                JOIN fulfillments f ON f.id = fi.fulfillment_id
                                WHERE f.order_id = NEW.id
                                  AND f.status <> 'cancelled'
                                  AND fi.delivered_quantity <> fi.quantity
                            ) THEN RAISE(ABORT, 'order has undelivered fulfillment items') END;
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
                        DROP TRIGGER IF EXISTS order_delivery_fulfillment_guard ON orders;
                        DROP FUNCTION IF EXISTS require_fulfillment_before_order_delivery();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        "DROP TRIGGER IF EXISTS order_delivery_fulfillment_guard;",
                    )
                    .await?;
            }
            _ => {}
        }

        Ok(())
    }
}
