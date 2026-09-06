use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(ForumTopicTags::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(ForumTopicTags::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(ForumTopicTags::TopicId).uuid().not_null())
                    .col(ColumnDef::new(ForumTopicTags::TermId).uuid().not_null())
                    .col(ColumnDef::new(ForumTopicTags::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(ForumTopicTags::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_forum_topic_tags_topic")
                            .from(ForumTopicTags::Table, ForumTopicTags::TopicId)
                            .to(ForumTopics::Table, ForumTopics::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_forum_topic_tags_term")
                            .from(ForumTopicTags::Table, ForumTopicTags::TermId)
                            .to(TaxonomyTerms::Table, TaxonomyTerms::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_forum_topic_tags_topic_term")
                    .table(ForumTopicTags::Table)
                    .col(ForumTopicTags::TopicId)
                    .col(ForumTopicTags::TermId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_forum_topic_tags_tenant_term")
                    .table(ForumTopicTags::Table)
                    .col(ForumTopicTags::TenantId)
                    .col(ForumTopicTags::TermId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(ForumTopicTags::Table).to_owned())
            .await
    }
}

#[derive(DeriveIden)]
enum ForumTopicTags {
    Table,
    Id,
    TopicId,
    TermId,
    TenantId,
    CreatedAt,
}

#[derive(DeriveIden)]
enum ForumTopics {
    Table,
    Id,
}

#[derive(DeriveIden)]
enum TaxonomyTerms {
    Table,
    Id,
}
