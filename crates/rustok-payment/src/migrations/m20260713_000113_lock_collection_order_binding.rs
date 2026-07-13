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
                        CREATE OR REPLACE FUNCTION guard_payment_collection_order_binding()
                        RETURNS trigger AS $$
                        BEGIN
                            IF OLD.order_id IS NOT NULL
                               AND NEW.order_id IS DISTINCT FROM OLD.order_id THEN
                                RAISE EXCEPTION 'payment collection order binding is immutable'
                                    USING ERRCODE = '23514';
                            END IF;
                            RETURN NEW;
                        END;
                        $$ LANGUAGE plpgsql;

                        CREATE TRIGGER payment_collections_order_binding_guard
                        BEFORE UPDATE OF order_id ON payment_collections
                        FOR EACH ROW
                        EXECUTE FUNCTION guard_payment_collection_order_binding();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        CREATE TRIGGER payment_collections_order_binding_guard
                        BEFORE UPDATE OF order_id ON payment_collections
                        FOR EACH ROW
                        WHEN OLD.order_id IS NOT NULL
                         AND NEW.order_id IS NOT OLD.order_id
                        BEGIN
                            SELECT RAISE(ABORT, 'payment collection order binding is immutable');
                        END;
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::MySql => {}
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
                        DROP TRIGGER IF EXISTS payment_collections_order_binding_guard
                            ON payment_collections;
                        DROP FUNCTION IF EXISTS guard_payment_collection_order_binding();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        "DROP TRIGGER IF EXISTS payment_collections_order_binding_guard;",
                    )
                    .await?;
            }
            DatabaseBackend::MySql => {}
        }
        Ok(())
    }
}
