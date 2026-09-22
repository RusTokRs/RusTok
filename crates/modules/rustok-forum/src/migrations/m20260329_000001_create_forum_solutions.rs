use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ForumSolutions::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ForumSolutions::TopicId)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ForumSolutions::TenantId).uuid().not_null())
                    .col(ColumnDef::new(ForumSolutions::ReplyId).uuid().not_null())
                    .col(ColumnDef::new(ForumSolutions::MarkedByUserId).uuid())
                    .col(
                        ColumnDef::new(ForumSolutions::MarkedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_forum_solutions_topic")
                            .from(ForumSolutions::Table, ForumSolutions::TopicId)
                            .to(ForumTopics::Table, ForumTopics::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_forum_solutions_reply")
                            .from(ForumSolutions::Table, ForumSolutions::ReplyId)
                            .to(ForumReplies::Table, ForumReplies::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_forum_solutions_tenant")
                    .table(ForumSolutions::Table)
                    .col(ForumSolutions::TenantId)
                    .col(ForumSolutions::MarkedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_forum_solutions_reply_unique")
                    .table(ForumSolutions::Table)
                    .col(ForumSolutions::ReplyId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ForumSolutions::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ForumSolutions {
    Table,
    TopicId,
    TenantId,
    ReplyId,
    MarkedByUserId,
    MarkedAt,
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
