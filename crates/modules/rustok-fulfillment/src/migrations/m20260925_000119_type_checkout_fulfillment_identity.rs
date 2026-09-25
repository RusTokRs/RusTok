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
            _ => {
                return Err(DbErr::Migration(
                    "typed checkout fulfillment identity migration does not support this database backend"
                        .to_string(),
                ));
            }
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => restore_postgres(manager).await?,
            DatabaseBackend::Sqlite => restore_sqlite(manager).await?,
            DatabaseBackend::MySql => restore_mysql(manager).await?,
            _ => {
                return Err(DbErr::Migration(
                    "typed checkout fulfillment identity migration does not support this database backend"
                        .to_string(),
                ));
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
                ADD COLUMN checkout_operation_id uuid,
                ADD COLUMN checkout_fulfillment_index bigint,
                ADD COLUMN checkout_plan_hash varchar(64);

            UPDATE fulfillments
            SET
                checkout_operation_id = CASE
                    WHEN btrim(metadata #>> '{checkout,operation_id}')
                        ~* '^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$'
                    THEN (btrim(metadata #>> '{checkout,operation_id}'))::uuid
                    ELSE NULL
                END,
                checkout_fulfillment_index = CASE
                    WHEN btrim(metadata #>> '{checkout,fulfillment_index}') ~ '^[0-9]+
                END,
                checkout_plan_hash = CASE
                    WHEN btrim(metadata #>> '{checkout,order_plan_hash}')
                        ~* '^[0-9a-f]{64}$'
                    THEN btrim(metadata #>> '{checkout,order_plan_hash}')
                    ELSE NULL
                END
            WHERE metadata #>> '{checkout,fulfillment_key}' IS NOT NULL;

            ALTER TABLE fulfillments
                ADD CONSTRAINT ck_fulfillments_checkout_identity_migration
                CHECK (
                    metadata #>> '{checkout,fulfillment_key}' IS NULL
                    OR (
                        checkout_operation_id IS NOT NULL
                        AND checkout_operation_id <> '00000000-0000-0000-0000-000000000000'::uuid
                        AND checkout_fulfillment_index IS NOT NULL
                        AND checkout_fulfillment_index >= 0
                        AND checkout_fulfillment_index <= 4294967295
                        AND checkout_plan_hash IS NOT NULL
                        AND checkout_plan_hash ~* '^[0-9a-f]{64}$'
                        AND lower(btrim(metadata #>> '{checkout,operation_id}')) = checkout_operation_id::text
                        AND lower(btrim(metadata #>> '{checkout,order_id}')) = order_id::text
                        AND btrim(metadata #>> '{checkout,order_plan_hash}') = checkout_plan_hash
                        AND CASE
                            WHEN btrim(metadata #>> '{checkout,fulfillment_index}') ~ '^[0-9]+
                        AND metadata #>> '{checkout,fulfillment_key}' = format(
                            'checkout:%s:fulfillment:%s',
                            checkout_operation_id,
                            checkout_fulfillment_index
                        )
                    )
                ) NOT VALID;
            ALTER TABLE fulfillments
                VALIDATE CONSTRAINT ck_fulfillments_checkout_identity_migration;

            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_guard ON fulfillments;
            DROP FUNCTION IF EXISTS enforce_fulfillment_checkout_identity();
            DROP INDEX IF EXISTS ux_fulfillments_checkout_identity;
            ALTER TABLE fulfillments
                DROP CONSTRAINT IF EXISTS ck_fulfillments_checkout_identity;

            CREATE UNIQUE INDEX ux_fulfillments_checkout_identity
                ON fulfillments (tenant_id, checkout_operation_id, checkout_fulfillment_index);

            UPDATE fulfillments
            SET metadata = metadata
                #- '{checkout,operation_id}'
                #- '{checkout,order_id}'
                #- '{checkout,order_plan_hash}'
                #- '{checkout,fulfillment_index}'
                #- '{checkout,fulfillment_key}'
            WHERE metadata #>> '{checkout,fulfillment_key}' IS NOT NULL;

            UPDATE fulfillment_items AS fi
            SET metadata = metadata
                #- '{checkout,operation_id}'
                #- '{checkout,order_plan_hash}'
                #- '{checkout,fulfillment_index}'
            FROM fulfillments AS f
            WHERE fi.fulfillment_id = f.id
              AND f.checkout_operation_id IS NOT NULL
              AND f.checkout_fulfillment_index IS NOT NULL
              AND f.checkout_plan_hash IS NOT NULL
              AND btrim(fi.metadata #>> '{checkout,operation_id}') = f.checkout_operation_id::text
              AND btrim(fi.metadata #>> '{checkout,order_plan_hash}') = f.checkout_plan_hash
              AND btrim(fi.metadata #>> '{checkout,fulfillment_index}') = f.checkout_fulfillment_index::text;

            ALTER TABLE fulfillments
                DROP CONSTRAINT ck_fulfillments_checkout_identity_migration;

            ALTER TABLE fulfillments
                ADD CONSTRAINT ck_fulfillments_checkout_identity_typed
                CHECK (
                    (
                        checkout_operation_id IS NULL
                        AND checkout_fulfillment_index IS NULL
                        AND checkout_plan_hash IS NULL
                    )
                    OR (
                        checkout_operation_id IS NOT NULL
                        AND checkout_operation_id <> '00000000-0000-0000-0000-000000000000'::uuid
                        AND checkout_fulfillment_index IS NOT NULL
                        AND checkout_fulfillment_index >= 0
                        AND checkout_fulfillment_index <= 4294967295
                        AND checkout_plan_hash IS NOT NULL
                        AND checkout_plan_hash ~* '^[0-9a-f]{64}$'
                    )
                );

            CREATE OR REPLACE FUNCTION enforce_fulfillment_checkout_identity_typed()
            RETURNS trigger AS $$
            BEGIN
                IF NEW.checkout_operation_id IS DISTINCT FROM OLD.checkout_operation_id
                    OR NEW.checkout_fulfillment_index IS DISTINCT FROM OLD.checkout_fulfillment_index
                    OR NEW.checkout_plan_hash IS DISTINCT FROM OLD.checkout_plan_hash
                THEN
                    RAISE EXCEPTION 'fulfillment checkout identity is immutable'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER fulfillments_checkout_identity_typed_guard
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            EXECUTE FUNCTION enforce_fulfillment_checkout_identity_typed();
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
            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_typed_guard ON fulfillments;
            DROP FUNCTION IF EXISTS enforce_fulfillment_checkout_identity_typed();
            DROP INDEX IF EXISTS ux_fulfillments_checkout_identity;

            UPDATE fulfillments
            SET metadata = jsonb_set(
                metadata,
                '{checkout}',
                COALESCE(metadata->'checkout', '{}'::jsonb) || jsonb_build_object(
                    'operation_id', checkout_operation_id::text,
                    'order_id', order_id::text,
                    'order_plan_hash', checkout_plan_hash,
                    'fulfillment_index', checkout_fulfillment_index,
                    'fulfillment_key', format(
                        'checkout:%s:fulfillment:%s',
                        checkout_operation_id,
                        checkout_fulfillment_index
                    )
                ),
                true
            )
            WHERE checkout_operation_id IS NOT NULL
              AND checkout_fulfillment_index IS NOT NULL
              AND checkout_plan_hash IS NOT NULL;

            UPDATE fulfillment_items AS fi
            SET metadata = jsonb_set(
                jsonb_set(
                    jsonb_set(
                        COALESCE(fi.metadata, '{}'::jsonb),
                        '{checkout,operation_id}',
                        to_jsonb(f.checkout_operation_id::text),
                        true
                    ),
                    '{checkout,order_plan_hash}',
                    to_jsonb(f.checkout_plan_hash),
                    true
                ),
                '{checkout,fulfillment_index}',
                to_jsonb(f.checkout_fulfillment_index),
                true
            )
            FROM fulfillments AS f
            WHERE fi.fulfillment_id = f.id
              AND f.checkout_operation_id IS NOT NULL
              AND f.checkout_fulfillment_index IS NOT NULL
              AND f.checkout_plan_hash IS NOT NULL;

            ALTER TABLE fulfillments
                DROP CONSTRAINT IF EXISTS ck_fulfillments_checkout_identity_typed;
            ALTER TABLE fulfillments
                DROP COLUMN checkout_plan_hash,
                DROP COLUMN checkout_fulfillment_index,
                DROP COLUMN checkout_operation_id;

            ALTER TABLE fulfillments
                ADD CONSTRAINT ck_fulfillments_checkout_identity
                CHECK (
                    metadata #>> '{checkout,fulfillment_key}' IS NULL
                    OR (
                        btrim(metadata #>> '{checkout,fulfillment_key}') <> ''
                        AND btrim(COALESCE(metadata #>> '{checkout,operation_id}', '')) <> ''
                    )
                );

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
            ALTER TABLE fulfillments ADD COLUMN checkout_operation_id TEXT;
            ALTER TABLE fulfillments ADD COLUMN checkout_fulfillment_index INTEGER;
            ALTER TABLE fulfillments ADD COLUMN checkout_plan_hash TEXT;

            UPDATE fulfillments
            SET
                checkout_operation_id = CASE
                    WHEN length(trim(json_extract(metadata, '$.checkout.operation_id'))) = 36
                        AND lower(trim(json_extract(metadata, '$.checkout.operation_id')))
                            NOT GLOB '*[^0-9a-f-]*'
                        AND substr(trim(json_extract(metadata, '$.checkout.operation_id')), 9, 1) = '-'
                        AND substr(trim(json_extract(metadata, '$.checkout.operation_id')), 14, 1) = '-'
                        AND substr(trim(json_extract(metadata, '$.checkout.operation_id')), 19, 1) = '-'
                        AND substr(trim(json_extract(metadata, '$.checkout.operation_id')), 24, 1) = '-'
                        AND lower(trim(json_extract(metadata, '$.checkout.operation_id')))
                            <> '00000000-0000-0000-0000-000000000000'
                    THEN lower(trim(json_extract(metadata, '$.checkout.operation_id')))
                    ELSE NULL
                END,
                checkout_fulfillment_index = CASE
                    WHEN trim(json_extract(metadata, '$.checkout.fulfillment_index'))
                        NOT GLOB '*[^0-9]*'
                        AND length(trim(json_extract(metadata, '$.checkout.fulfillment_index'))) > 0
                    THEN CAST(trim(json_extract(metadata, '$.checkout.fulfillment_index')) AS INTEGER)
                    ELSE NULL
                END,
                checkout_plan_hash = CASE
                    WHEN length(trim(json_extract(metadata, '$.checkout.order_plan_hash'))) = 64
                        AND lower(trim(json_extract(metadata, '$.checkout.order_plan_hash')))
                            NOT GLOB '*[^0-9a-f]*'
                    THEN trim(json_extract(metadata, '$.checkout.order_plan_hash'))
                    ELSE NULL
                END
            WHERE json_extract(metadata, '$.checkout.fulfillment_key') IS NOT NULL;

            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_guard_insert;
            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_guard_update;
            DROP INDEX IF EXISTS ux_fulfillments_checkout_identity;

            CREATE TRIGGER fulfillments_checkout_identity_typed_validation
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            WHEN json_extract(OLD.metadata, '$.checkout.fulfillment_key') IS NOT NULL
            BEGIN
                SELECT CASE WHEN
                    NEW.checkout_operation_id IS NULL
                    OR NEW.checkout_fulfillment_index IS NULL
                    OR NEW.checkout_fulfillment_index < 0
                    OR NEW.checkout_fulfillment_index > 4294967295
                    OR NEW.checkout_plan_hash IS NULL
                    OR length(NEW.checkout_plan_hash) <> 64
                    OR lower(NEW.checkout_plan_hash) GLOB '*[^0-9a-f]*'
                    OR lower(NEW.checkout_operation_id) <> lower(trim(json_extract(OLD.metadata, '$.checkout.operation_id')))
                    OR lower(trim(json_extract(OLD.metadata, '$.checkout.order_id'))) <> lower(NEW.order_id)
                    OR NEW.checkout_plan_hash <> trim(json_extract(OLD.metadata, '$.checkout.order_plan_hash'))
                    OR NEW.checkout_fulfillment_index <> CAST(json_extract(OLD.metadata, '$.checkout.fulfillment_index') AS INTEGER)
                    OR json_extract(OLD.metadata, '$.checkout.fulfillment_key')
                        <> 'checkout:' || lower(NEW.checkout_operation_id) || ':fulfillment:' || NEW.checkout_fulfillment_index
                    THEN RAISE(ABORT, 'invalid legacy checkout fulfillment identity') END;
            END;

            UPDATE fulfillments
            SET checkout_operation_id = checkout_operation_id,
                checkout_fulfillment_index = checkout_fulfillment_index,
                checkout_plan_hash = checkout_plan_hash;

            DROP TRIGGER fulfillments_checkout_identity_typed_validation;

            UPDATE fulfillments
            SET metadata = json_remove(
                metadata,
                '$.checkout.operation_id',
                '$.checkout.order_id',
                '$.checkout.order_plan_hash',
                '$.checkout.fulfillment_index',
                '$.checkout.fulfillment_key'
            )
            WHERE json_extract(metadata, '$.checkout.fulfillment_key') IS NOT NULL;

            UPDATE fulfillment_items
            SET metadata = json_remove(
                metadata,
                '$.checkout.operation_id',
                '$.checkout.order_plan_hash',
                '$.checkout.fulfillment_index'
            )
            WHERE EXISTS (
                SELECT 1
                FROM fulfillments AS f
                WHERE f.id = fulfillment_items.fulfillment_id
                  AND f.checkout_operation_id IS NOT NULL
                  AND f.checkout_fulfillment_index IS NOT NULL
                  AND f.checkout_plan_hash IS NOT NULL
                  AND lower(trim(json_extract(fulfillment_items.metadata, '$.checkout.operation_id')))
                        = lower(f.checkout_operation_id)
                  AND trim(json_extract(fulfillment_items.metadata, '$.checkout.order_plan_hash'))
                        = f.checkout_plan_hash
                  AND trim(json_extract(fulfillment_items.metadata, '$.checkout.fulfillment_index'))
                        = CAST(f.checkout_fulfillment_index AS TEXT)
            );

            CREATE TRIGGER fulfillments_checkout_identity_typed_insert
            BEFORE INSERT ON fulfillments
            FOR EACH ROW
            WHEN NOT (
                (NEW.checkout_operation_id IS NULL
                    AND NEW.checkout_fulfillment_index IS NULL
                    AND NEW.checkout_plan_hash IS NULL)
                OR (
                    NEW.checkout_operation_id IS NOT NULL
                    AND lower(NEW.checkout_operation_id)
                        <> '00000000-0000-0000-0000-000000000000'
                    AND NEW.checkout_fulfillment_index IS NOT NULL
                    AND NEW.checkout_fulfillment_index BETWEEN 0 AND 4294967295
                    AND NEW.checkout_plan_hash IS NOT NULL
                    AND length(NEW.checkout_plan_hash) = 64
                    AND lower(NEW.checkout_plan_hash) NOT GLOB '*[^0-9a-f]*'
                )
            )
            BEGIN
                SELECT RAISE(ABORT, 'invalid checkout fulfillment identity');
            END;

            CREATE TRIGGER fulfillments_checkout_identity_typed_update
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NOT (
                    NEW.checkout_operation_id IS OLD.checkout_operation_id
                    AND NEW.checkout_fulfillment_index IS OLD.checkout_fulfillment_index
                    AND NEW.checkout_plan_hash IS OLD.checkout_plan_hash
                ) THEN RAISE(ABORT, 'fulfillment checkout identity is immutable') END;
            END;

            CREATE UNIQUE INDEX ux_fulfillments_checkout_identity
                ON fulfillments (tenant_id, checkout_operation_id, checkout_fulfillment_index);
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
            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_typed_update;
            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_typed_insert;
            DROP INDEX IF EXISTS ux_fulfillments_checkout_identity;

            UPDATE fulfillments
            SET metadata = json_set(
                COALESCE(metadata, '{}'),
                '$.checkout.operation_id', checkout_operation_id,
                '$.checkout.order_id', order_id,
                '$.checkout.order_plan_hash', checkout_plan_hash,
                '$.checkout.fulfillment_index', checkout_fulfillment_index,
                '$.checkout.fulfillment_key',
                    'checkout:' || checkout_operation_id || ':fulfillment:' || checkout_fulfillment_index
            )
            WHERE checkout_operation_id IS NOT NULL
              AND checkout_fulfillment_index IS NOT NULL
              AND checkout_plan_hash IS NOT NULL;

            UPDATE fulfillment_items
            SET metadata = json_set(
                COALESCE(metadata, '{}'),
                '$.checkout.operation_id',
                    (SELECT f.checkout_operation_id
                     FROM fulfillments AS f
                     WHERE f.id = fulfillment_items.fulfillment_id
                       AND f.checkout_operation_id IS NOT NULL
                       AND f.checkout_fulfillment_index IS NOT NULL
                       AND f.checkout_plan_hash IS NOT NULL),
                '$.checkout.order_plan_hash',
                    (SELECT f.checkout_plan_hash
                     FROM fulfillments AS f
                     WHERE f.id = fulfillment_items.fulfillment_id
                       AND f.checkout_operation_id IS NOT NULL
                       AND f.checkout_fulfillment_index IS NOT NULL
                       AND f.checkout_plan_hash IS NOT NULL),
                '$.checkout.fulfillment_index',
                    (SELECT f.checkout_fulfillment_index
                     FROM fulfillments AS f
                     WHERE f.id = fulfillment_items.fulfillment_id
                       AND f.checkout_operation_id IS NOT NULL
                       AND f.checkout_fulfillment_index IS NOT NULL
                       AND f.checkout_plan_hash IS NOT NULL)
            )
            WHERE EXISTS (
                SELECT 1
                FROM fulfillments AS f
                WHERE f.id = fulfillment_items.fulfillment_id
                  AND f.checkout_operation_id IS NOT NULL
                  AND f.checkout_fulfillment_index IS NOT NULL
                  AND f.checkout_plan_hash IS NOT NULL
            );

            ALTER TABLE fulfillments DROP COLUMN checkout_plan_hash;
            ALTER TABLE fulfillments DROP COLUMN checkout_fulfillment_index;
            ALTER TABLE fulfillments DROP COLUMN checkout_operation_id;

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
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN json_extract(OLD.metadata, '$.checkout.fulfillment_key')
                    IS NOT json_extract(NEW.metadata, '$.checkout.fulfillment_key')
                    THEN RAISE(ABORT, 'fulfillment checkout identity is immutable') END;
            END;

            CREATE UNIQUE INDEX ux_fulfillments_checkout_identity
                ON fulfillments (
                    tenant_id,
                    json_extract(metadata, '$.checkout.fulfillment_key')
                )
                WHERE json_extract(metadata, '$.checkout.fulfillment_key') IS NOT NULL;
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
                ADD COLUMN checkout_operation_id CHAR(36),
                ADD COLUMN checkout_fulfillment_index BIGINT,
                ADD COLUMN checkout_plan_hash VARCHAR(64);

            UPDATE fulfillments
            SET
                checkout_operation_id = CASE
                    WHEN JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.operation_id'))
                        REGEXP '^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$'
                     AND lower(JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.operation_id')))
                        <> '00000000-0000-0000-0000-000000000000'
                    THEN lower(JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.operation_id')))
                    ELSE NULL
                END,
                checkout_fulfillment_index = CASE
                    WHEN JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.fulfillment_index')) REGEXP '^[0-9]+$'
                    THEN CAST(JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.fulfillment_index')) AS UNSIGNED)
                    ELSE NULL
                END,
                checkout_plan_hash = CASE
                    WHEN JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.order_plan_hash'))
                        REGEXP '^[0-9a-fA-F]{64}$'
                    THEN JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.order_plan_hash'))
                    ELSE NULL
                END
            WHERE JSON_EXTRACT(metadata, '$.checkout.fulfillment_key') IS NOT NULL;

            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_guard_update;
            DROP INDEX ux_fulfillments_checkout_identity ON fulfillments;
            ALTER TABLE fulfillments DROP COLUMN checkout_fulfillment_identity;

            CREATE TRIGGER fulfillments_checkout_identity_typed_validation
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            BEGIN
                IF JSON_EXTRACT(OLD.metadata, '$.checkout.fulfillment_key') IS NOT NULL
                    AND (
                        NEW.checkout_operation_id IS NULL
                        OR NEW.checkout_fulfillment_index IS NULL
                        OR NEW.checkout_fulfillment_index < 0
                        OR NEW.checkout_fulfillment_index > 4294967295
                        OR NEW.checkout_plan_hash IS NULL
                        OR NEW.checkout_plan_hash NOT REGEXP '^[0-9a-fA-F]{64}$'
                        OR lower(NEW.checkout_operation_id)
                            <> lower(JSON_UNQUOTE(JSON_EXTRACT(OLD.metadata, '$.checkout.operation_id')))
                        OR lower(JSON_UNQUOTE(JSON_EXTRACT(OLD.metadata, '$.checkout.order_id')))
                            <> lower(NEW.order_id)
                        OR NEW.checkout_plan_hash
                            <> JSON_UNQUOTE(JSON_EXTRACT(OLD.metadata, '$.checkout.order_plan_hash'))
                        OR NEW.checkout_fulfillment_index
                            <> CAST(JSON_UNQUOTE(JSON_EXTRACT(OLD.metadata, '$.checkout.fulfillment_index')) AS UNSIGNED)
                        OR JSON_UNQUOTE(JSON_EXTRACT(OLD.metadata, '$.checkout.fulfillment_key'))
                            <> CONCAT('checkout:', lower(NEW.checkout_operation_id), ':fulfillment:', NEW.checkout_fulfillment_index)
                    )
                THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'invalid legacy checkout fulfillment identity';
                END IF;
            END;
            "#,
        )
        .await?;
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            UPDATE fulfillments
            SET checkout_operation_id = checkout_operation_id,
                checkout_fulfillment_index = checkout_fulfillment_index,
                checkout_plan_hash = checkout_plan_hash;
            "#,
        )
        .await?;
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER fulfillments_checkout_identity_typed_validation;

            UPDATE fulfillments
            SET metadata = JSON_REMOVE(
                metadata,
                '$.checkout.operation_id',
                '$.checkout.order_id',
                '$.checkout.order_plan_hash',
                '$.checkout.fulfillment_index',
                '$.checkout.fulfillment_key'
            )
            WHERE JSON_EXTRACT(metadata, '$.checkout.fulfillment_key') IS NOT NULL;

            UPDATE fulfillment_items AS fi
            JOIN fulfillments AS f
              ON f.id = fi.fulfillment_id
            SET fi.metadata = JSON_REMOVE(
                fi.metadata,
                '$.checkout.operation_id',
                '$.checkout.order_plan_hash',
                '$.checkout.fulfillment_index'
            )
            WHERE f.checkout_operation_id IS NOT NULL
              AND f.checkout_fulfillment_index IS NOT NULL
              AND f.checkout_plan_hash IS NOT NULL
              AND TRIM(JSON_UNQUOTE(JSON_EXTRACT(fi.metadata, '$.checkout.operation_id')))
                    = LOWER(f.checkout_operation_id)
              AND TRIM(JSON_UNQUOTE(JSON_EXTRACT(fi.metadata, '$.checkout.order_plan_hash')))
                    = f.checkout_plan_hash
              AND TRIM(JSON_UNQUOTE(JSON_EXTRACT(fi.metadata, '$.checkout.fulfillment_index')))
                    = CAST(f.checkout_fulfillment_index AS CHAR);

            CREATE TRIGGER fulfillments_checkout_identity_typed_insert
            BEFORE INSERT ON fulfillments
            FOR EACH ROW
            BEGIN
                IF NOT (
                    (NEW.checkout_operation_id IS NULL
                        AND NEW.checkout_fulfillment_index IS NULL
                        AND NEW.checkout_plan_hash IS NULL)
                    OR (
                        NEW.checkout_operation_id IS NOT NULL
                        AND lower(NEW.checkout_operation_id)
                            <> '00000000-0000-0000-0000-000000000000'
                        AND NEW.checkout_fulfillment_index IS NOT NULL
                        AND NEW.checkout_fulfillment_index BETWEEN 0 AND 4294967295
                        AND NEW.checkout_plan_hash IS NOT NULL
                        AND NEW.checkout_plan_hash REGEXP '^[0-9a-fA-F]{64}$'
                    )
                ) THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'invalid checkout fulfillment identity';
                END IF;
            END;

            CREATE TRIGGER fulfillments_checkout_identity_typed_update
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            BEGIN
                IF NOT (NEW.checkout_operation_id <=> OLD.checkout_operation_id)
                    OR NOT (NEW.checkout_fulfillment_index <=> OLD.checkout_fulfillment_index)
                    OR NOT (NEW.checkout_plan_hash <=> OLD.checkout_plan_hash)
                THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'fulfillment checkout identity is immutable';
                END IF;
            END;

            CREATE UNIQUE INDEX ux_fulfillments_checkout_identity
                ON fulfillments (tenant_id, checkout_operation_id, checkout_fulfillment_index);
            "#,
        )
        .await?;
    Ok(())
}

async fn restore_mysql(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_typed_update;
            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_typed_insert;
            DROP INDEX ux_fulfillments_checkout_identity ON fulfillments;

            UPDATE fulfillments
            SET metadata = JSON_SET(
                COALESCE(metadata, JSON_OBJECT()),
                '$.checkout.operation_id', checkout_operation_id,
                '$.checkout.order_id', order_id,
                '$.checkout.order_plan_hash', checkout_plan_hash,
                '$.checkout.fulfillment_index', checkout_fulfillment_index,
                '$.checkout.fulfillment_key',
                    CONCAT('checkout:', checkout_operation_id, ':fulfillment:', checkout_fulfillment_index)
            )
            WHERE checkout_operation_id IS NOT NULL
              AND checkout_fulfillment_index IS NOT NULL
              AND checkout_plan_hash IS NOT NULL;

            UPDATE fulfillment_items AS fi
            JOIN fulfillments AS f
              ON f.id = fi.fulfillment_id
            SET fi.metadata = JSON_SET(
                COALESCE(fi.metadata, JSON_OBJECT()),
                '$.checkout.operation_id', f.checkout_operation_id,
                '$.checkout.order_plan_hash', f.checkout_plan_hash,
                '$.checkout.fulfillment_index', f.checkout_fulfillment_index
            )
            WHERE f.checkout_operation_id IS NOT NULL
              AND f.checkout_fulfillment_index IS NOT NULL
              AND f.checkout_plan_hash IS NOT NULL;

            ALTER TABLE fulfillments
                ADD COLUMN checkout_fulfillment_identity VARCHAR(191)
                    GENERATED ALWAYS AS (
                        JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.fulfillment_key'))
                    ) STORED;

            CREATE UNIQUE INDEX ux_fulfillments_checkout_identity
                ON fulfillments (tenant_id, checkout_fulfillment_identity);

            CREATE TRIGGER fulfillments_checkout_identity_guard_update
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            BEGIN
                IF NOT (
                    OLD.checkout_fulfillment_identity <=> NEW.checkout_fulfillment_identity
                ) THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'fulfillment checkout identity is immutable';
                END IF;
            END;

            ALTER TABLE fulfillments
                DROP COLUMN checkout_plan_hash,
                DROP COLUMN checkout_fulfillment_index,
                DROP COLUMN checkout_operation_id;
            "#,
        )
        .await?;
    Ok(())
}

                        AND length(btrim(metadata #>> '{checkout,fulfillment_index}')) <= 10
                    THEN (btrim(metadata #>> '{checkout,fulfillment_index}'))::bigint
                    ELSE NULL
                END,
                checkout_plan_hash = CASE
                    WHEN btrim(metadata #>> '{checkout,order_plan_hash}')
                        ~* '^[0-9a-f]{64}$'
                    THEN btrim(metadata #>> '{checkout,order_plan_hash}')
                    ELSE NULL
                END
            WHERE metadata #>> '{checkout,fulfillment_key}' IS NOT NULL;

            ALTER TABLE fulfillments
                ADD CONSTRAINT ck_fulfillments_checkout_identity_migration
                CHECK (
                    metadata #>> '{checkout,fulfillment_key}' IS NULL
                    OR (
                        checkout_operation_id IS NOT NULL
                        AND checkout_operation_id <> '00000000-0000-0000-0000-000000000000'::uuid
                        AND checkout_fulfillment_index IS NOT NULL
                        AND checkout_fulfillment_index >= 0
                        AND checkout_fulfillment_index <= 4294967295
                        AND checkout_plan_hash IS NOT NULL
                        AND checkout_plan_hash ~* '^[0-9a-f]{64}$'
                        AND lower(btrim(metadata #>> '{checkout,operation_id}')) = checkout_operation_id::text
                        AND lower(btrim(metadata #>> '{checkout,order_id}')) = order_id::text
                        AND btrim(metadata #>> '{checkout,order_plan_hash}') = checkout_plan_hash
                        AND (metadata #>> '{checkout,fulfillment_index}')::bigint = checkout_fulfillment_index
                        AND metadata #>> '{checkout,fulfillment_key}' = format(
                            'checkout:%s:fulfillment:%s',
                            checkout_operation_id,
                            checkout_fulfillment_index
                        )
                    )
                ) NOT VALID;
            ALTER TABLE fulfillments
                VALIDATE CONSTRAINT ck_fulfillments_checkout_identity_migration;

            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_guard ON fulfillments;
            DROP FUNCTION IF EXISTS enforce_fulfillment_checkout_identity();
            DROP INDEX IF EXISTS ux_fulfillments_checkout_identity;
            ALTER TABLE fulfillments
                DROP CONSTRAINT IF EXISTS ck_fulfillments_checkout_identity;

            CREATE UNIQUE INDEX ux_fulfillments_checkout_identity
                ON fulfillments (tenant_id, checkout_operation_id, checkout_fulfillment_index);

            UPDATE fulfillments
            SET metadata = metadata
                #- '{checkout,operation_id}'
                #- '{checkout,order_id}'
                #- '{checkout,order_plan_hash}'
                #- '{checkout,fulfillment_index}'
                #- '{checkout,fulfillment_key}'
            WHERE metadata #>> '{checkout,fulfillment_key}' IS NOT NULL;

            UPDATE fulfillment_items
            SET metadata = metadata
                #- '{checkout,operation_id}'
                #- '{checkout,order_plan_hash}'
                #- '{checkout,fulfillment_index}'
            WHERE metadata #>> '{checkout,operation_id}' IS NOT NULL;

            ALTER TABLE fulfillments
                DROP CONSTRAINT ck_fulfillments_checkout_identity_migration;

            ALTER TABLE fulfillments
                ADD CONSTRAINT ck_fulfillments_checkout_identity_typed
                CHECK (
                    (
                        checkout_operation_id IS NULL
                        AND checkout_fulfillment_index IS NULL
                        AND checkout_plan_hash IS NULL
                    )
                    OR (
                        checkout_operation_id IS NOT NULL
                        AND checkout_operation_id <> '00000000-0000-0000-0000-000000000000'::uuid
                        AND checkout_fulfillment_index IS NOT NULL
                        AND checkout_fulfillment_index >= 0
                        AND checkout_fulfillment_index <= 4294967295
                        AND checkout_plan_hash IS NOT NULL
                        AND checkout_plan_hash ~* '^[0-9a-f]{64}$'
                    )
                );

            CREATE OR REPLACE FUNCTION enforce_fulfillment_checkout_identity_typed()
            RETURNS trigger AS $$
            BEGIN
                IF NEW.checkout_operation_id IS DISTINCT FROM OLD.checkout_operation_id
                    OR NEW.checkout_fulfillment_index IS DISTINCT FROM OLD.checkout_fulfillment_index
                    OR NEW.checkout_plan_hash IS DISTINCT FROM OLD.checkout_plan_hash
                THEN
                    RAISE EXCEPTION 'fulfillment checkout identity is immutable'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER fulfillments_checkout_identity_typed_guard
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            EXECUTE FUNCTION enforce_fulfillment_checkout_identity_typed();
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
            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_typed_guard ON fulfillments;
            DROP FUNCTION IF EXISTS enforce_fulfillment_checkout_identity_typed();
            DROP INDEX IF EXISTS ux_fulfillments_checkout_identity;

            UPDATE fulfillments
            SET metadata = jsonb_set(
                metadata,
                '{checkout}',
                COALESCE(metadata->'checkout', '{}'::jsonb) || jsonb_build_object(
                    'operation_id', checkout_operation_id::text,
                    'order_id', order_id::text,
                    'order_plan_hash', checkout_plan_hash,
                    'fulfillment_index', checkout_fulfillment_index,
                    'fulfillment_key', format(
                        'checkout:%s:fulfillment:%s',
                        checkout_operation_id,
                        checkout_fulfillment_index
                    )
                ),
                true
            )
            WHERE checkout_operation_id IS NOT NULL
              AND checkout_fulfillment_index IS NOT NULL
              AND checkout_plan_hash IS NOT NULL;

            ALTER TABLE fulfillments
                DROP CONSTRAINT IF EXISTS ck_fulfillments_checkout_identity_typed;
            ALTER TABLE fulfillments
                DROP COLUMN checkout_plan_hash,
                DROP COLUMN checkout_fulfillment_index,
                DROP COLUMN checkout_operation_id;

            ALTER TABLE fulfillments
                ADD CONSTRAINT ck_fulfillments_checkout_identity
                CHECK (
                    metadata #>> '{checkout,fulfillment_key}' IS NULL
                    OR (
                        btrim(metadata #>> '{checkout,fulfillment_key}') <> ''
                        AND btrim(COALESCE(metadata #>> '{checkout,operation_id}', '')) <> ''
                    )
                );

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
            ALTER TABLE fulfillments ADD COLUMN checkout_operation_id TEXT;
            ALTER TABLE fulfillments ADD COLUMN checkout_fulfillment_index INTEGER;
            ALTER TABLE fulfillments ADD COLUMN checkout_plan_hash TEXT;

            UPDATE fulfillments
            SET
                checkout_operation_id = CASE
                    WHEN length(trim(json_extract(metadata, '$.checkout.operation_id'))) = 36
                        AND lower(trim(json_extract(metadata, '$.checkout.operation_id')))
                            NOT GLOB '*[^0-9a-f-]*'
                        AND substr(trim(json_extract(metadata, '$.checkout.operation_id')), 9, 1) = '-'
                        AND substr(trim(json_extract(metadata, '$.checkout.operation_id')), 14, 1) = '-'
                        AND substr(trim(json_extract(metadata, '$.checkout.operation_id')), 19, 1) = '-'
                        AND substr(trim(json_extract(metadata, '$.checkout.operation_id')), 24, 1) = '-'
                        AND lower(trim(json_extract(metadata, '$.checkout.operation_id')))
                            <> '00000000-0000-0000-0000-000000000000'
                    THEN lower(trim(json_extract(metadata, '$.checkout.operation_id')))
                    ELSE NULL
                END,
                checkout_fulfillment_index = CASE
                    WHEN trim(json_extract(metadata, '$.checkout.fulfillment_index'))
                        NOT GLOB '*[^0-9]*'
                        AND length(trim(json_extract(metadata, '$.checkout.fulfillment_index'))) > 0
                    THEN CAST(trim(json_extract(metadata, '$.checkout.fulfillment_index')) AS INTEGER)
                    ELSE NULL
                END,
                checkout_plan_hash = CASE
                    WHEN length(trim(json_extract(metadata, '$.checkout.order_plan_hash'))) = 64
                        AND lower(trim(json_extract(metadata, '$.checkout.order_plan_hash')))
                            NOT GLOB '*[^0-9a-f]*'
                    THEN trim(json_extract(metadata, '$.checkout.order_plan_hash'))
                    ELSE NULL
                END
            WHERE json_extract(metadata, '$.checkout.fulfillment_key') IS NOT NULL;

            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_guard_insert;
            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_guard_update;
            DROP INDEX IF EXISTS ux_fulfillments_checkout_identity;

            CREATE TRIGGER fulfillments_checkout_identity_typed_validation
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            WHEN json_extract(OLD.metadata, '$.checkout.fulfillment_key') IS NOT NULL
            BEGIN
                SELECT CASE WHEN
                    NEW.checkout_operation_id IS NULL
                    OR NEW.checkout_fulfillment_index IS NULL
                    OR NEW.checkout_fulfillment_index < 0
                    OR NEW.checkout_fulfillment_index > 4294967295
                    OR NEW.checkout_plan_hash IS NULL
                    OR length(NEW.checkout_plan_hash) <> 64
                    OR lower(NEW.checkout_plan_hash) GLOB '*[^0-9a-f]*'
                    OR lower(NEW.checkout_operation_id) <> lower(trim(json_extract(OLD.metadata, '$.checkout.operation_id')))
                    OR lower(trim(json_extract(OLD.metadata, '$.checkout.order_id'))) <> lower(NEW.order_id)
                    OR NEW.checkout_plan_hash <> trim(json_extract(OLD.metadata, '$.checkout.order_plan_hash'))
                    OR NEW.checkout_fulfillment_index <> CAST(json_extract(OLD.metadata, '$.checkout.fulfillment_index') AS INTEGER)
                    OR json_extract(OLD.metadata, '$.checkout.fulfillment_key')
                        <> 'checkout:' || lower(NEW.checkout_operation_id) || ':fulfillment:' || NEW.checkout_fulfillment_index
                    THEN RAISE(ABORT, 'invalid legacy checkout fulfillment identity') END;
            END;

            UPDATE fulfillments
            SET checkout_operation_id = checkout_operation_id,
                checkout_fulfillment_index = checkout_fulfillment_index,
                checkout_plan_hash = checkout_plan_hash;

            DROP TRIGGER fulfillments_checkout_identity_typed_validation;

            UPDATE fulfillments
            SET metadata = json_remove(
                metadata,
                '$.checkout.operation_id',
                '$.checkout.order_id',
                '$.checkout.order_plan_hash',
                '$.checkout.fulfillment_index',
                '$.checkout.fulfillment_key'
            )
            WHERE json_extract(metadata, '$.checkout.fulfillment_key') IS NOT NULL;

            UPDATE fulfillment_items
            SET metadata = json_remove(
                metadata,
                '$.checkout.operation_id',
                '$.checkout.order_plan_hash',
                '$.checkout.fulfillment_index'
            )
            WHERE json_extract(metadata, '$.checkout.operation_id') IS NOT NULL;

            CREATE TRIGGER fulfillments_checkout_identity_typed_insert
            BEFORE INSERT ON fulfillments
            FOR EACH ROW
            WHEN NOT (
                (NEW.checkout_operation_id IS NULL
                    AND NEW.checkout_fulfillment_index IS NULL
                    AND NEW.checkout_plan_hash IS NULL)
                OR (
                    NEW.checkout_operation_id IS NOT NULL
                    AND lower(NEW.checkout_operation_id)
                        <> '00000000-0000-0000-0000-000000000000'
                    AND NEW.checkout_fulfillment_index IS NOT NULL
                    AND NEW.checkout_fulfillment_index BETWEEN 0 AND 4294967295
                    AND NEW.checkout_plan_hash IS NOT NULL
                    AND length(NEW.checkout_plan_hash) = 64
                    AND lower(NEW.checkout_plan_hash) NOT GLOB '*[^0-9a-f]*'
                )
            )
            BEGIN
                SELECT RAISE(ABORT, 'invalid checkout fulfillment identity');
            END;

            CREATE TRIGGER fulfillments_checkout_identity_typed_update
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NOT (
                    NEW.checkout_operation_id IS OLD.checkout_operation_id
                    AND NEW.checkout_fulfillment_index IS OLD.checkout_fulfillment_index
                    AND NEW.checkout_plan_hash IS OLD.checkout_plan_hash
                ) THEN RAISE(ABORT, 'fulfillment checkout identity is immutable') END;
            END;

            CREATE UNIQUE INDEX ux_fulfillments_checkout_identity
                ON fulfillments (tenant_id, checkout_operation_id, checkout_fulfillment_index);
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
            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_typed_update;
            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_typed_insert;
            DROP INDEX IF EXISTS ux_fulfillments_checkout_identity;

            UPDATE fulfillments
            SET metadata = json_set(
                COALESCE(metadata, '{}'),
                '$.checkout.operation_id', checkout_operation_id,
                '$.checkout.order_id', order_id,
                '$.checkout.order_plan_hash', checkout_plan_hash,
                '$.checkout.fulfillment_index', checkout_fulfillment_index,
                '$.checkout.fulfillment_key',
                    'checkout:' || checkout_operation_id || ':fulfillment:' || checkout_fulfillment_index
            )
            WHERE checkout_operation_id IS NOT NULL
              AND checkout_fulfillment_index IS NOT NULL
              AND checkout_plan_hash IS NOT NULL;

            ALTER TABLE fulfillments DROP COLUMN checkout_plan_hash;
            ALTER TABLE fulfillments DROP COLUMN checkout_fulfillment_index;
            ALTER TABLE fulfillments DROP COLUMN checkout_operation_id;

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
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN json_extract(OLD.metadata, '$.checkout.fulfillment_key')
                    IS NOT json_extract(NEW.metadata, '$.checkout.fulfillment_key')
                    THEN RAISE(ABORT, 'fulfillment checkout identity is immutable') END;
            END;

            CREATE UNIQUE INDEX ux_fulfillments_checkout_identity
                ON fulfillments (
                    tenant_id,
                    json_extract(metadata, '$.checkout.fulfillment_key')
                )
                WHERE json_extract(metadata, '$.checkout.fulfillment_key') IS NOT NULL;
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
                ADD COLUMN checkout_operation_id CHAR(36),
                ADD COLUMN checkout_fulfillment_index BIGINT,
                ADD COLUMN checkout_plan_hash VARCHAR(64);

            UPDATE fulfillments
            SET
                checkout_operation_id = CASE
                    WHEN JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.operation_id'))
                        REGEXP '^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$'
                     AND lower(JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.operation_id')))
                        <> '00000000-0000-0000-0000-000000000000'
                    THEN lower(JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.operation_id')))
                    ELSE NULL
                END,
                checkout_fulfillment_index = CASE
                    WHEN JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.fulfillment_index')) REGEXP '^[0-9]+$'
                    THEN CAST(JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.fulfillment_index')) AS UNSIGNED)
                    ELSE NULL
                END,
                checkout_plan_hash = CASE
                    WHEN JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.order_plan_hash'))
                        REGEXP '^[0-9a-fA-F]{64}$'
                    THEN JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.order_plan_hash'))
                    ELSE NULL
                END
            WHERE JSON_EXTRACT(metadata, '$.checkout.fulfillment_key') IS NOT NULL;

            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_guard_update;
            DROP INDEX ux_fulfillments_checkout_identity ON fulfillments;
            ALTER TABLE fulfillments DROP COLUMN checkout_fulfillment_identity;

            CREATE TRIGGER fulfillments_checkout_identity_typed_validation
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            BEGIN
                IF JSON_EXTRACT(OLD.metadata, '$.checkout.fulfillment_key') IS NOT NULL
                    AND (
                        NEW.checkout_operation_id IS NULL
                        OR NEW.checkout_fulfillment_index IS NULL
                        OR NEW.checkout_fulfillment_index < 0
                        OR NEW.checkout_fulfillment_index > 4294967295
                        OR NEW.checkout_plan_hash IS NULL
                        OR NEW.checkout_plan_hash NOT REGEXP '^[0-9a-fA-F]{64}$'
                        OR lower(NEW.checkout_operation_id)
                            <> lower(JSON_UNQUOTE(JSON_EXTRACT(OLD.metadata, '$.checkout.operation_id')))
                        OR lower(JSON_UNQUOTE(JSON_EXTRACT(OLD.metadata, '$.checkout.order_id')))
                            <> lower(NEW.order_id)
                        OR NEW.checkout_plan_hash
                            <> JSON_UNQUOTE(JSON_EXTRACT(OLD.metadata, '$.checkout.order_plan_hash'))
                        OR NEW.checkout_fulfillment_index
                            <> CAST(JSON_UNQUOTE(JSON_EXTRACT(OLD.metadata, '$.checkout.fulfillment_index')) AS UNSIGNED)
                        OR JSON_UNQUOTE(JSON_EXTRACT(OLD.metadata, '$.checkout.fulfillment_key'))
                            <> CONCAT('checkout:', lower(NEW.checkout_operation_id), ':fulfillment:', NEW.checkout_fulfillment_index)
                    )
                THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'invalid legacy checkout fulfillment identity';
                END IF;
            END;
            "#,
        )
        .await?;
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            UPDATE fulfillments
            SET checkout_operation_id = checkout_operation_id,
                checkout_fulfillment_index = checkout_fulfillment_index,
                checkout_plan_hash = checkout_plan_hash;
            "#,
        )
        .await?;
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER fulfillments_checkout_identity_typed_validation;

            UPDATE fulfillments
            SET metadata = JSON_REMOVE(
                metadata,
                '$.checkout.operation_id',
                '$.checkout.order_id',
                '$.checkout.order_plan_hash',
                '$.checkout.fulfillment_index',
                '$.checkout.fulfillment_key'
            )
            WHERE JSON_EXTRACT(metadata, '$.checkout.fulfillment_key') IS NOT NULL;

            UPDATE fulfillment_items
            SET metadata = JSON_REMOVE(
                metadata,
                '$.checkout.operation_id',
                '$.checkout.order_plan_hash',
                '$.checkout.fulfillment_index'
            )
            WHERE JSON_EXTRACT(metadata, '$.checkout.operation_id') IS NOT NULL;

            CREATE TRIGGER fulfillments_checkout_identity_typed_insert
            BEFORE INSERT ON fulfillments
            FOR EACH ROW
            BEGIN
                IF NOT (
                    (NEW.checkout_operation_id IS NULL
                        AND NEW.checkout_fulfillment_index IS NULL
                        AND NEW.checkout_plan_hash IS NULL)
                    OR (
                        NEW.checkout_operation_id IS NOT NULL
                        AND lower(NEW.checkout_operation_id)
                            <> '00000000-0000-0000-0000-000000000000'
                        AND NEW.checkout_fulfillment_index IS NOT NULL
                        AND NEW.checkout_fulfillment_index BETWEEN 0 AND 4294967295
                        AND NEW.checkout_plan_hash IS NOT NULL
                        AND NEW.checkout_plan_hash REGEXP '^[0-9a-fA-F]{64}$'
                    )
                ) THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'invalid checkout fulfillment identity';
                END IF;
            END;

            CREATE TRIGGER fulfillments_checkout_identity_typed_update
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            BEGIN
                IF NOT (NEW.checkout_operation_id <=> OLD.checkout_operation_id)
                    OR NOT (NEW.checkout_fulfillment_index <=> OLD.checkout_fulfillment_index)
                    OR NOT (NEW.checkout_plan_hash <=> OLD.checkout_plan_hash)
                THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'fulfillment checkout identity is immutable';
                END IF;
            END;

            CREATE UNIQUE INDEX ux_fulfillments_checkout_identity
                ON fulfillments (tenant_id, checkout_operation_id, checkout_fulfillment_index);
            "#,
        )
        .await?;
    Ok(())
}

async fn restore_mysql(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_typed_update;
            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_typed_insert;
            DROP INDEX ux_fulfillments_checkout_identity ON fulfillments;

            UPDATE fulfillments
            SET metadata = JSON_SET(
                COALESCE(metadata, JSON_OBJECT()),
                '$.checkout.operation_id', checkout_operation_id,
                '$.checkout.order_id', order_id,
                '$.checkout.order_plan_hash', checkout_plan_hash,
                '$.checkout.fulfillment_index', checkout_fulfillment_index,
                '$.checkout.fulfillment_key',
                    CONCAT('checkout:', checkout_operation_id, ':fulfillment:', checkout_fulfillment_index)
            )
            WHERE checkout_operation_id IS NOT NULL
              AND checkout_fulfillment_index IS NOT NULL
              AND checkout_plan_hash IS NOT NULL;

            ALTER TABLE fulfillments
                ADD COLUMN checkout_fulfillment_identity VARCHAR(191)
                    GENERATED ALWAYS AS (
                        JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.fulfillment_key'))
                    ) STORED;

            CREATE UNIQUE INDEX ux_fulfillments_checkout_identity
                ON fulfillments (tenant_id, checkout_fulfillment_identity);

            CREATE TRIGGER fulfillments_checkout_identity_guard_update
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            BEGIN
                IF NOT (
                    OLD.checkout_fulfillment_identity <=> NEW.checkout_fulfillment_identity
                ) THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'fulfillment checkout identity is immutable';
                END IF;
            END;

            ALTER TABLE fulfillments
                DROP COLUMN checkout_plan_hash,
                DROP COLUMN checkout_fulfillment_index,
                DROP COLUMN checkout_operation_id;
            "#,
        )
        .await?;
    Ok(())
}

                                AND length(btrim(metadata #>> '{checkout,fulfillment_index}')) <= 10
                            THEN (btrim(metadata #>> '{checkout,fulfillment_index}'))::bigint = checkout_fulfillment_index
                            ELSE FALSE
                        END
                        AND metadata #>> '{checkout,fulfillment_key}' = format(
                            'checkout:%s:fulfillment:%s',
                            checkout_operation_id,
                            checkout_fulfillment_index
                        )
                    )
                ) NOT VALID;
            ALTER TABLE fulfillments
                VALIDATE CONSTRAINT ck_fulfillments_checkout_identity_migration;

            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_guard ON fulfillments;
            DROP FUNCTION IF EXISTS enforce_fulfillment_checkout_identity();
            DROP INDEX IF EXISTS ux_fulfillments_checkout_identity;
            ALTER TABLE fulfillments
                DROP CONSTRAINT IF EXISTS ck_fulfillments_checkout_identity;

            CREATE UNIQUE INDEX ux_fulfillments_checkout_identity
                ON fulfillments (tenant_id, checkout_operation_id, checkout_fulfillment_index);

            UPDATE fulfillments
            SET metadata = metadata
                #- '{checkout,operation_id}'
                #- '{checkout,order_id}'
                #- '{checkout,order_plan_hash}'
                #- '{checkout,fulfillment_index}'
                #- '{checkout,fulfillment_key}'
            WHERE metadata #>> '{checkout,fulfillment_key}' IS NOT NULL;

            UPDATE fulfillment_items
            SET metadata = metadata
                #- '{checkout,operation_id}'
                #- '{checkout,order_plan_hash}'
                #- '{checkout,fulfillment_index}'
            WHERE metadata #>> '{checkout,operation_id}' IS NOT NULL;

            ALTER TABLE fulfillments
                DROP CONSTRAINT ck_fulfillments_checkout_identity_migration;

            ALTER TABLE fulfillments
                ADD CONSTRAINT ck_fulfillments_checkout_identity_typed
                CHECK (
                    (
                        checkout_operation_id IS NULL
                        AND checkout_fulfillment_index IS NULL
                        AND checkout_plan_hash IS NULL
                    )
                    OR (
                        checkout_operation_id IS NOT NULL
                        AND checkout_operation_id <> '00000000-0000-0000-0000-000000000000'::uuid
                        AND checkout_fulfillment_index IS NOT NULL
                        AND checkout_fulfillment_index >= 0
                        AND checkout_fulfillment_index <= 4294967295
                        AND checkout_plan_hash IS NOT NULL
                        AND checkout_plan_hash ~* '^[0-9a-f]{64}$'
                    )
                );

            CREATE OR REPLACE FUNCTION enforce_fulfillment_checkout_identity_typed()
            RETURNS trigger AS $$
            BEGIN
                IF NEW.checkout_operation_id IS DISTINCT FROM OLD.checkout_operation_id
                    OR NEW.checkout_fulfillment_index IS DISTINCT FROM OLD.checkout_fulfillment_index
                    OR NEW.checkout_plan_hash IS DISTINCT FROM OLD.checkout_plan_hash
                THEN
                    RAISE EXCEPTION 'fulfillment checkout identity is immutable'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER fulfillments_checkout_identity_typed_guard
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            EXECUTE FUNCTION enforce_fulfillment_checkout_identity_typed();
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
            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_typed_guard ON fulfillments;
            DROP FUNCTION IF EXISTS enforce_fulfillment_checkout_identity_typed();
            DROP INDEX IF EXISTS ux_fulfillments_checkout_identity;

            UPDATE fulfillments
            SET metadata = jsonb_set(
                metadata,
                '{checkout}',
                COALESCE(metadata->'checkout', '{}'::jsonb) || jsonb_build_object(
                    'operation_id', checkout_operation_id::text,
                    'order_id', order_id::text,
                    'order_plan_hash', checkout_plan_hash,
                    'fulfillment_index', checkout_fulfillment_index,
                    'fulfillment_key', format(
                        'checkout:%s:fulfillment:%s',
                        checkout_operation_id,
                        checkout_fulfillment_index
                    )
                ),
                true
            )
            WHERE checkout_operation_id IS NOT NULL
              AND checkout_fulfillment_index IS NOT NULL
              AND checkout_plan_hash IS NOT NULL;

            ALTER TABLE fulfillments
                DROP CONSTRAINT IF EXISTS ck_fulfillments_checkout_identity_typed;
            ALTER TABLE fulfillments
                DROP COLUMN checkout_plan_hash,
                DROP COLUMN checkout_fulfillment_index,
                DROP COLUMN checkout_operation_id;

            ALTER TABLE fulfillments
                ADD CONSTRAINT ck_fulfillments_checkout_identity
                CHECK (
                    metadata #>> '{checkout,fulfillment_key}' IS NULL
                    OR (
                        btrim(metadata #>> '{checkout,fulfillment_key}') <> ''
                        AND btrim(COALESCE(metadata #>> '{checkout,operation_id}', '')) <> ''
                    )
                );

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
            ALTER TABLE fulfillments ADD COLUMN checkout_operation_id TEXT;
            ALTER TABLE fulfillments ADD COLUMN checkout_fulfillment_index INTEGER;
            ALTER TABLE fulfillments ADD COLUMN checkout_plan_hash TEXT;

            UPDATE fulfillments
            SET
                checkout_operation_id = CASE
                    WHEN length(trim(json_extract(metadata, '$.checkout.operation_id'))) = 36
                        AND lower(trim(json_extract(metadata, '$.checkout.operation_id')))
                            NOT GLOB '*[^0-9a-f-]*'
                        AND substr(trim(json_extract(metadata, '$.checkout.operation_id')), 9, 1) = '-'
                        AND substr(trim(json_extract(metadata, '$.checkout.operation_id')), 14, 1) = '-'
                        AND substr(trim(json_extract(metadata, '$.checkout.operation_id')), 19, 1) = '-'
                        AND substr(trim(json_extract(metadata, '$.checkout.operation_id')), 24, 1) = '-'
                        AND lower(trim(json_extract(metadata, '$.checkout.operation_id')))
                            <> '00000000-0000-0000-0000-000000000000'
                    THEN lower(trim(json_extract(metadata, '$.checkout.operation_id')))
                    ELSE NULL
                END,
                checkout_fulfillment_index = CASE
                    WHEN trim(json_extract(metadata, '$.checkout.fulfillment_index'))
                        NOT GLOB '*[^0-9]*'
                        AND length(trim(json_extract(metadata, '$.checkout.fulfillment_index'))) > 0
                    THEN CAST(trim(json_extract(metadata, '$.checkout.fulfillment_index')) AS INTEGER)
                    ELSE NULL
                END,
                checkout_plan_hash = CASE
                    WHEN length(trim(json_extract(metadata, '$.checkout.order_plan_hash'))) = 64
                        AND lower(trim(json_extract(metadata, '$.checkout.order_plan_hash')))
                            NOT GLOB '*[^0-9a-f]*'
                    THEN trim(json_extract(metadata, '$.checkout.order_plan_hash'))
                    ELSE NULL
                END
            WHERE json_extract(metadata, '$.checkout.fulfillment_key') IS NOT NULL;

            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_guard_insert;
            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_guard_update;
            DROP INDEX IF EXISTS ux_fulfillments_checkout_identity;

            CREATE TRIGGER fulfillments_checkout_identity_typed_validation
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            WHEN json_extract(OLD.metadata, '$.checkout.fulfillment_key') IS NOT NULL
            BEGIN
                SELECT CASE WHEN
                    NEW.checkout_operation_id IS NULL
                    OR NEW.checkout_fulfillment_index IS NULL
                    OR NEW.checkout_fulfillment_index < 0
                    OR NEW.checkout_fulfillment_index > 4294967295
                    OR NEW.checkout_plan_hash IS NULL
                    OR length(NEW.checkout_plan_hash) <> 64
                    OR lower(NEW.checkout_plan_hash) GLOB '*[^0-9a-f]*'
                    OR lower(NEW.checkout_operation_id) <> lower(trim(json_extract(OLD.metadata, '$.checkout.operation_id')))
                    OR lower(trim(json_extract(OLD.metadata, '$.checkout.order_id'))) <> lower(NEW.order_id)
                    OR NEW.checkout_plan_hash <> trim(json_extract(OLD.metadata, '$.checkout.order_plan_hash'))
                    OR NEW.checkout_fulfillment_index <> CAST(json_extract(OLD.metadata, '$.checkout.fulfillment_index') AS INTEGER)
                    OR json_extract(OLD.metadata, '$.checkout.fulfillment_key')
                        <> 'checkout:' || lower(NEW.checkout_operation_id) || ':fulfillment:' || NEW.checkout_fulfillment_index
                    THEN RAISE(ABORT, 'invalid legacy checkout fulfillment identity') END;
            END;

            UPDATE fulfillments
            SET checkout_operation_id = checkout_operation_id,
                checkout_fulfillment_index = checkout_fulfillment_index,
                checkout_plan_hash = checkout_plan_hash;

            DROP TRIGGER fulfillments_checkout_identity_typed_validation;

            UPDATE fulfillments
            SET metadata = json_remove(
                metadata,
                '$.checkout.operation_id',
                '$.checkout.order_id',
                '$.checkout.order_plan_hash',
                '$.checkout.fulfillment_index',
                '$.checkout.fulfillment_key'
            )
            WHERE json_extract(metadata, '$.checkout.fulfillment_key') IS NOT NULL;

            UPDATE fulfillment_items
            SET metadata = json_remove(
                metadata,
                '$.checkout.operation_id',
                '$.checkout.order_plan_hash',
                '$.checkout.fulfillment_index'
            )
            WHERE json_extract(metadata, '$.checkout.operation_id') IS NOT NULL;

            CREATE TRIGGER fulfillments_checkout_identity_typed_insert
            BEFORE INSERT ON fulfillments
            FOR EACH ROW
            WHEN NOT (
                (NEW.checkout_operation_id IS NULL
                    AND NEW.checkout_fulfillment_index IS NULL
                    AND NEW.checkout_plan_hash IS NULL)
                OR (
                    NEW.checkout_operation_id IS NOT NULL
                    AND lower(NEW.checkout_operation_id)
                        <> '00000000-0000-0000-0000-000000000000'
                    AND NEW.checkout_fulfillment_index IS NOT NULL
                    AND NEW.checkout_fulfillment_index BETWEEN 0 AND 4294967295
                    AND NEW.checkout_plan_hash IS NOT NULL
                    AND length(NEW.checkout_plan_hash) = 64
                    AND lower(NEW.checkout_plan_hash) NOT GLOB '*[^0-9a-f]*'
                )
            )
            BEGIN
                SELECT RAISE(ABORT, 'invalid checkout fulfillment identity');
            END;

            CREATE TRIGGER fulfillments_checkout_identity_typed_update
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NOT (
                    NEW.checkout_operation_id IS OLD.checkout_operation_id
                    AND NEW.checkout_fulfillment_index IS OLD.checkout_fulfillment_index
                    AND NEW.checkout_plan_hash IS OLD.checkout_plan_hash
                ) THEN RAISE(ABORT, 'fulfillment checkout identity is immutable') END;
            END;

            CREATE UNIQUE INDEX ux_fulfillments_checkout_identity
                ON fulfillments (tenant_id, checkout_operation_id, checkout_fulfillment_index);
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
            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_typed_update;
            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_typed_insert;
            DROP INDEX IF EXISTS ux_fulfillments_checkout_identity;

            UPDATE fulfillments
            SET metadata = json_set(
                COALESCE(metadata, '{}'),
                '$.checkout.operation_id', checkout_operation_id,
                '$.checkout.order_id', order_id,
                '$.checkout.order_plan_hash', checkout_plan_hash,
                '$.checkout.fulfillment_index', checkout_fulfillment_index,
                '$.checkout.fulfillment_key',
                    'checkout:' || checkout_operation_id || ':fulfillment:' || checkout_fulfillment_index
            )
            WHERE checkout_operation_id IS NOT NULL
              AND checkout_fulfillment_index IS NOT NULL
              AND checkout_plan_hash IS NOT NULL;

            ALTER TABLE fulfillments DROP COLUMN checkout_plan_hash;
            ALTER TABLE fulfillments DROP COLUMN checkout_fulfillment_index;
            ALTER TABLE fulfillments DROP COLUMN checkout_operation_id;

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
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN json_extract(OLD.metadata, '$.checkout.fulfillment_key')
                    IS NOT json_extract(NEW.metadata, '$.checkout.fulfillment_key')
                    THEN RAISE(ABORT, 'fulfillment checkout identity is immutable') END;
            END;

            CREATE UNIQUE INDEX ux_fulfillments_checkout_identity
                ON fulfillments (
                    tenant_id,
                    json_extract(metadata, '$.checkout.fulfillment_key')
                )
                WHERE json_extract(metadata, '$.checkout.fulfillment_key') IS NOT NULL;
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
                ADD COLUMN checkout_operation_id CHAR(36),
                ADD COLUMN checkout_fulfillment_index BIGINT,
                ADD COLUMN checkout_plan_hash VARCHAR(64);

            UPDATE fulfillments
            SET
                checkout_operation_id = CASE
                    WHEN JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.operation_id'))
                        REGEXP '^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$'
                     AND lower(JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.operation_id')))
                        <> '00000000-0000-0000-0000-000000000000'
                    THEN lower(JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.operation_id')))
                    ELSE NULL
                END,
                checkout_fulfillment_index = CASE
                    WHEN JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.fulfillment_index')) REGEXP '^[0-9]+$'
                    THEN CAST(JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.fulfillment_index')) AS UNSIGNED)
                    ELSE NULL
                END,
                checkout_plan_hash = CASE
                    WHEN JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.order_plan_hash'))
                        REGEXP '^[0-9a-fA-F]{64}$'
                    THEN JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.order_plan_hash'))
                    ELSE NULL
                END
            WHERE JSON_EXTRACT(metadata, '$.checkout.fulfillment_key') IS NOT NULL;

            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_guard_update;
            DROP INDEX ux_fulfillments_checkout_identity ON fulfillments;
            ALTER TABLE fulfillments DROP COLUMN checkout_fulfillment_identity;

            CREATE TRIGGER fulfillments_checkout_identity_typed_validation
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            BEGIN
                IF JSON_EXTRACT(OLD.metadata, '$.checkout.fulfillment_key') IS NOT NULL
                    AND (
                        NEW.checkout_operation_id IS NULL
                        OR NEW.checkout_fulfillment_index IS NULL
                        OR NEW.checkout_fulfillment_index < 0
                        OR NEW.checkout_fulfillment_index > 4294967295
                        OR NEW.checkout_plan_hash IS NULL
                        OR NEW.checkout_plan_hash NOT REGEXP '^[0-9a-fA-F]{64}$'
                        OR lower(NEW.checkout_operation_id)
                            <> lower(JSON_UNQUOTE(JSON_EXTRACT(OLD.metadata, '$.checkout.operation_id')))
                        OR lower(JSON_UNQUOTE(JSON_EXTRACT(OLD.metadata, '$.checkout.order_id')))
                            <> lower(NEW.order_id)
                        OR NEW.checkout_plan_hash
                            <> JSON_UNQUOTE(JSON_EXTRACT(OLD.metadata, '$.checkout.order_plan_hash'))
                        OR NEW.checkout_fulfillment_index
                            <> CAST(JSON_UNQUOTE(JSON_EXTRACT(OLD.metadata, '$.checkout.fulfillment_index')) AS UNSIGNED)
                        OR JSON_UNQUOTE(JSON_EXTRACT(OLD.metadata, '$.checkout.fulfillment_key'))
                            <> CONCAT('checkout:', lower(NEW.checkout_operation_id), ':fulfillment:', NEW.checkout_fulfillment_index)
                    )
                THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'invalid legacy checkout fulfillment identity';
                END IF;
            END;
            "#,
        )
        .await?;
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            UPDATE fulfillments
            SET checkout_operation_id = checkout_operation_id,
                checkout_fulfillment_index = checkout_fulfillment_index,
                checkout_plan_hash = checkout_plan_hash;
            "#,
        )
        .await?;
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER fulfillments_checkout_identity_typed_validation;

            UPDATE fulfillments
            SET metadata = JSON_REMOVE(
                metadata,
                '$.checkout.operation_id',
                '$.checkout.order_id',
                '$.checkout.order_plan_hash',
                '$.checkout.fulfillment_index',
                '$.checkout.fulfillment_key'
            )
            WHERE JSON_EXTRACT(metadata, '$.checkout.fulfillment_key') IS NOT NULL;

            UPDATE fulfillment_items
            SET metadata = JSON_REMOVE(
                metadata,
                '$.checkout.operation_id',
                '$.checkout.order_plan_hash',
                '$.checkout.fulfillment_index'
            )
            WHERE JSON_EXTRACT(metadata, '$.checkout.operation_id') IS NOT NULL;

            CREATE TRIGGER fulfillments_checkout_identity_typed_insert
            BEFORE INSERT ON fulfillments
            FOR EACH ROW
            BEGIN
                IF NOT (
                    (NEW.checkout_operation_id IS NULL
                        AND NEW.checkout_fulfillment_index IS NULL
                        AND NEW.checkout_plan_hash IS NULL)
                    OR (
                        NEW.checkout_operation_id IS NOT NULL
                        AND lower(NEW.checkout_operation_id)
                            <> '00000000-0000-0000-0000-000000000000'
                        AND NEW.checkout_fulfillment_index IS NOT NULL
                        AND NEW.checkout_fulfillment_index BETWEEN 0 AND 4294967295
                        AND NEW.checkout_plan_hash IS NOT NULL
                        AND NEW.checkout_plan_hash REGEXP '^[0-9a-fA-F]{64}$'
                    )
                ) THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'invalid checkout fulfillment identity';
                END IF;
            END;

            CREATE TRIGGER fulfillments_checkout_identity_typed_update
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            BEGIN
                IF NOT (NEW.checkout_operation_id <=> OLD.checkout_operation_id)
                    OR NOT (NEW.checkout_fulfillment_index <=> OLD.checkout_fulfillment_index)
                    OR NOT (NEW.checkout_plan_hash <=> OLD.checkout_plan_hash)
                THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'fulfillment checkout identity is immutable';
                END IF;
            END;

            CREATE UNIQUE INDEX ux_fulfillments_checkout_identity
                ON fulfillments (tenant_id, checkout_operation_id, checkout_fulfillment_index);
            "#,
        )
        .await?;
    Ok(())
}

async fn restore_mysql(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_typed_update;
            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_typed_insert;
            DROP INDEX ux_fulfillments_checkout_identity ON fulfillments;

            UPDATE fulfillments
            SET metadata = JSON_SET(
                COALESCE(metadata, JSON_OBJECT()),
                '$.checkout.operation_id', checkout_operation_id,
                '$.checkout.order_id', order_id,
                '$.checkout.order_plan_hash', checkout_plan_hash,
                '$.checkout.fulfillment_index', checkout_fulfillment_index,
                '$.checkout.fulfillment_key',
                    CONCAT('checkout:', checkout_operation_id, ':fulfillment:', checkout_fulfillment_index)
            )
            WHERE checkout_operation_id IS NOT NULL
              AND checkout_fulfillment_index IS NOT NULL
              AND checkout_plan_hash IS NOT NULL;

            ALTER TABLE fulfillments
                ADD COLUMN checkout_fulfillment_identity VARCHAR(191)
                    GENERATED ALWAYS AS (
                        JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.fulfillment_key'))
                    ) STORED;

            CREATE UNIQUE INDEX ux_fulfillments_checkout_identity
                ON fulfillments (tenant_id, checkout_fulfillment_identity);

            CREATE TRIGGER fulfillments_checkout_identity_guard_update
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            BEGIN
                IF NOT (
                    OLD.checkout_fulfillment_identity <=> NEW.checkout_fulfillment_identity
                ) THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'fulfillment checkout identity is immutable';
                END IF;
            END;

            ALTER TABLE fulfillments
                DROP COLUMN checkout_plan_hash,
                DROP COLUMN checkout_fulfillment_index,
                DROP COLUMN checkout_operation_id;
            "#,
        )
        .await?;
    Ok(())
}

                        AND length(btrim(metadata #>> '{checkout,fulfillment_index}')) <= 10
                    THEN (btrim(metadata #>> '{checkout,fulfillment_index}'))::bigint
                    ELSE NULL
                END,
                checkout_plan_hash = CASE
                    WHEN btrim(metadata #>> '{checkout,order_plan_hash}')
                        ~* '^[0-9a-f]{64}$'
                    THEN btrim(metadata #>> '{checkout,order_plan_hash}')
                    ELSE NULL
                END
            WHERE metadata #>> '{checkout,fulfillment_key}' IS NOT NULL;

            ALTER TABLE fulfillments
                ADD CONSTRAINT ck_fulfillments_checkout_identity_migration
                CHECK (
                    metadata #>> '{checkout,fulfillment_key}' IS NULL
                    OR (
                        checkout_operation_id IS NOT NULL
                        AND checkout_operation_id <> '00000000-0000-0000-0000-000000000000'::uuid
                        AND checkout_fulfillment_index IS NOT NULL
                        AND checkout_fulfillment_index >= 0
                        AND checkout_fulfillment_index <= 4294967295
                        AND checkout_plan_hash IS NOT NULL
                        AND checkout_plan_hash ~* '^[0-9a-f]{64}$'
                        AND lower(btrim(metadata #>> '{checkout,operation_id}')) = checkout_operation_id::text
                        AND lower(btrim(metadata #>> '{checkout,order_id}')) = order_id::text
                        AND btrim(metadata #>> '{checkout,order_plan_hash}') = checkout_plan_hash
                        AND (metadata #>> '{checkout,fulfillment_index}')::bigint = checkout_fulfillment_index
                        AND metadata #>> '{checkout,fulfillment_key}' = format(
                            'checkout:%s:fulfillment:%s',
                            checkout_operation_id,
                            checkout_fulfillment_index
                        )
                    )
                ) NOT VALID;
            ALTER TABLE fulfillments
                VALIDATE CONSTRAINT ck_fulfillments_checkout_identity_migration;

            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_guard ON fulfillments;
            DROP FUNCTION IF EXISTS enforce_fulfillment_checkout_identity();
            DROP INDEX IF EXISTS ux_fulfillments_checkout_identity;
            ALTER TABLE fulfillments
                DROP CONSTRAINT IF EXISTS ck_fulfillments_checkout_identity;

            CREATE UNIQUE INDEX ux_fulfillments_checkout_identity
                ON fulfillments (tenant_id, checkout_operation_id, checkout_fulfillment_index);

            UPDATE fulfillments
            SET metadata = metadata
                #- '{checkout,operation_id}'
                #- '{checkout,order_id}'
                #- '{checkout,order_plan_hash}'
                #- '{checkout,fulfillment_index}'
                #- '{checkout,fulfillment_key}'
            WHERE metadata #>> '{checkout,fulfillment_key}' IS NOT NULL;

            UPDATE fulfillment_items
            SET metadata = metadata
                #- '{checkout,operation_id}'
                #- '{checkout,order_plan_hash}'
                #- '{checkout,fulfillment_index}'
            WHERE metadata #>> '{checkout,operation_id}' IS NOT NULL;

            ALTER TABLE fulfillments
                DROP CONSTRAINT ck_fulfillments_checkout_identity_migration;

            ALTER TABLE fulfillments
                ADD CONSTRAINT ck_fulfillments_checkout_identity_typed
                CHECK (
                    (
                        checkout_operation_id IS NULL
                        AND checkout_fulfillment_index IS NULL
                        AND checkout_plan_hash IS NULL
                    )
                    OR (
                        checkout_operation_id IS NOT NULL
                        AND checkout_operation_id <> '00000000-0000-0000-0000-000000000000'::uuid
                        AND checkout_fulfillment_index IS NOT NULL
                        AND checkout_fulfillment_index >= 0
                        AND checkout_fulfillment_index <= 4294967295
                        AND checkout_plan_hash IS NOT NULL
                        AND checkout_plan_hash ~* '^[0-9a-f]{64}$'
                    )
                );

            CREATE OR REPLACE FUNCTION enforce_fulfillment_checkout_identity_typed()
            RETURNS trigger AS $$
            BEGIN
                IF NEW.checkout_operation_id IS DISTINCT FROM OLD.checkout_operation_id
                    OR NEW.checkout_fulfillment_index IS DISTINCT FROM OLD.checkout_fulfillment_index
                    OR NEW.checkout_plan_hash IS DISTINCT FROM OLD.checkout_plan_hash
                THEN
                    RAISE EXCEPTION 'fulfillment checkout identity is immutable'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER fulfillments_checkout_identity_typed_guard
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            EXECUTE FUNCTION enforce_fulfillment_checkout_identity_typed();
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
            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_typed_guard ON fulfillments;
            DROP FUNCTION IF EXISTS enforce_fulfillment_checkout_identity_typed();
            DROP INDEX IF EXISTS ux_fulfillments_checkout_identity;

            UPDATE fulfillments
            SET metadata = jsonb_set(
                metadata,
                '{checkout}',
                COALESCE(metadata->'checkout', '{}'::jsonb) || jsonb_build_object(
                    'operation_id', checkout_operation_id::text,
                    'order_id', order_id::text,
                    'order_plan_hash', checkout_plan_hash,
                    'fulfillment_index', checkout_fulfillment_index,
                    'fulfillment_key', format(
                        'checkout:%s:fulfillment:%s',
                        checkout_operation_id,
                        checkout_fulfillment_index
                    )
                ),
                true
            )
            WHERE checkout_operation_id IS NOT NULL
              AND checkout_fulfillment_index IS NOT NULL
              AND checkout_plan_hash IS NOT NULL;

            ALTER TABLE fulfillments
                DROP CONSTRAINT IF EXISTS ck_fulfillments_checkout_identity_typed;
            ALTER TABLE fulfillments
                DROP COLUMN checkout_plan_hash,
                DROP COLUMN checkout_fulfillment_index,
                DROP COLUMN checkout_operation_id;

            ALTER TABLE fulfillments
                ADD CONSTRAINT ck_fulfillments_checkout_identity
                CHECK (
                    metadata #>> '{checkout,fulfillment_key}' IS NULL
                    OR (
                        btrim(metadata #>> '{checkout,fulfillment_key}') <> ''
                        AND btrim(COALESCE(metadata #>> '{checkout,operation_id}', '')) <> ''
                    )
                );

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
            ALTER TABLE fulfillments ADD COLUMN checkout_operation_id TEXT;
            ALTER TABLE fulfillments ADD COLUMN checkout_fulfillment_index INTEGER;
            ALTER TABLE fulfillments ADD COLUMN checkout_plan_hash TEXT;

            UPDATE fulfillments
            SET
                checkout_operation_id = CASE
                    WHEN length(trim(json_extract(metadata, '$.checkout.operation_id'))) = 36
                        AND lower(trim(json_extract(metadata, '$.checkout.operation_id')))
                            NOT GLOB '*[^0-9a-f-]*'
                        AND substr(trim(json_extract(metadata, '$.checkout.operation_id')), 9, 1) = '-'
                        AND substr(trim(json_extract(metadata, '$.checkout.operation_id')), 14, 1) = '-'
                        AND substr(trim(json_extract(metadata, '$.checkout.operation_id')), 19, 1) = '-'
                        AND substr(trim(json_extract(metadata, '$.checkout.operation_id')), 24, 1) = '-'
                        AND lower(trim(json_extract(metadata, '$.checkout.operation_id')))
                            <> '00000000-0000-0000-0000-000000000000'
                    THEN lower(trim(json_extract(metadata, '$.checkout.operation_id')))
                    ELSE NULL
                END,
                checkout_fulfillment_index = CASE
                    WHEN trim(json_extract(metadata, '$.checkout.fulfillment_index'))
                        NOT GLOB '*[^0-9]*'
                        AND length(trim(json_extract(metadata, '$.checkout.fulfillment_index'))) > 0
                    THEN CAST(trim(json_extract(metadata, '$.checkout.fulfillment_index')) AS INTEGER)
                    ELSE NULL
                END,
                checkout_plan_hash = CASE
                    WHEN length(trim(json_extract(metadata, '$.checkout.order_plan_hash'))) = 64
                        AND lower(trim(json_extract(metadata, '$.checkout.order_plan_hash')))
                            NOT GLOB '*[^0-9a-f]*'
                    THEN trim(json_extract(metadata, '$.checkout.order_plan_hash'))
                    ELSE NULL
                END
            WHERE json_extract(metadata, '$.checkout.fulfillment_key') IS NOT NULL;

            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_guard_insert;
            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_guard_update;
            DROP INDEX IF EXISTS ux_fulfillments_checkout_identity;

            CREATE TRIGGER fulfillments_checkout_identity_typed_validation
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            WHEN json_extract(OLD.metadata, '$.checkout.fulfillment_key') IS NOT NULL
            BEGIN
                SELECT CASE WHEN
                    NEW.checkout_operation_id IS NULL
                    OR NEW.checkout_fulfillment_index IS NULL
                    OR NEW.checkout_fulfillment_index < 0
                    OR NEW.checkout_fulfillment_index > 4294967295
                    OR NEW.checkout_plan_hash IS NULL
                    OR length(NEW.checkout_plan_hash) <> 64
                    OR lower(NEW.checkout_plan_hash) GLOB '*[^0-9a-f]*'
                    OR lower(NEW.checkout_operation_id) <> lower(trim(json_extract(OLD.metadata, '$.checkout.operation_id')))
                    OR lower(trim(json_extract(OLD.metadata, '$.checkout.order_id'))) <> lower(NEW.order_id)
                    OR NEW.checkout_plan_hash <> trim(json_extract(OLD.metadata, '$.checkout.order_plan_hash'))
                    OR NEW.checkout_fulfillment_index <> CAST(json_extract(OLD.metadata, '$.checkout.fulfillment_index') AS INTEGER)
                    OR json_extract(OLD.metadata, '$.checkout.fulfillment_key')
                        <> 'checkout:' || lower(NEW.checkout_operation_id) || ':fulfillment:' || NEW.checkout_fulfillment_index
                    THEN RAISE(ABORT, 'invalid legacy checkout fulfillment identity') END;
            END;

            UPDATE fulfillments
            SET checkout_operation_id = checkout_operation_id,
                checkout_fulfillment_index = checkout_fulfillment_index,
                checkout_plan_hash = checkout_plan_hash;

            DROP TRIGGER fulfillments_checkout_identity_typed_validation;

            UPDATE fulfillments
            SET metadata = json_remove(
                metadata,
                '$.checkout.operation_id',
                '$.checkout.order_id',
                '$.checkout.order_plan_hash',
                '$.checkout.fulfillment_index',
                '$.checkout.fulfillment_key'
            )
            WHERE json_extract(metadata, '$.checkout.fulfillment_key') IS NOT NULL;

            UPDATE fulfillment_items
            SET metadata = json_remove(
                metadata,
                '$.checkout.operation_id',
                '$.checkout.order_plan_hash',
                '$.checkout.fulfillment_index'
            )
            WHERE json_extract(metadata, '$.checkout.operation_id') IS NOT NULL;

            CREATE TRIGGER fulfillments_checkout_identity_typed_insert
            BEFORE INSERT ON fulfillments
            FOR EACH ROW
            WHEN NOT (
                (NEW.checkout_operation_id IS NULL
                    AND NEW.checkout_fulfillment_index IS NULL
                    AND NEW.checkout_plan_hash IS NULL)
                OR (
                    NEW.checkout_operation_id IS NOT NULL
                    AND lower(NEW.checkout_operation_id)
                        <> '00000000-0000-0000-0000-000000000000'
                    AND NEW.checkout_fulfillment_index IS NOT NULL
                    AND NEW.checkout_fulfillment_index BETWEEN 0 AND 4294967295
                    AND NEW.checkout_plan_hash IS NOT NULL
                    AND length(NEW.checkout_plan_hash) = 64
                    AND lower(NEW.checkout_plan_hash) NOT GLOB '*[^0-9a-f]*'
                )
            )
            BEGIN
                SELECT RAISE(ABORT, 'invalid checkout fulfillment identity');
            END;

            CREATE TRIGGER fulfillments_checkout_identity_typed_update
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NOT (
                    NEW.checkout_operation_id IS OLD.checkout_operation_id
                    AND NEW.checkout_fulfillment_index IS OLD.checkout_fulfillment_index
                    AND NEW.checkout_plan_hash IS OLD.checkout_plan_hash
                ) THEN RAISE(ABORT, 'fulfillment checkout identity is immutable') END;
            END;

            CREATE UNIQUE INDEX ux_fulfillments_checkout_identity
                ON fulfillments (tenant_id, checkout_operation_id, checkout_fulfillment_index);
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
            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_typed_update;
            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_typed_insert;
            DROP INDEX IF EXISTS ux_fulfillments_checkout_identity;

            UPDATE fulfillments
            SET metadata = json_set(
                COALESCE(metadata, '{}'),
                '$.checkout.operation_id', checkout_operation_id,
                '$.checkout.order_id', order_id,
                '$.checkout.order_plan_hash', checkout_plan_hash,
                '$.checkout.fulfillment_index', checkout_fulfillment_index,
                '$.checkout.fulfillment_key',
                    'checkout:' || checkout_operation_id || ':fulfillment:' || checkout_fulfillment_index
            )
            WHERE checkout_operation_id IS NOT NULL
              AND checkout_fulfillment_index IS NOT NULL
              AND checkout_plan_hash IS NOT NULL;

            ALTER TABLE fulfillments DROP COLUMN checkout_plan_hash;
            ALTER TABLE fulfillments DROP COLUMN checkout_fulfillment_index;
            ALTER TABLE fulfillments DROP COLUMN checkout_operation_id;

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
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN json_extract(OLD.metadata, '$.checkout.fulfillment_key')
                    IS NOT json_extract(NEW.metadata, '$.checkout.fulfillment_key')
                    THEN RAISE(ABORT, 'fulfillment checkout identity is immutable') END;
            END;

            CREATE UNIQUE INDEX ux_fulfillments_checkout_identity
                ON fulfillments (
                    tenant_id,
                    json_extract(metadata, '$.checkout.fulfillment_key')
                )
                WHERE json_extract(metadata, '$.checkout.fulfillment_key') IS NOT NULL;
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
                ADD COLUMN checkout_operation_id CHAR(36),
                ADD COLUMN checkout_fulfillment_index BIGINT,
                ADD COLUMN checkout_plan_hash VARCHAR(64);

            UPDATE fulfillments
            SET
                checkout_operation_id = CASE
                    WHEN JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.operation_id'))
                        REGEXP '^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$'
                     AND lower(JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.operation_id')))
                        <> '00000000-0000-0000-0000-000000000000'
                    THEN lower(JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.operation_id')))
                    ELSE NULL
                END,
                checkout_fulfillment_index = CASE
                    WHEN JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.fulfillment_index')) REGEXP '^[0-9]+$'
                    THEN CAST(JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.fulfillment_index')) AS UNSIGNED)
                    ELSE NULL
                END,
                checkout_plan_hash = CASE
                    WHEN JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.order_plan_hash'))
                        REGEXP '^[0-9a-fA-F]{64}$'
                    THEN JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.order_plan_hash'))
                    ELSE NULL
                END
            WHERE JSON_EXTRACT(metadata, '$.checkout.fulfillment_key') IS NOT NULL;

            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_guard_update;
            DROP INDEX ux_fulfillments_checkout_identity ON fulfillments;
            ALTER TABLE fulfillments DROP COLUMN checkout_fulfillment_identity;

            CREATE TRIGGER fulfillments_checkout_identity_typed_validation
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            BEGIN
                IF JSON_EXTRACT(OLD.metadata, '$.checkout.fulfillment_key') IS NOT NULL
                    AND (
                        NEW.checkout_operation_id IS NULL
                        OR NEW.checkout_fulfillment_index IS NULL
                        OR NEW.checkout_fulfillment_index < 0
                        OR NEW.checkout_fulfillment_index > 4294967295
                        OR NEW.checkout_plan_hash IS NULL
                        OR NEW.checkout_plan_hash NOT REGEXP '^[0-9a-fA-F]{64}$'
                        OR lower(NEW.checkout_operation_id)
                            <> lower(JSON_UNQUOTE(JSON_EXTRACT(OLD.metadata, '$.checkout.operation_id')))
                        OR lower(JSON_UNQUOTE(JSON_EXTRACT(OLD.metadata, '$.checkout.order_id')))
                            <> lower(NEW.order_id)
                        OR NEW.checkout_plan_hash
                            <> JSON_UNQUOTE(JSON_EXTRACT(OLD.metadata, '$.checkout.order_plan_hash'))
                        OR NEW.checkout_fulfillment_index
                            <> CAST(JSON_UNQUOTE(JSON_EXTRACT(OLD.metadata, '$.checkout.fulfillment_index')) AS UNSIGNED)
                        OR JSON_UNQUOTE(JSON_EXTRACT(OLD.metadata, '$.checkout.fulfillment_key'))
                            <> CONCAT('checkout:', lower(NEW.checkout_operation_id), ':fulfillment:', NEW.checkout_fulfillment_index)
                    )
                THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'invalid legacy checkout fulfillment identity';
                END IF;
            END;
            "#,
        )
        .await?;
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            UPDATE fulfillments
            SET checkout_operation_id = checkout_operation_id,
                checkout_fulfillment_index = checkout_fulfillment_index,
                checkout_plan_hash = checkout_plan_hash;
            "#,
        )
        .await?;
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER fulfillments_checkout_identity_typed_validation;

            UPDATE fulfillments
            SET metadata = JSON_REMOVE(
                metadata,
                '$.checkout.operation_id',
                '$.checkout.order_id',
                '$.checkout.order_plan_hash',
                '$.checkout.fulfillment_index',
                '$.checkout.fulfillment_key'
            )
            WHERE JSON_EXTRACT(metadata, '$.checkout.fulfillment_key') IS NOT NULL;

            UPDATE fulfillment_items
            SET metadata = JSON_REMOVE(
                metadata,
                '$.checkout.operation_id',
                '$.checkout.order_plan_hash',
                '$.checkout.fulfillment_index'
            )
            WHERE JSON_EXTRACT(metadata, '$.checkout.operation_id') IS NOT NULL;

            CREATE TRIGGER fulfillments_checkout_identity_typed_insert
            BEFORE INSERT ON fulfillments
            FOR EACH ROW
            BEGIN
                IF NOT (
                    (NEW.checkout_operation_id IS NULL
                        AND NEW.checkout_fulfillment_index IS NULL
                        AND NEW.checkout_plan_hash IS NULL)
                    OR (
                        NEW.checkout_operation_id IS NOT NULL
                        AND lower(NEW.checkout_operation_id)
                            <> '00000000-0000-0000-0000-000000000000'
                        AND NEW.checkout_fulfillment_index IS NOT NULL
                        AND NEW.checkout_fulfillment_index BETWEEN 0 AND 4294967295
                        AND NEW.checkout_plan_hash IS NOT NULL
                        AND NEW.checkout_plan_hash REGEXP '^[0-9a-fA-F]{64}$'
                    )
                ) THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'invalid checkout fulfillment identity';
                END IF;
            END;

            CREATE TRIGGER fulfillments_checkout_identity_typed_update
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            BEGIN
                IF NOT (NEW.checkout_operation_id <=> OLD.checkout_operation_id)
                    OR NOT (NEW.checkout_fulfillment_index <=> OLD.checkout_fulfillment_index)
                    OR NOT (NEW.checkout_plan_hash <=> OLD.checkout_plan_hash)
                THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'fulfillment checkout identity is immutable';
                END IF;
            END;

            CREATE UNIQUE INDEX ux_fulfillments_checkout_identity
                ON fulfillments (tenant_id, checkout_operation_id, checkout_fulfillment_index);
            "#,
        )
        .await?;
    Ok(())
}

async fn restore_mysql(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_typed_update;
            DROP TRIGGER IF EXISTS fulfillments_checkout_identity_typed_insert;
            DROP INDEX ux_fulfillments_checkout_identity ON fulfillments;

            UPDATE fulfillments
            SET metadata = JSON_SET(
                COALESCE(metadata, JSON_OBJECT()),
                '$.checkout.operation_id', checkout_operation_id,
                '$.checkout.order_id', order_id,
                '$.checkout.order_plan_hash', checkout_plan_hash,
                '$.checkout.fulfillment_index', checkout_fulfillment_index,
                '$.checkout.fulfillment_key',
                    CONCAT('checkout:', checkout_operation_id, ':fulfillment:', checkout_fulfillment_index)
            )
            WHERE checkout_operation_id IS NOT NULL
              AND checkout_fulfillment_index IS NOT NULL
              AND checkout_plan_hash IS NOT NULL;

            ALTER TABLE fulfillments
                ADD COLUMN checkout_fulfillment_identity VARCHAR(191)
                    GENERATED ALWAYS AS (
                        JSON_UNQUOTE(JSON_EXTRACT(metadata, '$.checkout.fulfillment_key'))
                    ) STORED;

            CREATE UNIQUE INDEX ux_fulfillments_checkout_identity
                ON fulfillments (tenant_id, checkout_fulfillment_identity);

            CREATE TRIGGER fulfillments_checkout_identity_guard_update
            BEFORE UPDATE ON fulfillments
            FOR EACH ROW
            BEGIN
                IF NOT (
                    OLD.checkout_fulfillment_identity <=> NEW.checkout_fulfillment_identity
                ) THEN
                    SIGNAL SQLSTATE '45000'
                        SET MESSAGE_TEXT = 'fulfillment checkout identity is immutable';
                END IF;
            END;

            ALTER TABLE fulfillments
                DROP COLUMN checkout_plan_hash,
                DROP COLUMN checkout_fulfillment_index,
                DROP COLUMN checkout_operation_id;
            "#,
        )
        .await?;
    Ok(())
}
