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
                    .table(ReturnCompletionOperations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ReturnCompletionOperations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(ReturnCompletionOperations::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ReturnCompletionOperations::ReturnId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ReturnCompletionOperations::RequestHash)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ReturnCompletionOperations::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ReturnCompletionOperations::Stage)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(ColumnDef::new(ReturnCompletionOperations::RefundId).uuid())
                    .col(ColumnDef::new(ReturnCompletionOperations::OrderChangeId).uuid())
                    .col(
                        ColumnDef::new(ReturnCompletionOperations::AttemptCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(ReturnCompletionOperations::LeaseOwner).string_len(191))
                    .col(
                        ColumnDef::new(ReturnCompletionOperations::LeaseExpiresAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(ColumnDef::new(ReturnCompletionOperations::LastErrorCode).string_len(100))
                    .col(
                        ColumnDef::new(ReturnCompletionOperations::LastErrorMessage)
                            .string_len(2000),
                    )
                    .col(
                        ColumnDef::new(ReturnCompletionOperations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ReturnCompletionOperations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ReturnCompletionOperations::CompletedAt)
                            .timestamp_with_time_zone(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_return_completion_operation_return")
                            .from(
                                ReturnCompletionOperations::Table,
                                ReturnCompletionOperations::ReturnId,
                            )
                            .to(OrderReturns::Table, OrderReturns::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_return_completion_operation_refund")
                            .from(
                                ReturnCompletionOperations::Table,
                                ReturnCompletionOperations::RefundId,
                            )
                            .to(Refunds::Table, Refunds::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_return_completion_operation_order_change")
                            .from(
                                ReturnCompletionOperations::Table,
                                ReturnCompletionOperations::OrderChangeId,
                            )
                            .to(OrderChanges::Table, OrderChanges::Id)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("ux_return_completion_operations_return")
                    .table(ReturnCompletionOperations::Table)
                    .col(ReturnCompletionOperations::TenantId)
                    .col(ReturnCompletionOperations::ReturnId)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_return_completion_operations_recovery")
                    .table(ReturnCompletionOperations::Table)
                    .col(ReturnCompletionOperations::Status)
                    .col(ReturnCompletionOperations::LeaseExpiresAt)
                    .col(ReturnCompletionOperations::UpdatedAt)
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
        if manager.get_database_backend() == DatabaseBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared(
                    "DROP FUNCTION IF EXISTS enforce_return_completion_operation_integrity() CASCADE;",
                )
                .await?;
        }
        manager
            .drop_table(
                Table::drop()
                    .table(ReturnCompletionOperations::Table)
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
            ALTER TABLE return_completion_operations
                ADD CONSTRAINT ck_return_completion_operations_status
                CHECK (status IN (
                    'pending', 'executing', 'retryable_error',
                    'reconciliation_required', 'completed', 'failed'
                )),
                ADD CONSTRAINT ck_return_completion_operations_stage
                CHECK (stage IN (
                    'created', 'resolution_created', 'return_completed', 'completed'
                )),
                ADD CONSTRAINT ck_return_completion_operations_request_hash
                CHECK (btrim(request_hash) <> ''),
                ADD CONSTRAINT ck_return_completion_operations_attempt_count
                CHECK (attempt_count >= 0),
                ADD CONSTRAINT ck_return_completion_operations_resolution
                CHECK (NOT (refund_id IS NOT NULL AND order_change_id IS NOT NULL)),
                ADD CONSTRAINT ck_return_completion_operations_lease
                CHECK (
                    (status = 'executing'
                        AND lease_owner IS NOT NULL
                        AND btrim(lease_owner) <> ''
                        AND lease_expires_at IS NOT NULL)
                    OR
                    (status <> 'executing'
                        AND lease_owner IS NULL
                        AND lease_expires_at IS NULL)
                ),
                ADD CONSTRAINT ck_return_completion_operations_completion
                CHECK (
                    (status IN ('completed', 'failed') AND completed_at IS NOT NULL)
                    OR
                    (status NOT IN ('completed', 'failed') AND completed_at IS NULL)
                );

            CREATE OR REPLACE FUNCTION enforce_return_completion_operation_integrity()
            RETURNS trigger AS $$
            DECLARE
                referenced_tenant UUID;
            BEGIN
                IF TG_OP = 'UPDATE' AND (
                    NEW.id IS DISTINCT FROM OLD.id
                    OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
                    OR NEW.return_id IS DISTINCT FROM OLD.return_id
                    OR NEW.request_hash IS DISTINCT FROM OLD.request_hash
                ) THEN
                    RAISE EXCEPTION 'return completion operation identity is immutable'
                        USING ERRCODE = '23514';
                END IF;

                SELECT tenant_id INTO referenced_tenant
                FROM order_returns WHERE id = NEW.return_id;
                IF referenced_tenant IS NULL OR referenced_tenant <> NEW.tenant_id THEN
                    RAISE EXCEPTION 'return completion operation return tenant mismatch'
                        USING ERRCODE = '23514';
                END IF;

                IF NEW.refund_id IS NOT NULL THEN
                    SELECT tenant_id INTO referenced_tenant FROM refunds WHERE id = NEW.refund_id;
                    IF referenced_tenant IS NULL OR referenced_tenant <> NEW.tenant_id THEN
                        RAISE EXCEPTION 'return completion operation refund tenant mismatch'
                            USING ERRCODE = '23514';
                    END IF;
                END IF;

                IF NEW.order_change_id IS NOT NULL THEN
                    SELECT tenant_id INTO referenced_tenant
                    FROM order_changes WHERE id = NEW.order_change_id;
                    IF referenced_tenant IS NULL OR referenced_tenant <> NEW.tenant_id THEN
                        RAISE EXCEPTION 'return completion operation order change tenant mismatch'
                            USING ERRCODE = '23514';
                    END IF;
                END IF;

                IF TG_OP = 'UPDATE' AND NOT (
                    OLD.status = NEW.status
                    OR (OLD.status IN ('pending', 'retryable_error') AND NEW.status = 'executing')
                    OR (OLD.status = 'executing' AND NEW.status IN (
                        'retryable_error', 'reconciliation_required', 'completed', 'failed'
                    ))
                ) THEN
                    RAISE EXCEPTION 'invalid return completion operation transition from % to %', OLD.status, NEW.status
                        USING ERRCODE = '23514';
                END IF;

                IF TG_OP = 'UPDATE' AND NOT (
                    OLD.stage = NEW.stage
                    OR (OLD.stage = 'created' AND NEW.stage IN (
                        'resolution_created', 'return_completed', 'completed'
                    ))
                    OR (OLD.stage = 'resolution_created' AND NEW.stage IN (
                        'return_completed', 'completed'
                    ))
                    OR (OLD.stage = 'return_completed' AND NEW.stage = 'completed')
                ) THEN
                    RAISE EXCEPTION 'invalid return completion stage transition from % to %', OLD.stage, NEW.stage
                        USING ERRCODE = '23514';
                END IF;

                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER return_completion_operations_integrity_guard
            BEFORE INSERT OR UPDATE ON return_completion_operations
            FOR EACH ROW
            EXECUTE FUNCTION enforce_return_completion_operation_integrity();
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
            CREATE TRIGGER return_completion_operations_guard_insert
            BEFORE INSERT ON return_completion_operations
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NEW.status NOT IN (
                    'pending', 'executing', 'retryable_error',
                    'reconciliation_required', 'completed', 'failed'
                ) THEN RAISE(ABORT, 'invalid return completion operation status') END;
                SELECT CASE WHEN NEW.stage NOT IN (
                    'created', 'resolution_created', 'return_completed', 'completed'
                ) THEN RAISE(ABORT, 'invalid return completion operation stage') END;
                SELECT CASE WHEN trim(NEW.request_hash) = ''
                    THEN RAISE(ABORT, 'invalid return completion request hash') END;
                SELECT CASE WHEN NEW.attempt_count < 0
                    THEN RAISE(ABORT, 'invalid return completion attempt count') END;
                SELECT CASE WHEN NEW.refund_id IS NOT NULL AND NEW.order_change_id IS NOT NULL
                    THEN RAISE(ABORT, 'multiple return completion resolution links') END;
                SELECT CASE WHEN NOT (
                    (NEW.status = 'executing'
                        AND NEW.lease_owner IS NOT NULL
                        AND trim(NEW.lease_owner) <> ''
                        AND NEW.lease_expires_at IS NOT NULL)
                    OR
                    (NEW.status <> 'executing'
                        AND NEW.lease_owner IS NULL
                        AND NEW.lease_expires_at IS NULL)
                ) THEN RAISE(ABORT, 'invalid return completion lease') END;
                SELECT CASE WHEN NOT (
                    (NEW.status IN ('completed', 'failed') AND NEW.completed_at IS NOT NULL)
                    OR
                    (NEW.status NOT IN ('completed', 'failed') AND NEW.completed_at IS NULL)
                ) THEN RAISE(ABORT, 'invalid return completion timestamp') END;
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1 FROM order_returns
                    WHERE id = NEW.return_id AND tenant_id = NEW.tenant_id
                ) THEN RAISE(ABORT, 'return completion operation return tenant mismatch') END;
                SELECT CASE WHEN NEW.refund_id IS NOT NULL AND NOT EXISTS (
                    SELECT 1 FROM refunds
                    WHERE id = NEW.refund_id AND tenant_id = NEW.tenant_id
                ) THEN RAISE(ABORT, 'return completion operation refund tenant mismatch') END;
                SELECT CASE WHEN NEW.order_change_id IS NOT NULL AND NOT EXISTS (
                    SELECT 1 FROM order_changes
                    WHERE id = NEW.order_change_id AND tenant_id = NEW.tenant_id
                ) THEN RAISE(ABORT, 'return completion operation order change tenant mismatch') END;
            END;

            CREATE TRIGGER return_completion_operations_guard_update
            BEFORE UPDATE ON return_completion_operations
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NEW.id IS NOT OLD.id
                    OR NEW.tenant_id IS NOT OLD.tenant_id
                    OR NEW.return_id IS NOT OLD.return_id
                    OR NEW.request_hash IS NOT OLD.request_hash
                    THEN RAISE(ABORT, 'return completion operation identity is immutable') END;
                SELECT CASE WHEN NOT (
                    OLD.status = NEW.status
                    OR (OLD.status IN ('pending', 'retryable_error') AND NEW.status = 'executing')
                    OR (OLD.status = 'executing' AND NEW.status IN (
                        'retryable_error', 'reconciliation_required', 'completed', 'failed'
                    ))
                ) THEN RAISE(ABORT, 'invalid return completion operation status transition') END;
                SELECT CASE WHEN NOT (
                    OLD.stage = NEW.stage
                    OR (OLD.stage = 'created' AND NEW.stage IN (
                        'resolution_created', 'return_completed', 'completed'
                    ))
                    OR (OLD.stage = 'resolution_created' AND NEW.stage IN (
                        'return_completed', 'completed'
                    ))
                    OR (OLD.stage = 'return_completed' AND NEW.stage = 'completed')
                ) THEN RAISE(ABORT, 'invalid return completion stage transition') END;
                SELECT CASE WHEN NEW.status NOT IN (
                    'pending', 'executing', 'retryable_error',
                    'reconciliation_required', 'completed', 'failed'
                ) THEN RAISE(ABORT, 'invalid return completion operation status') END;
                SELECT CASE WHEN NEW.stage NOT IN (
                    'created', 'resolution_created', 'return_completed', 'completed'
                ) THEN RAISE(ABORT, 'invalid return completion operation stage') END;
                SELECT CASE WHEN NEW.attempt_count < 0
                    THEN RAISE(ABORT, 'invalid return completion attempt count') END;
                SELECT CASE WHEN NEW.refund_id IS NOT NULL AND NEW.order_change_id IS NOT NULL
                    THEN RAISE(ABORT, 'multiple return completion resolution links') END;
                SELECT CASE WHEN NOT (
                    (NEW.status = 'executing'
                        AND NEW.lease_owner IS NOT NULL
                        AND trim(NEW.lease_owner) <> ''
                        AND NEW.lease_expires_at IS NOT NULL)
                    OR
                    (NEW.status <> 'executing'
                        AND NEW.lease_owner IS NULL
                        AND NEW.lease_expires_at IS NULL)
                ) THEN RAISE(ABORT, 'invalid return completion lease') END;
                SELECT CASE WHEN NOT (
                    (NEW.status IN ('completed', 'failed') AND NEW.completed_at IS NOT NULL)
                    OR
                    (NEW.status NOT IN ('completed', 'failed') AND NEW.completed_at IS NULL)
                ) THEN RAISE(ABORT, 'invalid return completion timestamp') END;
                SELECT CASE WHEN NOT EXISTS (
                    SELECT 1 FROM order_returns
                    WHERE id = NEW.return_id AND tenant_id = NEW.tenant_id
                ) THEN RAISE(ABORT, 'return completion operation return tenant mismatch') END;
                SELECT CASE WHEN NEW.refund_id IS NOT NULL AND NOT EXISTS (
                    SELECT 1 FROM refunds
                    WHERE id = NEW.refund_id AND tenant_id = NEW.tenant_id
                ) THEN RAISE(ABORT, 'return completion operation refund tenant mismatch') END;
                SELECT CASE WHEN NEW.order_change_id IS NOT NULL AND NOT EXISTS (
                    SELECT 1 FROM order_changes
                    WHERE id = NEW.order_change_id AND tenant_id = NEW.tenant_id
                ) THEN RAISE(ABORT, 'return completion operation order change tenant mismatch') END;
            END;
            "#,
        )
        .await?;
    Ok(())
}

#[derive(DeriveIden)]
enum ReturnCompletionOperations {
    Table,
    Id,
    TenantId,
    ReturnId,
    RequestHash,
    Status,
    Stage,
    RefundId,
    OrderChangeId,
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
enum OrderReturns {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum Refunds {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum OrderChanges {
    Table,
    Id,
}
