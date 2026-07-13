use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CheckoutOperations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CheckoutOperations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(CheckoutOperations::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutOperations::CartId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutOperations::IdempotencyKey)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutOperations::RequestHash)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(ColumnDef::new(CheckoutOperations::SnapshotHash).string_len(128))
                    .col(
                        ColumnDef::new(CheckoutOperations::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CheckoutOperations::Stage)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(ColumnDef::new(CheckoutOperations::OrderId).uuid())
                    .col(ColumnDef::new(CheckoutOperations::PaymentCollectionId).uuid())
                    .col(
                        ColumnDef::new(CheckoutOperations::AttemptCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(CheckoutOperations::LeaseOwner).string_len(191))
                    .col(
                        ColumnDef::new(CheckoutOperations::LeaseExpiresAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(ColumnDef::new(CheckoutOperations::LastErrorCode).string_len(100))
                    .col(
                        ColumnDef::new(CheckoutOperations::LastErrorMessage)
                            .string_len(2000),
                    )
                    .col(
                        ColumnDef::new(CheckoutOperations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CheckoutOperations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CheckoutOperations::CompletedAt)
                            .timestamp_with_time_zone(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(CheckoutOperations::Table, CheckoutOperations::CartId)
                            .to(Carts::Table, Carts::Id),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(CheckoutOperations::Table, CheckoutOperations::OrderId)
                            .to(Orders::Table, Orders::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                CheckoutOperations::Table,
                                CheckoutOperations::PaymentCollectionId,
                            )
                            .to(PaymentCollections::Table, PaymentCollections::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("ux_checkout_operations_idempotency")
                    .table(CheckoutOperations::Table)
                    .col(CheckoutOperations::TenantId)
                    .col(CheckoutOperations::CartId)
                    .col(CheckoutOperations::IdempotencyKey)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_checkout_operations_cart")
                    .table(CheckoutOperations::Table)
                    .col(CheckoutOperations::TenantId)
                    .col(CheckoutOperations::CartId)
                    .col(CheckoutOperations::CreatedAt)
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_checkout_operations_recovery")
                    .table(CheckoutOperations::Table)
                    .col(CheckoutOperations::Status)
                    .col(CheckoutOperations::LeaseExpiresAt)
                    .col(CheckoutOperations::UpdatedAt)
                    .to_owned(),
            )
            .await?;

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres_guards(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite_guards(manager).await?,
            _ => {}
        }

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(CheckoutOperations::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

async fn install_postgres_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            ALTER TABLE checkout_operations
                ADD CONSTRAINT ck_checkout_operations_status
                CHECK (status IN (
                    'pending',
                    'executing',
                    'retryable_error',
                    'compensation_required',
                    'compensating',
                    'completed',
                    'compensated',
                    'failed'
                )),
                ADD CONSTRAINT ck_checkout_operations_stage
                CHECK (stage IN (
                    'created',
                    'cart_locked',
                    'order_created',
                    'inventory_reserved',
                    'payment_ready',
                    'payment_authorized',
                    'payment_captured',
                    'fulfillment_created',
                    'cart_completed',
                    'completed'
                )),
                ADD CONSTRAINT ck_checkout_operations_identity
                CHECK (
                    btrim(idempotency_key) <> ''
                    AND btrim(request_hash) <> ''
                    AND (snapshot_hash IS NULL OR btrim(snapshot_hash) <> '')
                ),
                ADD CONSTRAINT ck_checkout_operations_attempt_count
                CHECK (attempt_count >= 0),
                ADD CONSTRAINT ck_checkout_operations_lease
                CHECK (
                    (status IN ('executing', 'compensating')
                        AND lease_owner IS NOT NULL
                        AND btrim(lease_owner) <> ''
                        AND lease_expires_at IS NOT NULL)
                    OR
                    (status NOT IN ('executing', 'compensating')
                        AND lease_owner IS NULL
                        AND lease_expires_at IS NULL)
                ),
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

async fn install_sqlite_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE UNIQUE INDEX ux_checkout_operations_active_cart
            ON checkout_operations (tenant_id, cart_id)
            WHERE status IN (
                'pending', 'executing', 'retryable_error',
                'compensation_required', 'compensating'
            );

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

#[derive(DeriveIden)]
enum CheckoutOperations {
    Table,
    Id,
    TenantId,
    CartId,
    IdempotencyKey,
    RequestHash,
    SnapshotHash,
    Status,
    Stage,
    OrderId,
    PaymentCollectionId,
    AttemptCount,
    LeaseOwner,
    LeaseExpiresAt,
    LastErrorCode,
    LastErrorMessage,
    CreatedAt,
    UpdatedAt,
    CompletedAt,
}

#[derive(DeriveIden)]
enum Carts {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Orders {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum PaymentCollections {
    Table,
    Id,
}
