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
                        ALTER TABLE return_completion_operations
                            ADD CONSTRAINT ck_return_completion_operations_resolution_identity
                            CHECK (
                                stage <> 'resolution_created'
                                OR (
                                    (refund_id IS NOT NULL AND order_change_id IS NULL)
                                    OR
                                    (refund_id IS NULL AND order_change_id IS NOT NULL)
                                )
                            );
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        CREATE TRIGGER return_completion_resolution_identity_guard_insert
                        BEFORE INSERT ON return_completion_operations
                        FOR EACH ROW
                        WHEN NEW.stage = 'resolution_created' AND NOT (
                            (NEW.refund_id IS NOT NULL AND NEW.order_change_id IS NULL)
                            OR
                            (NEW.refund_id IS NULL AND NEW.order_change_id IS NOT NULL)
                        )
                        BEGIN
                            SELECT RAISE(ABORT, 'return completion resolution identity is required');
                        END;

                        CREATE TRIGGER return_completion_resolution_identity_guard_update
                        BEFORE UPDATE ON return_completion_operations
                        FOR EACH ROW
                        WHEN NEW.stage = 'resolution_created' AND NOT (
                            (NEW.refund_id IS NOT NULL AND NEW.order_change_id IS NULL)
                            OR
                            (NEW.refund_id IS NULL AND NEW.order_change_id IS NOT NULL)
                        )
                        BEGIN
                            SELECT RAISE(ABORT, 'return completion resolution identity is required');
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
                        "ALTER TABLE return_completion_operations DROP CONSTRAINT IF EXISTS ck_return_completion_operations_resolution_identity;",
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS return_completion_resolution_identity_guard_insert;
                        DROP TRIGGER IF EXISTS return_completion_resolution_identity_guard_update;
                        "#,
                    )
                    .await?;
            }
            _ => {}
        }
        Ok(())
    }
}
