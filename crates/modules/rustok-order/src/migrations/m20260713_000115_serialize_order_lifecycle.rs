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
                        ALTER TABLE orders
                            ADD CONSTRAINT ck_orders_lifecycle_status
                            CHECK (status IN ('pending', 'confirmed', 'paid', 'shipped', 'delivered', 'cancelled')) NOT VALID,
                            ADD CONSTRAINT ck_orders_lifecycle_timestamps
                            CHECK (
                                (status = 'pending'
                                    AND confirmed_at IS NULL
                                    AND paid_at IS NULL
                                    AND shipped_at IS NULL
                                    AND delivered_at IS NULL
                                    AND cancelled_at IS NULL)
                                OR
                                (status = 'confirmed'
                                    AND confirmed_at IS NOT NULL
                                    AND paid_at IS NULL
                                    AND shipped_at IS NULL
                                    AND delivered_at IS NULL
                                    AND cancelled_at IS NULL)
                                OR
                                (status = 'paid'
                                    AND confirmed_at IS NOT NULL
                                    AND paid_at IS NOT NULL
                                    AND shipped_at IS NULL
                                    AND delivered_at IS NULL
                                    AND cancelled_at IS NULL)
                                OR
                                (status = 'shipped'
                                    AND confirmed_at IS NOT NULL
                                    AND paid_at IS NOT NULL
                                    AND shipped_at IS NOT NULL
                                    AND delivered_at IS NULL
                                    AND cancelled_at IS NULL)
                                OR
                                (status = 'delivered'
                                    AND confirmed_at IS NOT NULL
                                    AND paid_at IS NOT NULL
                                    AND shipped_at IS NOT NULL
                                    AND delivered_at IS NOT NULL
                                    AND cancelled_at IS NULL)
                                OR
                                (status = 'cancelled'
                                    AND delivered_at IS NULL
                                    AND cancelled_at IS NOT NULL)
                            ) NOT VALID;

                        CREATE OR REPLACE FUNCTION enforce_order_lifecycle_transition() RETURNS trigger AS $$
                        BEGIN
                            IF NEW.id IS DISTINCT FROM OLD.id
                               OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id THEN
                                RAISE EXCEPTION 'order identity is immutable'
                                    USING ERRCODE = '23514';
                            END IF;

                            IF NEW.status = OLD.status THEN
                                RAISE EXCEPTION 'stale order lifecycle update for status %', OLD.status
                                    USING ERRCODE = '40001';
                            END IF;

                            IF NOT (
                                (OLD.status = 'pending' AND NEW.status IN ('confirmed', 'cancelled'))
                                OR (OLD.status = 'confirmed' AND NEW.status IN ('paid', 'cancelled'))
                                OR (OLD.status = 'paid' AND NEW.status IN ('shipped', 'cancelled'))
                                OR (OLD.status = 'shipped' AND NEW.status IN ('delivered', 'cancelled'))
                            ) THEN
                                RAISE EXCEPTION 'invalid order transition from % to %', OLD.status, NEW.status
                                    USING ERRCODE = '23514';
                            END IF;

                            RETURN NEW;
                        END;
                        $$ LANGUAGE plpgsql;

                        CREATE TRIGGER orders_lifecycle_transition_guard
                        BEFORE UPDATE OF status ON orders
                        FOR EACH ROW
                        EXECUTE FUNCTION enforce_order_lifecycle_transition();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        CREATE TRIGGER orders_lifecycle_state_guard_insert
                        BEFORE INSERT ON orders
                        FOR EACH ROW
                        BEGIN
                            SELECT CASE WHEN NEW.status NOT IN (
                                'pending', 'confirmed', 'paid', 'shipped', 'delivered', 'cancelled'
                            ) THEN RAISE(ABORT, 'invalid order status') END;
                            SELECT CASE WHEN NOT (
                                (NEW.status = 'pending'
                                    AND NEW.confirmed_at IS NULL
                                    AND NEW.paid_at IS NULL
                                    AND NEW.shipped_at IS NULL
                                    AND NEW.delivered_at IS NULL
                                    AND NEW.cancelled_at IS NULL)
                                OR
                                (NEW.status = 'confirmed'
                                    AND NEW.confirmed_at IS NOT NULL
                                    AND NEW.paid_at IS NULL
                                    AND NEW.shipped_at IS NULL
                                    AND NEW.delivered_at IS NULL
                                    AND NEW.cancelled_at IS NULL)
                                OR
                                (NEW.status = 'paid'
                                    AND NEW.confirmed_at IS NOT NULL
                                    AND NEW.paid_at IS NOT NULL
                                    AND NEW.shipped_at IS NULL
                                    AND NEW.delivered_at IS NULL
                                    AND NEW.cancelled_at IS NULL)
                                OR
                                (NEW.status = 'shipped'
                                    AND NEW.confirmed_at IS NOT NULL
                                    AND NEW.paid_at IS NOT NULL
                                    AND NEW.shipped_at IS NOT NULL
                                    AND NEW.delivered_at IS NULL
                                    AND NEW.cancelled_at IS NULL)
                                OR
                                (NEW.status = 'delivered'
                                    AND NEW.confirmed_at IS NOT NULL
                                    AND NEW.paid_at IS NOT NULL
                                    AND NEW.shipped_at IS NOT NULL
                                    AND NEW.delivered_at IS NOT NULL
                                    AND NEW.cancelled_at IS NULL)
                                OR
                                (NEW.status = 'cancelled'
                                    AND NEW.delivered_at IS NULL
                                    AND NEW.cancelled_at IS NOT NULL)
                            ) THEN RAISE(ABORT, 'invalid order lifecycle timestamps') END;
                        END;

                        CREATE TRIGGER orders_lifecycle_state_guard_update
                        BEFORE UPDATE OF status, confirmed_at, paid_at, shipped_at, delivered_at, cancelled_at
                        ON orders
                        FOR EACH ROW
                        BEGIN
                            SELECT CASE WHEN NEW.status NOT IN (
                                'pending', 'confirmed', 'paid', 'shipped', 'delivered', 'cancelled'
                            ) THEN RAISE(ABORT, 'invalid order status') END;
                            SELECT CASE WHEN NOT (
                                (NEW.status = 'pending'
                                    AND NEW.confirmed_at IS NULL
                                    AND NEW.paid_at IS NULL
                                    AND NEW.shipped_at IS NULL
                                    AND NEW.delivered_at IS NULL
                                    AND NEW.cancelled_at IS NULL)
                                OR
                                (NEW.status = 'confirmed'
                                    AND NEW.confirmed_at IS NOT NULL
                                    AND NEW.paid_at IS NULL
                                    AND NEW.shipped_at IS NULL
                                    AND NEW.delivered_at IS NULL
                                    AND NEW.cancelled_at IS NULL)
                                OR
                                (NEW.status = 'paid'
                                    AND NEW.confirmed_at IS NOT NULL
                                    AND NEW.paid_at IS NOT NULL
                                    AND NEW.shipped_at IS NULL
                                    AND NEW.delivered_at IS NULL
                                    AND NEW.cancelled_at IS NULL)
                                OR
                                (NEW.status = 'shipped'
                                    AND NEW.confirmed_at IS NOT NULL
                                    AND NEW.paid_at IS NOT NULL
                                    AND NEW.shipped_at IS NOT NULL
                                    AND NEW.delivered_at IS NULL
                                    AND NEW.cancelled_at IS NULL)
                                OR
                                (NEW.status = 'delivered'
                                    AND NEW.confirmed_at IS NOT NULL
                                    AND NEW.paid_at IS NOT NULL
                                    AND NEW.shipped_at IS NOT NULL
                                    AND NEW.delivered_at IS NOT NULL
                                    AND NEW.cancelled_at IS NULL)
                                OR
                                (NEW.status = 'cancelled'
                                    AND NEW.delivered_at IS NULL
                                    AND NEW.cancelled_at IS NOT NULL)
                            ) THEN RAISE(ABORT, 'invalid order lifecycle timestamps') END;
                        END;

                        CREATE TRIGGER orders_lifecycle_transition_guard
                        BEFORE UPDATE OF status ON orders
                        FOR EACH ROW
                        BEGIN
                            SELECT CASE WHEN NEW.id IS NOT OLD.id OR NEW.tenant_id IS NOT OLD.tenant_id
                                THEN RAISE(ABORT, 'order identity is immutable') END;
                            SELECT CASE WHEN NEW.status = OLD.status
                                THEN RAISE(ABORT, 'stale order lifecycle update') END;
                            SELECT CASE WHEN NOT (
                                (OLD.status = 'pending' AND NEW.status IN ('confirmed', 'cancelled'))
                                OR (OLD.status = 'confirmed' AND NEW.status IN ('paid', 'cancelled'))
                                OR (OLD.status = 'paid' AND NEW.status IN ('shipped', 'cancelled'))
                                OR (OLD.status = 'shipped' AND NEW.status IN ('delivered', 'cancelled'))
                            ) THEN RAISE(ABORT, 'invalid order transition') END;
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
                        DROP TRIGGER IF EXISTS orders_lifecycle_transition_guard ON orders;
                        DROP FUNCTION IF EXISTS enforce_order_lifecycle_transition();
                        ALTER TABLE orders
                            DROP CONSTRAINT IF EXISTS ck_orders_lifecycle_timestamps,
                            DROP CONSTRAINT IF EXISTS ck_orders_lifecycle_status;
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS orders_lifecycle_transition_guard;
                        DROP TRIGGER IF EXISTS orders_lifecycle_state_guard_update;
                        DROP TRIGGER IF EXISTS orders_lifecycle_state_guard_insert;
                        "#,
                    )
                    .await?;
            }
            _ => {}
        }

        Ok(())
    }
}
