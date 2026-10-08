use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        // Forum owns its redirect routes. Topic slugs are locale-aware, so the
        // source route is keyed by locale. The canonical route of a topic is
        // derived from its id and is never stored here.
        manager
            .create_table(
                Table::create()
                    .table(ForumTopicRoutes::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(ForumTopicRoutes::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(ForumTopicRoutes::Locale)
                            .string_len(32)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ForumTopicRoutes::SourceRoute)
                            .string_len(512)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ForumTopicRoutes::TargetKind)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(ColumnDef::new(ForumTopicRoutes::TargetId).uuid().not_null())
                    .col(
                        ColumnDef::new(ForumTopicRoutes::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(ForumTopicRoutes::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null(),
                    )
                    .primary_key(
                        Index::create()
                            .name("pk_forum_topic_routes")
                            .col(ForumTopicRoutes::TenantId)
                            .col(ForumTopicRoutes::Locale)
                            .col(ForumTopicRoutes::SourceRoute),
                    )
                    .to_owned(),
            )
            .await?;

        // Removing a target (topic deletion, merge, promotion) finds its
        // redirects through this index.
        manager
            .create_index(
                Index::create()
                    .name("idx_forum_topic_routes_target")
                    .table(ForumTopicRoutes::Table)
                    .col(ForumTopicRoutes::TenantId)
                    .col(ForumTopicRoutes::TargetKind)
                    .col(ForumTopicRoutes::TargetId)
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(ForumTopicRoutes::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum ForumTopicRoutes {
    #[iden = "forum_topic_routes"]
    Table,
    TenantId,
    Locale,
    SourceRoute,
    TargetKind,
    TargetId,
    CreatedAt,
    UpdatedAt,
}
