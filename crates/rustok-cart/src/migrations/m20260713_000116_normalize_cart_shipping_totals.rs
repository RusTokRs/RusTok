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
                        DROP TRIGGER IF EXISTS cart_shipping_total_update_guard ON carts;
                        DROP TRIGGER IF EXISTS cart_shipping_total_insert_guard ON carts;
                        DROP FUNCTION IF EXISTS normalize_cart_shipping_total();
                        DROP VIEW IF EXISTS cart_expected_shipping_totals;
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS cart_shipping_total_update_guard;
                        DROP TRIGGER IF EXISTS cart_shipping_total_insert_guard;
                        DROP VIEW IF EXISTS cart_expected_shipping_totals;
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
            CREATE OR REPLACE VIEW cart_expected_shipping_totals AS
            SELECT
                c.id AS cart_id,
                CASE
                    WHEN EXISTS (
                        SELECT 1
                        FROM cart_shipping_selections css
                        WHERE css.cart_id = c.id
                    ) THEN COALESCE((
                        SELECT SUM(so.amount)
                        FROM cart_shipping_selections css
                        JOIN shipping_options so ON so.id = css.selected_shipping_option_id
                        WHERE css.cart_id = c.id
                          AND css.selected_shipping_option_id IS NOT NULL
                    ), 0)
                    ELSE COALESCE((
                        SELECT so.amount
                        FROM shipping_options so
                        WHERE so.id = c.selected_shipping_option_id
                    ), 0)
                END AS expected_shipping_total
            FROM carts c;

            CREATE OR REPLACE FUNCTION normalize_cart_shipping_total()
            RETURNS trigger AS $$
            DECLARE
                expected_shipping NUMERIC;
                adjusted_total NUMERIC;
            BEGIN
                IF EXISTS (
                    SELECT 1
                    FROM cart_shipping_selections css
                    WHERE css.cart_id = NEW.id
                ) THEN
                    SELECT COALESCE(SUM(so.amount), 0)
                    INTO expected_shipping
                    FROM cart_shipping_selections css
                    JOIN shipping_options so ON so.id = css.selected_shipping_option_id
                    WHERE css.cart_id = NEW.id
                      AND css.selected_shipping_option_id IS NOT NULL;
                ELSE
                    SELECT COALESCE((
                        SELECT so.amount
                        FROM shipping_options so
                        WHERE so.id = NEW.selected_shipping_option_id
                    ), 0)
                    INTO expected_shipping;
                END IF;

                expected_shipping := COALESCE(expected_shipping, 0);
                adjusted_total := NEW.total_amount + expected_shipping - NEW.shipping_total;
                IF adjusted_total < 0 THEN
                    RAISE EXCEPTION 'cart total cannot remain non-negative after shipping normalization'
                        USING ERRCODE = '23514';
                END IF;

                NEW.shipping_total := expected_shipping;
                NEW.total_amount := adjusted_total;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER cart_shipping_total_insert_guard
            BEFORE INSERT ON carts
            FOR EACH ROW
            EXECUTE FUNCTION normalize_cart_shipping_total();

            CREATE TRIGGER cart_shipping_total_update_guard
            BEFORE UPDATE OF shipping_total, total_amount, selected_shipping_option_id ON carts
            FOR EACH ROW
            EXECUTE FUNCTION normalize_cart_shipping_total();
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
            CREATE VIEW cart_expected_shipping_totals AS
            SELECT
                c.id AS cart_id,
                CASE
                    WHEN EXISTS (
                        SELECT 1
                        FROM cart_shipping_selections css
                        WHERE css.cart_id = c.id
                    ) THEN COALESCE((
                        SELECT SUM(so.amount)
                        FROM cart_shipping_selections css
                        JOIN shipping_options so ON so.id = css.selected_shipping_option_id
                        WHERE css.cart_id = c.id
                          AND css.selected_shipping_option_id IS NOT NULL
                    ), 0)
                    ELSE COALESCE((
                        SELECT so.amount
                        FROM shipping_options so
                        WHERE so.id = c.selected_shipping_option_id
                    ), 0)
                END AS expected_shipping_total
            FROM carts c;

            CREATE TRIGGER cart_shipping_total_insert_guard
            AFTER INSERT ON carts
            FOR EACH ROW
            WHEN NEW.shipping_total <> COALESCE((
                SELECT expected_shipping_total
                FROM cart_expected_shipping_totals
                WHERE cart_id = NEW.id
            ), 0)
            BEGIN
                UPDATE carts
                SET total_amount = NEW.total_amount
                        + COALESCE((
                            SELECT expected_shipping_total
                            FROM cart_expected_shipping_totals
                            WHERE cart_id = NEW.id
                        ), 0)
                        - NEW.shipping_total,
                    shipping_total = COALESCE((
                        SELECT expected_shipping_total
                        FROM cart_expected_shipping_totals
                        WHERE cart_id = NEW.id
                    ), 0)
                WHERE id = NEW.id;
            END;

            CREATE TRIGGER cart_shipping_total_update_guard
            AFTER UPDATE OF shipping_total, total_amount, selected_shipping_option_id ON carts
            FOR EACH ROW
            WHEN NEW.shipping_total <> COALESCE((
                SELECT expected_shipping_total
                FROM cart_expected_shipping_totals
                WHERE cart_id = NEW.id
            ), 0)
            BEGIN
                UPDATE carts
                SET total_amount = NEW.total_amount
                        + COALESCE((
                            SELECT expected_shipping_total
                            FROM cart_expected_shipping_totals
                            WHERE cart_id = NEW.id
                        ), 0)
                        - NEW.shipping_total,
                    shipping_total = COALESCE((
                        SELECT expected_shipping_total
                        FROM cart_expected_shipping_totals
                        WHERE cart_id = NEW.id
                    ), 0)
                WHERE id = NEW.id;
            END;
            "#,
        )
        .await?;
    Ok(())
}
