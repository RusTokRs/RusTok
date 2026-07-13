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
                        DROP TRIGGER IF EXISTS fulfillments_checkout_identity_guard ON fulfillments;
                        DROP FUNCTION IF EXISTS enforce_fulfillment_checkout_identity();
                        DROP INDEX IF EXISTS ux_fulfillments_checkout_identity;
                        ALTER TABLE fulfillments
                            DROP CONSTRAINT IF EXISTS ck_fulfillments_checkout_identity;
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS fulfillments_checkout_identity_guard_insert;
                        DROP TRIGGER IF EXISTS fulfillments_checkout_identity_guard_update;
                        DROP INDEX IF EXISTS ux_fulfillments_checkout_identity;
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::MySql => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS fulfillments_checkout_identity_guard_update;
                        DROP INDEX ux_fulfillments_checkout_identity ON fulfillments;
                        ALTER TABLE fulfillments DROP COLUMN checkout_fulfillment_identity;
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
            ALTER TABLE fulfillments
                ADD CONSTRAINT ck_fulfillments_checkout_identity
                CHECK (
                    metadata #>> '{checkout,fulfillment_key}' IS NULL
                    OR (
                        btrim(metadata #>> '{checkout,fulfillment_key}') <> ''
                        AND btrim(COALESCE(metadata #>> '{checkout,operation_id}', '')) <> ''
                    )
                ) NOT VALID;

            CREATE UNIQUE INDEX ux_fulfillments_checkout_identity
            ON fulfillments (
                tenant_id,
                ((metadata #>> '{checkout,fulfillment_key}'))
            )
            WHERE metadata #>> '{checkout,fulfillment_key}' IS NOT NULL;

            CREATE OR REPLACE FUNCTION enforce_fulfillment_checkout_identity()
            RETURNS trigger AS $$
            DECLARE
                old_identity TEXT;
                new_identity TEXT;
            BEGIN
                old_identity := OLD.metadata #>> '{checkout,fulfillment_key}';
                new_identity := NEW.metadata #>> '{checkout,fulfillment_key}';
                IF old_identity IS DISTINCT FROM new_identity THEN
                    RAISE EXCEPTION 'fulfillment checkout identity is immutable'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER fulfillments_checkout_identity_guard
            BEFORE UPDATE OF metadata ON fulfillments
            FOR EACH ROW
            EXECUTE FUNCTION enforce_fulfillment_checkout_identity();
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
            CREATE UNIQUE INDEX ux_fulfillments_checkout_identity
            ON fulfillments (
                tenant_id,
                json_extract(metadata, '$.checkout.fulfillment_key')
            )
            WHERE json_extract(metadata, '$.checkout.fulfillment_key') IS NOT NULL;

            CREATE TRIGGER fulfillments_checkout_identity_guard_insert
            BEFORE INSERT ON fulfillments
            FOR EACH ROW
            WHEN json_extract(NEW.metadata, '$.checkout.fulfillment_key') IS NOT NULL
            BEGIN
                SELECT CASE WHEN
                    trim(json_extract(NEW.metadata, '$.checkout.fulfillment_key')) = ''
                    OR trim(COALESCE(json_extract(NEW.metadata, '$.checkout.operation_id'), '')) = ''
                    THEN RAISE(ABORT, 'invalid fulfillment checkout identity') END;
            END;

            CREATE TRIGGER fulfillments_checkout_identity_guard_update
            BEFORE UPDATE OF metadata ON fulfillments
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN json_extract(OLD.metadata, '$.checkout.fulfillment_key')
                    IS NOT json_extract(NEW.metadata, '$.checkout.fulfillment_key')
                    THEN RAISE(ABORT, 'fulfillment checkout identity is immutable') END;
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
            ALTER TABLE fulfillments
                ADD COLUMN checkout_fulfillment_identity VARCHAR(191)
                    GENERATED ALWAYS AS (
                        JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.fulfillment_key'))
                    ) STORED,
                ADD UNIQUE INDEX ux_fulfillments_checkout_identity (
                    tenant_id,
                    checkout_fulfillment_identity
                );

            CREATE TRIGGER fulfillments_checkout_identity_guard_update
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            BEGIN
                IF NOT (
                    OLD.checkout_fulfillment_identity
                    <=> NEW.checkout_fulfillment_identity
                ) THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'fulfillment checkout identity is immutable';
                END IF;
            END;
            "#,
        )
        .await?;
    Ok(())
}
