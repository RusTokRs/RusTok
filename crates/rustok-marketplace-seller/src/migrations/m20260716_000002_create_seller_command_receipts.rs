use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(MarketplaceSellerCommandReceipts::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(MarketplaceSellerCommandReceipts::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceSellerCommandReceipts::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceSellerCommandReceipts::ActorId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceSellerCommandReceipts::IdempotencyKey)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceSellerCommandReceipts::CommandKind)
                            .string_len(80)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceSellerCommandReceipts::RequestHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceSellerCommandReceipts::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceSellerCommandReceipts::ResponseKind)
                            .string_len(32),
                    )
                    .col(
                        ColumnDef::new(MarketplaceSellerCommandReceipts::ResponseJson)
                            .json_binary(),
                    )
                    .col(
                        ColumnDef::new(MarketplaceSellerCommandReceipts::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(MarketplaceSellerCommandReceipts::CompletedAt)
                            .timestamp_with_time_zone(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_marketplace_seller_command_receipt_key")
                    .table(MarketplaceSellerCommandReceipts::Table)
                    .col(MarketplaceSellerCommandReceipts::TenantId)
                    .col(MarketplaceSellerCommandReceipts::IdempotencyKey)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_marketplace_seller_command_receipt_audit")
                    .table(MarketplaceSellerCommandReceipts::Table)
                    .col(MarketplaceSellerCommandReceipts::TenantId)
                    .col(MarketplaceSellerCommandReceipts::CommandKind)
                    .col(MarketplaceSellerCommandReceipts::CreatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(MarketplaceSellerCommandReceipts::Table)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum MarketplaceSellerCommandReceipts {
    Table,
    Id,
    TenantId,
    ActorId,
    IdempotencyKey,
    CommandKind,
    RequestHash,
    Status,
    ResponseKind,
    ResponseJson,
    CreatedAt,
    CompletedAt,
}
