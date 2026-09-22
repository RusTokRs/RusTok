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
                        DROP TRIGGER IF EXISTS checkout_create_label_payment_guard
                            ON fulfillment_provider_operations;
                        DROP TRIGGER IF EXISTS checkout_create_label_enqueue
                            ON fulfillments;
                        DROP FUNCTION IF EXISTS enforce_checkout_create_label_payment();
                        DROP FUNCTION IF EXISTS enqueue_checkout_create_label();
                        "#,
                    )
                    .await?;
            }
            DatabaseBackend::Sqlite => {
                manager
                    .get_connection()
                    .execute_unprepared(
                        r#"
                        DROP TRIGGER IF EXISTS checkout_create_label_payment_guard;
                        DROP TRIGGER IF EXISTS checkout_create_label_enqueue;
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
            CREATE OR REPLACE FUNCTION enqueue_checkout_create_label()
            RETURNS trigger AS $$
            DECLARE
                resolved_provider_id VARCHAR(100);
                operation_key VARCHAR(191);
            BEGIN
                IF NOT (NEW.metadata ? 'delivery_group') THEN
                    RETURN NEW;
                END IF;

                SELECT COALESCE(NULLIF(btrim(so.provider_id), ''), 'manual')
                INTO resolved_provider_id
                FROM shipping_options so
                WHERE so.id = NEW.shipping_option_id;
                resolved_provider_id := COALESCE(resolved_provider_id, 'manual');
                operation_key := 'checkout:fulfillment_label:' || NEW.id::text;

                INSERT INTO fulfillment_provider_operations (
                    id,
                    tenant_id,
                    fulfillment_id,
                    operation,
                    provider_id,
                    idempotency_key,
                    status,
                    request_payload,
                    provider_reference,
                    provider_result,
                    error_message,
                    created_at,
                    updated_at,
                    provider_completed_at,
                    committed_at
                ) VALUES (
                    NEW.id,
                    NEW.tenant_id,
                    NEW.id,
                    'create_label',
                    resolved_provider_id,
                    operation_key,
                    'pending',
                    jsonb_build_object(
                        'tenant_id', NEW.tenant_id,
                        'fulfillment_id', NEW.id,
                        'idempotency_key', operation_key,
                        'metadata', NEW.metadata
                    ),
                    NULL,
                    NULL,
                    NULL,
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP,
                    NULL,
                    NULL
                )
                ON CONFLICT (tenant_id, provider_id, idempotency_key) DO NOTHING;

                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER checkout_create_label_enqueue
            AFTER INSERT ON fulfillments
            FOR EACH ROW
            EXECUTE FUNCTION enqueue_checkout_create_label();

            CREATE OR REPLACE FUNCTION enforce_checkout_create_label_payment()
            RETURNS trigger AS $$
            DECLARE
                parent_order_status VARCHAR(32);
            BEGIN
                IF NEW.operation = 'create_label'
                   AND NEW.status = 'executing'
                   AND OLD.status IN ('pending', 'provider_error') THEN
                    SELECT o.status
                    INTO parent_order_status
                    FROM fulfillments f
                    JOIN orders o ON o.id = f.order_id AND o.tenant_id = f.tenant_id
                    WHERE f.id = NEW.fulfillment_id
                      AND f.tenant_id = NEW.tenant_id;

                    IF parent_order_status IS NULL THEN
                        RAISE EXCEPTION 'create-label operation has no tenant-scoped order'
                            USING ERRCODE = '23503';
                    END IF;
                    IF parent_order_status <> 'paid' THEN
                        RAISE EXCEPTION 'create-label operation cannot execute before order payment'
                            USING ERRCODE = '23514';
                    END IF;
                END IF;

                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER checkout_create_label_payment_guard
            BEFORE UPDATE OF status ON fulfillment_provider_operations
            FOR EACH ROW
            EXECUTE FUNCTION enforce_checkout_create_label_payment();
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
            CREATE TRIGGER checkout_create_label_enqueue
            AFTER INSERT ON fulfillments
            FOR EACH ROW
            WHEN json_type(NEW.metadata, '$.delivery_group') IS NOT NULL
            BEGIN
                INSERT OR IGNORE INTO fulfillment_provider_operations (
                    id,
                    tenant_id,
                    fulfillment_id,
                    operation,
                    provider_id,
                    idempotency_key,
                    status,
                    request_payload,
                    provider_reference,
                    provider_result,
                    error_message,
                    created_at,
                    updated_at,
                    provider_completed_at,
                    committed_at
                ) VALUES (
                    NEW.id,
                    NEW.tenant_id,
                    NEW.id,
                    'create_label',
                    COALESCE((
                        SELECT NULLIF(trim(so.provider_id), '')
                        FROM shipping_options so
                        WHERE so.id = NEW.shipping_option_id
                    ), 'manual'),
                    'checkout:fulfillment_label:' || NEW.id,
                    'pending',
                    json_object(
                        'tenant_id', NEW.tenant_id,
                        'fulfillment_id', NEW.id,
                        'idempotency_key', 'checkout:fulfillment_label:' || NEW.id,
                        'metadata', json(NEW.metadata)
                    ),
                    NULL,
                    NULL,
                    NULL,
                    CURRENT_TIMESTAMP,
                    CURRENT_TIMESTAMP,
                    NULL,
                    NULL
                );
            END;

            CREATE TRIGGER checkout_create_label_payment_guard
            BEFORE UPDATE OF status ON fulfillment_provider_operations
            FOR EACH ROW
            WHEN NEW.operation = 'create_label'
              AND NEW.status = 'executing'
              AND OLD.status IN ('pending', 'provider_error')
            BEGIN
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1
                    FROM fulfillments f
                    JOIN orders o ON o.id = f.order_id AND o.tenant_id = f.tenant_id
                    WHERE f.id = NEW.fulfillment_id
                      AND f.tenant_id = NEW.tenant_id
                      AND o.status = 'paid'
                ) THEN RAISE(ABORT, 'create-label operation cannot execute before order payment') END;
            END;
            "#,
        )
        .await?;
    Ok(())
}
