use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ForumUserStats::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(ForumUserStats::TenantId).uuid().not_null())
                    .col(ColumnDef::new(ForumUserStats::UserId).uuid().not_null())
                    .col(
                        ColumnDef::new(ForumUserStats::TopicCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ForumUserStats::ReplyCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ForumUserStats::SolutionCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(
                        ColumnDef::new(ForumUserStats::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ForumUserStats::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(ForumUserStats::TenantId)
                            .col(ForumUserStats::UserId),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_forum_user_stats_user")
                    .table(ForumUserStats::Table)
                    .col(ForumUserStats::UserId)
                    .col(ForumUserStats::TenantId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ForumUserStats::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ForumUserStats {
    Table,
    TenantId,
    UserId,
    TopicCount,
    ReplyCount,
    SolutionCount,
    CreatedAt,
    UpdatedAt,
}
