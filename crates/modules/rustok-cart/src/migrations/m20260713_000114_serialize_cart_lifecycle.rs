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
                        ALTER TABLE carts
                            ADD CONSTRAINT ck_carts_lifecycle_status
                            CHECK (status IN ('active', 'checking_out', 'completed', 'abandoned')) NOT VALID,
                            ADD CONSTRAINT ck_carts_lifecycle_completed_at
                            CHECK (
                                (status = 'completed' AND completed_at IS NOT NULL)
                                OR (status <> 'completed' AND completed_at IS NULL)
                            ) NOT VALID;

                        CREATE OR REPLACE FUNCTION enforce_cart_lifecycle_transition() RETURNS trigger AS $$
                        BEGIN
                            IF NEW.id IS DISTINCT FROM OLD.id
                               OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id THEN
                                RAISE EXCEPTION 'cart identity is immutable'
                                    USING ERRCODE = '23514';
                            END IF;

                            IF NEW.status = OLD.status THEN
                                RAISE EXCEPTION 'stale cart lifecycle update for status %', OLD.status
                                    USING ERRCODE = '40001';
                            END IF;

                            IF NOT (
                                (OLD.status = 'active' AND NEW.status IN ('checking_out', 'completed', 'abandoned'))
                                OR (OLD.status = 'checking_out' AND NEW.status IN ('active', 'completed'))
                            ) THEN
                                RAISE EXCEPTION 'invalid cart transition from % to %', OLD.status, NEW.status
                                    USING ERRCODE = '23514';
                            END IF;

                            RETURN NEW;
                        END;
                        $$ LANGUAGE plpgsql;

                        CREATE TRIGGER carts_lifecycle_transition_guard
                        BEFORE UPDATE OF status ON carts
                        FOR EACH ROW
                        EXECUTE FUNCTION enforce_cart_lifecycle_transition();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        CREATE TRIGGER carts_lifecycle_state_guard_insert
                        BEFORE INSERT ON carts
                        FOR EACH ROW
                        BEGIN
                            SELECT CASE WHEN NEW.status NOT IN ('active', 'checking_out', 'completed', 'abandoned')
                                THEN RAISE(ABORT, 'invalid cart status') END;
                            SELECT CASE WHEN NOT (
                                (NEW.status = 'completed' AND NEW.completed_at IS NOT NULL)
                                OR (NEW.status <> 'completed' AND NEW.completed_at IS NULL)
                            ) THEN RAISE(ABORT, 'invalid cart completion timestamp') END;
                        END;

                        CREATE TRIGGER carts_lifecycle_state_guard_update
                        BEFORE UPDATE OF status, completed_at ON carts
                        FOR EACH ROW
                        BEGIN
                            SELECT CASE WHEN NEW.status NOT IN ('active', 'checking_out', 'completed', 'abandoned')
                                THEN RAISE(ABORT, 'invalid cart status') END;
                            SELECT CASE WHEN NOT (
                                (NEW.status = 'completed' AND NEW.completed_at IS NOT NULL)
                                OR (NEW.status <> 'completed' AND NEW.completed_at IS NULL)
                            ) THEN RAISE(ABORT, 'invalid cart completion timestamp') END;
                        END;

                        CREATE TRIGGER carts_lifecycle_transition_guard
                        BEFORE UPDATE OF status ON carts
                        FOR EACH ROW
                        BEGIN
                            SELECT CASE WHEN NEW.id IS NOT OLD.id OR NEW.tenant_id IS NOT OLD.tenant_id
                                THEN RAISE(ABORT, 'cart identity is immutable') END;
                            SELECT CASE WHEN NEW.status = OLD.status
                                THEN RAISE(ABORT, 'stale cart lifecycle update') END;
                            SELECT CASE WHEN NOT (
                                (OLD.status = 'active' AND NEW.status IN ('checking_out', 'completed', 'abandoned'))
                                OR (OLD.status = 'checking_out' AND NEW.status IN ('active', 'completed'))
                            ) THEN RAISE(ABORT, 'invalid cart transition') END;
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
                        DROP TRIGGER IF EXISTS carts_lifecycle_transition_guard ON carts;
                        DROP FUNCTION IF EXISTS enforce_cart_lifecycle_transition();
                        ALTER TABLE carts
                            DROP CONSTRAINT IF EXISTS ck_carts_lifecycle_completed_at,
                            DROP CONSTRAINT IF EXISTS ck_carts_lifecycle_status;
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS carts_lifecycle_transition_guard;
                        DROP TRIGGER IF EXISTS carts_lifecycle_state_guard_update;
                        DROP TRIGGER IF EXISTS carts_lifecycle_state_guard_insert;
                        "#,
                    )
                    .await?;
            }
            _ => {}
        }

        Ok(())
    }
}
