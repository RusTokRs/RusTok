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
                        CREATE OR REPLACE FUNCTION enforce_refund_capacity() RETURNS trigger AS $$
                        DECLARE
                            collection_tenant UUID;
                            collection_currency VARCHAR(3);
                            collection_status VARCHAR(32);
                            captured NUMERIC;
                            reserved NUMERIC;
                        BEGIN
                            SELECT tenant_id, currency_code, status, captured_amount
                            INTO collection_tenant, collection_currency, collection_status, captured
                            FROM payment_collections
                            WHERE id = NEW.payment_collection_id
                            FOR UPDATE;

                            IF NOT FOUND THEN
                                RAISE EXCEPTION 'payment collection % does not exist', NEW.payment_collection_id
                                    USING ERRCODE = '23503';
                            END IF;
                            IF collection_tenant <> NEW.tenant_id THEN
                                RAISE EXCEPTION 'refund tenant does not match payment collection tenant'
                                    USING ERRCODE = '23514';
                            END IF;
                            IF collection_currency <> NEW.currency_code THEN
                                RAISE EXCEPTION 'refund currency does not match payment collection currency'
                                    USING ERRCODE = '23514';
                            END IF;
                            IF NEW.amount <= 0 THEN
                                RAISE EXCEPTION 'refund amount must be greater than zero'
                                    USING ERRCODE = '23514';
                            END IF;
                            IF NEW.status NOT IN ('pending', 'refunded', 'cancelled') THEN
                                RAISE EXCEPTION 'invalid refund status %', NEW.status
                                    USING ERRCODE = '23514';
                            END IF;

                            IF NEW.status IN ('pending', 'refunded') THEN
                                IF collection_status <> 'captured' THEN
                                    RAISE EXCEPTION 'refund requires a captured payment collection'
                                        USING ERRCODE = '23514';
                                END IF;

                                SELECT COALESCE(SUM(amount), 0)
                                INTO reserved
                                FROM refunds
                                WHERE payment_collection_id = NEW.payment_collection_id
                                  AND status IN ('pending', 'refunded')
                                  AND id <> NEW.id;

                                IF reserved + NEW.amount > captured THEN
                                    RAISE EXCEPTION 'refund amount exceeds remaining refundable capacity'
                                        USING ERRCODE = '23514';
                                END IF;
                            END IF;

                            RETURN NEW;
                        END;
                        $$ LANGUAGE plpgsql;

                        CREATE TRIGGER refunds_capacity_guard
                        BEFORE INSERT OR UPDATE OF payment_collection_id, tenant_id, currency_code, amount, status
                        ON refunds
                        FOR EACH ROW
                        EXECUTE FUNCTION enforce_refund_capacity();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        CREATE TRIGGER refunds_capacity_guard_insert
                        BEFORE INSERT ON refunds
                        FOR EACH ROW
                        BEGIN
                            SELECT CASE WHEN NOT EXISTS (
                                SELECT 1 FROM payment_collections WHERE id = NEW.payment_collection_id
                            ) THEN RAISE(ABORT, 'payment collection does not exist') END;
                            SELECT CASE WHEN (
                                SELECT tenant_id FROM payment_collections WHERE id = NEW.payment_collection_id
                            ) <> NEW.tenant_id THEN RAISE(ABORT, 'refund tenant mismatch') END;
                            SELECT CASE WHEN (
                                SELECT currency_code FROM payment_collections WHERE id = NEW.payment_collection_id
                            ) <> NEW.currency_code THEN RAISE(ABORT, 'refund currency mismatch') END;
                            SELECT CASE WHEN NEW.amount <= 0
                                THEN RAISE(ABORT, 'refund amount must be greater than zero') END;
                            SELECT CASE WHEN NEW.status NOT IN ('pending', 'refunded', 'cancelled')
                                THEN RAISE(ABORT, 'invalid refund status') END;
                            SELECT CASE WHEN NEW.status IN ('pending', 'refunded') AND (
                                SELECT status FROM payment_collections WHERE id = NEW.payment_collection_id
                            ) <> 'captured' THEN RAISE(ABORT, 'refund requires captured collection') END;
                            SELECT CASE WHEN NEW.status IN ('pending', 'refunded') AND
                                COALESCE((
                                    SELECT SUM(amount) FROM refunds
                                    WHERE payment_collection_id = NEW.payment_collection_id
                                      AND status IN ('pending', 'refunded')
                                ), 0) + NEW.amount > (
                                    SELECT captured_amount FROM payment_collections
                                    WHERE id = NEW.payment_collection_id
                                ) THEN RAISE(ABORT, 'refund capacity exceeded') END;
                        END;

                        CREATE TRIGGER refunds_capacity_guard_update
                        BEFORE UPDATE OF payment_collection_id, tenant_id, currency_code, amount, status ON refunds
                        FOR EACH ROW
                        BEGIN
                            SELECT CASE WHEN NOT EXISTS (
                                SELECT 1 FROM payment_collections WHERE id = NEW.payment_collection_id
                            ) THEN RAISE(ABORT, 'payment collection does not exist') END;
                            SELECT CASE WHEN (
                                SELECT tenant_id FROM payment_collections WHERE id = NEW.payment_collection_id
                            ) <> NEW.tenant_id THEN RAISE(ABORT, 'refund tenant mismatch') END;
                            SELECT CASE WHEN (
                                SELECT currency_code FROM payment_collections WHERE id = NEW.payment_collection_id
                            ) <> NEW.currency_code THEN RAISE(ABORT, 'refund currency mismatch') END;
                            SELECT CASE WHEN NEW.amount <= 0
                                THEN RAISE(ABORT, 'refund amount must be greater than zero') END;
                            SELECT CASE WHEN NEW.status NOT IN ('pending', 'refunded', 'cancelled')
                                THEN RAISE(ABORT, 'invalid refund status') END;
                            SELECT CASE WHEN NEW.status IN ('pending', 'refunded') AND (
                                SELECT status FROM payment_collections WHERE id = NEW.payment_collection_id
                            ) <> 'captured' THEN RAISE(ABORT, 'refund requires captured collection') END;
                            SELECT CASE WHEN NEW.status IN ('pending', 'refunded') AND
                                COALESCE((
                                    SELECT SUM(amount) FROM refunds
                                    WHERE payment_collection_id = NEW.payment_collection_id
                                      AND status IN ('pending', 'refunded')
                                      AND id <> OLD.id
                                ), 0) + NEW.amount > (
                                    SELECT captured_amount FROM payment_collections
                                    WHERE id = NEW.payment_collection_id
                                ) THEN RAISE(ABORT, 'refund capacity exceeded') END;
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
                        DROP TRIGGER IF EXISTS refunds_capacity_guard ON refunds;
                        DROP FUNCTION IF EXISTS enforce_refund_capacity();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS refunds_capacity_guard_insert;
                        DROP TRIGGER IF EXISTS refunds_capacity_guard_update;
                        "#,
                    )
                    .await?;
            }
            _ => {}
        }

        Ok(())
    }
}
