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
                            ADD CONSTRAINT ck_return_completion_operations_request_hash_sha256
                            CHECK (request_hash ~ '^[0-9a-f]{64}$');

                        ALTER TABLE return_completion_operations
                            ADD CONSTRAINT ck_return_completion_operations_pending_stage
                            CHECK (status <> 'pending' OR stage = 'created'),
                            ADD CONSTRAINT ck_return_completion_operations_completed_stage
                            CHECK (
                                (status = 'completed' AND stage = 'completed')
                                OR
                                (status <> 'completed' AND stage <> 'completed')
                            );
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::MySql => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        CREATE TRIGGER return_completion_operation_identity_guard_insert
                        BEFORE INSERT ON return_completion_operations
                        FOR EACH ROW
                        BEGIN
                            IF REGEXP_LIKE(NEW.request_hash, '^[0-9a-f]{64}$', 'c') = 0 THEN
                                SIGNAL SQLSTATE '45000'
                                    SET MESSAGE_TEXT = 'invalid return completion operation request hash';
                            END IF;
                            IF NEW.status = 'pending'
                                AND NEW.stage <> 'created'
                            THEN
                                SIGNAL SQLSTATE '45000'
                                    SET MESSAGE_TEXT = 'pending return completion operation must be at created stage';
                            END IF;
                            IF (
                                (NEW.status = 'completed' AND NEW.stage <> 'completed')
                                OR
                                (NEW.status <> 'completed' AND NEW.stage = 'completed')
                            ) THEN
                                SIGNAL SQLSTATE '45000'
                                    SET MESSAGE_TEXT = 'completed return completion operation stage/status mismatch';
                            END IF;
                        END;

                        CREATE TRIGGER return_completion_operation_identity_guard_update
                        BEFORE UPDATE ON return_completion_operations
                        FOR EACH ROW
                        BEGIN
                            IF REGEXP_LIKE(NEW.request_hash, '^[0-9a-f]{64}$', 'c') = 0 THEN
                                SIGNAL SQLSTATE '45000'
                                    SET MESSAGE_TEXT = 'invalid return completion operation request hash';
                            END IF;
                            IF NEW.status = 'pending'
                                AND NEW.stage <> 'created'
                            THEN
                                SIGNAL SQLSTATE '45000'
                                    SET MESSAGE_TEXT = 'pending return completion operation must be at created stage';
                            END IF;
                            IF (
                                (NEW.status = 'completed' AND NEW.stage <> 'completed')
                                OR
                                (NEW.status <> 'completed' AND NEW.stage = 'completed')
                            ) THEN
                                SIGNAL SQLSTATE '45000'
                                    SET MESSAGE_TEXT = 'completed return completion operation stage/status mismatch';
                            END IF;
                        END
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        CREATE TRIGGER return_completion_operation_identity_guard_insert
                        BEFORE INSERT ON return_completion_operations
                        FOR EACH ROW
                        BEGIN
                            SELECT CASE WHEN length(NEW.request_hash) <> 64
                                OR NEW.request_hash GLOB '*[^0-9a-f]*'
                                THEN RAISE(ABORT, 'invalid return completion operation request hash') END;
                            SELECT CASE WHEN NEW.status = 'pending'
                                AND NEW.stage <> 'created'
                                THEN RAISE(ABORT, 'pending return completion operation must be at created stage') END;
                            SELECT CASE WHEN (
                                (NEW.status = 'completed' AND NEW.stage <> 'completed')
                                OR
                                (NEW.status <> 'completed' AND NEW.stage = 'completed')
                            ) THEN RAISE(ABORT, 'completed return completion operation stage/status mismatch') END;
                        END;

                        CREATE TRIGGER return_completion_operation_identity_guard_update
                        BEFORE UPDATE ON return_completion_operations
                        FOR EACH ROW
                        BEGIN
                            SELECT CASE WHEN length(NEW.request_hash) <> 64
                                OR NEW.request_hash GLOB '*[^0-9a-f]*'
                                THEN RAISE(ABORT, 'invalid return completion operation request hash') END;
                            SELECT CASE WHEN NEW.status = 'pending'
                                AND NEW.stage <> 'created'
                                THEN RAISE(ABORT, 'pending return completion operation must be at created stage') END;
                            SELECT CASE WHEN (
                                (NEW.status = 'completed' AND NEW.stage <> 'completed')
                                OR
                                (NEW.status <> 'completed' AND NEW.stage = 'completed')
                            ) THEN RAISE(ABORT, 'completed return completion operation stage/status mismatch') END;
                        END;
                        "#,
                    )
                    .await?;
            }
            _ => unreachable!("unsupported SeaORM database backend"),
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
                        ALTER TABLE return_completion_operations
                            DROP CONSTRAINT IF EXISTS ck_return_completion_operations_request_hash_sha256,
                            DROP CONSTRAINT IF EXISTS ck_return_completion_operations_pending_stage,
                            DROP CONSTRAINT IF EXISTS ck_return_completion_operations_completed_stage;
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::MySql | DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS return_completion_operation_identity_guard_update;
                        DROP TRIGGER IF EXISTS return_completion_operation_identity_guard_insert;
                        "#,
                    )
                    .await?;
            }
            _ => unreachable!("unsupported SeaORM database backend"),
        }

        Ok(())
    }
}
