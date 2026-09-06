use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ForumTopicVotes::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(ForumTopicVotes::TopicId).uuid().not_null())
                    .col(ColumnDef::new(ForumTopicVotes::UserId).uuid().not_null())
                    .col(ColumnDef::new(ForumTopicVotes::TenantId).uuid().not_null())
                    .col(ColumnDef::new(ForumTopicVotes::Value).integer().not_null())
                    .col(
                        ColumnDef::new(ForumTopicVotes::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ForumTopicVotes::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(ForumTopicVotes::TopicId)
                            .col(ForumTopicVotes::UserId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_forum_topic_votes_topic")
                            .from(ForumTopicVotes::Table, ForumTopicVotes::TopicId)
                            .to(ForumTopics::Table, ForumTopics::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::cust(format!(
                        "{} in (-1, 1)",
                        ForumTopicVotes::Value.to_string()
                    )))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_forum_topic_votes_tenant_topic")
                    .table(ForumTopicVotes::Table)
                    .col(ForumTopicVotes::TenantId)
                    .col(ForumTopicVotes::TopicId)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(ForumReplyVotes::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(ForumReplyVotes::ReplyId).uuid().not_null())
                    .col(ColumnDef::new(ForumReplyVotes::UserId).uuid().not_null())
                    .col(ColumnDef::new(ForumReplyVotes::TenantId).uuid().not_null())
                    .col(ColumnDef::new(ForumReplyVotes::Value).integer().not_null())
                    .col(
                        ColumnDef::new(ForumReplyVotes::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(ForumReplyVotes::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .primary_key(
                        Index::create()
                            .col(ForumReplyVotes::ReplyId)
                            .col(ForumReplyVotes::UserId),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_forum_reply_votes_reply")
                            .from(ForumReplyVotes::Table, ForumReplyVotes::ReplyId)
                            .to(ForumReplies::Table, ForumReplies::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .check(Expr::cust(format!(
                        "{} in (-1, 1)",
                        ForumReplyVotes::Value.to_string()
                    )))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_forum_reply_votes_tenant_reply")
                    .table(ForumReplyVotes::Table)
                    .col(ForumReplyVotes::TenantId)
                    .col(ForumReplyVotes::ReplyId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ForumReplyVotes::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(ForumTopicVotes::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ForumTopicVotes {
    Table,
    TopicId,
    UserId,
    TenantId,
    Value,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum ForumReplyVotes {
    Table,
    ReplyId,
    UserId,
    TenantId,
    Value,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum ForumTopics {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum ForumReplies {
    Table,
    Id,
}
