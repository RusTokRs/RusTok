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
                        DROP TRIGGER IF EXISTS shipping_option_cart_reference_guard ON shipping_options;
                        DROP TRIGGER IF EXISTS cart_shipping_selection_option_guard ON cart_shipping_selections;
                        DROP TRIGGER IF EXISTS cart_selected_shipping_option_guard ON carts;
                        DROP FUNCTION IF EXISTS protect_referenced_shipping_option();
                        DROP FUNCTION IF EXISTS enforce_cart_shipping_selection_option();
                        DROP FUNCTION IF EXISTS enforce_cart_selected_shipping_option();
                        DROP INDEX IF EXISTS idx_cart_shipping_selections_option;
                        DROP INDEX IF EXISTS idx_carts_selected_shipping_option;
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS shipping_option_cart_reference_delete_guard;
                        DROP TRIGGER IF EXISTS shipping_option_cart_reference_update_guard;
                        DROP TRIGGER IF EXISTS cart_shipping_selection_option_update_guard;
                        DROP TRIGGER IF EXISTS cart_shipping_selection_option_insert_guard;
                        DROP TRIGGER IF EXISTS cart_selected_shipping_option_update_guard;
                        DROP TRIGGER IF EXISTS cart_selected_shipping_option_insert_guard;
                        DROP INDEX IF EXISTS idx_cart_shipping_selections_option;
                        DROP INDEX IF EXISTS idx_carts_selected_shipping_option;
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
            DO $$
            BEGIN
                IF EXISTS (
                    SELECT 1
                    FROM carts c
                    LEFT JOIN shipping_options so ON so.id = c.selected_shipping_option_id
                    WHERE c.selected_shipping_option_id IS NOT NULL
                      AND (
                          so.id IS NULL
                          OR so.tenant_id <> c.tenant_id
                          OR upper(so.currency_code) <> upper(c.currency_code)
                          OR (c.status IN ('active', 'checking_out') AND NOT so.active)
                      )
                ) OR EXISTS (
                    SELECT 1
                    FROM cart_shipping_selections css
                    JOIN carts c ON c.id = css.cart_id
                    LEFT JOIN shipping_options so ON so.id = css.selected_shipping_option_id
                    WHERE css.selected_shipping_option_id IS NOT NULL
                      AND (
                          so.id IS NULL
                          OR so.tenant_id <> c.tenant_id
                          OR upper(so.currency_code) <> upper(c.currency_code)
                          OR (c.status IN ('active', 'checking_out') AND NOT so.active)
                      )
                ) THEN
                    RAISE EXCEPTION 'existing cart shipping option references violate tenant, currency or active-state integrity'
                        USING ERRCODE = '23514';
                END IF;
            END;
            $$;

            CREATE INDEX IF NOT EXISTS idx_carts_selected_shipping_option
                ON carts (selected_shipping_option_id)
                WHERE selected_shipping_option_id IS NOT NULL;
            CREATE INDEX IF NOT EXISTS idx_cart_shipping_selections_option
                ON cart_shipping_selections (selected_shipping_option_id)
                WHERE selected_shipping_option_id IS NOT NULL;

            CREATE OR REPLACE FUNCTION enforce_cart_selected_shipping_option()
            RETURNS trigger AS $$
            DECLARE
                option_row shipping_options%ROWTYPE;
            BEGIN
                IF NEW.selected_shipping_option_id IS NULL THEN
                    RETURN NEW;
                END IF;

                SELECT * INTO option_row
                FROM shipping_options
                WHERE id = NEW.selected_shipping_option_id;

                IF NOT FOUND THEN
                    RAISE EXCEPTION 'selected shipping option does not exist'
                        USING ERRCODE = '23503';
                END IF;
                IF option_row.tenant_id <> NEW.tenant_id THEN
                    RAISE EXCEPTION 'selected shipping option belongs to another tenant'
                        USING ERRCODE = '23514';
                END IF;
                IF upper(option_row.currency_code) <> upper(NEW.currency_code) THEN
                    RAISE EXCEPTION 'selected shipping option currency does not match cart currency'
                        USING ERRCODE = '23514';
                END IF;
                IF NEW.status IN ('active', 'checking_out') AND NOT option_row.active THEN
                    RAISE EXCEPTION 'selected shipping option is inactive'
                        USING ERRCODE = '23514';
                END IF;

                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER cart_selected_shipping_option_guard
            BEFORE INSERT OR UPDATE OF selected_shipping_option_id, tenant_id, currency_code, status
            ON carts
            FOR EACH ROW
            EXECUTE FUNCTION enforce_cart_selected_shipping_option();

            CREATE OR REPLACE FUNCTION enforce_cart_shipping_selection_option()
            RETURNS trigger AS $$
            DECLARE
                cart_row carts%ROWTYPE;
                option_row shipping_options%ROWTYPE;
            BEGIN
                IF NEW.selected_shipping_option_id IS NULL THEN
                    RETURN NEW;
                END IF;

                SELECT * INTO cart_row FROM carts WHERE id = NEW.cart_id;
                IF NOT FOUND THEN
                    RAISE EXCEPTION 'cart shipping selection references a missing cart'
                        USING ERRCODE = '23503';
                END IF;

                SELECT * INTO option_row
                FROM shipping_options
                WHERE id = NEW.selected_shipping_option_id;
                IF NOT FOUND THEN
                    RAISE EXCEPTION 'cart shipping selection references a missing option'
                        USING ERRCODE = '23503';
                END IF;
                IF option_row.tenant_id <> cart_row.tenant_id THEN
                    RAISE EXCEPTION 'cart shipping selection references another tenant option'
                        USING ERRCODE = '23514';
                END IF;
                IF upper(option_row.currency_code) <> upper(cart_row.currency_code) THEN
                    RAISE EXCEPTION 'cart shipping selection option currency does not match cart currency'
                        USING ERRCODE = '23514';
                END IF;
                IF cart_row.status IN ('active', 'checking_out') AND NOT option_row.active THEN
                    RAISE EXCEPTION 'cart shipping selection references an inactive option'
                        USING ERRCODE = '23514';
                END IF;

                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER cart_shipping_selection_option_guard
            BEFORE INSERT OR UPDATE OF cart_id, selected_shipping_option_id
            ON cart_shipping_selections
            FOR EACH ROW
            EXECUTE FUNCTION enforce_cart_shipping_selection_option();

            CREATE OR REPLACE FUNCTION protect_referenced_shipping_option()
            RETURNS trigger AS $$
            DECLARE
                referenced_any BOOLEAN;
                referenced_live BOOLEAN;
            BEGIN
                SELECT EXISTS (
                    SELECT 1 FROM carts c WHERE c.selected_shipping_option_id = OLD.id
                    UNION ALL
                    SELECT 1 FROM cart_shipping_selections css WHERE css.selected_shipping_option_id = OLD.id
                ) INTO referenced_any;

                SELECT EXISTS (
                    SELECT 1
                    FROM carts c
                    WHERE c.selected_shipping_option_id = OLD.id
                      AND c.status IN ('active', 'checking_out')
                    UNION ALL
                    SELECT 1
                    FROM cart_shipping_selections css
                    JOIN carts c ON c.id = css.cart_id
                    WHERE css.selected_shipping_option_id = OLD.id
                      AND c.status IN ('active', 'checking_out')
                ) INTO referenced_live;

                IF TG_OP = 'DELETE' THEN
                    IF referenced_any THEN
                        RAISE EXCEPTION 'shipping option is referenced by a cart'
                            USING ERRCODE = '23503';
                    END IF;
                    RETURN OLD;
                END IF;

                IF referenced_any AND (
                    NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
                    OR upper(NEW.currency_code) IS DISTINCT FROM upper(OLD.currency_code)
                ) THEN
                    RAISE EXCEPTION 'referenced shipping option tenant and currency are immutable'
                        USING ERRCODE = '23514';
                END IF;
                IF referenced_live AND OLD.active AND NOT NEW.active THEN
                    RAISE EXCEPTION 'shipping option is selected by an active checkout cart'
                        USING ERRCODE = '23514';
                END IF;

                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER shipping_option_cart_reference_guard
            BEFORE UPDATE OF tenant_id, currency_code, active OR DELETE
            ON shipping_options
            FOR EACH ROW
            EXECUTE FUNCTION protect_referenced_shipping_option();
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
            CREATE TEMP TABLE cart_shipping_option_integrity_validation (
                valid INTEGER NOT NULL CHECK (valid = 1)
            );

            INSERT INTO cart_shipping_option_integrity_validation (valid)
            SELECT CASE WHEN EXISTS (
                SELECT 1
                FROM carts c
                LEFT JOIN shipping_options so ON so.id = c.selected_shipping_option_id
                WHERE c.selected_shipping_option_id IS NOT NULL
                  AND (
                      so.id IS NULL
                      OR so.tenant_id <> c.tenant_id
                      OR upper(so.currency_code) <> upper(c.currency_code)
                      OR (c.status IN ('active', 'checking_out') AND so.active = 0)
                  )
            ) OR EXISTS (
                SELECT 1
                FROM cart_shipping_selections css
                JOIN carts c ON c.id = css.cart_id
                LEFT JOIN shipping_options so ON so.id = css.selected_shipping_option_id
                WHERE css.selected_shipping_option_id IS NOT NULL
                  AND (
                      so.id IS NULL
                      OR so.tenant_id <> c.tenant_id
                      OR upper(so.currency_code) <> upper(c.currency_code)
                      OR (c.status IN ('active', 'checking_out') AND so.active = 0)
                  )
            ) THEN 0 ELSE 1 END;

            DROP TABLE cart_shipping_option_integrity_validation;

            CREATE INDEX IF NOT EXISTS idx_carts_selected_shipping_option
                ON carts (selected_shipping_option_id);
            CREATE INDEX IF NOT EXISTS idx_cart_shipping_selections_option
                ON cart_shipping_selections (selected_shipping_option_id);

            CREATE TRIGGER cart_selected_shipping_option_insert_guard
            BEFORE INSERT ON carts
            FOR EACH ROW
            WHEN NEW.selected_shipping_option_id IS NOT NULL
            BEGIN
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1
                    FROM shipping_options so
                    WHERE so.id = NEW.selected_shipping_option_id
                      AND so.tenant_id = NEW.tenant_id
                      AND upper(so.currency_code) = upper(NEW.currency_code)
                      AND (NEW.status NOT IN ('active', 'checking_out') OR so.active = 1)
                ) THEN RAISE(ABORT, 'invalid selected shipping option for cart') END;
            END;

            CREATE TRIGGER cart_selected_shipping_option_update_guard
            BEFORE UPDATE OF selected_shipping_option_id, tenant_id, currency_code, status ON carts
            FOR EACH ROW
            WHEN NEW.selected_shipping_option_id IS NOT NULL
            BEGIN
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1
                    FROM shipping_options so
                    WHERE so.id = NEW.selected_shipping_option_id
                      AND so.tenant_id = NEW.tenant_id
                      AND upper(so.currency_code) = upper(NEW.currency_code)
                      AND (NEW.status NOT IN ('active', 'checking_out') OR so.active = 1)
                ) THEN RAISE(ABORT, 'invalid selected shipping option for cart') END;
            END;

            CREATE TRIGGER cart_shipping_selection_option_insert_guard
            BEFORE INSERT ON cart_shipping_selections
            FOR EACH ROW
            WHEN NEW.selected_shipping_option_id IS NOT NULL
            BEGIN
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1
                    FROM carts c
                    JOIN shipping_options so ON so.id = NEW.selected_shipping_option_id
                    WHERE c.id = NEW.cart_id
                      AND so.tenant_id = c.tenant_id
                      AND upper(so.currency_code) = upper(c.currency_code)
                      AND (c.status NOT IN ('active', 'checking_out') OR so.active = 1)
                ) THEN RAISE(ABORT, 'invalid shipping option for cart selection') END;
            END;

            CREATE TRIGGER cart_shipping_selection_option_update_guard
            BEFORE UPDATE OF cart_id, selected_shipping_option_id ON cart_shipping_selections
            FOR EACH ROW
            WHEN NEW.selected_shipping_option_id IS NOT NULL
            BEGIN
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1
                    FROM carts c
                    JOIN shipping_options so ON so.id = NEW.selected_shipping_option_id
                    WHERE c.id = NEW.cart_id
                      AND so.tenant_id = c.tenant_id
                      AND upper(so.currency_code) = upper(c.currency_code)
                      AND (c.status NOT IN ('active', 'checking_out') OR so.active = 1)
                ) THEN RAISE(ABORT, 'invalid shipping option for cart selection') END;
            END;

            CREATE TRIGGER shipping_option_cart_reference_update_guard
            BEFORE UPDATE OF tenant_id, currency_code, active ON shipping_options
            FOR EACH ROW
            WHEN (
                NEW.tenant_id <> OLD.tenant_id
                OR upper(NEW.currency_code) <> upper(OLD.currency_code)
                OR (OLD.active = 1 AND NEW.active = 0)
            )
            BEGIN
                SELECT CASE WHEN (
                    (NEW.tenant_id <> OLD.tenant_id OR upper(NEW.currency_code) <> upper(OLD.currency_code))
                    AND (
                        EXISTS (SELECT 1 FROM carts c WHERE c.selected_shipping_option_id = OLD.id)
                        OR EXISTS (
                            SELECT 1 FROM cart_shipping_selections css
                            WHERE css.selected_shipping_option_id = OLD.id
                        )
                    )
                ) THEN RAISE(ABORT, 'referenced shipping option tenant and currency are immutable') END;

                SELECT CASE WHEN (
                    OLD.active = 1 AND NEW.active = 0
                    AND (
                        EXISTS (
                            SELECT 1 FROM carts c
                            WHERE c.selected_shipping_option_id = OLD.id
                              AND c.status IN ('active', 'checking_out')
                        )
                        OR EXISTS (
                            SELECT 1
                            FROM cart_shipping_selections css
                            JOIN carts c ON c.id = css.cart_id
                            WHERE css.selected_shipping_option_id = OLD.id
                              AND c.status IN ('active', 'checking_out')
                        )
                    )
                ) THEN RAISE(ABORT, 'shipping option is selected by an active checkout cart') END;
            END;

            CREATE TRIGGER shipping_option_cart_reference_delete_guard
            BEFORE DELETE ON shipping_options
            FOR EACH ROW
            WHEN EXISTS (SELECT 1 FROM carts c WHERE c.selected_shipping_option_id = OLD.id)
              OR EXISTS (
                  SELECT 1 FROM cart_shipping_selections css
                  WHERE css.selected_shipping_option_id = OLD.id
              )
            BEGIN
                SELECT RAISE(ABORT, 'shipping option is referenced by a cart');
            END;
            "#,
        )
        .await?;
    Ok(())
}
