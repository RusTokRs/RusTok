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
                    .table(MarketplaceFinancialOperations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MarketplaceFinancialOperations::CheckoutOperationId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceFinancialOperations::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceFinancialOperations::OrderId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceFinancialOperations::PaymentCollectionId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceFinancialOperations::PlanHash)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceFinancialOperations::CurrencyCode)
                            .string_len(3)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceFinancialOperations::IdempotencyKey)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceFinancialOperations::RequestHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceFinancialOperations::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceFinancialOperations::Stage)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceFinancialOperations::AttemptCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(MarketplaceFinancialOperations::LeaseOwner)
                            .string_len(191),
                    )
                    .col(
                        ColumnDef::new(MarketplaceFinancialOperations::LeaseExpiresAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceFinancialOperations::LedgerTransactionId).uuid(),
                    )
                    .col(
                        ColumnDef::new(
                            MarketplaceFinancialOperations::LedgerDebitTotalAmount,
                        )
                        .big_integer(),
                    )
                    .col(
                        ColumnDef::new(
                            MarketplaceFinancialOperations::LedgerCreditTotalAmount,
                        )
                        .big_integer(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceFinancialOperations::LastErrorCode)
                            .string_len(100),
                    )
                    .col(
                        ColumnDef::new(MarketplaceFinancialOperations::LastErrorMessage)
                            .string_len(2000),
                    )
                    .col(
                        ColumnDef::new(MarketplaceFinancialOperations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MarketplaceFinancialOperations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MarketplaceFinancialOperations::CompletedAt)
                            .timestamp_with_time_zone(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .from(
                                MarketplaceFinancialOperations::Table,
                                MarketplaceFinancialOperations::CheckoutOperationId,
                            )
                            .to(CheckoutOperations::Table, CheckoutOperations::Id)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("ux_marketplace_financial_operations_order")
                    .table(MarketplaceFinancialOperations::Table)
                    .col(MarketplaceFinancialOperations::TenantId)
                    .col(MarketplaceFinancialOperations::OrderId)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_marketplace_financial_operations_recovery")
                    .table(MarketplaceFinancialOperations::Table)
                    .col(MarketplaceFinancialOperations::Status)
                    .col(MarketplaceFinancialOperations::LeaseExpiresAt)
                    .col(MarketplaceFinancialOperations::UpdatedAt)
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
                    .table(MarketplaceFinancialOperations::Table)
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
            ALTER TABLE marketplace_financial_operations
                ADD CONSTRAINT ck_marketplace_financial_operations_status
                CHECK (status IN ('pending', 'executing', 'retryable_error', 'operator_review', 'completed')),
                ADD CONSTRAINT ck_marketplace_financial_operations_stage
                CHECK (stage IN ('admitted', 'ledger_posted')),
                ADD CONSTRAINT ck_marketplace_financial_operations_identity
                CHECK (
                    btrim(plan_hash) <> ''
                    AND currency_code ~ '^[A-Z]{3}$'
                    AND btrim(idempotency_key) <> ''
                    AND request_hash ~ '^[0-9a-f]{64}$'
                    AND attempt_count >= 0
                ),
                ADD CONSTRAINT ck_marketplace_financial_operations_lease
                CHECK (
                    (status = 'executing' AND lease_owner IS NOT NULL AND btrim(lease_owner) <> '' AND lease_expires_at IS NOT NULL)
                    OR
                    (status <> 'executing' AND lease_owner IS NULL AND lease_expires_at IS NULL)
                ),
                ADD CONSTRAINT ck_marketplace_financial_operations_ledger
                CHECK (
                    (stage = 'admitted'
                        AND ledger_transaction_id IS NULL
                        AND ledger_debit_total_amount IS NULL
                        AND ledger_credit_total_amount IS NULL)
                    OR
                    (stage = 'ledger_posted'
                        AND ledger_transaction_id IS NOT NULL
                        AND ledger_debit_total_amount IS NOT NULL
                        AND ledger_credit_total_amount IS NOT NULL
                        AND ledger_debit_total_amount >= 0
                        AND ledger_debit_total_amount = ledger_credit_total_amount)
                ),
                ADD CONSTRAINT ck_marketplace_financial_operations_completion
                CHECK (
                    (status = 'completed' AND stage = 'ledger_posted' AND completed_at IS NOT NULL)
                    OR
                    (status <> 'completed' AND completed_at IS NULL)
                );

            CREATE OR REPLACE FUNCTION enforce_marketplace_financial_operation_integrity()
            RETURNS trigger AS $$
            BEGIN
                IF TG_OP = 'UPDATE' AND (
                    NEW.checkout_operation_id IS DISTINCT FROM OLD.checkout_operation_id
                    OR NEW.tenant_id IS DISTINCT FROM OLD.tenant_id
                    OR NEW.order_id IS DISTINCT FROM OLD.order_id
                    OR NEW.payment_collection_id IS DISTINCT FROM OLD.payment_collection_id
                    OR NEW.plan_hash IS DISTINCT FROM OLD.plan_hash
                    OR NEW.currency_code IS DISTINCT FROM OLD.currency_code
                    OR NEW.idempotency_key IS DISTINCT FROM OLD.idempotency_key
                    OR NEW.request_hash IS DISTINCT FROM OLD.request_hash
                ) THEN
                    RAISE EXCEPTION 'marketplace financial operation identity is immutable'
                        USING ERRCODE = '23514';
                END IF;
                IF TG_OP = 'UPDATE' AND OLD.stage = 'ledger_posted' AND (
                    NEW.stage IS DISTINCT FROM OLD.stage
                    OR NEW.ledger_transaction_id IS DISTINCT FROM OLD.ledger_transaction_id
                    OR NEW.ledger_debit_total_amount IS DISTINCT FROM OLD.ledger_debit_total_amount
                    OR NEW.ledger_credit_total_amount IS DISTINCT FROM OLD.ledger_credit_total_amount
                ) THEN
                    RAISE EXCEPTION 'marketplace ledger evidence is immutable'
                        USING ERRCODE = '23514';
                END IF;
                RETURN NEW;
            END;
            $$ LANGUAGE plpgsql;

            CREATE TRIGGER marketplace_financial_operations_integrity_guard
            BEFORE UPDATE ON marketplace_financial_operations
            FOR EACH ROW EXECUTE FUNCTION enforce_marketplace_financial_operation_integrity();
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
            CREATE TRIGGER marketplace_financial_operations_guard_insert
            BEFORE INSERT ON marketplace_financial_operations
            FOR EACH ROW BEGIN
                SELECT CASE WHEN NEW.status NOT IN ('pending', 'executing', 'retryable_error', 'operator_review', 'completed')
                    THEN RAISE(ABORT, 'invalid marketplace financial operation status') END;
                SELECT CASE WHEN NEW.stage NOT IN ('admitted', 'ledger_posted')
                    THEN RAISE(ABORT, 'invalid marketplace financial operation stage') END;
                SELECT CASE WHEN trim(NEW.plan_hash) = '' OR length(NEW.currency_code) <> 3
                    OR NEW.currency_code <> upper(NEW.currency_code)
                    OR trim(NEW.idempotency_key) = '' OR length(NEW.request_hash) <> 64
                    OR NEW.attempt_count < 0
                    THEN RAISE(ABORT, 'invalid marketplace financial operation identity') END;
                SELECT CASE WHEN NOT (
                    (NEW.status = 'executing' AND NEW.lease_owner IS NOT NULL AND trim(NEW.lease_owner) <> '' AND NEW.lease_expires_at IS NOT NULL)
                    OR
                    (NEW.status <> 'executing' AND NEW.lease_owner IS NULL AND NEW.lease_expires_at IS NULL)
                ) THEN RAISE(ABORT, 'invalid marketplace financial operation lease') END;
                SELECT CASE WHEN NOT (
                    (NEW.stage = 'admitted' AND NEW.ledger_transaction_id IS NULL AND NEW.ledger_debit_total_amount IS NULL AND NEW.ledger_credit_total_amount IS NULL)
                    OR
                    (NEW.stage = 'ledger_posted' AND NEW.ledger_transaction_id IS NOT NULL
                        AND NEW.ledger_debit_total_amount IS NOT NULL AND NEW.ledger_credit_total_amount IS NOT NULL
                        AND NEW.ledger_debit_total_amount >= 0
                        AND NEW.ledger_debit_total_amount = NEW.ledger_credit_total_amount)
                ) THEN RAISE(ABORT, 'invalid marketplace ledger evidence') END;
                SELECT CASE WHEN NOT (
                    (NEW.status = 'completed' AND NEW.stage = 'ledger_posted' AND NEW.completed_at IS NOT NULL)
                    OR
                    (NEW.status <> 'completed' AND NEW.completed_at IS NULL)
                ) THEN RAISE(ABORT, 'invalid marketplace financial operation completion') END;
            END;

            CREATE TRIGGER marketplace_financial_operations_guard_update
            BEFORE UPDATE ON marketplace_financial_operations
            FOR EACH ROW BEGIN
                SELECT CASE WHEN NEW.checkout_operation_id IS NOT OLD.checkout_operation_id
                    OR NEW.tenant_id IS NOT OLD.tenant_id
                    OR NEW.order_id IS NOT OLD.order_id
                    OR NEW.payment_collection_id IS NOT OLD.payment_collection_id
                    OR NEW.plan_hash IS NOT OLD.plan_hash
                    OR NEW.currency_code IS NOT OLD.currency_code
                    OR NEW.idempotency_key IS NOT OLD.idempotency_key
                    OR NEW.request_hash IS NOT OLD.request_hash
                    THEN RAISE(ABORT, 'marketplace financial operation identity is immutable') END;
                SELECT CASE WHEN OLD.stage = 'ledger_posted' AND (
                    NEW.stage IS NOT OLD.stage
                    OR NEW.ledger_transaction_id IS NOT OLD.ledger_transaction_id
                    OR NEW.ledger_debit_total_amount IS NOT OLD.ledger_debit_total_amount
                    OR NEW.ledger_credit_total_amount IS NOT OLD.ledger_credit_total_amount)
                    THEN RAISE(ABORT, 'marketplace ledger evidence is immutable') END;
                SELECT CASE WHEN NEW.status NOT IN ('pending', 'executing', 'retryable_error', 'operator_review', 'completed')
                    THEN RAISE(ABORT, 'invalid marketplace financial operation status') END;
                SELECT CASE WHEN NOT (
                    (NEW.status = 'executing' AND NEW.lease_owner IS NOT NULL AND trim(NEW.lease_owner) <> '' AND NEW.lease_expires_at IS NOT NULL)
                    OR
                    (NEW.status <> 'executing' AND NEW.lease_owner IS NULL AND NEW.lease_expires_at IS NULL)
                ) THEN RAISE(ABORT, 'invalid marketplace financial operation lease') END;
                SELECT CASE WHEN NOT (
                    (NEW.stage = 'admitted' AND NEW.ledger_transaction_id IS NULL AND NEW.ledger_debit_total_amount IS NULL AND NEW.ledger_credit_total_amount IS NULL)
                    OR
                    (NEW.stage = 'ledger_posted' AND NEW.ledger_transaction_id IS NOT NULL
                        AND NEW.ledger_debit_total_amount IS NOT NULL AND NEW.ledger_credit_total_amount IS NOT NULL
                        AND NEW.ledger_debit_total_amount >= 0
                        AND NEW.ledger_debit_total_amount = NEW.ledger_credit_total_amount)
                ) THEN RAISE(ABORT, 'invalid marketplace ledger evidence') END;
                SELECT CASE WHEN NOT (
                    (NEW.status = 'completed' AND NEW.stage = 'ledger_posted' AND NEW.completed_at IS NOT NULL)
                    OR
                    (NEW.status <> 'completed' AND NEW.completed_at IS NULL)
                ) THEN RAISE(ABORT, 'invalid marketplace financial operation completion') END;
            END;
            "#,
        )
        .await?;
    Ok(())
}

#[derive(DeriveIden)]
enum MarketplaceFinancialOperations {
    Table,
    CheckoutOperationId,
    TenantId,
    OrderId,
    PaymentCollectionId,
    PlanHash,
    CurrencyCode,
    IdempotencyKey,
    RequestHash,
    Status,
    Stage,
    AttemptCount,
    LeaseOwner,
    LeaseExpiresAt,
    LedgerTransactionId,
    LedgerDebitTotalAmount,
    LedgerCreditTotalAmount,
    LastErrorCode,
    LastErrorMessage,
    CreatedAt,
    UpdatedAt,
    CompletedAt,
}

#[derive(DeriveIden)]
enum CheckoutOperations {
    Table,
    Id,
}
