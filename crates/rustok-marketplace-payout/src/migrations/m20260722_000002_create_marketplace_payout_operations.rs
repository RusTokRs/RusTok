use sea_orm_migration::prelude::*;
use sea_orm_migration::sea_orm::DatabaseBackend;

#[derive(DeriveMigrationName)]
pub struct Migration;

const LEGACY_PAYOUTS: &str = "payouts";
const LEGACY_ITEMS: &str = "payout_items";
const LEGACY_RECEIPTS: &str = "payout_receipts";
const CANONICAL_PAYOUTS: &str = "marketplace_payouts";
const CANONICAL_ITEMS: &str = "marketplace_payout_items";
const CANONICAL_RECEIPTS: &str = "marketplace_payout_receipts";
const REPAIR_MARKER: &str = "marketplace_payout_legacy_name_repair_marker";

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        repair_legacy_table_names(manager).await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_marketplace_payouts_tenant_id")
                    .table(MarketplacePayouts::Table)
                    .col(MarketplacePayouts::TenantId)
                    .col(MarketplacePayouts::Id)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(MarketplacePayoutOperations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MarketplacePayoutOperations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperations::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperations::ActorId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperations::SellerId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperations::CurrencyCode)
                            .string_len(3)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperations::IdempotencyKey)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperations::RequestHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperations::RequestJson)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperations::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperations::Stage)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(ColumnDef::new(MarketplacePayoutOperations::PayoutId).uuid())
                    .col(
                        ColumnDef::new(MarketplacePayoutOperations::AttemptCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperations::Revision)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperations::LeaseOwner)
                            .string_len(191),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperations::LeaseExpiresAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperations::LastErrorCode)
                            .string_len(120),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperations::CompletedAt)
                            .timestamp_with_time_zone(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_marketplace_payout_operation_tenant_payout")
                            .from(
                                MarketplacePayoutOperations::Table,
                                MarketplacePayoutOperations::TenantId,
                            )
                            .from_col(MarketplacePayoutOperations::PayoutId)
                            .to(MarketplacePayouts::Table, MarketplacePayouts::TenantId)
                            .to_col(MarketplacePayouts::Id)
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
                    .check(Expr::cust("revision >= 0"))
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
                .table(MarketplacePayoutOperations::Table)
                .col(MarketplacePayoutOperations::TenantId)
                .col(MarketplacePayoutOperations::Id)
                .unique()
                .to_owned(),
            Index::create()
                .name("uq_marketplace_payout_operation_key")
                .table(MarketplacePayoutOperations::Table)
                .col(MarketplacePayoutOperations::TenantId)
                .col(MarketplacePayoutOperations::IdempotencyKey)
                .unique()
                .to_owned(),
            Index::create()
                .name("uq_marketplace_payout_operation_payout")
                .table(MarketplacePayoutOperations::Table)
                .col(MarketplacePayoutOperations::TenantId)
                .col(MarketplacePayoutOperations::PayoutId)
                .unique()
                .to_owned(),
            Index::create()
                .name("idx_marketplace_payout_operation_recovery")
                .table(MarketplacePayoutOperations::Table)
                .col(MarketplacePayoutOperations::Status)
                .col(MarketplacePayoutOperations::LeaseExpiresAt)
                .col(MarketplacePayoutOperations::UpdatedAt)
                .to_owned(),
            Index::create()
                .name("idx_marketplace_payout_operation_seller")
                .table(MarketplacePayoutOperations::Table)
                .col(MarketplacePayoutOperations::TenantId)
                .col(MarketplacePayoutOperations::SellerId)
                .col(MarketplacePayoutOperations::CurrencyCode)
                .col(MarketplacePayoutOperations::CreatedAt)
                .to_owned(),
        ] {
            manager.create_index(index).await?;
        }

        manager
            .create_table(
                Table::create()
                    .table(MarketplacePayoutOperationTransfers::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MarketplacePayoutOperationTransfers::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperationTransfers::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperationTransfers::OperationId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperationTransfers::SequenceNo)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperationTransfers::OrderId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperationTransfers::TransferKind)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperationTransfers::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperationTransfers::IdempotencyKey)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperationTransfers::RequestHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperationTransfers::RequestJson)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperationTransfers::TotalAmount)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperationTransfers::LedgerTransferId)
                            .uuid(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperationTransfers::LedgerTransactionId)
                            .uuid(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperationTransfers::AttemptCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperationTransfers::Revision)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperationTransfers::LastErrorCode)
                            .string_len(120),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperationTransfers::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperationTransfers::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutOperationTransfers::CompletedAt)
                            .timestamp_with_time_zone(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_mkt_payout_transfer_tenant_operation")
                            .from(
                                MarketplacePayoutOperationTransfers::Table,
                                MarketplacePayoutOperationTransfers::TenantId,
                            )
                            .from_col(MarketplacePayoutOperationTransfers::OperationId)
                            .to(
                                MarketplacePayoutOperations::Table,
                                MarketplacePayoutOperations::TenantId,
                            )
                            .to_col(MarketplacePayoutOperations::Id)
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
                    .check(Expr::cust("revision >= 0"))
                    .check(Expr::cust(
                        "((status IN ('posted', 'compensated', 'failed') AND completed_at IS NOT NULL) OR (status NOT IN ('posted', 'compensated', 'failed') AND completed_at IS NULL))",
                    ))
                    .check(Expr::cust(
                        "((status IN ('posted', 'compensated') AND ledger_transfer_id IS NOT NULL AND ledger_transaction_id IS NOT NULL) OR status NOT IN ('posted', 'compensated'))",
                    ))
                    .to_owned(),
            )
            .await?;

        for index in [
            Index::create()
                .name("uq_mkt_payout_operation_transfer_sequence")
                .table(MarketplacePayoutOperationTransfers::Table)
                .col(MarketplacePayoutOperationTransfers::TenantId)
                .col(MarketplacePayoutOperationTransfers::OperationId)
                .col(MarketplacePayoutOperationTransfers::SequenceNo)
                .unique()
                .to_owned(),
            Index::create()
                .name("uq_mkt_payout_operation_transfer_order_kind")
                .table(MarketplacePayoutOperationTransfers::Table)
                .col(MarketplacePayoutOperationTransfers::TenantId)
                .col(MarketplacePayoutOperationTransfers::OperationId)
                .col(MarketplacePayoutOperationTransfers::OrderId)
                .col(MarketplacePayoutOperationTransfers::TransferKind)
                .unique()
                .to_owned(),
            Index::create()
                .name("uq_mkt_payout_operation_transfer_key")
                .table(MarketplacePayoutOperationTransfers::Table)
                .col(MarketplacePayoutOperationTransfers::TenantId)
                .col(MarketplacePayoutOperationTransfers::IdempotencyKey)
                .unique()
                .to_owned(),
            Index::create()
                .name("uq_mkt_payout_operation_ledger_transfer")
                .table(MarketplacePayoutOperationTransfers::Table)
                .col(MarketplacePayoutOperationTransfers::TenantId)
                .col(MarketplacePayoutOperationTransfers::LedgerTransferId)
                .unique()
                .to_owned(),
            Index::create()
                .name("idx_mkt_payout_operation_transfer_recovery")
                .table(MarketplacePayoutOperationTransfers::Table)
                .col(MarketplacePayoutOperationTransfers::Status)
                .col(MarketplacePayoutOperationTransfers::UpdatedAt)
                .to_owned(),
        ] {
            manager.create_index(index).await?;
        }
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(MarketplacePayoutOperationTransfers::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_table(
                Table::drop()
                    .table(MarketplacePayoutOperations::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .drop_index(
                Index::drop()
                    .name("uq_marketplace_payouts_tenant_id")
                    .table(MarketplacePayouts::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await?;
        restore_legacy_table_names(manager).await
    }
}

async fn repair_legacy_table_names(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    let legacy = [
        manager.has_table(LEGACY_PAYOUTS).await?,
        manager.has_table(LEGACY_ITEMS).await?,
        manager.has_table(LEGACY_RECEIPTS).await?,
    ];
    let canonical = [
        manager.has_table(CANONICAL_PAYOUTS).await?,
        manager.has_table(CANONICAL_ITEMS).await?,
        manager.has_table(CANONICAL_RECEIPTS).await?,
    ];
    if canonical.iter().all(|exists| *exists) && legacy.iter().all(|exists| !*exists) {
        return Ok(());
    }
    if !legacy.iter().all(|exists| *exists) || !canonical.iter().all(|exists| !*exists) {
        return Err(DbErr::Custom(
            "marketplace payout table names are in a mixed legacy/canonical state".to_string(),
        ));
    }

    rename_tables(
        manager,
        [
            (LEGACY_PAYOUTS, CANONICAL_PAYOUTS),
            (LEGACY_ITEMS, CANONICAL_ITEMS),
            (LEGACY_RECEIPTS, CANONICAL_RECEIPTS),
        ],
    )
    .await?;
    manager
        .create_table(
            Table::create()
                .table(Alias::new(REPAIR_MARKER))
                .if_not_exists()
                .col(
                    ColumnDef::new(Alias::new("id"))
                        .integer()
                        .not_null()
                        .primary_key(),
                )
                .to_owned(),
        )
        .await
}

async fn restore_legacy_table_names(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
    if !manager.has_table(REPAIR_MARKER).await? {
        return Ok(());
    }
    manager
        .drop_table(
            Table::drop()
                .table(Alias::new(REPAIR_MARKER))
                .if_exists()
                .to_owned(),
        )
        .await?;
    rename_tables(
        manager,
        [
            (CANONICAL_PAYOUTS, LEGACY_PAYOUTS),
            (CANONICAL_ITEMS, LEGACY_ITEMS),
            (CANONICAL_RECEIPTS, LEGACY_RECEIPTS),
        ],
    )
    .await
}

async fn rename_tables(
    manager: &SchemaManager<'_>,
    renames: [(&str, &str); 3],
) -> Result<(), DbErr> {
    for (from, to) in renames {
        let sql = match manager.get_database_backend() {
            DatabaseBackend::MySql => format!("RENAME TABLE `{from}` TO `{to}`"),
            DatabaseBackend::Postgres | DatabaseBackend::Sqlite => {
                format!("ALTER TABLE \"{from}\" RENAME TO \"{to}\"")
            }
        };
        manager.get_connection().execute_unprepared(&sql).await?;
    }
    Ok(())
}

#[derive(Iden)]
enum MarketplacePayouts {
    Table,
    Id,
    TenantId,
}

#[derive(Iden)]
enum MarketplacePayoutOperations {
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
    Revision,
    LeaseOwner,
    LeaseExpiresAt,
    LastErrorCode,
    CreatedAt,
    UpdatedAt,
    CompletedAt,
}

#[derive(Iden)]
enum MarketplacePayoutOperationTransfers {
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
    Revision,
    LastErrorCode,
    CreatedAt,
    UpdatedAt,
    CompletedAt,
}
