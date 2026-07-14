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
                        DROP TRIGGER IF EXISTS payment_collections_bind_checkout_operation
                            ON payment_collections;
                        DROP FUNCTION IF EXISTS bind_checkout_payment_collection();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS payment_collections_bind_checkout_operation_insert;
                        DROP TRIGGER IF EXISTS payment_collections_bind_checkout_operation_update;
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::MySql => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS payment_collections_bind_checkout_operation_insert;
                        DROP TRIGGER IF EXISTS payment_collections_bind_checkout_operation_update;
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
            CREATE OR REPLACE FUNCTION bind_checkout_payment_collection()
            RETURNS trigger AS $$
            DECLARE
                operation_text TEXT;
                operation_id UUID;
                operation_tenant UUID;
                operation_cart UUID;
                operation_order UUID;
                current_collection UUID;
            BEGIN
                operation_text := NEW.metadata #>> '{checkout,operation_id}';
                IF operation_text IS NULL OR btrim(operation_text) = '' THEN
                    RETURN NEW;
                END IF;

                BEGIN
                    operation_id := operation_text::UUID;
                EXCEPTION WHEN invalid_text_representation THEN
                    RAISE EXCEPTION 'payment collection checkout operation id is invalid'
                        USING ERRCODE = '23514';
                END;

                SELECT tenant_id, cart_id, order_id, payment_collection_id
                INTO operation_tenant, operation_cart, operation_order, current_collection
                FROM checkout_operations
                WHERE id = operation_id
                FOR UPDATE;

                IF operation_tenant IS NULL
                    OR operation_tenant <> NEW.tenant_id
                    OR operation_cart IS DISTINCT FROM NEW.cart_id
                    OR operation_order IS DISTINCT FROM NEW.order_id
                THEN
                    RAISE EXCEPTION 'payment collection checkout identity mismatch'
                        USING ERRCODE = '23514';
                END IF;
                IF current_collection IS NOT NULL AND current_collection <> NEW.id THEN
                    RAISE EXCEPTION 'checkout operation is already bound to another payment collection'
                        USING ERRCODE = '23505';
                END IF;

                UPDATE checkout_operations
                SET payment_collection_id = NEW.id,
                    updated_at = CURRENT_TIMESTAMP
                WHERE id = operation_id
                  AND tenant_id = NEW.tenant_id
                  AND (payment_collection_id IS NULL OR payment_collection_id = NEW.id);

                IF NOT FOUND THEN
                    RAISE EXCEPTION 'failed to bind payment collection to checkout operation'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER payment_collections_bind_checkout_operation
            AFTER INSERT OR UPDATE OF metadata, order_id, cart_id
            ON payment_collections
            FOR EACH ROW
            EXECUTE FUNCTION bind_checkout_payment_collection();
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
            CREATE TRIGGER payment_collections_bind_checkout_operation_insert
            AFTER INSERT ON payment_collections
            FOR EACH ROW
            WHEN json_extract(NEW.metadata, '$.checkout.operation_id') IS NOT NULL
            BEGIN
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1
                    FROM checkout_operations co
                    WHERE co.id = json_extract(NEW.metadata, '$.checkout.operation_id')
                      AND co.tenant_id = NEW.tenant_id
                      AND co.cart_id IS NEW.cart_id
                      AND co.order_id IS NEW.order_id
                      AND (co.payment_collection_id IS NULL OR co.payment_collection_id = NEW.id)
                ) THEN RAISE(ABORT, 'payment collection checkout identity mismatch') END;

                UPDATE checkout_operations
                SET payment_collection_id = NEW.id,
                    updated_at = CURRENT_TIMESTAMP
                WHERE id = json_extract(NEW.metadata, '$.checkout.operation_id')
                  AND tenant_id = NEW.tenant_id
                  AND (payment_collection_id IS NULL OR payment_collection_id = NEW.id);
            END;

            CREATE TRIGGER payment_collections_bind_checkout_operation_update
            AFTER UPDATE OF metadata, order_id, cart_id ON payment_collections
            FOR EACH ROW
            WHEN json_extract(NEW.metadata, '$.checkout.operation_id') IS NOT NULL
            BEGIN
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1
                    FROM checkout_operations co
                    WHERE co.id = json_extract(NEW.metadata, '$.checkout.operation_id')
                      AND co.tenant_id = NEW.tenant_id
                      AND co.cart_id IS NEW.cart_id
                      AND co.order_id IS NEW.order_id
                      AND (co.payment_collection_id IS NULL OR co.payment_collection_id = NEW.id)
                ) THEN RAISE(ABORT, 'payment collection checkout identity mismatch') END;

                UPDATE checkout_operations
                SET payment_collection_id = NEW.id,
                    updated_at = CURRENT_TIMESTAMP
                WHERE id = json_extract(NEW.metadata, '$.checkout.operation_id')
                  AND tenant_id = NEW.tenant_id
                  AND (payment_collection_id IS NULL OR payment_collection_id = NEW.id);
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
            CREATE TRIGGER payment_collections_bind_checkout_operation_insert
            AFTER INSERT ON payment_collections
            FOR EACH ROW
            BEGIN
                DECLARE operation_id CHAR(36);
                DECLARE matching_operations INT DEFAULT 0;
                SET operation_id = JSON_UNQUOTE(JSON_EXTRACT(NEW.metadata, '$.checkout.operation_id'));
                IF operation_id IS NOT NULL AND operation_id <> '' THEN
                    SELECT COUNT(*) INTO matching_operations
                    FROM checkout_operations co
                    WHERE co.id = operation_id
                      AND co.tenant_id = NEW.tenant_id
                      AND co.cart_id <=> NEW.cart_id
                      AND co.order_id <=> NEW.order_id
                      AND (co.payment_collection_id IS NULL OR co.payment_collection_id = NEW.id);
                    IF matching_operations <> 1 THEN
                        SIGNAL SQLSTATE '45000'
                            SET MESSAGE_TEXT = 'payment collection checkout identity mismatch';
                    END IF;
                    UPDATE checkout_operations
                    SET payment_collection_id = NEW.id,
                        updated_at = CURRENT_TIMESTAMP
                    WHERE id = operation_id
                      AND tenant_id = NEW.tenant_id
                      AND (payment_collection_id IS NULL OR payment_collection_id = NEW.id);
                END IF;
            END;

            CREATE TRIGGER payment_collections_bind_checkout_operation_update
            AFTER UPDATE ON payment_collections
            FOR EACH ROW
            BEGIN
                DECLARE operation_id CHAR(36);
                DECLARE matching_operations INT DEFAULT 0;
                SET operation_id = JSON_UNQUOTE(JSON_EXTRACT(NEW.metadata, '$.checkout.operation_id'));
                IF operation_id IS NOT NULL AND operation_id <> '' THEN
                    SELECT COUNT(*) INTO matching_operations
                    FROM checkout_operations co
                    WHERE co.id = operation_id
                      AND co.tenant_id = NEW.tenant_id
                      AND co.cart_id <=> NEW.cart_id
                      AND co.order_id <=> NEW.order_id
                      AND (co.payment_collection_id IS NULL OR co.payment_collection_id = NEW.id);
                    IF matching_operations <> 1 THEN
                        SIGNAL SQLSTATE '45000'
                            SET MESSAGE_TEXT = 'payment collection checkout identity mismatch';
                    END IF;
                    UPDATE checkout_operations
                    SET payment_collection_id = NEW.id,
                        updated_at = CURRENT_TIMESTAMP
                    WHERE id = operation_id
                      AND tenant_id = NEW.tenant_id
                      AND (payment_collection_id IS NULL OR payment_collection_id = NEW.id);
                END IF;
            END;
            "#,
        )
        .await?;
    Ok(())
}
