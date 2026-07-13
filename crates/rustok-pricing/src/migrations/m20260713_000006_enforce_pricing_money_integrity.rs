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
                        ALTER TABLE prices
                            ADD CONSTRAINT ck_prices_currency
                            CHECK (currency_code ~ '^[A-Z]{3}$') NOT VALID,
                            ADD CONSTRAINT ck_prices_amount_minor_range
                            CHECK (
                                amount_decimal >= 0
                                AND amount_decimal <= 92233720368547758.07
                                AND (compare_at_amount_decimal IS NULL OR (
                                    compare_at_amount_decimal >= 0
                                    AND compare_at_amount_decimal <= 92233720368547758.07
                                ))
                            ) NOT VALID,
                            ADD CONSTRAINT ck_prices_quantity_range
                            CHECK (
                                (min_quantity IS NULL OR min_quantity > 0)
                                AND (max_quantity IS NULL OR max_quantity > 0)
                                AND (min_quantity IS NULL OR max_quantity IS NULL OR max_quantity >= min_quantity)
                            ) NOT VALID;

                        ALTER TABLE price_lists
                            ADD CONSTRAINT ck_price_lists_window
                            CHECK (starts_at IS NULL OR ends_at IS NULL OR ends_at >= starts_at) NOT VALID,
                            ADD CONSTRAINT ck_price_lists_rule
                            CHECK (
                                (rule_kind IS NULL AND adjustment_percent IS NULL)
                                OR (rule_kind = 'percentage_discount'
                                    AND adjustment_percent > 0
                                    AND adjustment_percent <= 100)
                            ) NOT VALID;

                        CREATE OR REPLACE FUNCTION enforce_price_tenant_integrity()
                        RETURNS trigger AS $$
                        DECLARE
                            variant_tenant UUID;
                            list_tenant UUID;
                            list_channel_id UUID;
                            list_channel_slug VARCHAR(100);
                            region_tenant UUID;
                            region_currency VARCHAR(3);
                            resolved_channel_tenant UUID;
                            resolved_channel_slug VARCHAR(100);
                        BEGIN
                            SELECT tenant_id INTO variant_tenant
                            FROM product_variants WHERE id = NEW.variant_id;
                            IF variant_tenant IS NULL THEN
                                RAISE EXCEPTION 'variant % does not exist', NEW.variant_id
                                    USING ERRCODE = '23503';
                            END IF;

                            IF NEW.price_list_id IS NOT NULL THEN
                                SELECT tenant_id, channel_id, channel_slug
                                INTO list_tenant, list_channel_id, list_channel_slug
                                FROM price_lists WHERE id = NEW.price_list_id;
                                IF list_tenant IS NULL THEN
                                    RAISE EXCEPTION 'price list % does not exist', NEW.price_list_id
                                        USING ERRCODE = '23503';
                                END IF;
                                IF list_tenant <> variant_tenant THEN
                                    RAISE EXCEPTION 'price list and variant belong to different tenants'
                                        USING ERRCODE = '23514';
                                END IF;
                                IF NEW.channel_id IS DISTINCT FROM list_channel_id
                                    OR lower(btrim(COALESCE(NEW.channel_slug, '')))
                                        IS DISTINCT FROM lower(btrim(COALESCE(list_channel_slug, '')))
                                THEN
                                    RAISE EXCEPTION 'price row channel scope does not match price list scope'
                                        USING ERRCODE = '23514';
                                END IF;
                            END IF;

                            IF NEW.region_id IS NOT NULL THEN
                                SELECT tenant_id, currency_code
                                INTO region_tenant, region_currency
                                FROM regions WHERE id = NEW.region_id;
                                IF region_tenant IS NULL THEN
                                    RAISE EXCEPTION 'region % does not exist', NEW.region_id
                                        USING ERRCODE = '23503';
                                END IF;
                                IF region_tenant <> variant_tenant OR region_currency <> NEW.currency_code THEN
                                    RAISE EXCEPTION 'region does not match price tenant and currency'
                                        USING ERRCODE = '23514';
                                END IF;
                            END IF;

                            IF NEW.channel_id IS NOT NULL THEN
                                SELECT tenant_id, slug
                                INTO resolved_channel_tenant, resolved_channel_slug
                                FROM channels WHERE id = NEW.channel_id;
                                IF resolved_channel_tenant IS NULL THEN
                                    RAISE EXCEPTION 'channel % does not exist', NEW.channel_id
                                        USING ERRCODE = '23503';
                                END IF;
                                IF resolved_channel_tenant <> variant_tenant THEN
                                    RAISE EXCEPTION 'channel and variant belong to different tenants'
                                        USING ERRCODE = '23514';
                                END IF;
                                IF NEW.channel_slug IS NOT NULL
                                    AND lower(btrim(NEW.channel_slug)) <> lower(btrim(resolved_channel_slug))
                                THEN
                                    RAISE EXCEPTION 'channel_id and channel_slug do not identify the same channel'
                                        USING ERRCODE = '23514';
                                END IF;
                            ELSIF NEW.channel_slug IS NOT NULL AND NOT EXISTS (
                                SELECT 1 FROM channels
                                WHERE tenant_id = variant_tenant
                                  AND lower(btrim(slug)) = lower(btrim(NEW.channel_slug))
                            ) THEN
                                RAISE EXCEPTION 'channel slug does not belong to price tenant'
                                    USING ERRCODE = '23514';
                            END IF;

                            RETURN NEW;
                        END;
                        $$ LANGUAGE plpgsql;

                        CREATE TRIGGER prices_tenant_integrity_guard
                        BEFORE INSERT OR UPDATE OF variant_id, price_list_id, region_id,
                            channel_id, channel_slug, currency_code
                        ON prices FOR EACH ROW
                        EXECUTE FUNCTION enforce_price_tenant_integrity();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        CREATE TRIGGER prices_integrity_guard_insert
                        BEFORE INSERT ON prices FOR EACH ROW BEGIN
                            SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid price currency') END;
                            SELECT CASE WHEN NEW.amount_decimal < 0 OR NEW.amount_decimal > 92233720368547758.07
                                OR (NEW.compare_at_amount_decimal IS NOT NULL AND (
                                    NEW.compare_at_amount_decimal < 0
                                    OR NEW.compare_at_amount_decimal > 92233720368547758.07
                                )) THEN RAISE(ABORT, 'price amount outside minor-unit range') END;
                            SELECT CASE WHEN (NEW.min_quantity IS NOT NULL AND NEW.min_quantity <= 0)
                                OR (NEW.max_quantity IS NOT NULL AND NEW.max_quantity <= 0)
                                OR (NEW.min_quantity IS NOT NULL AND NEW.max_quantity IS NOT NULL
                                    AND NEW.max_quantity < NEW.min_quantity)
                                THEN RAISE(ABORT, 'invalid price quantity range') END;
                            SELECT CASE WHEN NOT EXISTS (
                                SELECT 1 FROM product_variants WHERE id = NEW.variant_id
                            ) THEN RAISE(ABORT, 'price variant does not exist') END;
                            SELECT CASE WHEN NEW.price_list_id IS NOT NULL AND NOT EXISTS (
                                SELECT 1 FROM price_lists pl
                                JOIN product_variants pv ON pv.id = NEW.variant_id
                                WHERE pl.id = NEW.price_list_id
                                  AND pl.tenant_id = pv.tenant_id
                                  AND pl.channel_id IS NEW.channel_id
                                  AND lower(trim(COALESCE(pl.channel_slug, ''))) = lower(trim(COALESCE(NEW.channel_slug, '')))
                            ) THEN RAISE(ABORT, 'price list scope does not match variant') END;
                            SELECT CASE WHEN NEW.region_id IS NOT NULL AND NOT EXISTS (
                                SELECT 1 FROM regions r
                                JOIN product_variants pv ON pv.id = NEW.variant_id
                                WHERE r.id = NEW.region_id
                                  AND r.tenant_id = pv.tenant_id
                                  AND r.currency_code = NEW.currency_code
                            ) THEN RAISE(ABORT, 'price region does not match variant') END;
                            SELECT CASE WHEN NEW.channel_id IS NOT NULL AND NOT EXISTS (
                                SELECT 1 FROM channels c
                                JOIN product_variants pv ON pv.id = NEW.variant_id
                                WHERE c.id = NEW.channel_id
                                  AND c.tenant_id = pv.tenant_id
                                  AND (NEW.channel_slug IS NULL OR lower(trim(c.slug)) = lower(trim(NEW.channel_slug)))
                            ) THEN RAISE(ABORT, 'price channel does not match variant') END;
                            SELECT CASE WHEN NEW.channel_id IS NULL AND NEW.channel_slug IS NOT NULL AND NOT EXISTS (
                                SELECT 1 FROM channels c
                                JOIN product_variants pv ON pv.id = NEW.variant_id
                                WHERE c.tenant_id = pv.tenant_id
                                  AND lower(trim(c.slug)) = lower(trim(NEW.channel_slug))
                            ) THEN RAISE(ABORT, 'price channel slug does not match variant tenant') END;
                        END;

                        CREATE TRIGGER prices_integrity_guard_update
                        BEFORE UPDATE OF variant_id, price_list_id, region_id, channel_id, channel_slug,
                            currency_code, amount_decimal, compare_at_amount_decimal, min_quantity, max_quantity
                        ON prices FOR EACH ROW BEGIN
                            SELECT CASE WHEN length(NEW.currency_code) <> 3 OR NEW.currency_code GLOB '*[^A-Z]*'
                                THEN RAISE(ABORT, 'invalid price currency') END;
                            SELECT CASE WHEN NEW.amount_decimal < 0 OR NEW.amount_decimal > 92233720368547758.07
                                OR (NEW.compare_at_amount_decimal IS NOT NULL AND (
                                    NEW.compare_at_amount_decimal < 0
                                    OR NEW.compare_at_amount_decimal > 92233720368547758.07
                                )) THEN RAISE(ABORT, 'price amount outside minor-unit range') END;
                            SELECT CASE WHEN (NEW.min_quantity IS NOT NULL AND NEW.min_quantity <= 0)
                                OR (NEW.max_quantity IS NOT NULL AND NEW.max_quantity <= 0)
                                OR (NEW.min_quantity IS NOT NULL AND NEW.max_quantity IS NOT NULL
                                    AND NEW.max_quantity < NEW.min_quantity)
                                THEN RAISE(ABORT, 'invalid price quantity range') END;
                            SELECT CASE WHEN NOT EXISTS (
                                SELECT 1 FROM product_variants WHERE id = NEW.variant_id
                            ) THEN RAISE(ABORT, 'price variant does not exist') END;
                            SELECT CASE WHEN NEW.price_list_id IS NOT NULL AND NOT EXISTS (
                                SELECT 1 FROM price_lists pl
                                JOIN product_variants pv ON pv.id = NEW.variant_id
                                WHERE pl.id = NEW.price_list_id
                                  AND pl.tenant_id = pv.tenant_id
                                  AND pl.channel_id IS NEW.channel_id
                                  AND lower(trim(COALESCE(pl.channel_slug, ''))) = lower(trim(COALESCE(NEW.channel_slug, '')))
                            ) THEN RAISE(ABORT, 'price list scope does not match variant') END;
                            SELECT CASE WHEN NEW.region_id IS NOT NULL AND NOT EXISTS (
                                SELECT 1 FROM regions r
                                JOIN product_variants pv ON pv.id = NEW.variant_id
                                WHERE r.id = NEW.region_id
                                  AND r.tenant_id = pv.tenant_id
                                  AND r.currency_code = NEW.currency_code
                            ) THEN RAISE(ABORT, 'price region does not match variant') END;
                            SELECT CASE WHEN NEW.channel_id IS NOT NULL AND NOT EXISTS (
                                SELECT 1 FROM channels c
                                JOIN product_variants pv ON pv.id = NEW.variant_id
                                WHERE c.id = NEW.channel_id
                                  AND c.tenant_id = pv.tenant_id
                                  AND (NEW.channel_slug IS NULL OR lower(trim(c.slug)) = lower(trim(NEW.channel_slug)))
                            ) THEN RAISE(ABORT, 'price channel does not match variant') END;
                            SELECT CASE WHEN NEW.channel_id IS NULL AND NEW.channel_slug IS NOT NULL AND NOT EXISTS (
                                SELECT 1 FROM channels c
                                JOIN product_variants pv ON pv.id = NEW.variant_id
                                WHERE c.tenant_id = pv.tenant_id
                                  AND lower(trim(c.slug)) = lower(trim(NEW.channel_slug))
                            ) THEN RAISE(ABORT, 'price channel slug does not match variant tenant') END;
                        END;

                        CREATE TRIGGER price_lists_integrity_guard_insert
                        BEFORE INSERT ON price_lists FOR EACH ROW BEGIN
                            SELECT CASE WHEN NEW.starts_at IS NOT NULL AND NEW.ends_at IS NOT NULL
                                AND NEW.ends_at < NEW.starts_at
                                THEN RAISE(ABORT, 'invalid price list window') END;
                            SELECT CASE WHEN NOT (
                                (NEW.rule_kind IS NULL AND NEW.adjustment_percent IS NULL)
                                OR (NEW.rule_kind = 'percentage_discount'
                                    AND NEW.adjustment_percent > 0 AND NEW.adjustment_percent <= 100)
                            ) THEN RAISE(ABORT, 'invalid price list rule') END;
                        END;

                        CREATE TRIGGER price_lists_integrity_guard_update
                        BEFORE UPDATE OF starts_at, ends_at, rule_kind, adjustment_percent
                        ON price_lists FOR EACH ROW BEGIN
                            SELECT CASE WHEN NEW.starts_at IS NOT NULL AND NEW.ends_at IS NOT NULL
                                AND NEW.ends_at < NEW.starts_at
                                THEN RAISE(ABORT, 'invalid price list window') END;
                            SELECT CASE WHEN NOT (
                                (NEW.rule_kind IS NULL AND NEW.adjustment_percent IS NULL)
                                OR (NEW.rule_kind = 'percentage_discount'
                                    AND NEW.adjustment_percent > 0 AND NEW.adjustment_percent <= 100)
                            ) THEN RAISE(ABORT, 'invalid price list rule') END;
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
                        DROP TRIGGER IF EXISTS prices_tenant_integrity_guard ON prices;
                        DROP FUNCTION IF EXISTS enforce_price_tenant_integrity();
                        ALTER TABLE price_lists
                            DROP CONSTRAINT IF EXISTS ck_price_lists_rule,
                            DROP CONSTRAINT IF EXISTS ck_price_lists_window;
                        ALTER TABLE prices
                            DROP CONSTRAINT IF EXISTS ck_prices_quantity_range,
                            DROP CONSTRAINT IF EXISTS ck_prices_amount_minor_range,
                            DROP CONSTRAINT IF EXISTS ck_prices_currency;
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS price_lists_integrity_guard_update;
                        DROP TRIGGER IF EXISTS price_lists_integrity_guard_insert;
                        DROP TRIGGER IF EXISTS prices_integrity_guard_update;
                        DROP TRIGGER IF EXISTS prices_integrity_guard_insert;
                        "#,
                    )
                    .await?;
            }
            _ => {}
        }
        Ok(())
    }
}
