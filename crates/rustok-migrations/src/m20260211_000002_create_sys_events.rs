use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(SysEvents::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(SysEvents::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(SysEvents::EventType)
                            .string_len(255)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(SysEvents::SchemaVersion)
                            .small_integer()
                            .not_null(),
                    )
                    .col(ColumnDef::new(SysEvents::Payload).json_binary().not_null())
                    .col(ColumnDef::new(SysEvents::Status).string_len(32).not_null())
                    .col(
                        ColumnDef::new(SysEvents::RetryCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(SysEvents::NextAttemptAt).timestamp_with_time_zone())
                    .col(ColumnDef::new(SysEvents::LastError).string_len(2048))
                    .col(ColumnDef::new(SysEvents::ClaimedBy).string_len(128))
                    .col(ColumnDef::new(SysEvents::ClaimedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(SysEvents::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(ColumnDef::new(SysEvents::DispatchedAt).timestamp_with_time_zone())
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_sys_events_pending_next_attempt")
                    .table(SysEvents::Table)
                    .col(SysEvents::Status)
                    .col(SysEvents::NextAttemptAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .if_not_exists()
                    .name("idx_sys_events_claimed_at")
                    .table(SysEvents::Table)
                    .col(SysEvents::ClaimedAt)
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(SysEvents::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum SysEvents {
    Table,
    Id,
    EventType,
    SchemaVersion,
    Payload,
    Status,
    RetryCount,
    NextAttemptAt,
    LastError,
    ClaimedBy,
    ClaimedAt,
    CreatedAt,
    DispatchedAt,
}
