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
                            ADD CONSTRAINT ck_carts_status
                            CHECK (status IN ('active', 'checking_out', 'completed', 'abandoned')) NOT VALID,
                            ADD CONSTRAINT ck_carts_currency
                            CHECK (currency_code ~ '^[A-Z]{3}$') NOT VALID,
                            ADD CONSTRAINT ck_carts_money
                            CHECK (total_amount >= 0 AND shipping_total >= 0 AND tax_total >= 0) NOT VALID,
                            ADD CONSTRAINT ck_carts_completion_state
                            CHECK (
                                (status = 'completed' AND completed_at IS NOT NULL)
                                OR (status <> 'completed' AND completed_at IS NULL)
                            ) NOT VALID;

                        ALTER TABLE cart_line_items
                            ADD CONSTRAINT ck_cart_line_items_quantity
                            CHECK (quantity > 0) NOT VALID,
                            ADD CONSTRAINT ck_cart_line_items_currency
                            CHECK (currency_code ~ '^[A-Z]{3}$') NOT VALID,
                            ADD CONSTRAINT ck_cart_line_items_money
                            CHECK (
                                unit_price >= 0
                                AND total_price >= 0
                                AND total_price = unit_price * quantity
                            ) NOT VALID;

                        ALTER TABLE cart_adjustments
                            ADD CONSTRAINT ck_cart_adjustments_amount
                            CHECK (amount > 0) NOT VALID,
                            ADD CONSTRAINT ck_cart_adjustments_currency
                            CHECK (currency_code ~ '^[A-Z]{3}$') NOT VALID,
                            ADD CONSTRAINT ck_cart_adjustments_source
                            CHECK (btrim(source_type) <> '') NOT VALID;

                        ALTER TABLE cart_tax_lines
                            ADD CONSTRAINT ck_cart_tax_lines_amounts
                            CHECK (rate >= 0 AND amount >= 0) NOT VALID,
                            ADD CONSTRAINT ck_cart_tax_lines_currency
                            CHECK (currency_code ~ '^[A-Z]{3}$') NOT VALID,
                            ADD CONSTRAINT ck_cart_tax_lines_provider
                            CHECK (btrim(provider_id) <> '') NOT VALID,
                            ADD CONSTRAINT ck_cart_tax_lines_target
                            CHECK ((cart_line_item_id IS NULL) <> (shipping_option_id IS NULL)) NOT VALID;

                        CREATE OR REPLACE FUNCTION enforce_cart_child_integrity()
                        RETURNS trigger AS $$
                        DECLARE
                            cart_currency VARCHAR(3);
                            cart_tenant UUID;
                            line_cart UUID;
                            option_tenant UUID;
                            option_currency VARCHAR(3);
                        BEGIN
                            SELECT currency_code, tenant_id
                            INTO cart_currency, cart_tenant
                            FROM carts
                            WHERE id = NEW.cart_id;

                            IF NOT FOUND THEN
                                RAISE EXCEPTION 'cart % does not exist', NEW.cart_id
                                    USING ERRCODE = '23503';
                            END IF;
                            IF NEW.currency_code <> cart_currency THEN
                                RAISE EXCEPTION 'cart child currency does not match cart currency'
                                    USING ERRCODE = '23514';
                            END IF;

                            IF TG_TABLE_NAME = 'cart_line_items' THEN
                                RETURN NEW;
                            END IF;

                            IF NEW.cart_line_item_id IS NOT NULL THEN
                                SELECT cart_id INTO line_cart
                                FROM cart_line_items
                                WHERE id = NEW.cart_line_item_id;
                                IF line_cart IS NULL OR line_cart <> NEW.cart_id THEN
                                    RAISE EXCEPTION 'cart line item does not belong to cart'
                                        USING ERRCODE = '23514';
                                END IF;
                            END IF;

                            IF TG_TABLE_NAME = 'cart_tax_lines' AND NEW.shipping_option_id IS NOT NULL THEN
                                SELECT tenant_id, currency_code
                                INTO option_tenant, option_currency
                                FROM shipping_options
                                WHERE id = NEW.shipping_option_id;
                                IF option_tenant IS NULL THEN
                                    RAISE EXCEPTION 'shipping option % does not exist', NEW.shipping_option_id
                                        USING ERRCODE = '23503';
                                END IF;
                                IF option_tenant <> cart_tenant OR option_currency <> cart_currency THEN
                                    RAISE EXCEPTION 'shipping option does not match cart tenant and currency'
                                        USING ERRCODE = '23514';
                                END IF;
                            END IF;

                            RETURN NEW;
                        END;
                        $$ LANGUAGE plpgsql;

                        CREATE TRIGGER cart_line_items_integrity_guard
                        BEFORE INSERT OR UPDATE OF cart_id, currency_code
                        ON cart_line_items
                        FOR EACH ROW EXECUTE FUNCTION enforce_cart_child_integrity();

                        CREATE TRIGGER cart_adjustments_integrity_guard
                        BEFORE INSERT OR UPDATE OF cart_id, cart_line_item_id, currency_code
                        ON cart_adjustments
                        FOR EACH ROW EXECUTE FUNCTION enforce_cart_child_integrity();

                        CREATE TRIGGER cart_tax_lines_integrity_guard
                        BEFORE INSERT OR UPDATE OF cart_id, cart_line_item_id, shipping_option_id, currency_code
                        ON cart_tax_lines
                        FOR EACH ROW EXECUTE FUNCTION enforce_cart_child_integrity();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        CREATE TRIGGER carts_integrity_guard_insert
                        BEFORE INSERT ON carts
                        FOR EACH ROW BEGIN
                            SELECT CASE WHEN NEW.status NOT IN ('active', 'checking_out', 'completed', 'abandoned')
                                THEN RAISE(ABORT, 'invalid cart status') END;
                            SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid cart currency') END;
                            SELECT CASE WHEN NEW.total_amount < 0 OR NEW.shipping_total < 0 OR NEW.tax_total < 0
                                THEN RAISE(ABORT, 'invalid cart monetary totals') END;
                            SELECT CASE WHEN NOT (
                                (NEW.status = 'completed' AND NEW.completed_at IS NOT NULL)
                                OR (NEW.status <> 'completed' AND NEW.completed_at IS NULL)
                            ) THEN RAISE(ABORT, 'invalid cart completion state') END;
                        END;

                        CREATE TRIGGER carts_integrity_guard_update
                        BEFORE UPDATE OF status, currency_code, total_amount, shipping_total, tax_total, completed_at
                        ON carts FOR EACH ROW BEGIN
                            SELECT CASE WHEN NEW.status NOT IN ('active', 'checking_out', 'completed', 'abandoned')
                                THEN RAISE(ABORT, 'invalid cart status') END;
                            SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid cart currency') END;
                            SELECT CASE WHEN NEW.total_amount < 0 OR NEW.shipping_total < 0 OR NEW.tax_total < 0
                                THEN RAISE(ABORT, 'invalid cart monetary totals') END;
                            SELECT CASE WHEN NOT (
                                (NEW.status = 'completed' AND NEW.completed_at IS NOT NULL)
                                OR (NEW.status <> 'completed' AND NEW.completed_at IS NULL)
                            ) THEN RAISE(ABORT, 'invalid cart completion state') END;
                        END;

                        CREATE TRIGGER cart_line_items_integrity_guard_insert
                        BEFORE INSERT ON cart_line_items
                        FOR EACH ROW BEGIN
                            SELECT CASE WHEN NEW.quantity <= 0 OR NEW.unit_price < 0 OR NEW.total_price < 0
                                OR NEW.total_price <> NEW.unit_price * NEW.quantity
                                THEN RAISE(ABORT, 'invalid cart line item money') END;
                            SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid cart line item currency') END;
                            SELECT CASE WHEN NOT EXISTS (
                                SELECT 1 FROM carts c
                                WHERE c.id = NEW.cart_id AND c.currency_code = NEW.currency_code
                            ) THEN RAISE(ABORT, 'cart line item does not match cart') END;
                        END;

                        CREATE TRIGGER cart_line_items_integrity_guard_update
                        BEFORE UPDATE OF cart_id, quantity, unit_price, total_price, currency_code
                        ON cart_line_items FOR EACH ROW BEGIN
                            SELECT CASE WHEN NEW.quantity <= 0 OR NEW.unit_price < 0 OR NEW.total_price < 0
                                OR NEW.total_price <> NEW.unit_price * NEW.quantity
                                THEN RAISE(ABORT, 'invalid cart line item money') END;
                            SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid cart line item currency') END;
                            SELECT CASE WHEN NOT EXISTS (
                                SELECT 1 FROM carts c
                                WHERE c.id = NEW.cart_id AND c.currency_code = NEW.currency_code
                            ) THEN RAISE(ABORT, 'cart line item does not match cart') END;
                        END;

                        CREATE TRIGGER cart_adjustments_integrity_guard_insert
                        BEFORE INSERT ON cart_adjustments
                        FOR EACH ROW BEGIN
                            SELECT CASE WHEN NEW.amount <= 0 OR trim(NEW.source_type) = ''
                                THEN RAISE(ABORT, 'invalid cart adjustment') END;
                            SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid cart adjustment currency') END;
                            SELECT CASE WHEN NOT EXISTS (
                                SELECT 1 FROM carts c
                                WHERE c.id = NEW.cart_id AND c.currency_code = NEW.currency_code
                            ) THEN RAISE(ABORT, 'cart adjustment does not match cart') END;
                            SELECT CASE WHEN NEW.cart_line_item_id IS NOT NULL AND NOT EXISTS (
                                SELECT 1 FROM cart_line_items li
                                WHERE li.id = NEW.cart_line_item_id AND li.cart_id = NEW.cart_id
                            ) THEN RAISE(ABORT, 'adjustment line item does not belong to cart') END;
                        END;

                        CREATE TRIGGER cart_adjustments_integrity_guard_update
                        BEFORE UPDATE OF cart_id, cart_line_item_id, source_type, amount, currency_code
                        ON cart_adjustments FOR EACH ROW BEGIN
                            SELECT CASE WHEN NEW.amount <= 0 OR trim(NEW.source_type) = ''
                                THEN RAISE(ABORT, 'invalid cart adjustment') END;
                            SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid cart adjustment currency') END;
                            SELECT CASE WHEN NOT EXISTS (
                                SELECT 1 FROM carts c
                                WHERE c.id = NEW.cart_id AND c.currency_code = NEW.currency_code
                            ) THEN RAISE(ABORT, 'cart adjustment does not match cart') END;
                            SELECT CASE WHEN NEW.cart_line_item_id IS NOT NULL AND NOT EXISTS (
                                SELECT 1 FROM cart_line_items li
                                WHERE li.id = NEW.cart_line_item_id AND li.cart_id = NEW.cart_id
                            ) THEN RAISE(ABORT, 'adjustment line item does not belong to cart') END;
                        END;

                        CREATE TRIGGER cart_tax_lines_integrity_guard_insert
                        BEFORE INSERT ON cart_tax_lines
                        FOR EACH ROW BEGIN
                            SELECT CASE WHEN NEW.rate < 0 OR NEW.amount < 0 OR trim(NEW.provider_id) = ''
                                THEN RAISE(ABORT, 'invalid cart tax line') END;
                            SELECT CASE WHEN (NEW.cart_line_item_id IS NULL) = (NEW.shipping_option_id IS NULL)
                                THEN RAISE(ABORT, 'tax line requires exactly one taxable target') END;
                            SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid cart tax line currency') END;
                            SELECT CASE WHEN NOT EXISTS (
                                SELECT 1 FROM carts c
                                WHERE c.id = NEW.cart_id AND c.currency_code = NEW.currency_code
                            ) THEN RAISE(ABORT, 'cart tax line does not match cart') END;
                            SELECT CASE WHEN NEW.cart_line_item_id IS NOT NULL AND NOT EXISTS (
                                SELECT 1 FROM cart_line_items li
                                WHERE li.id = NEW.cart_line_item_id AND li.cart_id = NEW.cart_id
                            ) THEN RAISE(ABORT, 'tax line item does not belong to cart') END;
                            SELECT CASE WHEN NEW.shipping_option_id IS NOT NULL AND NOT EXISTS (
                                SELECT 1 FROM shipping_options so
                                JOIN carts c ON c.id = NEW.cart_id
                                WHERE so.id = NEW.shipping_option_id
                                  AND so.tenant_id = c.tenant_id
                                  AND so.currency_code = c.currency_code
                            ) THEN RAISE(ABORT, 'shipping option does not match cart') END;
                        END;

                        CREATE TRIGGER cart_tax_lines_integrity_guard_update
                        BEFORE UPDATE OF cart_id, cart_line_item_id, shipping_option_id, provider_id, rate, amount, currency_code
                        ON cart_tax_lines FOR EACH ROW BEGIN
                            SELECT CASE WHEN NEW.rate < 0 OR NEW.amount < 0 OR trim(NEW.provider_id) = ''
                                THEN RAISE(ABORT, 'invalid cart tax line') END;
                            SELECT CASE WHEN (NEW.cart_line_item_id IS NULL) = (NEW.shipping_option_id IS NULL)
                                THEN RAISE(ABORT, 'tax line requires exactly one taxable target') END;
                            SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid cart tax line currency') END;
                            SELECT CASE WHEN NOT EXISTS (
                                SELECT 1 FROM carts c
                                WHERE c.id = NEW.cart_id AND c.currency_code = NEW.currency_code
                            ) THEN RAISE(ABORT, 'cart tax line does not match cart') END;
                            SELECT CASE WHEN NEW.cart_line_item_id IS NOT NULL AND NOT EXISTS (
                                SELECT 1 FROM cart_line_items li
                                WHERE li.id = NEW.cart_line_item_id AND li.cart_id = NEW.cart_id
                            ) THEN RAISE(ABORT, 'tax line item does not belong to cart') END;
                            SELECT CASE WHEN NEW.shipping_option_id IS NOT NULL AND NOT EXISTS (
                                SELECT 1 FROM shipping_options so
                                JOIN carts c ON c.id = NEW.cart_id
                                WHERE so.id = NEW.shipping_option_id
                                  AND so.tenant_id = c.tenant_id
                                  AND so.currency_code = c.currency_code
                            ) THEN RAISE(ABORT, 'shipping option does not match cart') END;
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
                        DROP TRIGGER IF EXISTS cart_tax_lines_integrity_guard ON cart_tax_lines;
                        DROP TRIGGER IF EXISTS cart_adjustments_integrity_guard ON cart_adjustments;
                        DROP TRIGGER IF EXISTS cart_line_items_integrity_guard ON cart_line_items;
                        DROP FUNCTION IF EXISTS enforce_cart_child_integrity();
                        ALTER TABLE cart_tax_lines
                            DROP CONSTRAINT IF EXISTS ck_cart_tax_lines_target,
                            DROP CONSTRAINT IF EXISTS ck_cart_tax_lines_provider,
                            DROP CONSTRAINT IF EXISTS ck_cart_tax_lines_currency,
                            DROP CONSTRAINT IF EXISTS ck_cart_tax_lines_amounts;
                        ALTER TABLE cart_adjustments
                            DROP CONSTRAINT IF EXISTS ck_cart_adjustments_source,
                            DROP CONSTRAINT IF EXISTS ck_cart_adjustments_currency,
                            DROP CONSTRAINT IF EXISTS ck_cart_adjustments_amount;
                        ALTER TABLE cart_line_items
                            DROP CONSTRAINT IF EXISTS ck_cart_line_items_money,
                            DROP CONSTRAINT IF EXISTS ck_cart_line_items_currency,
                            DROP CONSTRAINT IF EXISTS ck_cart_line_items_quantity;
                        ALTER TABLE carts
                            DROP CONSTRAINT IF EXISTS ck_carts_completion_state,
                            DROP CONSTRAINT IF EXISTS ck_carts_money,
                            DROP CONSTRAINT IF EXISTS ck_carts_currency,
                            DROP CONSTRAINT IF EXISTS ck_carts_status;
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS cart_tax_lines_integrity_guard_update;
                        DROP TRIGGER IF EXISTS cart_tax_lines_integrity_guard_insert;
                        DROP TRIGGER IF EXISTS cart_adjustments_integrity_guard_update;
                        DROP TRIGGER IF EXISTS cart_adjustments_integrity_guard_insert;
                        DROP TRIGGER IF EXISTS cart_line_items_integrity_guard_update;
                        DROP TRIGGER IF EXISTS cart_line_items_integrity_guard_insert;
                        DROP TRIGGER IF EXISTS carts_integrity_guard_update;
                        DROP TRIGGER IF EXISTS carts_integrity_guard_insert;
                        "#,
                    )
                    .await?;
            }
            _ => {}
        }
        Ok(())
    }
}
