use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SeoEventDeliveries::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SeoEventDeliveries::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SeoEventDeliveries::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SeoEventDeliveries::EventType)
                            .string_len(128)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SeoEventDeliveries::IdempotencyKey)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(ColumnDef::new(SeoEventDeliveries::SourceKind).string_len(64))
                    .col(ColumnDef::new(SeoEventDeliveries::SourceId).uuid())
                    .col(
                        ColumnDef::new(SeoEventDeliveries::Status)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(ColumnDef::new(SeoEventDeliveries::OutboxEventId).uuid())
                    .col(ColumnDef::new(SeoEventDeliveries::LastError).string_len(2048))
                    .col(
                        ColumnDef::new(SeoEventDeliveries::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SeoEventDeliveries::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SeoEventDeliveries::DispatchedAt).timestamp_with_time_zone(),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_seo_event_deliveries_idempotency")
                    .table(SeoEventDeliveries::Table)
                    .col(SeoEventDeliveries::TenantId)
                    .col(SeoEventDeliveries::IdempotencyKey)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_seo_event_deliveries_status")
                    .table(SeoEventDeliveries::Table)
                    .col(SeoEventDeliveries::TenantId)
                    .col(SeoEventDeliveries::Status)
                    .col(SeoEventDeliveries::UpdatedAt)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SeoEventDeliveries::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(Iden)]
enum SeoEventDeliveries {
    Table,
    Id,
    TenantId,
    EventType,
    IdempotencyKey,
    SourceKind,
    SourceId,
    Status,
    OutboxEventId,
    LastError,
    CreatedAt,
    UpdatedAt,
    DispatchedAt,
}
