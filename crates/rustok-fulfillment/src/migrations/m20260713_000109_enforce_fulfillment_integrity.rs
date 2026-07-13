use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name("ux_fulfillment_items_order_line")
                    .table(Alias::new("fulfillment_items"))
                    .col(Alias::new("fulfillment_id"))
                    .col(Alias::new("order_line_item_id"))
                    .unique()
                    .to_owned(),
            )
            .await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        ALTER TABLE shipping_options
                            ADD CONSTRAINT ck_shipping_options_currency
                            CHECK (currency_code ~ '^[A-Z]{3}$') NOT VALID,
                            ADD CONSTRAINT ck_shipping_options_amount
                            CHECK (amount >= 0 AND amount <= 92233720368547758.07) NOT VALID,
                            ADD CONSTRAINT ck_shipping_options_provider
                            CHECK (btrim(provider_id) <> '') NOT VALID;

                        ALTER TABLE fulfillments
                            ADD CONSTRAINT ck_fulfillments_status
                            CHECK (status IN ('pending', 'shipped', 'delivered', 'cancelled')) NOT VALID,
                            ADD CONSTRAINT ck_fulfillments_lifecycle
                            CHECK (
                                (status = 'pending'
                                    AND shipped_at IS NULL AND delivered_at IS NULL AND cancelled_at IS NULL)
                                OR
                                (status = 'shipped'
                                    AND shipped_at IS NOT NULL AND delivered_at IS NULL AND cancelled_at IS NULL
                                    AND carrier IS NOT NULL AND btrim(carrier) <> ''
                                    AND tracking_number IS NOT NULL AND btrim(tracking_number) <> '')
                                OR
                                (status = 'delivered'
                                    AND shipped_at IS NOT NULL AND delivered_at IS NOT NULL AND cancelled_at IS NULL
                                    AND carrier IS NOT NULL AND btrim(carrier) <> ''
                                    AND tracking_number IS NOT NULL AND btrim(tracking_number) <> '')
                                OR
                                (status = 'cancelled'
                                    AND delivered_at IS NULL AND cancelled_at IS NOT NULL)
                            ) NOT VALID;

                        ALTER TABLE fulfillment_items
                            ADD CONSTRAINT ck_fulfillment_items_progress
                            CHECK (
                                quantity > 0
                                AND shipped_quantity >= 0
                                AND delivered_quantity >= 0
                                AND delivered_quantity <= shipped_quantity
                                AND shipped_quantity <= quantity
                            ) NOT VALID;

                        CREATE OR REPLACE FUNCTION enforce_fulfillment_ownership()
                        RETURNS trigger AS $$
                        DECLARE
                            order_tenant UUID;
                            option_tenant UUID;
                            fulfillment_order UUID;
                            line_order UUID;
                        BEGIN
                            IF TG_TABLE_NAME = 'fulfillments' THEN
                                SELECT tenant_id INTO order_tenant
                                FROM orders WHERE id = NEW.order_id;
                                IF order_tenant IS NULL THEN
                                    RAISE EXCEPTION 'order % does not exist', NEW.order_id
                                        USING ERRCODE = '23503';
                                END IF;
                                IF order_tenant <> NEW.tenant_id THEN
                                    RAISE EXCEPTION 'fulfillment and order belong to different tenants'
                                        USING ERRCODE = '23514';
                                END IF;
                                IF NEW.shipping_option_id IS NOT NULL THEN
                                    SELECT tenant_id INTO option_tenant
                                    FROM shipping_options WHERE id = NEW.shipping_option_id;
                                    IF option_tenant IS NULL THEN
                                        RAISE EXCEPTION 'shipping option % does not exist', NEW.shipping_option_id
                                            USING ERRCODE = '23503';
                                    END IF;
                                    IF option_tenant <> NEW.tenant_id THEN
                                        RAISE EXCEPTION 'fulfillment and shipping option belong to different tenants'
                                            USING ERRCODE = '23514';
                                    END IF;
                                END IF;
                                RETURN NEW;
                            END IF;

                            SELECT order_id INTO fulfillment_order
                            FROM fulfillments WHERE id = NEW.fulfillment_id;
                            SELECT order_id INTO line_order
                            FROM order_line_items WHERE id = NEW.order_line_item_id;
                            IF fulfillment_order IS NULL OR line_order IS NULL THEN
                                RAISE EXCEPTION 'fulfillment item references missing fulfillment or order line'
                                    USING ERRCODE = '23503';
                            END IF;
                            IF fulfillment_order <> line_order THEN
                                RAISE EXCEPTION 'fulfillment item order line belongs to another order'
                                    USING ERRCODE = '23514';
                            END IF;
                            RETURN NEW;
                        END;
                        $$ LANGUAGE plpgsql;

                        CREATE TRIGGER fulfillments_ownership_guard
                        BEFORE INSERT OR UPDATE OF tenant_id, order_id, shipping_option_id
                        ON fulfillments FOR EACH ROW
                        EXECUTE FUNCTION enforce_fulfillment_ownership();

                        CREATE TRIGGER fulfillment_items_ownership_guard
                        BEFORE INSERT OR UPDATE OF fulfillment_id, order_line_item_id
                        ON fulfillment_items FOR EACH ROW
                        EXECUTE FUNCTION enforce_fulfillment_ownership();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        CREATE TRIGGER shipping_options_integrity_guard_insert
                        BEFORE INSERT ON shipping_options FOR EACH ROW BEGIN
                            SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid shipping option currency') END;
                            SELECT CASE WHEN NEW.amount < 0 OR NEW.amount > 92233720368547758.07
                                OR trim(NEW.provider_id) = ''
                                THEN RAISE(ABORT, 'invalid shipping option') END;
                        END;

                        CREATE TRIGGER shipping_options_integrity_guard_update
                        BEFORE UPDATE OF currency_code, amount, provider_id
                        ON shipping_options FOR EACH ROW BEGIN
                            SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid shipping option currency') END;
                            SELECT CASE WHEN NEW.amount < 0 OR NEW.amount > 92233720368547758.07
                                OR trim(NEW.provider_id) = ''
                                THEN RAISE(ABORT, 'invalid shipping option') END;
                        END;

                        CREATE TRIGGER fulfillments_integrity_guard_insert
                        BEFORE INSERT ON fulfillments FOR EACH ROW BEGIN
                            SELECT CASE WHEN NEW.status NOT IN ('pending', 'shipped', 'delivered', 'cancelled')
                                THEN RAISE(ABORT, 'invalid fulfillment status') END;
                            SELECT CASE WHEN NOT (
                                (NEW.status = 'pending'
                                    AND NEW.shipped_at IS NULL AND NEW.delivered_at IS NULL AND NEW.cancelled_at IS NULL)
                                OR (NEW.status = 'shipped'
                                    AND NEW.shipped_at IS NOT NULL AND NEW.delivered_at IS NULL AND NEW.cancelled_at IS NULL
                                    AND NEW.carrier IS NOT NULL AND trim(NEW.carrier) <> ''
                                    AND NEW.tracking_number IS NOT NULL AND trim(NEW.tracking_number) <> '')
                                OR (NEW.status = 'delivered'
                                    AND NEW.shipped_at IS NOT NULL AND NEW.delivered_at IS NOT NULL AND NEW.cancelled_at IS NULL
                                    AND NEW.carrier IS NOT NULL AND trim(NEW.carrier) <> ''
                                    AND NEW.tracking_number IS NOT NULL AND trim(NEW.tracking_number) <> '')
                                OR (NEW.status = 'cancelled'
                                    AND NEW.delivered_at IS NULL AND NEW.cancelled_at IS NOT NULL)
                            ) THEN RAISE(ABORT, 'invalid fulfillment lifecycle') END;
                            SELECT CASE WHEN NOT EXISTS (
                                SELECT 1 FROM orders o
                                WHERE o.id = NEW.order_id AND o.tenant_id = NEW.tenant_id
                            ) THEN RAISE(ABORT, 'fulfillment order tenant mismatch') END;
                            SELECT CASE WHEN NEW.shipping_option_id IS NOT NULL AND NOT EXISTS (
                                SELECT 1 FROM shipping_options so
                                WHERE so.id = NEW.shipping_option_id AND so.tenant_id = NEW.tenant_id
                            ) THEN RAISE(ABORT, 'fulfillment shipping option tenant mismatch') END;
                        END;

                        CREATE TRIGGER fulfillments_integrity_guard_update
                        BEFORE UPDATE OF tenant_id, order_id, shipping_option_id, status, carrier,
                            tracking_number, shipped_at, delivered_at, cancelled_at
                        ON fulfillments FOR EACH ROW BEGIN
                            SELECT CASE WHEN NEW.status NOT IN ('pending', 'shipped', 'delivered', 'cancelled')
                                THEN RAISE(ABORT, 'invalid fulfillment status') END;
                            SELECT CASE WHEN NOT (
                                (NEW.status = 'pending'
                                    AND NEW.shipped_at IS NULL AND NEW.delivered_at IS NULL AND NEW.cancelled_at IS NULL)
                                OR (NEW.status = 'shipped'
                                    AND NEW.shipped_at IS NOT NULL AND NEW.delivered_at IS NULL AND NEW.cancelled_at IS NULL
                                    AND NEW.carrier IS NOT NULL AND trim(NEW.carrier) <> ''
                                    AND NEW.tracking_number IS NOT NULL AND trim(NEW.tracking_number) <> '')
                                OR (NEW.status = 'delivered'
                                    AND NEW.shipped_at IS NOT NULL AND NEW.delivered_at IS NOT NULL AND NEW.cancelled_at IS NULL
                                    AND NEW.carrier IS NOT NULL AND trim(NEW.carrier) <> ''
                                    AND NEW.tracking_number IS NOT NULL AND trim(NEW.tracking_number) <> '')
                                OR (NEW.status = 'cancelled'
                                    AND NEW.delivered_at IS NULL AND NEW.cancelled_at IS NOT NULL)
                            ) THEN RAISE(ABORT, 'invalid fulfillment lifecycle') END;
                            SELECT CASE WHEN NOT EXISTS (
                                SELECT 1 FROM orders o
                                WHERE o.id = NEW.order_id AND o.tenant_id = NEW.tenant_id
                            ) THEN RAISE(ABORT, 'fulfillment order tenant mismatch') END;
                            SELECT CASE WHEN NEW.shipping_option_id IS NOT NULL AND NOT EXISTS (
                                SELECT 1 FROM shipping_options so
                                WHERE so.id = NEW.shipping_option_id AND so.tenant_id = NEW.tenant_id
                            ) THEN RAISE(ABORT, 'fulfillment shipping option tenant mismatch') END;
                        END;

                        CREATE TRIGGER fulfillment_items_integrity_guard_insert
                        BEFORE INSERT ON fulfillment_items FOR EACH ROW BEGIN
                            SELECT CASE WHEN NEW.quantity <= 0 OR NEW.shipped_quantity < 0
                                OR NEW.delivered_quantity < 0
                                OR NEW.delivered_quantity > NEW.shipped_quantity
                                OR NEW.shipped_quantity > NEW.quantity
                                THEN RAISE(ABORT, 'invalid fulfillment item progress') END;
                            SELECT CASE WHEN NOT EXISTS (
                                SELECT 1
                                FROM fulfillments f
                                JOIN order_line_items li ON li.id = NEW.order_line_item_id
                                WHERE f.id = NEW.fulfillment_id AND f.order_id = li.order_id
                            ) THEN RAISE(ABORT, 'fulfillment item order mismatch') END;
                        END;

                        CREATE TRIGGER fulfillment_items_integrity_guard_update
                        BEFORE UPDATE OF fulfillment_id, order_line_item_id, quantity,
                            shipped_quantity, delivered_quantity
                        ON fulfillment_items FOR EACH ROW BEGIN
                            SELECT CASE WHEN NEW.quantity <= 0 OR NEW.shipped_quantity < 0
                                OR NEW.delivered_quantity < 0
                                OR NEW.delivered_quantity > NEW.shipped_quantity
                                OR NEW.shipped_quantity > NEW.quantity
                                THEN RAISE(ABORT, 'invalid fulfillment item progress') END;
                            SELECT CASE WHEN NOT EXISTS (
                                SELECT 1
                                FROM fulfillments f
                                JOIN order_line_items li ON li.id = NEW.order_line_item_id
                                WHERE f.id = NEW.fulfillment_id AND f.order_id = li.order_id
                            ) THEN RAISE(ABORT, 'fulfillment item order mismatch') END;
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
                        DROP TRIGGER IF EXISTS fulfillment_items_ownership_guard ON fulfillment_items;
                        DROP TRIGGER IF EXISTS fulfillments_ownership_guard ON fulfillments;
                        DROP FUNCTION IF EXISTS enforce_fulfillment_ownership();
                        ALTER TABLE fulfillment_items DROP CONSTRAINT IF EXISTS ck_fulfillment_items_progress;
                        ALTER TABLE fulfillments
                            DROP CONSTRAINT IF EXISTS ck_fulfillments_lifecycle,
                            DROP CONSTRAINT IF EXISTS ck_fulfillments_status;
                        ALTER TABLE shipping_options
                            DROP CONSTRAINT IF EXISTS ck_shipping_options_provider,
                            DROP CONSTRAINT IF EXISTS ck_shipping_options_amount,
                            DROP CONSTRAINT IF EXISTS ck_shipping_options_currency;
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS fulfillment_items_integrity_guard_update;
                        DROP TRIGGER IF EXISTS fulfillment_items_integrity_guard_insert;
                        DROP TRIGGER IF EXISTS fulfillments_integrity_guard_update;
                        DROP TRIGGER IF EXISTS fulfillments_integrity_guard_insert;
                        DROP TRIGGER IF EXISTS shipping_options_integrity_guard_update;
                        DROP TRIGGER IF EXISTS shipping_options_integrity_guard_insert;
                        "#,
                    )
                    .await?;
            }
            _ => {}
        }

        manager
            .drop_index(
                Index::drop()
                    .name("ux_fulfillment_items_order_line")
                    .table(Alias::new("fulfillment_items"))
                    .to_owned(),
            )
            .await
    }
}
