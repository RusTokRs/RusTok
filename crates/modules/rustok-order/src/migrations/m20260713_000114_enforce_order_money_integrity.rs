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
                            ADD CONSTRAINT ck_orders_currency
                            CHECK (currency_code ~ '^[A-Z]{3}$') NOT VALID,
                            ADD CONSTRAINT ck_orders_money_minor_range
                            CHECK (
                                total_amount >= 0 AND total_amount <= 92233720368547758.07
                                AND shipping_total >= 0 AND shipping_total <= 92233720368547758.07
                                AND tax_total >= 0 AND tax_total <= 92233720368547758.07
                            ) NOT VALID;

                        ALTER TABLE order_line_items
                            ADD CONSTRAINT ck_order_line_items_quantity
                            CHECK (quantity > 0) NOT VALID,
                            ADD CONSTRAINT ck_order_line_items_currency
                            CHECK (currency_code ~ '^[A-Z]{3}$') NOT VALID,
                            ADD CONSTRAINT ck_order_line_items_money
                            CHECK (
                                unit_price >= 0 AND unit_price <= 92233720368547758.07
                                AND total_price >= 0 AND total_price <= 92233720368547758.07
                                AND total_price = unit_price * quantity
                            ) NOT VALID;

                        ALTER TABLE order_adjustments
                            ADD CONSTRAINT ck_order_adjustments_amount
                            CHECK (amount >= 0 AND amount <= 92233720368547758.07) NOT VALID,
                            ADD CONSTRAINT ck_order_adjustments_currency
                            CHECK (currency_code ~ '^[A-Z]{3}$') NOT VALID,
                            ADD CONSTRAINT ck_order_adjustments_source
                            CHECK (btrim(source_type) <> '') NOT VALID;

                        ALTER TABLE order_tax_lines
                            ADD CONSTRAINT ck_order_tax_lines_amounts
                            CHECK (
                                rate >= 0
                                AND amount >= 0 AND amount <= 92233720368547758.07
                            ) NOT VALID,
                            ADD CONSTRAINT ck_order_tax_lines_currency
                            CHECK (currency_code ~ '^[A-Z]{3}$') NOT VALID,
                            ADD CONSTRAINT ck_order_tax_lines_provider
                            CHECK (btrim(provider_id) <> '') NOT VALID,
                            ADD CONSTRAINT ck_order_tax_lines_target
                            CHECK (NOT (order_line_item_id IS NOT NULL AND shipping_option_id IS NOT NULL)) NOT VALID;

                        CREATE OR REPLACE FUNCTION enforce_order_child_integrity()
                        RETURNS trigger AS $$
                        DECLARE
                            order_currency VARCHAR(3);
                            line_order UUID;
                        BEGIN
                            SELECT currency_code INTO order_currency
                            FROM orders WHERE id = NEW.order_id;
                            IF NOT FOUND THEN
                                RAISE EXCEPTION 'order % does not exist', NEW.order_id
                                    USING ERRCODE = '23503';
                            END IF;
                            IF NEW.currency_code <> order_currency THEN
                                RAISE EXCEPTION 'order child currency does not match order currency'
                                    USING ERRCODE = '23514';
                            END IF;

                            IF TG_TABLE_NAME = 'order_line_items' THEN
                                RETURN NEW;
                            END IF;

                            IF NEW.order_line_item_id IS NOT NULL THEN
                                SELECT order_id INTO line_order
                                FROM order_line_items WHERE id = NEW.order_line_item_id;
                                IF line_order IS NULL OR line_order <> NEW.order_id THEN
                                    RAISE EXCEPTION 'order line item does not belong to order'
                                        USING ERRCODE = '23514';
                                END IF;
                            END IF;
                            RETURN NEW;
                        END;
                        $$ LANGUAGE plpgsql;

                        CREATE TRIGGER order_line_items_integrity_guard
                        BEFORE INSERT OR UPDATE OF order_id, currency_code
                        ON order_line_items FOR EACH ROW
                        EXECUTE FUNCTION enforce_order_child_integrity();

                        CREATE TRIGGER order_adjustments_integrity_guard
                        BEFORE INSERT OR UPDATE OF order_id, order_line_item_id, currency_code
                        ON order_adjustments FOR EACH ROW
                        EXECUTE FUNCTION enforce_order_child_integrity();

                        CREATE TRIGGER order_tax_lines_integrity_guard
                        BEFORE INSERT OR UPDATE OF order_id, order_line_item_id, currency_code
                        ON order_tax_lines FOR EACH ROW
                        EXECUTE FUNCTION enforce_order_child_integrity();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        CREATE TRIGGER orders_money_guard_insert
                        BEFORE INSERT ON orders FOR EACH ROW BEGIN
                            SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid order currency') END;
                            SELECT CASE WHEN NEW.total_amount < 0 OR NEW.total_amount > 92233720368547758.07
                                OR NEW.shipping_total < 0 OR NEW.shipping_total > 92233720368547758.07
                                OR NEW.tax_total < 0 OR NEW.tax_total > 92233720368547758.07
                                THEN RAISE(ABORT, 'order amount is outside minor-unit range') END;
                        END;

                        CREATE TRIGGER orders_money_guard_update
                        BEFORE UPDATE OF currency_code, total_amount, shipping_total, tax_total
                        ON orders FOR EACH ROW BEGIN
                            SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid order currency') END;
                            SELECT CASE WHEN NEW.total_amount < 0 OR NEW.total_amount > 92233720368547758.07
                                OR NEW.shipping_total < 0 OR NEW.shipping_total > 92233720368547758.07
                                OR NEW.tax_total < 0 OR NEW.tax_total > 92233720368547758.07
                                THEN RAISE(ABORT, 'order amount is outside minor-unit range') END;
                        END;

                        CREATE TRIGGER order_line_items_money_guard_insert
                        BEFORE INSERT ON order_line_items FOR EACH ROW BEGIN
                            SELECT CASE WHEN NEW.quantity <= 0
                                OR NEW.unit_price < 0 OR NEW.unit_price > 92233720368547758.07
                                OR NEW.total_price < 0 OR NEW.total_price > 92233720368547758.07
                                OR NEW.total_price <> NEW.unit_price * NEW.quantity
                                THEN RAISE(ABORT, 'invalid order line item money') END;
                            SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid order line currency') END;
                            SELECT CASE WHEN NOT EXISTS (
                                SELECT 1 FROM orders o
                                WHERE o.id = NEW.order_id AND o.currency_code = NEW.currency_code
                            ) THEN RAISE(ABORT, 'order line item does not match order') END;
                        END;

                        CREATE TRIGGER order_line_items_money_guard_update
                        BEFORE UPDATE OF order_id, quantity, unit_price, total_price, currency_code
                        ON order_line_items FOR EACH ROW BEGIN
                            SELECT CASE WHEN NEW.quantity <= 0
                                OR NEW.unit_price < 0 OR NEW.unit_price > 92233720368547758.07
                                OR NEW.total_price < 0 OR NEW.total_price > 92233720368547758.07
                                OR NEW.total_price <> NEW.unit_price * NEW.quantity
                                THEN RAISE(ABORT, 'invalid order line item money') END;
                            SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid order line currency') END;
                            SELECT CASE WHEN NOT EXISTS (
                                SELECT 1 FROM orders o
                                WHERE o.id = NEW.order_id AND o.currency_code = NEW.currency_code
                            ) THEN RAISE(ABORT, 'order line item does not match order') END;
                        END;

                        CREATE TRIGGER order_adjustments_integrity_guard_insert
                        BEFORE INSERT ON order_adjustments FOR EACH ROW BEGIN
                            SELECT CASE WHEN NEW.amount < 0 OR NEW.amount > 92233720368547758.07
                                OR trim(NEW.source_type) = ''
                                THEN RAISE(ABORT, 'invalid order adjustment') END;
                            SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid order adjustment currency') END;
                            SELECT CASE WHEN NOT EXISTS (
                                SELECT 1 FROM orders o
                                WHERE o.id = NEW.order_id AND o.currency_code = NEW.currency_code
                            ) THEN RAISE(ABORT, 'order adjustment does not match order') END;
                            SELECT CASE WHEN NEW.order_line_item_id IS NOT NULL AND NOT EXISTS (
                                SELECT 1 FROM order_line_items li
                                WHERE li.id = NEW.order_line_item_id AND li.order_id = NEW.order_id
                            ) THEN RAISE(ABORT, 'adjustment line item does not belong to order') END;
                        END;

                        CREATE TRIGGER order_adjustments_integrity_guard_update
                        BEFORE UPDATE OF order_id, order_line_item_id, source_type, amount, currency_code
                        ON order_adjustments FOR EACH ROW BEGIN
                            SELECT CASE WHEN NEW.amount < 0 OR NEW.amount > 92233720368547758.07
                                OR trim(NEW.source_type) = ''
                                THEN RAISE(ABORT, 'invalid order adjustment') END;
                            SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid order adjustment currency') END;
                            SELECT CASE WHEN NOT EXISTS (
                                SELECT 1 FROM orders o
                                WHERE o.id = NEW.order_id AND o.currency_code = NEW.currency_code
                            ) THEN RAISE(ABORT, 'order adjustment does not match order') END;
                            SELECT CASE WHEN NEW.order_line_item_id IS NOT NULL AND NOT EXISTS (
                                SELECT 1 FROM order_line_items li
                                WHERE li.id = NEW.order_line_item_id AND li.order_id = NEW.order_id
                            ) THEN RAISE(ABORT, 'adjustment line item does not belong to order') END;
                        END;

                        CREATE TRIGGER order_tax_lines_integrity_guard_insert
                        BEFORE INSERT ON order_tax_lines FOR EACH ROW BEGIN
                            SELECT CASE WHEN NEW.rate < 0 OR NEW.amount < 0
                                OR NEW.amount > 92233720368547758.07 OR trim(NEW.provider_id) = ''
                                THEN RAISE(ABORT, 'invalid order tax line') END;
                            SELECT CASE WHEN NEW.order_line_item_id IS NOT NULL AND NEW.shipping_option_id IS NOT NULL
                                THEN RAISE(ABORT, 'order tax line has multiple targets') END;
                            SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid order tax currency') END;
                            SELECT CASE WHEN NOT EXISTS (
                                SELECT 1 FROM orders o
                                WHERE o.id = NEW.order_id AND o.currency_code = NEW.currency_code
                            ) THEN RAISE(ABORT, 'order tax line does not match order') END;
                            SELECT CASE WHEN NEW.order_line_item_id IS NOT NULL AND NOT EXISTS (
                                SELECT 1 FROM order_line_items li
                                WHERE li.id = NEW.order_line_item_id AND li.order_id = NEW.order_id
                            ) THEN RAISE(ABORT, 'tax line item does not belong to order') END;
                        END;

                        CREATE TRIGGER order_tax_lines_integrity_guard_update
                        BEFORE UPDATE OF order_id, order_line_item_id, shipping_option_id, provider_id, rate, amount, currency_code
                        ON order_tax_lines FOR EACH ROW BEGIN
                            SELECT CASE WHEN NEW.rate < 0 OR NEW.amount < 0
                                OR NEW.amount > 92233720368547758.07 OR trim(NEW.provider_id) = ''
                                THEN RAISE(ABORT, 'invalid order tax line') END;
                            SELECT CASE WHEN NEW.order_line_item_id IS NOT NULL AND NEW.shipping_option_id IS NOT NULL
                                THEN RAISE(ABORT, 'order tax line has multiple targets') END;
                            SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid order tax currency') END;
                            SELECT CASE WHEN NOT EXISTS (
                                SELECT 1 FROM orders o
                                WHERE o.id = NEW.order_id AND o.currency_code = NEW.currency_code
                            ) THEN RAISE(ABORT, 'order tax line does not match order') END;
                            SELECT CASE WHEN NEW.order_line_item_id IS NOT NULL AND NOT EXISTS (
                                SELECT 1 FROM order_line_items li
                                WHERE li.id = NEW.order_line_item_id AND li.order_id = NEW.order_id
                            ) THEN RAISE(ABORT, 'tax line item does not belong to order') END;
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
                        DROP TRIGGER IF EXISTS order_tax_lines_integrity_guard ON order_tax_lines;
                        DROP TRIGGER IF EXISTS order_adjustments_integrity_guard ON order_adjustments;
                        DROP TRIGGER IF EXISTS order_line_items_integrity_guard ON order_line_items;
                        DROP FUNCTION IF EXISTS enforce_order_child_integrity();
                        ALTER TABLE order_tax_lines
                            DROP CONSTRAINT IF EXISTS ck_order_tax_lines_target,
                            DROP CONSTRAINT IF EXISTS ck_order_tax_lines_provider,
                            DROP CONSTRAINT IF EXISTS ck_order_tax_lines_currency,
                            DROP CONSTRAINT IF EXISTS ck_order_tax_lines_amounts;
                        ALTER TABLE order_adjustments
                            DROP CONSTRAINT IF EXISTS ck_order_adjustments_source,
                            DROP CONSTRAINT IF EXISTS ck_order_adjustments_currency,
                            DROP CONSTRAINT IF EXISTS ck_order_adjustments_amount;
                        ALTER TABLE order_line_items
                            DROP CONSTRAINT IF EXISTS ck_order_line_items_money,
                            DROP CONSTRAINT IF EXISTS ck_order_line_items_currency,
                            DROP CONSTRAINT IF EXISTS ck_order_line_items_quantity;
                        ALTER TABLE orders
                            DROP CONSTRAINT IF EXISTS ck_orders_money_minor_range,
                            DROP CONSTRAINT IF EXISTS ck_orders_currency;
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS order_tax_lines_integrity_guard_update;
                        DROP TRIGGER IF EXISTS order_tax_lines_integrity_guard_insert;
                        DROP TRIGGER IF EXISTS order_adjustments_integrity_guard_update;
                        DROP TRIGGER IF EXISTS order_adjustments_integrity_guard_insert;
                        DROP TRIGGER IF EXISTS order_line_items_money_guard_update;
                        DROP TRIGGER IF EXISTS order_line_items_money_guard_insert;
                        DROP TRIGGER IF EXISTS orders_money_guard_update;
                        DROP TRIGGER IF EXISTS orders_money_guard_insert;
                        "#,
                    )
                    .await?;
            }
            _ => {}
        }
        Ok(())
    }
}
