use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(PagePublishOperations::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(PagePublishOperations::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(PagePublishOperations::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PagePublishOperations::PageId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PagePublishOperations::IdempotencyKey)
                            .string_len(191)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PagePublishOperations::RequestHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PagePublishOperations::ReviewHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PagePublishOperations::SanitizedSetHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PagePublishOperations::ArtifactSetHash)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PagePublishOperations::ResultVersion)
                            .integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PagePublishOperations::PublishedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(PagePublishOperations::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_page_publish_operations_page")
                            .from(
                                PagePublishOperations::Table,
                                PagePublishOperations::PageId,
                            )
                            .to(Pages::Table, Pages::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_page_publish_operations_idempotency")
                    .table(PagePublishOperations::Table)
                    .col(PagePublishOperations::TenantId)
                    .col(PagePublishOperations::PageId)
                    .col(PagePublishOperations::IdempotencyKey)
                    .unique()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_page_publish_operations_result")
                    .table(PagePublishOperations::Table)
                    .col(PagePublishOperations::TenantId)
                    .col(PagePublishOperations::PageId)
                    .col(PagePublishOperations::ResultVersion)
                    .to_owned(),
            )
            .await?;
        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(PagePublishOperations::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(DeriveIden)]
enum PagePublishOperations {
    Table,
    Id,
    TenantId,
    PageId,
    IdempotencyKey,
    RequestHash,
    ReviewHash,
    SanitizedSetHash,
    ArtifactSetHash,
    ResultVersion,
    PublishedAt,
    CreatedAt,
}

#[derive(DeriveIden)]
enum Pages {
    Table,
    Id,
}
