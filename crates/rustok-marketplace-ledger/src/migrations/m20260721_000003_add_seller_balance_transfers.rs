use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(EntryBalanceBuckets::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(EntryBalanceBuckets::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(EntryBalanceBuckets::TenantId).uuid().not_null())
                    .col(ColumnDef::new(EntryBalanceBuckets::EntryId).uuid().not_null())
                    .col(ColumnDef::new(EntryBalanceBuckets::SellerId).uuid().not_null())
                    .col(
                        ColumnDef::new(EntryBalanceBuckets::BalanceBucket)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(EntryBalanceBuckets::SourceKind)
                            .string_len(80)
                            .not_null(),
                    )
                    .col(ColumnDef::new(EntryBalanceBuckets::SourceId).uuid().not_null())
                    .col(
                        ColumnDef::new(EntryBalanceBuckets::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_mkt_entry_bucket_tenant_entry")
                            .from(EntryBalanceBuckets::Table, EntryBalanceBuckets::TenantId)
                            .from_col(EntryBalanceBuckets::EntryId)
                            .to(LedgerEntries::Table, LedgerEntries::TenantId)
                            .to_col(LedgerEntries::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .check(Expr::cust(
                        "balance_bucket IN ('pending', 'available', 'reserved', 'paid')",
                    ))
                    .check(Expr::cust("TRIM(source_kind) <> ''"))
                    .to_owned(),
            )
            .await?;
        for index in [
            Index::create()
                .name("uq_mkt_entry_balance_bucket_entry")
                .table(EntryBalanceBuckets::Table)
                .col(EntryBalanceBuckets::TenantId)
                .col(EntryBalanceBuckets::EntryId)
                .unique()
                .to_owned(),
            Index::create()
                .name("idx_mkt_entry_balance_bucket_seller")
                .table(EntryBalanceBuckets::Table)
                .col(EntryBalanceBuckets::TenantId)
                .col(EntryBalanceBuckets::SellerId)
                .col(EntryBalanceBuckets::BalanceBucket)
                .col(EntryBalanceBuckets::CreatedAt)
                .to_owned(),
            Index::create()
                .name("idx_mkt_entry_balance_bucket_source")
                .table(EntryBalanceBuckets::Table)
                .col(EntryBalanceBuckets::TenantId)
                .col(EntryBalanceBuckets::SourceKind)
                .col(EntryBalanceBuckets::SourceId)
                .to_owned(),
        ] {
            manager.create_index(index).await?;
        }

        manager
            .create_table(
                Table::create()
                    .table(SellerBalanceTransfers::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SellerBalanceTransfers::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(SellerBalanceTransfers::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(SellerBalanceTransfers::TransactionId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SellerBalanceTransfers::TransferKind)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(ColumnDef::new(SellerBalanceTransfers::SourceId).uuid().not_null())
                    .col(ColumnDef::new(SellerBalanceTransfers::SellerId).uuid().not_null())
                    .col(
                        ColumnDef::new(SellerBalanceTransfers::CurrencyCode)
                            .string_len(3)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SellerBalanceTransfers::FromBucket)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SellerBalanceTransfers::ToBucket)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SellerBalanceTransfers::TotalAmount)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SellerBalanceTransfers::TransferredAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SellerBalanceTransfers::Metadata)
                            .json_binary()
                            .not_null()
                            .default("{}"),
                    )
                    .col(
                        ColumnDef::new(SellerBalanceTransfers::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_mkt_balance_transfer_tenant_transaction")
                            .from(SellerBalanceTransfers::Table, SellerBalanceTransfers::TenantId)
                            .from_col(SellerBalanceTransfers::TransactionId)
                            .to(LedgerTransactions::Table, LedgerTransactions::TenantId)
                            .to_col(LedgerTransactions::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .check(Expr::cust(
                        "transfer_kind IN ('pending_release', 'reserve_hold', 'reserve_release', 'payout_settlement', 'payout_reversal')",
                    ))
                    .check(Expr::cust("currency_code = UPPER(currency_code)"))
                    .check(Expr::cust("total_amount > 0"))
                    .check(Expr::cust(
                        "(transfer_kind = 'pending_release' AND from_bucket = 'pending' AND to_bucket = 'available') OR (transfer_kind = 'reserve_hold' AND from_bucket = 'available' AND to_bucket = 'reserved') OR (transfer_kind = 'reserve_release' AND from_bucket = 'reserved' AND to_bucket = 'available') OR (transfer_kind = 'payout_settlement' AND from_bucket = 'reserved' AND to_bucket = 'paid') OR (transfer_kind = 'payout_reversal' AND from_bucket = 'paid' AND to_bucket = 'available')",
                    ))
                    .to_owned(),
            )
            .await?;
        for index in [
            Index::create()
                .name("uq_mkt_balance_transfer_tenant_id")
                .table(SellerBalanceTransfers::Table)
                .col(SellerBalanceTransfers::TenantId)
                .col(SellerBalanceTransfers::Id)
                .unique()
                .to_owned(),
            Index::create()
                .name("uq_mkt_balance_transfer_transaction")
                .table(SellerBalanceTransfers::Table)
                .col(SellerBalanceTransfers::TenantId)
                .col(SellerBalanceTransfers::TransactionId)
                .unique()
                .to_owned(),
            Index::create()
                .name("uq_mkt_balance_transfer_source")
                .table(SellerBalanceTransfers::Table)
                .col(SellerBalanceTransfers::TenantId)
                .col(SellerBalanceTransfers::TransferKind)
                .col(SellerBalanceTransfers::SourceId)
                .unique()
                .to_owned(),
            Index::create()
                .name("idx_mkt_balance_transfer_seller")
                .table(SellerBalanceTransfers::Table)
                .col(SellerBalanceTransfers::TenantId)
                .col(SellerBalanceTransfers::SellerId)
                .col(SellerBalanceTransfers::CurrencyCode)
                .col(SellerBalanceTransfers::CreatedAt)
                .to_owned(),
        ] {
            manager.create_index(index).await?;
        }

        manager
            .create_table(
                Table::create()
                    .table(SellerBalanceTransferLines::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SellerBalanceTransferLines::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SellerBalanceTransferLines::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SellerBalanceTransferLines::TransferId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SellerBalanceTransferLines::ReferenceEntryId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SellerBalanceTransferLines::DebitEntryId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SellerBalanceTransferLines::CreditEntryId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SellerBalanceTransferLines::Amount)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SellerBalanceTransferLines::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_mkt_balance_transfer_line_tenant_transfer")
                            .from(
                                SellerBalanceTransferLines::Table,
                                SellerBalanceTransferLines::TenantId,
                            )
                            .from_col(SellerBalanceTransferLines::TransferId)
                            .to(SellerBalanceTransfers::Table, SellerBalanceTransfers::TenantId)
                            .to_col(SellerBalanceTransfers::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_mkt_balance_transfer_line_tenant_reference")
                            .from(
                                SellerBalanceTransferLines::Table,
                                SellerBalanceTransferLines::TenantId,
                            )
                            .from_col(SellerBalanceTransferLines::ReferenceEntryId)
                            .to(LedgerEntries::Table, LedgerEntries::TenantId)
                            .to_col(LedgerEntries::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_mkt_balance_transfer_line_tenant_debit")
                            .from(
                                SellerBalanceTransferLines::Table,
                                SellerBalanceTransferLines::TenantId,
                            )
                            .from_col(SellerBalanceTransferLines::DebitEntryId)
                            .to(LedgerEntries::Table, LedgerEntries::TenantId)
                            .to_col(LedgerEntries::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_mkt_balance_transfer_line_tenant_credit")
                            .from(
                                SellerBalanceTransferLines::Table,
                                SellerBalanceTransferLines::TenantId,
                            )
                            .from_col(SellerBalanceTransferLines::CreditEntryId)
                            .to(LedgerEntries::Table, LedgerEntries::TenantId)
                            .to_col(LedgerEntries::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .check(Expr::cust("amount > 0"))
                    .check(Expr::cust("debit_entry_id <> credit_entry_id"))
                    .to_owned(),
            )
            .await?;
        for index in [
            Index::create()
                .name("uq_mkt_balance_transfer_line_reference")
                .table(SellerBalanceTransferLines::Table)
                .col(SellerBalanceTransferLines::TenantId)
                .col(SellerBalanceTransferLines::TransferId)
                .col(SellerBalanceTransferLines::ReferenceEntryId)
                .unique()
                .to_owned(),
            Index::create()
                .name("uq_mkt_balance_transfer_line_debit")
                .table(SellerBalanceTransferLines::Table)
                .col(SellerBalanceTransferLines::TenantId)
                .col(SellerBalanceTransferLines::DebitEntryId)
                .unique()
                .to_owned(),
            Index::create()
                .name("uq_mkt_balance_transfer_line_credit")
                .table(SellerBalanceTransferLines::Table)
                .col(SellerBalanceTransferLines::TenantId)
                .col(SellerBalanceTransferLines::CreditEntryId)
                .unique()
                .to_owned(),
        ] {
            manager.create_index(index).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        for table in [
            SellerBalanceTransferLines::Table.into_iden(),
            SellerBalanceTransfers::Table.into_iden(),
            EntryBalanceBuckets::Table.into_iden(),
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
                     WHERE source_kind IN (
                         'seller_balance_pending_release',
                         'seller_balance_reserve_hold',
                         'seller_balance_reserve_release',
                         'seller_balance_payout_settlement',
                         'seller_balance_payout_reversal'
                     )
                 )",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "DELETE FROM marketplace_ledger_transactions
                 WHERE source_kind IN (
                     'seller_balance_pending_release',
                     'seller_balance_reserve_hold',
                     'seller_balance_reserve_release',
                     'seller_balance_payout_settlement',
                     'seller_balance_payout_reversal'
                 )",
            )
            .await?;
        manager
            .get_connection()
            .execute_unprepared(
                "DELETE FROM marketplace_ledger_receipts
                 WHERE command_kind = 'post_seller_balance_transfer'",
            )
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum LedgerTransactions {
    Table,
    Id,
    TenantId,
}

#[derive(Iden)]
enum LedgerEntries {
    Table,
    Id,
    TenantId,
}

#[derive(Iden)]
enum EntryBalanceBuckets {
    Table,
    Id,
    TenantId,
    EntryId,
    SellerId,
    BalanceBucket,
    SourceKind,
    SourceId,
    CreatedAt,
}

#[derive(Iden)]
enum SellerBalanceTransfers {
    Table,
    Id,
    TenantId,
    TransactionId,
    TransferKind,
    SourceId,
    SellerId,
    CurrencyCode,
    FromBucket,
    ToBucket,
    TotalAmount,
    TransferredAt,
    Metadata,
    CreatedAt,
}

#[derive(Iden)]
enum SellerBalanceTransferLines {
    Table,
    Id,
    TenantId,
    TransferId,
    ReferenceEntryId,
    DebitEntryId,
    CreditEntryId,
    Amount,
    CreatedAt,
}
