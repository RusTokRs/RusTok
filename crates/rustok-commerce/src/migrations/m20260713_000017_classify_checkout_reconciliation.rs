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
            DatabaseBackend::Postgres => uninstall_postgres(manager).await?,
            DatabaseBackend::Sqlite => uninstall_sqlite(manager).await?,
            DatabaseBackend::MySql => uninstall_mysql(manager).await?,
        }
        Ok(())
    }
}

async fn install_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP INDEX IF EXISTS ux_checkout_operations_active_cart;
            ALTER TABLE checkout_operations
                DROP CONSTRAINT IF EXISTS ck_checkout_operations_status,
                DROP CONSTRAINT IF EXISTS ck_checkout_operations_completion;

            ALTER TABLE checkout_operations
                ADD CONSTRAINT ck_checkout_operations_status
                CHECK (status IN (
                    'pending',
                    'executing',
                    'retryable_error',
                    'compensation_required',
                    'compensating',
                    'reconciliation_required',
                    'completed',
                    'compensated',
                    'failed'
                )),
                ADD CONSTRAINT ck_checkout_operations_completion
                CHECK (
                    (status IN (
                        'reconciliation_required', 'completed', 'compensated', 'failed'
                    ) AND completed_at IS NOT NULL)
                    OR
                    (status NOT IN (
                        'reconciliation_required', 'completed', 'compensated', 'failed'
                    ) AND completed_at IS NULL)
                );

            CREATE UNIQUE INDEX ux_checkout_operations_active_cart
            ON checkout_operations (tenant_id, cart_id)
            WHERE status IN (
                'pending', 'executing', 'retryable_error',
                'compensation_required', 'compensating', 'reconciliation_required'
            );

            CREATE OR REPLACE FUNCTION enforce_checkout_operation_integrity()
            RETURNS trigger AS $$
            DECLARE
                referenced_tenant UUID;
            BEGIN
                IF TG_OP = 'UPDATE' AND (
                    NEW.id IS DISTINCT FROM OLD.id
                    OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
                    OR NEW.cart_id IS DISTINCT FROM OLD.cart_id
                    OR NEW.idempotency_key IS DISTINCT FROM OLD.idempotency_key
                    OR NEW.request_hash IS DISTINCT FROM OLD.request_hash
                ) THEN
                    RAISE EXCEPTION 'checkout operation identity is immutable'
                        USING ERRCODE = '23514';
                END IF;

                IF NEW.status = 'compensation_required'
                    AND NEW.last_error_code = 'checkout.compensation_manual_reconciliation'
                THEN
                    NEW.status := 'reconciliation_required';
                    NEW.completed_at := COALESCE(NEW.completed_at, CURRENT_TIMESTAMP);
                END IF;

                SELECT tenant_id INTO referenced_tenant FROM carts WHERE id = NEW.cart_id;
                IF referenced_tenant IS NULL OR referenced_tenant <> NEW.tenant_id THEN
                    RAISE EXCEPTION 'checkout operation cart tenant mismatch'
                        USING ERRCODE = '23514';
                END IF;

                IF NEW.order_id IS NOT NULL THEN
                    SELECT tenant_id INTO referenced_tenant FROM orders WHERE id = NEW.order_id;
                    IF referenced_tenant IS NULL OR referenced_tenant <> NEW.tenant_id THEN
                        RAISE EXCEPTION 'checkout operation order tenant mismatch'
                            USING ERRCODE = '23514';
                    END IF;
                END IF;

                IF NEW.payment_collection_id IS NOT NULL THEN
                    SELECT tenant_id INTO referenced_tenant
                    FROM payment_collections
                    WHERE id = NEW.payment_collection_id;
                    IF referenced_tenant IS NULL OR referenced_tenant <> NEW.tenant_id THEN
                        RAISE EXCEPTION 'checkout operation payment tenant mismatch'
                            USING ERRCODE = '23514';
                    END IF;
                END IF;

                IF TG_OP = 'UPDATE' AND NOT (
                    OLD.status = NEW.status
                    OR (OLD.status IN ('pending', 'retryable_error') AND NEW.status = 'executing')
                    OR (OLD.status = 'executing' AND NEW.status IN (
                        'retryable_error', 'compensation_required', 'completed', 'failed'
                    ))
                    OR (OLD.status = 'compensation_required' AND NEW.status IN (
                        'compensating', 'reconciliation_required'
                    ))
                    OR (OLD.status = 'compensating' AND NEW.status IN (
                        'compensation_required', 'reconciliation_required', 'compensated', 'failed'
                    ))
                ) THEN
                    RAISE EXCEPTION 'invalid checkout operation transition from % to %', OLD.status, NEW.status
                        USING ERRCODE = '23514';
                END IF;

                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;
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
            DROP INDEX IF EXISTS ux_checkout_operations_active_cart;
            DROP TRIGGER IF EXISTS checkout_operations_guard_update;

            CREATE UNIQUE INDEX ux_checkout_operations_active_cart
            ON checkout_operations (tenant_id, cart_id)
            WHERE status IN (
                'pending', 'executing', 'retryable_error',
                'compensation_required', 'compensating', 'reconciliation_required'
            );

            CREATE TRIGGER checkout_operations_guard_update
            BEFORE UPDATE ON checkout_operations
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NEW.id IS NOT OLD.id
                    OR NEW.tenant_id IS NOT OLD.tenant_id
                    OR NEW.cart_id IS NOT OLD.cart_id
                    OR NEW.idempotency_key IS NOT OLD.idempotency_key
                    OR NEW.request_hash IS NOT OLD.request_hash
                    THEN RAISE(ABORT, 'checkout operation identity is immutable') END;
                SELECT CASE WHEN NEW.status NOT IN (
                    'pending', 'executing', 'retryable_error', 'compensation_required',
                    'compensating', 'reconciliation_required', 'completed', 'compensated', 'failed'
                ) THEN RAISE(ABORT, 'invalid checkout operation status') END;
                SELECT CASE WHEN NEW.stage NOT IN (
                    'created', 'cart_locked', 'order_created', 'inventory_reserved',
                    'payment_ready', 'payment_authorized', 'payment_captured',
                    'fulfillment_created', 'cart_completed', 'completed'
                ) THEN RAISE(ABORT, 'invalid checkout operation stage') END;
                SELECT CASE WHEN NOT (
                    OLD.status = NEW.status
                    OR (OLD.status IN ('pending', 'retryable_error') AND NEW.status = 'executing')
                    OR (OLD.status = 'executing' AND NEW.status IN (
                        'retryable_error', 'compensation_required', 'completed', 'failed'
                    ))
                    OR (OLD.status = 'compensation_required' AND NEW.status IN (
                        'compensating', 'reconciliation_required'
                    ))
                    OR (OLD.status = 'compensating' AND NEW.status IN (
                        'compensation_required', 'reconciliation_required', 'compensated', 'failed'
                    ))
                ) THEN RAISE(ABORT, 'invalid checkout operation transition') END;
                SELECT CASE WHEN NOT (
                    (NEW.status IN ('executing', 'compensating')
                        AND NEW.lease_owner IS NOT NULL
                        AND trim(NEW.lease_owner) <> ''
                        AND NEW.lease_expires_at IS NOT NULL)
                    OR
                    (NEW.status NOT IN ('executing', 'compensating')
                        AND NEW.lease_owner IS NULL
                        AND NEW.lease_expires_at IS NULL)
                ) THEN RAISE(ABORT, 'invalid checkout operation lease') END;
                SELECT CASE WHEN NOT (
                    (NEW.status IN (
                        'reconciliation_required', 'completed', 'compensated', 'failed'
                    ) AND NEW.completed_at IS NOT NULL)
                    OR
                    (NEW.status NOT IN (
                        'reconciliation_required', 'completed', 'compensated', 'failed'
                    ) AND NEW.completed_at IS NULL)
                ) THEN RAISE(ABORT, 'invalid checkout operation completion') END;
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1 FROM carts WHERE id = NEW.cart_id AND tenant_id = NEW.tenant_id
                ) THEN RAISE(ABORT, 'checkout operation cart tenant mismatch') END;
                SELECT CASE WHEN NEW.order_id IS NOT NULL AND NOT EXISTS (
                    SELECT 1 FROM orders WHERE id = NEW.order_id AND tenant_id = NEW.tenant_id
                ) THEN RAISE(ABORT, 'checkout operation order tenant mismatch') END;
                SELECT CASE WHEN NEW.payment_collection_id IS NOT NULL AND NOT EXISTS (
                    SELECT 1 FROM payment_collections
                    WHERE id = NEW.payment_collection_id AND tenant_id = NEW.tenant_id
                ) THEN RAISE(ABORT, 'checkout operation payment tenant mismatch') END;
            END;

            CREATE TRIGGER checkout_operations_manual_reconciliation
            AFTER UPDATE OF status, last_error_code ON checkout_operations
            FOR EACH ROW
            WHEN NEW.status = 'compensation_required'
             AND NEW.last_error_code = 'checkout.compensation_manual_reconciliation'
            BEGIN
                UPDATE checkout_operations
                SET status = 'reconciliation_required',
                    lease_owner = NULL,
                    lease_expires_at = NULL,
                    completed_at = CURRENT_TIMESTAMP,
                    updated_at = CURRENT_TIMESTAMP
                WHERE id = NEW.id
                  AND tenant_id = NEW.tenant_id
                  AND status = 'compensation_required';
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
            CREATE TRIGGER checkout_operations_manual_reconciliation
            BEFORE UPDATE ON checkout_operations
            FOR EACH ROW
            BEGIN
                IF NEW.status = 'compensation_required'
                    AND NEW.last_error_code = 'checkout.compensation_manual_reconciliation'
                THEN
                    SET NEW.status = 'reconciliation_required';
                    SET NEW.lease_owner = NULL;
                    SET NEW.lease_expires_at = NULL;
                    SET NEW.completed_at = CURRENT_TIMESTAMP;
                    SET NEW.updated_at = CURRENT_TIMESTAMP;
                END IF;
            END;
            "#,
        )
        .await?;
    Ok(())
}

async fn uninstall_postgres(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER IF EXISTS checkout_operations_integrity_guard ON checkout_operations;

            UPDATE checkout_operations
            SET status = 'failed',
                last_error_code = COALESCE(last_error_code, 'checkout.reconciliation_required'),
                lease_owner = NULL,
                lease_expires_at = NULL,
                completed_at = COALESCE(completed_at, CURRENT_TIMESTAMP),
                updated_at = CURRENT_TIMESTAMP
            WHERE status = 'reconciliation_required';

            DROP INDEX IF EXISTS ux_checkout_operations_active_cart;
            ALTER TABLE checkout_operations
                DROP CONSTRAINT IF EXISTS ck_checkout_operations_status,
                DROP CONSTRAINT IF EXISTS ck_checkout_operations_completion;

            ALTER TABLE checkout_operations
                ADD CONSTRAINT ck_checkout_operations_status
                CHECK (status IN (
                    'pending', 'executing', 'retryable_error', 'compensation_required',
                    'compensating', 'completed', 'compensated', 'failed'
                )),
                ADD CONSTRAINT ck_checkout_operations_completion
                CHECK (
                    (status IN ('completed', 'compensated', 'failed') AND completed_at IS NOT NULL)
                    OR
                    (status NOT IN ('completed', 'compensated', 'failed') AND completed_at IS NULL)
                );

            CREATE UNIQUE INDEX ux_checkout_operations_active_cart
            ON checkout_operations (tenant_id, cart_id)
            WHERE status IN (
                'pending', 'executing', 'retryable_error',
                'compensation_required', 'compensating'
            );

            CREATE OR REPLACE FUNCTION enforce_checkout_operation_integrity()
            RETURNS trigger AS $$
            DECLARE
                referenced_tenant UUID;
            BEGIN
                IF TG_OP = 'UPDATE' AND (
                    NEW.id IS DISTINCT FROM OLD.id
                    OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
                    OR NEW.cart_id IS DISTINCT FROM OLD.cart_id
                    OR NEW.idempotency_key IS DISTINCT FROM OLD.idempotency_key
                    OR NEW.request_hash IS DISTINCT FROM OLD.request_hash
                ) THEN
                    RAISE EXCEPTION 'checkout operation identity is immutable'
                        USING ERRCODE = '23514';
                END IF;

                SELECT tenant_id INTO referenced_tenant FROM carts WHERE id = NEW.cart_id;
                IF referenced_tenant IS NULL OR referenced_tenant <> NEW.tenant_id THEN
                    RAISE EXCEPTION 'checkout operation cart tenant mismatch'
                        USING ERRCODE = '23514';
                END IF;

                IF NEW.order_id IS NOT NULL THEN
                    SELECT tenant_id INTO referenced_tenant FROM orders WHERE id = NEW.order_id;
                    IF referenced_tenant IS NULL OR referenced_tenant <> NEW.tenant_id THEN
                        RAISE EXCEPTION 'checkout operation order tenant mismatch'
                            USING ERRCODE = '23514';
                    END IF;
                END IF;

                IF NEW.payment_collection_id IS NOT NULL THEN
                    SELECT tenant_id INTO referenced_tenant
                    FROM payment_collections
                    WHERE id = NEW.payment_collection_id;
                    IF referenced_tenant IS NULL OR referenced_tenant <> NEW.tenant_id THEN
                        RAISE EXCEPTION 'checkout operation payment tenant mismatch'
                            USING ERRCODE = '23514';
                    END IF;
                END IF;

                IF TG_OP = 'UPDATE' AND NOT (
                    OLD.status = NEW.status
                    OR (OLD.status IN ('pending', 'retryable_error') AND NEW.status = 'executing')
                    OR (OLD.status = 'executing' AND NEW.status IN (
                        'retryable_error', 'compensation_required', 'completed', 'failed'
                    ))
                    OR (OLD.status = 'compensation_required' AND NEW.status = 'compensating')
                    OR (OLD.status = 'compensating' AND NEW.status IN (
                        'compensation_required', 'compensated', 'failed'
                    ))
                ) THEN
                    RAISE EXCEPTION 'invalid checkout operation transition from % to %', OLD.status, NEW.status
                        USING ERRCODE = '23514';
                END IF;

                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER checkout_operations_integrity_guard
            BEFORE INSERT OR UPDATE ON checkout_operations
            FOR EACH ROW
            EXECUTE FUNCTION enforce_checkout_operation_integrity();
            "#,
        )
        .await?;
    Ok(())
}

async fn uninstall_sqlite(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER IF EXISTS checkout_operations_manual_reconciliation;
            DROP TRIGGER IF EXISTS checkout_operations_guard_update;

            UPDATE checkout_operations
            SET status = 'failed',
                last_error_code = COALESCE(last_error_code, 'checkout.reconciliation_required'),
                lease_owner = NULL,
                lease_expires_at = NULL,
                completed_at = COALESCE(completed_at, CURRENT_TIMESTAMP),
                updated_at = CURRENT_TIMESTAMP
            WHERE status = 'reconciliation_required';

            DROP INDEX IF EXISTS ux_checkout_operations_active_cart;
            CREATE UNIQUE INDEX ux_checkout_operations_active_cart
            ON checkout_operations (tenant_id, cart_id)
            WHERE status IN (
                'pending', 'executing', 'retryable_error',
                'compensation_required', 'compensating'
            );

            CREATE TRIGGER checkout_operations_guard_update
            BEFORE UPDATE ON checkout_operations
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NEW.id IS NOT OLD.id
                    OR NEW.tenant_id IS NOT OLD.tenant_id
                    OR NEW.cart_id IS NOT OLD.cart_id
                    OR NEW.idempotency_key IS NOT OLD.idempotency_key
                    OR NEW.request_hash IS NOT OLD.request_hash
                    THEN RAISE(ABORT, 'checkout operation identity is immutable') END;
                SELECT CASE WHEN NEW.status NOT IN (
                    'pending', 'executing', 'retryable_error', 'compensation_required',
                    'compensating', 'completed', 'compensated', 'failed'
                ) THEN RAISE(ABORT, 'invalid checkout operation status') END;
                SELECT CASE WHEN NEW.stage NOT IN (
                    'created', 'cart_locked', 'order_created', 'inventory_reserved',
                    'payment_ready', 'payment_authorized', 'payment_captured',
                    'fulfillment_created', 'cart_completed', 'completed'
                ) THEN RAISE(ABORT, 'invalid checkout operation stage') END;
                SELECT CASE WHEN NOT (
                    OLD.status = NEW.status
                    OR (OLD.status IN ('pending', 'retryable_error') AND NEW.status = 'executing')
                    OR (OLD.status = 'executing' AND NEW.status IN (
                        'retryable_error', 'compensation_required', 'completed', 'failed'
                    ))
                    OR (OLD.status = 'compensation_required' AND NEW.status = 'compensating')
                    OR (OLD.status = 'compensating' AND NEW.status IN (
                        'compensation_required', 'compensated', 'failed'
                    ))
                ) THEN RAISE(ABORT, 'invalid checkout operation transition') END;
                SELECT CASE WHEN NOT (
                    (NEW.status IN ('executing', 'compensating')
                        AND NEW.lease_owner IS NOT NULL
                        AND trim(NEW.lease_owner) <> ''
                        AND NEW.lease_expires_at IS NOT NULL)
                    OR
                    (NEW.status NOT IN ('executing', 'compensating')
                        AND NEW.lease_owner IS NULL
                        AND NEW.lease_expires_at IS NULL)
                ) THEN RAISE(ABORT, 'invalid checkout operation lease') END;
                SELECT CASE WHEN NOT (
                    (NEW.status IN ('completed', 'compensated', 'failed') AND NEW.completed_at IS NOT NULL)
                    OR
                    (NEW.status NOT IN ('completed', 'compensated', 'failed') AND NEW.completed_at IS NULL)
                ) THEN RAISE(ABORT, 'invalid checkout operation completion') END;
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1 FROM carts WHERE id = NEW.cart_id AND tenant_id = NEW.tenant_id
                ) THEN RAISE(ABORT, 'checkout operation cart tenant mismatch') END;
                SELECT CASE WHEN NEW.order_id IS NOT NULL AND NOT EXISTS (
                    SELECT 1 FROM orders WHERE id = NEW.order_id AND tenant_id = NEW.tenant_id
                ) THEN RAISE(ABORT, 'checkout operation order tenant mismatch') END;
                SELECT CASE WHEN NEW.payment_collection_id IS NOT NULL AND NOT EXISTS (
                    SELECT 1 FROM payment_collections
                    WHERE id = NEW.payment_collection_id AND tenant_id = NEW.tenant_id
                ) THEN RAISE(ABORT, 'checkout operation payment tenant mismatch') END;
            END;
            "#,
        )
        .await?;
    Ok(())
}

async fn uninstall_mysql(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER IF EXISTS checkout_operations_manual_reconciliation;
            UPDATE checkout_operations
            SET status = 'failed',
                last_error_code = COALESCE(last_error_code, 'checkout.reconciliation_required'),
                lease_owner = NULL,
                lease_expires_at = NULL,
                completed_at = COALESCE(completed_at, CURRENT_TIMESTAMP),
                updated_at = CURRENT_TIMESTAMP
            WHERE status = 'reconciliation_required';
            "#,
        )
        .await?;
    Ok(())
}
