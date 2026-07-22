use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MarketplacePayoutProviderOperations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MarketplacePayoutProviderOperations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutProviderOperations::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutProviderOperations::PayoutId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutProviderOperations::Operation)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutProviderOperations::ProviderId)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutProviderOperations::IdempotencyKey)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutProviderOperations::RequestHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutProviderOperations::RequestJson)
                            .json_binary()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutProviderOperations::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutProviderOperations::ProviderReference)
                            .string_len(191),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutProviderOperations::ProviderResultJson)
                            .json_binary(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutProviderOperations::AttemptCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutProviderOperations::Revision)
                            .big_integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutProviderOperations::LeaseOwner)
                            .string_len(191),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutProviderOperations::LeaseExpiresAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutProviderOperations::LastErrorCode)
                            .string_len(120),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutProviderOperations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutProviderOperations::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutProviderOperations::ProviderCompletedAt)
                            .timestamp_with_time_zone(),
                    )
                    .col(
                        ColumnDef::new(MarketplacePayoutProviderOperations::CommittedAt)
                            .timestamp_with_time_zone(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_mkt_payout_provider_operation_tenant_payout")
                            .from(
                                MarketplacePayoutProviderOperations::Table,
                                MarketplacePayoutProviderOperations::TenantId,
                            )
                            .from_col(MarketplacePayoutProviderOperations::PayoutId)
                            .to(MarketplacePayouts::Table, MarketplacePayouts::TenantId)
                            .to_col(MarketplacePayouts::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .check(Expr::cust("operation IN ('submit', 'lookup', 'cancel')"))
                    .check(Expr::cust("TRIM(provider_id) <> ''"))
                    .check(Expr::cust("TRIM(idempotency_key) <> ''"))
                    .check(Expr::cust("LENGTH(request_hash) = 64"))
                    .check(Expr::cust(
                        "status IN ('pending', 'executing', 'provider_succeeded', 'provider_failed', 'retryable_error', 'reconciliation_required', 'committed')",
                    ))
                    .check(Expr::cust("attempt_count >= 0"))
                    .check(Expr::cust("revision >= 0"))
                    .check(Expr::cust(
                        "((status = 'executing' AND lease_owner IS NOT NULL AND TRIM(lease_owner) <> '' AND lease_expires_at IS NOT NULL) OR (status <> 'executing' AND lease_owner IS NULL AND lease_expires_at IS NULL))",
                    ))
                    .check(Expr::cust(
                        "((status IN ('provider_succeeded', 'committed') AND provider_result_json IS NOT NULL AND provider_completed_at IS NOT NULL) OR status NOT IN ('provider_succeeded', 'committed'))",
                    ))
                    .check(Expr::cust(
                        "(status <> 'provider_failed' OR (last_error_code IS NOT NULL AND TRIM(last_error_code) <> '' AND provider_completed_at IS NOT NULL))",
                    ))
                    .check(Expr::cust(
                        "((status = 'committed' AND committed_at IS NOT NULL) OR (status <> 'committed' AND committed_at IS NULL))",
                    ))
                    .to_owned(),
            )
            .await?;

        for index in [
            Index::create()
                .name("uq_mkt_payout_provider_operation_tenant_id")
                .table(MarketplacePayoutProviderOperations::Table)
                .col(MarketplacePayoutProviderOperations::TenantId)
                .col(MarketplacePayoutProviderOperations::Id)
                .unique()
                .to_owned(),
            Index::create()
                .name("uq_mkt_payout_provider_operation_key")
                .table(MarketplacePayoutProviderOperations::Table)
                .col(MarketplacePayoutProviderOperations::TenantId)
                .col(MarketplacePayoutProviderOperations::ProviderId)
                .col(MarketplacePayoutProviderOperations::IdempotencyKey)
                .unique()
                .to_owned(),
            Index::create()
                .name("uq_mkt_payout_provider_operation_kind")
                .table(MarketplacePayoutProviderOperations::Table)
                .col(MarketplacePayoutProviderOperations::TenantId)
                .col(MarketplacePayoutProviderOperations::PayoutId)
                .col(MarketplacePayoutProviderOperations::Operation)
                .unique()
                .to_owned(),
            Index::create()
                .name("idx_mkt_payout_provider_operation_recovery")
                .table(MarketplacePayoutProviderOperations::Table)
                .col(MarketplacePayoutProviderOperations::Status)
                .col(MarketplacePayoutProviderOperations::LeaseExpiresAt)
                .col(MarketplacePayoutProviderOperations::UpdatedAt)
                .to_owned(),
            Index::create()
                .name("idx_mkt_payout_provider_operation_payout")
                .table(MarketplacePayoutProviderOperations::Table)
                .col(MarketplacePayoutProviderOperations::TenantId)
                .col(MarketplacePayoutProviderOperations::PayoutId)
                .col(MarketplacePayoutProviderOperations::CreatedAt)
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
                    .table(MarketplacePayoutProviderOperations::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum MarketplacePayoutProviderOperations {
    #[sea_orm(iden = "marketplace_payout_provider_operations")]
    Table,
    Id,
    TenantId,
    PayoutId,
    Operation,
    ProviderId,
    IdempotencyKey,
    RequestHash,
    RequestJson,
    Status,
    ProviderReference,
    ProviderResultJson,
    AttemptCount,
    Revision,
    LeaseOwner,
    LeaseExpiresAt,
    LastErrorCode,
    CreatedAt,
    UpdatedAt,
    ProviderCompletedAt,
    CommittedAt,
}

#[derive(DeriveIden)]
enum MarketplacePayouts {
    #[sea_orm(iden = "marketplace_payouts")]
    Table,
    TenantId,
    Id,
}
