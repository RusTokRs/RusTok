use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

/// Moves ownership of the checkout operation state machine from the database
/// back to typed Rust code.
///
/// `m20260713_000009` installed `enforce_checkout_operation_integrity()` (plus
/// its `checkout_operations_integrity_guard` trigger and the SQLite guards) and
/// `m20260713_000017` extended it with the compensation parking rewrite, the
/// transition matrix and cross-row tenant lookups. That is business logic inside
/// constraint triggers, which the repository contract forbids ("Multi-row domain
/// rules, state transition validations, workflow guards, and cross-aggregate
/// invariants MUST be owned and validated by typed Rust domain entities and
/// services inside a transaction boundary. Do NOT implement complex business
/// rules, cascading side-effects, or cross-row business validations inside
/// PL/pgSQL constraint triggers").
///
/// Everything the guards did is now enforced by
/// `CheckoutOperationJournal`: the transition matrix in
/// `CheckoutOperationStatus::allowed_transitions` (every status write goes
/// through `CheckedTransition`), the parking decision in
/// `compensation_next_status`, the lease/completion shape by construction, and
/// the tenant lookups by the tenant-scoped queries the service already uses. The
/// database keeps only column-level `CHECK` constraints and the
/// `ux_checkout_operations_active_cart` uniqueness index, which are declarative
/// schema constraints rather than workflow logic.
///
/// `down()` restores the exact text installed by `m20260713_000017`.
#[derive(DeriveMigrationName)]
pub struct Migration;

/// Rows written before this migration in a deployment whose parking trigger was
/// missing or bypassed. The journal now parks such rows itself, so any left-over
/// `compensation_required` row carrying the manual reconciliation code is parked
/// here instead of being swept forever.
const PARK_STALE_MANUAL_RECONCILIATIONS: &str = r#"
UPDATE checkout_operations
SET status = 'reconciliation_required',
    lease_owner = NULL,
    lease_expires_at = NULL,
    completed_at = COALESCE(completed_at, CURRENT_TIMESTAMP),
    updated_at = CURRENT_TIMESTAMP
WHERE status = 'compensation_required'
  AND last_error_code = 'checkout.compensation_manual_reconciliation';
"#;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => uninstall_postgres_guards(manager).await?,
            DatabaseBackend::Sqlite => uninstall_sqlite_guards(manager).await?,
            DatabaseBackend::MySql => uninstall_mysql_guards(manager).await?,
            _ => unreachable!("unsupported SeaORM database backend"),
        }
        manager
            .get_connection()
            .execute_unprepared(PARK_STALE_MANUAL_RECONCILIATIONS)
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres_guards(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite_guards(manager).await?,
            DatabaseBackend::MySql => install_mysql_guards(manager).await?,
            _ => unreachable!("unsupported SeaORM database backend"),
        }
        Ok(())
    }
}

async fn uninstall_postgres_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER IF EXISTS checkout_operations_integrity_guard ON checkout_operations;
            DROP FUNCTION IF EXISTS enforce_checkout_operation_integrity();
            "#,
        )
        .await?;
    Ok(())
}

async fn uninstall_sqlite_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            DROP TRIGGER IF EXISTS checkout_operations_guard_insert;
            DROP TRIGGER IF EXISTS checkout_operations_guard_update;
            DROP TRIGGER IF EXISTS checkout_operations_manual_reconciliation;
            "#,
        )
        .await?;
    Ok(())
}

async fn uninstall_mysql_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared("DROP TRIGGER IF EXISTS checkout_operations_manual_reconciliation;")
        .await?;
    Ok(())
}
async fn install_postgres_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
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

CREATE TRIGGER checkout_operations_integrity_guard
            BEFORE INSERT OR UPDATE ON checkout_operations
            FOR EACH ROW
            EXECUTE FUNCTION enforce_checkout_operation_integrity();
            "#,
        )
        .await?;
    Ok(())
}

async fn install_sqlite_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
CREATE TRIGGER checkout_operations_guard_insert
            BEFORE INSERT ON checkout_operations
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NEW.status NOT IN (
                    'pending', 'executing', 'retryable_error', 'compensation_required',
                    'compensating', 'completed', 'compensated', 'failed'
                ) THEN RAISE(ABORT, 'invalid checkout operation status') END;
                SELECT CASE WHEN NEW.stage NOT IN (
                    'created', 'cart_locked', 'order_created', 'inventory_reserved',
                    'payment_ready', 'payment_authorized', 'payment_captured',
                    'fulfillment_created', 'cart_completed', 'completed'
                ) THEN RAISE(ABORT, 'invalid checkout operation stage') END;
                SELECT CASE WHEN trim(NEW.idempotency_key) = '' OR trim(NEW.request_hash) = ''
                    THEN RAISE(ABORT, 'invalid checkout operation identity') END;
                SELECT CASE WHEN NEW.attempt_count < 0
                    THEN RAISE(ABORT, 'invalid checkout operation attempt count') END;
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

async fn install_mysql_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
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
