use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_index(
                Index::drop()
                    .name("uq_marketplace_ledger_entry_assessment_account")
                    .table(LedgerEntries::Table)
                    .to_owned(),
            )
            .await?;
        for index in [
            Index::create()
                .name("uq_mkt_ledger_entry_tx_assessment_account_dir")
                .table(LedgerEntries::Table)
                .col(LedgerEntries::TenantId)
                .col(LedgerEntries::TransactionId)
                .col(LedgerEntries::AssessmentId)
                .col(LedgerEntries::AccountCode)
                .col(LedgerEntries::Direction)
                .unique()
                .to_owned(),
            Index::create()
                .name("uq_marketplace_ledger_transaction_tenant_id")
                .table(LedgerTransactions::Table)
                .col(LedgerTransactions::TenantId)
                .col(LedgerTransactions::Id)
                .unique()
                .to_owned(),
            Index::create()
                .name("uq_marketplace_ledger_entry_tenant_id")
                .table(LedgerEntries::Table)
                .col(LedgerEntries::TenantId)
                .col(LedgerEntries::Id)
                .unique()
                .to_owned(),
        ] {
            manager.create_index(index).await?;
        }

        manager.create_table(reversals_table()).await?;
        for index in [
            Index::create()
                .name("uq_marketplace_ledger_reversal_transaction")
                .table(LedgerReversals::Table)
                .col(LedgerReversals::TenantId)
                .col(LedgerReversals::TransactionId)
                .unique()
                .to_owned(),
            Index::create()
                .name("uq_marketplace_ledger_reversal_source")
                .table(LedgerReversals::Table)
                .col(LedgerReversals::TenantId)
                .col(LedgerReversals::ReversalKind)
                .col(LedgerReversals::SourceId)
                .unique()
                .to_owned(),
            Index::create()
                .name("idx_marketplace_ledger_reversal_original")
                .table(LedgerReversals::Table)
                .col(LedgerReversals::TenantId)
                .col(LedgerReversals::ReversedTransactionId)
                .col(LedgerReversals::CreatedAt)
                .to_owned(),
        ] {
            manager.create_index(index).await?;
        }

        manager.create_table(reversal_lines_table()).await?;
        for index in [
            Index::create()
                .name("uq_marketplace_ledger_reversal_line_entry")
                .table(LedgerReversalLines::Table)
                .col(LedgerReversalLines::TenantId)
                .col(LedgerReversalLines::EntryId)
                .unique()
                .to_owned(),
            Index::create()
                .name("idx_marketplace_ledger_reversal_line_original")
                .table(LedgerReversalLines::Table)
                .col(LedgerReversalLines::TenantId)
                .col(LedgerReversalLines::ReversedEntryId)
                .col(LedgerReversalLines::CreatedAt)
                .to_owned(),
            Index::create()
                .name("idx_marketplace_ledger_reversal_line_seller")
                .table(LedgerReversalLines::Table)
                .col(LedgerReversalLines::TenantId)
                .col(LedgerReversalLines::SellerId)
                .col(LedgerReversalLines::SellerBalanceBucket)
                .col(LedgerReversalLines::CreatedAt)
                .to_owned(),
        ] {
            manager.create_index(index).await?;
        }

        manager.create_table(seller_balances_table()).await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_marketplace_seller_balance_scope")
                    .table(SellerBalanceProjections::Table)
                    .col(SellerBalanceProjections::TenantId)
                    .col(SellerBalanceProjections::SellerId)
                    .col(SellerBalanceProjections::CurrencyCode)
                    .unique()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in [
            SellerBalanceProjections::Table.into_iden(),
            LedgerReversalLines::Table.into_iden(),
            LedgerReversals::Table.into_iden(),
        ] {
            manager
                .drop_table(Table::drop().table(table).if_exists().to_owned())
                .await?;
        }
        manager
            .get_connection()
            .execute_unprepared(
                "DELETE FROM marketplace_ledger_entries
                 WHERE transaction_id IN (
                     SELECT id FROM marketplace_ledger_transactions
                     WHERE source_kind IN ('refund_reversal', 'chargeback_reversal')
                 )",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "DELETE FROM marketplace_ledger_transactions
                 WHERE source_kind IN ('refund_reversal', 'chargeback_reversal')",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "DELETE FROM marketplace_ledger_receipts
                 WHERE command_kind = 'post_financial_reversal'",
            )
            .await?;
        for index_name in [
            "uq_mkt_ledger_entry_tx_assessment_account_dir",
            "uq_marketplace_ledger_entry_tenant_id",
            "uq_marketplace_ledger_transaction_tenant_id",
        ] {
            manager
                .drop_index(Index::drop().name(index_name).to_owned())
                .await?;
        }
        manager
            .create_index(
                Index::create()
                    .name("uq_marketplace_ledger_entry_assessment_account")
                    .table(LedgerEntries::Table)
                    .col(LedgerEntries::TenantId)
                    .col(LedgerEntries::AssessmentId)
                    .col(LedgerEntries::AccountCode)
                    .unique()
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

fn reversals_table() -> TableCreateStatement {
    Table::create()
        .table(LedgerReversals::Table)
        .if_not_exists()
        .col(
            ColumnDef::new(LedgerReversals::Id)
                .uuid()
                .not_null()
                .primary_key(),
        )
        .col(ColumnDef::new(LedgerReversals::TenantId).uuid().not_null())
        .col(
            ColumnDef::new(LedgerReversals::TransactionId)
                .uuid()
                .not_null(),
        )
        .col(
            ColumnDef::new(LedgerReversals::ReversedTransactionId)
                .uuid()
                .not_null(),
        )
        .col(
            ColumnDef::new(LedgerReversals::ReversalKind)
                .string_len(32)
                .not_null(),
        )
        .col(ColumnDef::new(LedgerReversals::SourceId).uuid().not_null())
        .col(ColumnDef::new(LedgerReversals::OrderId).uuid().not_null())
        .col(
            ColumnDef::new(LedgerReversals::CurrencyCode)
                .string_len(3)
                .not_null(),
        )
        .col(
            ColumnDef::new(LedgerReversals::TotalAmount)
                .big_integer()
                .not_null(),
        )
        .col(
            ColumnDef::new(LedgerReversals::ReversedAt)
                .timestamp_with_time_zone()
                .not_null(),
        )
        .col(
            ColumnDef::new(LedgerReversals::Metadata)
                .json_binary()
                .not_null()
                .default("{}"),
        )
        .col(
            ColumnDef::new(LedgerReversals::CreatedAt)
                .timestamp_with_time_zone()
                .not_null()
                .default(Expr::current_timestamp()),
        )
        .foreign_key(
            ForeignKey::create()
                .name("fk_marketplace_ledger_reversal_tenant_transaction")
                .from(LedgerReversals::Table, LedgerReversals::TenantId)
                .from_col(LedgerReversals::TransactionId)
                .to(LedgerTransactions::Table, LedgerTransactions::TenantId)
                .to_col(LedgerTransactions::Id)
                .on_update(ForeignKeyAction::Cascade)
                .on_delete(ForeignKeyAction::Restrict),
        )
        .foreign_key(
            ForeignKey::create()
                .name("fk_marketplace_ledger_reversal_tenant_original")
                .from(LedgerReversals::Table, LedgerReversals::TenantId)
                .from_col(LedgerReversals::ReversedTransactionId)
                .to(LedgerTransactions::Table, LedgerTransactions::TenantId)
                .to_col(LedgerTransactions::Id)
                .on_update(ForeignKeyAction::Cascade)
                .on_delete(ForeignKeyAction::Restrict),
        )
        .check(Expr::cust("reversal_kind IN ('refund', 'chargeback')"))
        .check(Expr::cust("total_amount > 0"))
        .to_owned()
}

fn reversal_lines_table() -> TableCreateStatement {
    Table::create()
        .table(LedgerReversalLines::Table)
        .if_not_exists()
        .col(ColumnDef::new(LedgerReversalLines::Id).uuid().not_null().primary_key())
        .col(ColumnDef::new(LedgerReversalLines::TenantId).uuid().not_null())
        .col(ColumnDef::new(LedgerReversalLines::ReversalId).uuid().not_null())
        .col(ColumnDef::new(LedgerReversalLines::EntryId).uuid().not_null())
        .col(ColumnDef::new(LedgerReversalLines::ReversedEntryId).uuid().not_null())
        .col(ColumnDef::new(LedgerReversalLines::SellerId).uuid())
        .col(ColumnDef::new(LedgerReversalLines::AssessmentId).uuid().not_null())
        .col(ColumnDef::new(LedgerReversalLines::AllocationId).uuid().not_null())
        .col(ColumnDef::new(LedgerReversalLines::OrderLineItemId).uuid().not_null())
        .col(ColumnDef::new(LedgerReversalLines::AccountCode).string_len(80).not_null())
        .col(ColumnDef::new(LedgerReversalLines::Direction).string_len(16).not_null())
        .col(ColumnDef::new(LedgerReversalLines::SellerBalanceBucket).string_len(32))
        .col(ColumnDef::new(LedgerReversalLines::Amount).big_integer().not_null())
        .col(
            ColumnDef::new(LedgerReversalLines::CreatedAt)
                .timestamp_with_time_zone()
                .not_null()
                .default(Expr::current_timestamp()),
        )
        .foreign_key(
            ForeignKey::create()
                .name("fk_marketplace_ledger_reversal_line_tenant_reversal")
                .from(LedgerReversalLines::Table, LedgerReversalLines::TenantId)
                .from_col(LedgerReversalLines::ReversalId)
                .to(LedgerReversals::Table, LedgerReversals::TenantId)
                .to_col(LedgerReversals::Id)
                .on_update(ForeignKeyAction::Cascade)
                .on_delete(ForeignKeyAction::Cascade),
        )
        .foreign_key(
            ForeignKey::create()
                .name("fk_marketplace_ledger_reversal_line_tenant_entry")
                .from(LedgerReversalLines::Table, LedgerReversalLines::TenantId)
                .from_col(LedgerReversalLines::EntryId)
                .to(LedgerEntries::Table, LedgerEntries::TenantId)
                .to_col(LedgerEntries::Id)
                .on_update(ForeignKeyAction::Cascade)
                .on_delete(ForeignKeyAction::Restrict),
        )
        .foreign_key(
            ForeignKey::create()
                .name("fk_marketplace_ledger_reversal_line_tenant_original_entry")
                .from(LedgerReversalLines::Table, LedgerReversalLines::TenantId)
                .from_col(LedgerReversalLines::ReversedEntryId)
                .to(LedgerEntries::Table, LedgerEntries::TenantId)
                .to_col(LedgerEntries::Id)
                .on_update(ForeignKeyAction::Cascade)
                .on_delete(ForeignKeyAction::Restrict),
        )
        .check(Expr::cust("amount > 0"))
        .check(Expr::cust(
            "seller_balance_bucket IS NULL OR seller_balance_bucket IN ('pending', 'available', 'reserved', 'paid')",
        ))
        .check(Expr::cust(
            "(account_code = 'seller_payable' AND direction = 'debit' AND seller_id IS NOT NULL AND seller_balance_bucket IS NOT NULL) OR (account_code = 'platform_commission_revenue' AND direction = 'debit' AND seller_id IS NULL AND seller_balance_bucket IS NULL) OR (account_code = 'marketplace_clearing' AND direction = 'credit' AND seller_id IS NULL AND seller_balance_bucket IS NULL)",
        ))
        .to_owned()
}

fn seller_balances_table() -> TableCreateStatement {
    Table::create()
        .table(SellerBalanceProjections::Table)
        .if_not_exists()
        .col(
            ColumnDef::new(SellerBalanceProjections::Id)
                .uuid()
                .not_null()
                .primary_key(),
        )
        .col(
            ColumnDef::new(SellerBalanceProjections::TenantId)
                .uuid()
                .not_null(),
        )
        .col(
            ColumnDef::new(SellerBalanceProjections::SellerId)
                .uuid()
                .not_null(),
        )
        .col(
            ColumnDef::new(SellerBalanceProjections::CurrencyCode)
                .string_len(3)
                .not_null(),
        )
        .col(
            ColumnDef::new(SellerBalanceProjections::PendingAmount)
                .big_integer()
                .not_null()
                .default(0),
        )
        .col(
            ColumnDef::new(SellerBalanceProjections::AvailableAmount)
                .big_integer()
                .not_null()
                .default(0),
        )
        .col(
            ColumnDef::new(SellerBalanceProjections::ReservedAmount)
                .big_integer()
                .not_null()
                .default(0),
        )
        .col(
            ColumnDef::new(SellerBalanceProjections::PaidAmount)
                .big_integer()
                .not_null()
                .default(0),
        )
        .col(
            ColumnDef::new(SellerBalanceProjections::NegativeAmount)
                .big_integer()
                .not_null()
                .default(0),
        )
        .col(
            ColumnDef::new(SellerBalanceProjections::SourceEntryCount)
                .big_integer()
                .not_null()
                .default(0),
        )
        .col(ColumnDef::new(SellerBalanceProjections::LastEntryId).uuid())
        .col(
            ColumnDef::new(SellerBalanceProjections::LastEntryCreatedAt).timestamp_with_time_zone(),
        )
        .col(
            ColumnDef::new(SellerBalanceProjections::RebuiltAt)
                .timestamp_with_time_zone()
                .not_null()
                .default(Expr::current_timestamp()),
        )
        .col(
            ColumnDef::new(SellerBalanceProjections::UpdatedAt)
                .timestamp_with_time_zone()
                .not_null()
                .default(Expr::current_timestamp()),
        )
        .check(Expr::cust("negative_amount >= 0"))
        .check(Expr::cust("source_entry_count >= 0"))
        .to_owned()
}

#[derive(Iden)]
enum LedgerEntries {
    Table,
    Id,
    TenantId,
    TransactionId,
    AssessmentId,
    AccountCode,
    Direction,
}

#[derive(Iden)]
enum LedgerTransactions {
    Table,
    Id,
    TenantId,
}

#[derive(Iden)]
enum LedgerReversals {
    Table,
    Id,
    TenantId,
    TransactionId,
    ReversedTransactionId,
    ReversalKind,
    SourceId,
    OrderId,
    CurrencyCode,
    TotalAmount,
    ReversedAt,
    Metadata,
    CreatedAt,
}

#[derive(Iden)]
enum LedgerReversalLines {
    Table,
    Id,
    TenantId,
    ReversalId,
    EntryId,
    ReversedEntryId,
    SellerId,
    AssessmentId,
    AllocationId,
    OrderLineItemId,
    AccountCode,
    Direction,
    SellerBalanceBucket,
    Amount,
    CreatedAt,
}

#[derive(Iden)]
enum SellerBalanceProjections {
    Table,
    Id,
    TenantId,
    SellerId,
    CurrencyCode,
    PendingAmount,
    AvailableAmount,
    ReservedAmount,
    PaidAmount,
    NegativeAmount,
    SourceEntryCount,
    LastEntryId,
    LastEntryCreatedAt,
    RebuiltAt,
    UpdatedAt,
}
