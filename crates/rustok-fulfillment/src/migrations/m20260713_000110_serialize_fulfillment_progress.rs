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
                        DROP TRIGGER IF EXISTS fulfillments_transition_guard ON fulfillments;
                        DROP TRIGGER IF EXISTS fulfillment_items_progress_serialization_guard ON fulfillment_items;
                        DROP FUNCTION IF EXISTS enforce_fulfillment_transition();
                        DROP FUNCTION IF EXISTS serialize_fulfillment_item_progress();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS fulfillments_transition_guard;
                        DROP TRIGGER IF EXISTS fulfillment_items_progress_serialization_guard;
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
            CREATE OR REPLACE FUNCTION serialize_fulfillment_item_progress() RETURNS trigger AS $$
            BEGIN
                IF NEW.id IS DISTINCT FROM OLD.id
                   OR NEW.fulfillment_id IS DISTINCT FROM OLD.fulfillment_id
                   OR NEW.order_line_item_id IS DISTINCT FROM OLD.order_line_item_id
                   OR NEW.quantity IS DISTINCT FROM OLD.quantity THEN
                    RAISE EXCEPTION 'fulfillment item identity and quantity are immutable'
                        USING ERRCODE = '23514';
                END IF;

                IF NEW.shipped_quantity = OLD.shipped_quantity
                   AND NEW.delivered_quantity = OLD.delivered_quantity THEN
                    RAISE EXCEPTION 'stale fulfillment item progress update'
                        USING ERRCODE = '40001';
                END IF;

                IF NEW.shipped_quantity < OLD.shipped_quantity THEN
                    RAISE EXCEPTION 'shipped fulfillment quantity cannot decrease'
                        USING ERRCODE = '23514';
                END IF;

                IF NEW.shipped_quantity IS DISTINCT FROM OLD.shipped_quantity
                   AND NEW.delivered_quantity IS DISTINCT FROM OLD.delivered_quantity THEN
                    RAISE EXCEPTION 'ship and delivery progress must be updated separately'
                        USING ERRCODE = '23514';
                END IF;

                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER fulfillment_items_progress_serialization_guard
            BEFORE UPDATE OF fulfillment_id, order_line_item_id, quantity, shipped_quantity, delivered_quantity
            ON fulfillment_items
            FOR EACH ROW
            EXECUTE FUNCTION serialize_fulfillment_item_progress();

            CREATE OR REPLACE FUNCTION enforce_fulfillment_transition() RETURNS trigger AS $$
            DECLARE
                old_audit_count INTEGER;
                new_audit_count INTEGER;
            BEGIN
                IF NEW.id IS DISTINCT FROM OLD.id
                   OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
                   OR NEW.order_id IS DISTINCT FROM OLD.order_id
                   OR NEW.shipping_option_id IS DISTINCT FROM OLD.shipping_option_id
                   OR NEW.customer_id IS DISTINCT FROM OLD.customer_id THEN
                    RAISE EXCEPTION 'fulfillment ownership identity is immutable'
                        USING ERRCODE = '23514';
                END IF;

                IF NOT (
                    (OLD.status = 'pending' AND NEW.status IN ('shipped', 'cancelled'))
                    OR (OLD.status = 'shipped' AND NEW.status IN ('shipped', 'delivered', 'cancelled'))
                    OR (OLD.status = 'delivered' AND NEW.status = 'shipped')
                    OR (OLD.status = 'cancelled' AND NEW.status IN ('pending', 'shipped'))
                ) THEN
                    RAISE EXCEPTION 'invalid fulfillment transition from % to %', OLD.status, NEW.status
                        USING ERRCODE = '23514';
                END IF;

                IF OLD.status = NEW.status AND NOT EXISTS (
                    SELECT 1 FROM fulfillment_items WHERE fulfillment_id = NEW.id
                ) THEN
                    RAISE EXCEPTION 'stale fulfillment lifecycle update'
                        USING ERRCODE = '40001';
                END IF;

                old_audit_count := CASE
                    WHEN jsonb_typeof(OLD.metadata #> '{audit,events}') = 'array'
                    THEN jsonb_array_length(OLD.metadata #> '{audit,events}')
                    ELSE 0
                END;
                new_audit_count := CASE
                    WHEN jsonb_typeof(NEW.metadata #> '{audit,events}') = 'array'
                    THEN jsonb_array_length(NEW.metadata #> '{audit,events}')
                    ELSE 0
                END;
                IF new_audit_count <> old_audit_count + 1 THEN
                    RAISE EXCEPTION 'fulfillment lifecycle update must append exactly one audit event'
                        USING ERRCODE = '40001';
                END IF;

                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER fulfillments_transition_guard
            BEFORE UPDATE OF tenant_id, order_id, shipping_option_id, customer_id, status
            ON fulfillments
            FOR EACH ROW
            EXECUTE FUNCTION enforce_fulfillment_transition();
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
            CREATE TRIGGER fulfillment_items_progress_serialization_guard
            BEFORE UPDATE OF fulfillment_id, order_line_item_id, quantity, shipped_quantity, delivered_quantity
            ON fulfillment_items
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NEW.id IS NOT OLD.id
                    OR NEW.fulfillment_id IS NOT OLD.fulfillment_id
                    OR NEW.order_line_item_id IS NOT OLD.order_line_item_id
                    OR NEW.quantity IS NOT OLD.quantity
                    THEN RAISE(ABORT, 'fulfillment item identity and quantity are immutable') END;
                SELECT CASE WHEN NEW.shipped_quantity = OLD.shipped_quantity
                    AND NEW.delivered_quantity = OLD.delivered_quantity
                    THEN RAISE(ABORT, 'stale fulfillment item progress update') END;
                SELECT CASE WHEN NEW.shipped_quantity < OLD.shipped_quantity
                    THEN RAISE(ABORT, 'shipped fulfillment quantity cannot decrease') END;
                SELECT CASE WHEN NEW.shipped_quantity <> OLD.shipped_quantity
                    AND NEW.delivered_quantity <> OLD.delivered_quantity
                    THEN RAISE(ABORT, 'ship and delivery progress must be updated separately') END;
            END;

            CREATE TRIGGER fulfillments_transition_guard
            BEFORE UPDATE OF tenant_id, order_id, shipping_option_id, customer_id, status
            ON fulfillments
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NEW.id IS NOT OLD.id
                    OR NEW.tenant_id IS NOT OLD.tenant_id
                    OR NEW.order_id IS NOT OLD.order_id
                    OR NEW.shipping_option_id IS NOT OLD.shipping_option_id
                    OR NEW.customer_id IS NOT OLD.customer_id
                    THEN RAISE(ABORT, 'fulfillment ownership identity is immutable') END;

                SELECT CASE WHEN NOT (
                    (OLD.status = 'pending' AND NEW.status IN ('shipped', 'cancelled'))
                    OR (OLD.status = 'shipped' AND NEW.status IN ('shipped', 'delivered', 'cancelled'))
                    OR (OLD.status = 'delivered' AND NEW.status = 'shipped')
                    OR (OLD.status = 'cancelled' AND NEW.status IN ('pending', 'shipped'))
                ) THEN RAISE(ABORT, 'invalid fulfillment transition') END;

                SELECT CASE WHEN OLD.status = NEW.status AND NOT EXISTS (
                    SELECT 1 FROM fulfillment_items WHERE fulfillment_id = NEW.id
                ) THEN RAISE(ABORT, 'stale fulfillment lifecycle update') END;

                SELECT CASE WHEN
                    json_array_length(COALESCE(json_extract(NEW.metadata, '$.audit.events'), '[]'))
                    <>
                    json_array_length(COALESCE(json_extract(OLD.metadata, '$.audit.events'), '[]')) + 1
                    THEN RAISE(ABORT, 'fulfillment lifecycle update must append one audit event') END;
            END;
            "#,
        )
        .await?;
    Ok(())
}
