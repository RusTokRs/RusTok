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
            DatabaseBackend::Postgres => restore_postgres(manager).await?,
            DatabaseBackend::Sqlite => restore_sqlite(manager).await?,
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
            DROP TRIGGER IF EXISTS shipping_option_cart_reference_update_guard ON shipping_options;

            CREATE OR REPLACE FUNCTION protect_referenced_shipping_option()
            RETURNS trigger AS $$
            DECLARE
                referenced_any BOOLEAN;
                referenced_live BOOLEAN;
                referenced_checkout BOOLEAN;
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

                SELECT EXISTS (
                    SELECT 1
                    FROM carts c
                    WHERE c.selected_shipping_option_id = OLD.id
                      AND c.status = 'checking_out'
                    UNION ALL
                    SELECT 1
                    FROM cart_shipping_selections css
                    JOIN carts c ON c.id = css.cart_id
                    WHERE css.selected_shipping_option_id = OLD.id
                      AND c.status = 'checking_out'
                ) INTO referenced_checkout;

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
                IF referenced_checkout AND (
                    NEW.amount IS DISTINCT FROM OLD.amount
                    OR NEW.provider_id IS DISTINCT FROM OLD.provider_id
                    OR NEW.metadata IS DISTINCT FROM OLD.metadata
                ) THEN
                    RAISE EXCEPTION 'shipping option economics are locked by an active checkout'
                        USING ERRCODE = '23514';
                END IF;

                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER shipping_option_cart_reference_update_guard
            BEFORE UPDATE OF tenant_id, currency_code, active, amount, provider_id, metadata
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
            DROP TRIGGER IF EXISTS shipping_option_cart_reference_update_guard;

            CREATE TRIGGER shipping_option_cart_reference_update_guard
            BEFORE UPDATE OF tenant_id, currency_code, active, amount, provider_id, metadata
            ON shipping_options
            FOR EACH ROW
            WHEN (
                NEW.tenant_id <> OLD.tenant_id
                OR upper(NEW.currency_code) <> upper(OLD.currency_code)
                OR (OLD.active = 1 AND NEW.active = 0)
                OR NEW.amount IS NOT OLD.amount
                OR NEW.provider_id IS NOT OLD.provider_id
                OR NEW.metadata IS NOT OLD.metadata
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

                SELECT CASE WHEN (
                    (
                        NEW.amount IS NOT OLD.amount
                        OR NEW.provider_id IS NOT OLD.provider_id
                        OR NEW.metadata IS NOT OLD.metadata
                    )
                    AND (
                        EXISTS (
                            SELECT 1 FROM carts c
                            WHERE c.selected_shipping_option_id = OLD.id
                              AND c.status = 'checking_out'
                        )
                        OR EXISTS (
                            SELECT 1
                            FROM cart_shipping_selections css
                            JOIN carts c ON c.id = css.cart_id
                            WHERE css.selected_shipping_option_id = OLD.id
                              AND c.status = 'checking_out'
                        )
                    )
                ) THEN RAISE(ABORT, 'shipping option economics are locked by an active checkout') END;
            END;
            "#,
        )
        .await?;
    Ok(())
}

async fn restore_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER IF EXISTS shipping_option_cart_reference_update_guard ON shipping_options;

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

            CREATE TRIGGER shipping_option_cart_reference_update_guard
            BEFORE UPDATE OF tenant_id, currency_code, active ON shipping_options
            FOR EACH ROW
            EXECUTE FUNCTION protect_referenced_shipping_option();
            "#,
        )
        .await?;
    Ok(())
}

async fn restore_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER IF EXISTS shipping_option_cart_reference_update_guard;

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
            "#,
        )
        .await?;
    Ok(())
}
