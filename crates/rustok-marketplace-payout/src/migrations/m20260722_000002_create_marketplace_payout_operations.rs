use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_index(
                Index::create()
                    .name("uq_marketplace_payouts_tenant_id")
                    .table(Payouts::Table)
                    .col(Payouts::TenantId)
                    .col(Payouts::Id)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(PayoutOperations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PayoutOperations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(PayoutOperations::TenantId).uuid().not_null())
                    .col(ColumnDef::new(PayoutOperations::ActorId).uuid().not_null())
                    .col(ColumnDef::new(PayoutOperations::SellerId).uuid().not_null())
                    .col(
                        ColumnDef::new(PayoutOperations::CurrencyCode)
                            .string_len(3)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PayoutOperations::IdempotencyKey)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PayoutOperations::RequestHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PayoutOperations::RequestJson)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PayoutOperations::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PayoutOperations::Stage)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(ColumnDef::new(PayoutOperations::PayoutId).uuid())
                    .col(
                        ColumnDef::new(PayoutOperations::AttemptCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(PayoutOperations::LeaseOwner).string_len(191))
                    .col(
                        ColumnDef::new(PayoutOperations::LeaseExpiresAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(ColumnDef::new(PayoutOperations::LastErrorCode).string_len(120))
                    .col(
                        ColumnDef::new(PayoutOperations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(PayoutOperations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(PayoutOperations::CompletedAt)
                            .timestamp_with_time_zone(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_marketplace_payout_operation_tenant_payout")
                            .from(PayoutOperations::Table, PayoutOperations::TenantId)
                            .from_col(PayoutOperations::PayoutId)
                            .to(Payouts::Table, Payouts::TenantId)
                            .to_col(Payouts::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .check(Expr::cust("currency_code = UPPER(currency_code)"))
                    .check(Expr::cust("TRIM(idempotency_key) <> ''"))
                    .check(Expr::cust("LENGTH(request_hash) = 64"))
                    .check(Expr::cust(
                        "status IN ('pending', 'executing', 'retryable_error', 'compensation_required', 'compensating', 'reconciliation_required', 'completed', 'cancelled', 'failed')",
                    ))
                    .check(Expr::cust(
                        "stage IN ('created', 'reserving', 'reserved', 'payout_created', 'releasing', 'released', 'completed')",
                    ))
                    .check(Expr::cust("attempt_count >= 0"))
                    .check(Expr::cust(
                        "((status IN ('executing', 'compensating') AND lease_owner IS NOT NULL AND TRIM(lease_owner) <> '' AND lease_expires_at IS NOT NULL) OR (status NOT IN ('executing', 'compensating') AND lease_owner IS NULL AND lease_expires_at IS NULL))",
                    ))
                    .check(Expr::cust(
                        "((status IN ('completed', 'cancelled', 'failed') AND completed_at IS NOT NULL) OR (status NOT IN ('completed', 'cancelled', 'failed') AND completed_at IS NULL))",
                    ))
                    .check(Expr::cust(
                        "(status <> 'completed' OR (stage = 'completed' AND payout_id IS NOT NULL))",
                    ))
                    .to_owned(),
            )
            .await?;

        for index in [
            Index::create()
                .name("uq_marketplace_payout_operation_tenant_id")
                .table(PayoutOperations::Table)
                .col(PayoutOperations::TenantId)
                .col(PayoutOperations::Id)
                .unique()
                .to_owned(),
            Index::create()
                .name("uq_marketplace_payout_operation_key")
                .table(PayoutOperations::Table)
                .col(PayoutOperations::TenantId)
                .col(PayoutOperations::IdempotencyKey)
                .unique()
                .to_owned(),
            Index::create()
                .name("uq_marketplace_payout_operation_payout")
                .table(PayoutOperations::Table)
                .col(PayoutOperations::TenantId)
                .col(PayoutOperations::PayoutId)
                .unique()
                .to_owned(),
            Index::create()
                .name("idx_marketplace_payout_operation_recovery")
                .table(PayoutOperations::Table)
                .col(PayoutOperations::Status)
                .col(PayoutOperations::LeaseExpiresAt)
                .col(PayoutOperations::UpdatedAt)
                .to_owned(),
            Index::create()
                .name("idx_marketplace_payout_operation_seller")
                .table(PayoutOperations::Table)
                .col(PayoutOperations::TenantId)
                .col(PayoutOperations::SellerId)
                .col(PayoutOperations::CurrencyCode)
                .col(PayoutOperations::CreatedAt)
                .to_owned(),
        ] {
            manager.create_index(index).await?;
        }

        manager
            .create_table(
                Table::create()
                    .table(PayoutOperationTransfers::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PayoutOperationTransfers::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PayoutOperationTransfers::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PayoutOperationTransfers::OperationId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PayoutOperationTransfers::SequenceNo)
                            .integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PayoutOperationTransfers::OrderId).uuid().not_null())
                    .col(
                        ColumnDef::new(PayoutOperationTransfers::TransferKind)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PayoutOperationTransfers::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PayoutOperationTransfers::IdempotencyKey)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PayoutOperationTransfers::RequestHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PayoutOperationTransfers::RequestJson)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PayoutOperationTransfers::TotalAmount)
                            .big_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(PayoutOperationTransfers::LedgerTransferId).uuid())
                    .col(
                        ColumnDef::new(PayoutOperationTransfers::LedgerTransactionId)
                            .uuid(),
                    )
                    .col(
                        ColumnDef::new(PayoutOperationTransfers::AttemptCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(PayoutOperationTransfers::LastErrorCode)
                            .string_len(120),
                    )
                    .col(
                        ColumnDef::new(PayoutOperationTransfers::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(PayoutOperationTransfers::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(PayoutOperationTransfers::CompletedAt)
                            .timestamp_with_time_zone(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_mkt_payout_transfer_tenant_operation")
                            .from(
                                PayoutOperationTransfers::Table,
                                PayoutOperationTransfers::TenantId,
                            )
                            .from_col(PayoutOperationTransfers::OperationId)
                            .to(PayoutOperations::Table, PayoutOperations::TenantId)
                            .to_col(PayoutOperations::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::cust("sequence_no >= 0"))
                    .check(Expr::cust(
                        "transfer_kind IN ('reserve_hold', 'reserve_release')",
                    ))
                    .check(Expr::cust(
                        "status IN ('pending', 'executing', 'posted', 'retryable_error', 'reconciliation_required', 'compensated', 'failed')",
                    ))
                    .check(Expr::cust("TRIM(idempotency_key) <> ''"))
                    .check(Expr::cust("LENGTH(request_hash) = 64"))
                    .check(Expr::cust("total_amount > 0"))
                    .check(Expr::cust("attempt_count >= 0"))
                    .check(Expr::cust(
                        "((status IN ('posted', 'compensated', 'failed') AND completed_at IS NOT NULL) OR (status NOT IN ('posted', 'compensated', 'failed') AND completed_at IS NULL))",
                    ))
                    .check(Expr::cust(
                        "((status IN ('posted', 'compensated') AND ledger_transfer_id IS NOT NULL AND ledger_transaction_id IS NOT NULL) OR (status NOT IN ('posted', 'compensated'))) ",
                    ))
                    .to_owned(),
            )
            .await?;

        for index in [
            Index::create()
                .name("uq_mkt_payout_operation_transfer_sequence")
                .table(PayoutOperationTransfers::Table)
                .col(PayoutOperationTransfers::TenantId)
                .col(PayoutOperationTransfers::OperationId)
                .col(PayoutOperationTransfers::SequenceNo)
                .unique()
                .to_owned(),
            Index::create()
                .name("uq_mkt_payout_operation_transfer_order_kind")
                .table(PayoutOperationTransfers::Table)
                .col(PayoutOperationTransfers::TenantId)
                .col(PayoutOperationTransfers::OperationId)
                .col(PayoutOperationTransfers::OrderId)
                .col(PayoutOperationTransfers::TransferKind)
                .unique()
                .to_owned(),
            Index::create()
                .name("uq_mkt_payout_operation_transfer_key")
                .table(PayoutOperationTransfers::Table)
                .col(PayoutOperationTransfers::TenantId)
                .col(PayoutOperationTransfers::IdempotencyKey)
                .unique()
                .to_owned(),
            Index::create()
                .name("uq_mkt_payout_operation_ledger_transfer")
                .table(PayoutOperationTransfers::Table)
                .col(PayoutOperationTransfers::TenantId)
                .col(PayoutOperationTransfers::LedgerTransferId)
                .unique()
                .to_owned(),
            Index::create()
                .name("idx_mkt_payout_operation_transfer_recovery")
                .table(PayoutOperationTransfers::Table)
                .col(PayoutOperationTransfers::Status)
                .col(PayoutOperationTransfers::UpdatedAt)
                .to_owned(),
        ] {
            manager.create_index(index).await?;
        }

        match manager.get_database_backend() {
            DatabaseBackend::Postgres => install_postgres_guards(manager).await?,
            DatabaseBackend::Sqlite => install_sqlite_guards(manager).await?,
            DatabaseBackend::MySql => install_mysql_guards(manager).await?,
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        uninstall_guards(manager).await?;
        manager
            .drop_table(
                Table::drop()
                    .table(PayoutOperationTransfers::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(PayoutOperations::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("uq_marketplace_payouts_tenant_id")
                    .table(Payouts::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

async fn install_postgres_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE OR REPLACE FUNCTION enforce_marketplace_payout_operation_update()
            RETURNS trigger AS $$
            BEGIN
                IF NEW.id IS DISTINCT FROM OLD.id
                    OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
                    OR NEW.actor_id IS DISTINCT FROM OLD.actor_id
                    OR NEW.seller_id IS DISTINCT FROM OLD.seller_id
                    OR NEW.currency_code IS DISTINCT FROM OLD.currency_code
                    OR NEW.idempotency_key IS DISTINCT FROM OLD.idempotency_key
                    OR NEW.request_hash IS DISTINCT FROM OLD.request_hash
                    OR NEW.request_json IS DISTINCT FROM OLD.request_json
                THEN
                    RAISE EXCEPTION 'marketplace payout operation identity is immutable'
                        USING ERRCODE = '23514';
                END IF;
                IF NOT (
                    OLD.status = NEW.status
                    OR (OLD.status IN ('pending', 'retryable_error') AND NEW.status = 'executing')
                    OR (OLD.status = 'executing' AND NEW.status IN ('retryable_error', 'compensation_required', 'reconciliation_required', 'completed', 'failed'))
                    OR (OLD.status = 'compensation_required' AND NEW.status IN ('compensating', 'reconciliation_required'))
                    OR (OLD.status = 'compensating' AND NEW.status IN ('compensation_required', 'reconciliation_required', 'cancelled', 'failed'))
                ) THEN
                    RAISE EXCEPTION 'invalid marketplace payout operation status transition'
                        USING ERRCODE = '23514';
                END IF;
                IF NOT (
                    OLD.stage = NEW.stage
                    OR (OLD.stage = 'created' AND NEW.stage = 'reserving')
                    OR (OLD.stage = 'reserving' AND NEW.stage IN ('reserved', 'releasing'))
                    OR (OLD.stage = 'reserved' AND NEW.stage IN ('payout_created', 'releasing'))
                    OR (OLD.stage = 'payout_created' AND NEW.stage IN ('completed', 'releasing'))
                    OR (OLD.stage = 'releasing' AND NEW.stage = 'released')
                    OR (OLD.stage = 'released' AND NEW.stage = 'completed')
                ) THEN
                    RAISE EXCEPTION 'invalid marketplace payout operation stage transition'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER marketplace_payout_operation_update_guard
            BEFORE UPDATE ON marketplace_payout_operations
            FOR EACH ROW EXECUTE FUNCTION enforce_marketplace_payout_operation_update();

            CREATE OR REPLACE FUNCTION enforce_marketplace_payout_transfer_update()
            RETURNS trigger AS $$
            BEGIN
                IF NEW.id IS DISTINCT FROM OLD.id
                    OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
                    OR NEW.operation_id IS DISTINCT FROM OLD.operation_id
                    OR NEW.sequence_no IS DISTINCT FROM OLD.sequence_no
                    OR NEW.order_id IS DISTINCT FROM OLD.order_id
                    OR NEW.transfer_kind IS DISTINCT FROM OLD.transfer_kind
                    OR NEW.idempotency_key IS DISTINCT FROM OLD.idempotency_key
                    OR NEW.request_hash IS DISTINCT FROM OLD.request_hash
                    OR NEW.request_json IS DISTINCT FROM OLD.request_json
                    OR NEW.total_amount IS DISTINCT FROM OLD.total_amount
                THEN
                    RAISE EXCEPTION 'marketplace payout operation transfer identity is immutable'
                        USING ERRCODE = '23514';
                END IF;
                IF NOT (
                    OLD.status = NEW.status
                    OR (OLD.status IN ('pending', 'retryable_error') AND NEW.status = 'executing')
                    OR (OLD.status = 'executing' AND NEW.status IN ('posted', 'retryable_error', 'reconciliation_required', 'failed'))
                    OR (OLD.status = 'posted' AND NEW.status = 'compensated')
                ) THEN
                    RAISE EXCEPTION 'invalid marketplace payout transfer status transition'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER marketplace_payout_transfer_update_guard
            BEFORE UPDATE ON marketplace_payout_operation_transfers
            FOR EACH ROW EXECUTE FUNCTION enforce_marketplace_payout_transfer_update();
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
            CREATE TRIGGER marketplace_payout_operation_update_guard
            BEFORE UPDATE ON marketplace_payout_operations
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NEW.id IS NOT OLD.id
                    OR NEW.tenant_id IS NOT OLD.tenant_id
                    OR NEW.actor_id IS NOT OLD.actor_id
                    OR NEW.seller_id IS NOT OLD.seller_id
                    OR NEW.currency_code IS NOT OLD.currency_code
                    OR NEW.idempotency_key IS NOT OLD.idempotency_key
                    OR NEW.request_hash IS NOT OLD.request_hash
                    OR NEW.request_json IS NOT OLD.request_json
                    THEN RAISE(ABORT, 'marketplace payout operation identity is immutable') END;
                SELECT CASE WHEN NOT (
                    OLD.status = NEW.status
                    OR (OLD.status IN ('pending', 'retryable_error') AND NEW.status = 'executing')
                    OR (OLD.status = 'executing' AND NEW.status IN ('retryable_error', 'compensation_required', 'reconciliation_required', 'completed', 'failed'))
                    OR (OLD.status = 'compensation_required' AND NEW.status IN ('compensating', 'reconciliation_required'))
                    OR (OLD.status = 'compensating' AND NEW.status IN ('compensation_required', 'reconciliation_required', 'cancelled', 'failed'))
                ) THEN RAISE(ABORT, 'invalid marketplace payout operation status transition') END;
                SELECT CASE WHEN NOT (
                    OLD.stage = NEW.stage
                    OR (OLD.stage = 'created' AND NEW.stage = 'reserving')
                    OR (OLD.stage = 'reserving' AND NEW.stage IN ('reserved', 'releasing'))
                    OR (OLD.stage = 'reserved' AND NEW.stage IN ('payout_created', 'releasing'))
                    OR (OLD.stage = 'payout_created' AND NEW.stage IN ('completed', 'releasing'))
                    OR (OLD.stage = 'releasing' AND NEW.stage = 'released')
                    OR (OLD.stage = 'released' AND NEW.stage = 'completed')
                ) THEN RAISE(ABORT, 'invalid marketplace payout operation stage transition') END;
            END;

            CREATE TRIGGER marketplace_payout_transfer_update_guard
            BEFORE UPDATE ON marketplace_payout_operation_transfers
            FOR EACH ROW
            BEGIN
                SELECT CASE WHEN NEW.id IS NOT OLD.id
                    OR NEW.tenant_id IS NOT OLD.tenant_id
                    OR NEW.operation_id IS NOT OLD.operation_id
                    OR NEW.sequence_no IS NOT OLD.sequence_no
                    OR NEW.order_id IS NOT OLD.order_id
                    OR NEW.transfer_kind IS NOT OLD.transfer_kind
                    OR NEW.idempotency_key IS NOT OLD.idempotency_key
                    OR NEW.request_hash IS NOT OLD.request_hash
                    OR NEW.request_json IS NOT OLD.request_json
                    OR NEW.total_amount IS NOT OLD.total_amount
                    THEN RAISE(ABORT, 'marketplace payout operation transfer identity is immutable') END;
                SELECT CASE WHEN NOT (
                    OLD.status = NEW.status
                    OR (OLD.status IN ('pending', 'retryable_error') AND NEW.status = 'executing')
                    OR (OLD.status = 'executing' AND NEW.status IN ('posted', 'retryable_error', 'reconciliation_required', 'failed'))
                    OR (OLD.status = 'posted' AND NEW.status = 'compensated')
                ) THEN RAISE(ABORT, 'invalid marketplace payout transfer status transition') END;
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
            CREATE TRIGGER marketplace_payout_operation_update_guard
            BEFORE UPDATE ON marketplace_payout_operations
            FOR EACH ROW
            BEGIN
                IF NOT (NEW.id <=> OLD.id)
                    OR NOT (NEW.tenant_id <=> OLD.tenant_id)
                    OR NOT (NEW.actor_id <=> OLD.actor_id)
                    OR NOT (NEW.seller_id <=> OLD.seller_id)
                    OR NOT (NEW.currency_code <=> OLD.currency_code)
                    OR NOT (NEW.idempotency_key <=> OLD.idempotency_key)
                    OR NOT (NEW.request_hash <=> OLD.request_hash)
                    OR NOT (NEW.request_json <=> OLD.request_json)
                THEN
                    SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'marketplace payout operation identity is immutable';
                END IF;
                IF NOT (
                    OLD.status = NEW.status
                    OR (OLD.status IN ('pending', 'retryable_error') AND NEW.status = 'executing')
                    OR (OLD.status = 'executing' AND NEW.status IN ('retryable_error', 'compensation_required', 'reconciliation_required', 'completed', 'failed'))
                    OR (OLD.status = 'compensation_required' AND NEW.status IN ('compensating', 'reconciliation_required'))
                    OR (OLD.status = 'compensating' AND NEW.status IN ('compensation_required', 'reconciliation_required', 'cancelled', 'failed'))
                ) THEN
                    SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'invalid marketplace payout operation status transition';
                END IF;
            END;
            "#,
        )
        .await?;
    manager
        .get_connection()
        .execute_unprepared(
            r#"
            CREATE TRIGGER marketplace_payout_transfer_update_guard
            BEFORE UPDATE ON marketplace_payout_operation_transfers
            FOR EACH ROW
            BEGIN
                IF NOT (NEW.id <=> OLD.id)
                    OR NOT (NEW.tenant_id <=> OLD.tenant_id)
                    OR NOT (NEW.operation_id <=> OLD.operation_id)
                    OR NOT (NEW.sequence_no <=> OLD.sequence_no)
                    OR NOT (NEW.order_id <=> OLD.order_id)
                    OR NOT (NEW.transfer_kind <=> OLD.transfer_kind)
                    OR NOT (NEW.idempotency_key <=> OLD.idempotency_key)
                    OR NOT (NEW.request_hash <=> OLD.request_hash)
                    OR NOT (NEW.request_json <=> OLD.request_json)
                    OR NOT (NEW.total_amount <=> OLD.total_amount)
                THEN
                    SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'marketplace payout transfer identity is immutable';
                END IF;
                IF NOT (
                    OLD.status = NEW.status
                    OR (OLD.status IN ('pending', 'retryable_error') AND NEW.status = 'executing')
                    OR (OLD.status = 'executing' AND NEW.status IN ('posted', 'retryable_error', 'reconciliation_required', 'failed'))
                    OR (OLD.status = 'posted' AND NEW.status = 'compensated')
                ) THEN
                    SIGNAL SQLSTATE '45000' SET MESSAGE_TEXT = 'invalid marketplace payout transfer status transition';
                END IF;
            END;
            "#,
        )
        .await?;
    Ok(())
}

async fn uninstall_guards(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    match manager.get_database_backend() {
        DatabaseBackend::Postgres => {
            manager
                .get_connection()
                .execute_unprepared(
                    r#"
                    DROP TRIGGER IF EXISTS marketplace_payout_transfer_update_guard ON marketplace_payout_operation_transfers;
                    DROP TRIGGER IF EXISTS marketplace_payout_operation_update_guard ON marketplace_payout_operations;
                    DROP FUNCTION IF EXISTS enforce_marketplace_payout_transfer_update();
                    DROP FUNCTION IF EXISTS enforce_marketplace_payout_operation_update();
                    "#,
                )
                .await?;
        }
        DatabaseBackend::Sqlite | DatabaseBackend::MySql => {
            manager
                .get_connection()
                .execute_unprepared(
                    r#"
                    DROP TRIGGER IF EXISTS marketplace_payout_transfer_update_guard;
                    DROP TRIGGER IF EXISTS marketplace_payout_operation_update_guard;
                    "#,
                )
                .await?;
        }
    }
    Ok(())
}

#[derive(Iden)]
enum Payouts {
    Table,
    Id,
    TenantId,
}

#[derive(Iden)]
enum PayoutOperations {
    Table,
    Id,
    TenantId,
    ActorId,
    SellerId,
    CurrencyCode,
    IdempotencyKey,
    RequestHash,
    RequestJson,
    Status,
    Stage,
    PayoutId,
    AttemptCount,
    LeaseOwner,
    LeaseExpiresAt,
    LastErrorCode,
    CreatedAt,
    UpdatedAt,
    CompletedAt,
}

#[derive(Iden)]
enum PayoutOperationTransfers {
    Table,
    Id,
    TenantId,
    OperationId,
    SequenceNo,
    OrderId,
    TransferKind,
    Status,
    IdempotencyKey,
    RequestHash,
    RequestJson,
    TotalAmount,
    LedgerTransferId,
    LedgerTransactionId,
    AttemptCount,
    LastErrorCode,
    CreatedAt,
    UpdatedAt,
    CompletedAt,
}
