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
            DatabaseBackend::MySql => install_mysql(manager).await?,
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
                        DROP TRIGGER IF EXISTS orders_checkout_operation_identity_guard ON orders;
                        DROP FUNCTION IF EXISTS enforce_order_checkout_operation_identity();
                        DROP INDEX IF EXISTS ux_orders_checkout_operation;
                        ALTER TABLE orders
                            DROP CONSTRAINT IF EXISTS ck_orders_checkout_operation_identity;
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS orders_checkout_operation_identity_guard_insert;
                        DROP TRIGGER IF EXISTS orders_checkout_operation_identity_guard_update;
                        DROP INDEX IF EXISTS ux_orders_checkout_operation;
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::MySql => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS orders_checkout_operation_identity_guard_update;
                        DROP INDEX ux_orders_checkout_operation ON orders;
                        ALTER TABLE orders DROP COLUMN checkout_operation_identity;
                        "#,
                    )
                    .await?;
            }
        }
        Ok(())
    }
}

async fn install_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            ALTER TABLE orders
                ADD CONSTRAINT ck_orders_checkout_operation_identity
                CHECK (
                    metadata #>> '{checkout,operation_id}' IS NULL
                    OR btrim(metadata #>> '{checkout,operation_id}') <> ''
                ) NOT VALID;

            CREATE UNIQUE INDEX ux_orders_checkout_operation
            ON orders (tenant_id, ((metadata #>> '{checkout,operation_id}')))
            WHERE metadata #>> '{checkout,operation_id}' IS NOT NULL;

            CREATE OR REPLACE FUNCTION enforce_order_checkout_operation_identity()
            RETURNS trigger AS $$
            DECLARE
                old_identity TEXT;
                new_identity TEXT;
            BEGIN
                old_identity := OLD.metadata #>> '{checkout,operation_id}';
                new_identity := NEW.metadata #>> '{checkout,operation_id}';

                IF old_identity IS DISTINCT FROM new_identity THEN
                    RAISE EXCEPTION 'order checkout operation identity is immutable'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER orders_checkout_operation_identity_guard
            BEFORE UPDATE OF metadata ON orders
            FOR EACH ROW
            EXECUTE FUNCTION enforce_order_checkout_operation_identity();
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
            CREATE UNIQUE INDEX ux_orders_checkout_operation
            ON orders (
                tenant_id,
                json_extract(metadata, '$.checkout.operation_id')
            )
            WHERE json_extract(metadata, '$.checkout.operation_id') IS NOT NULL;

            CREATE TRIGGER orders_checkout_operation_identity_guard_insert
            BEFORE INSERT ON orders
            FOR EACH ROW
            WHEN json_extract(NEW.metadata, '$.checkout.operation_id') IS NOT NULL
            BEGIN
                SELECT CASE WHEN trim(json_extract(NEW.metadata, '$.checkout.operation_id')) = ''
                    THEN RAISE(ABORT, 'checkout operation identity must not be empty') END;
            END;

            CREATE TRIGGER orders_checkout_operation_identity_guard_update
            BEFORE UPDATE OF metadata ON orders
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN json_extract(OLD.metadata, '$.checkout.operation_id')
                    IS NOT json_extract(NEW.metadata, '$.checkout.operation_id')
                    THEN RAISE(ABORT, 'order checkout operation identity is immutable') END;
            END;
            "#,
        )
        .await?;
    Ok(())
}

async fn install_mysql(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            ALTER TABLE orders
                ADD COLUMN checkout_operation_identity VARCHAR(191)
                    GENERATED ALWAYS AS (
                        JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.operation_id'))
                    ) STORED,
                ADD UNIQUE INDEX ux_orders_checkout_operation (
                    tenant_id,
                    checkout_operation_identity
                );

            CREATE TRIGGER orders_checkout_operation_identity_guard_update
            BEFORE UPDATE ON orders
            FOR EACH ROW
            BEGIN
                IF NOT (OLD.checkout_operation_identity <=> NEW.checkout_operation_identity) THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'order checkout operation identity is immutable';
                END IF;
            END;
            "#,
        )
        .await?;
    Ok(())
}
