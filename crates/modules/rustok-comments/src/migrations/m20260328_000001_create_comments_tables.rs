use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CommentThreads::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CommentThreads::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(CommentThreads::TenantId).uuid().not_null())
                    .col(
                        ColumnDef::new(CommentThreads::TargetType)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(ColumnDef::new(CommentThreads::TargetId).uuid().not_null())
                    .col(
                        ColumnDef::new(CommentThreads::Status)
                            .string_len(32)
                            .not_null()
                            .default("open"),
                    )
                    .col(
                        ColumnDef::new(CommentThreads::CommentCount)
                            .integer()
                            .not_null()
                            .default(0),
                    )
                    .col(ColumnDef::new(CommentThreads::LastCommentedAt).timestamp_with_time_zone())
                    .col(
                        ColumnDef::new(CommentThreads::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CommentThreads::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_comment_threads_target")
                    .table(CommentThreads::Table)
                    .col(CommentThreads::TenantId)
                    .col(CommentThreads::TargetType)
                    .col(CommentThreads::TargetId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(Comments::Table)
                    .if_not_exists()
                    .col(ColumnDef::new(Comments::Id).uuid().not_null().primary_key())
                    .col(ColumnDef::new(Comments::TenantId).uuid().not_null())
                    .col(ColumnDef::new(Comments::ThreadId).uuid().not_null())
                    .col(ColumnDef::new(Comments::AuthorId).uuid().not_null())
                    .col(ColumnDef::new(Comments::ParentCommentId).uuid())
                    .col(
                        ColumnDef::new(Comments::Status)
                            .string_len(32)
                            .not_null()
                            .default("pending"),
                    )
                    .col(ColumnDef::new(Comments::Position).big_integer().not_null())
                    .col(
                        ColumnDef::new(Comments::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(Comments::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(ColumnDef::new(Comments::DeletedAt).timestamp_with_time_zone())
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_comments_thread")
                            .from(Comments::Table, Comments::ThreadId)
                            .to(CommentThreads::Table, CommentThreads::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_comments_parent")
                            .from(Comments::Table, Comments::ParentCommentId)
                            .to(Comments::Table, Comments::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::SetNull),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_comments_thread_position")
                    .table(Comments::Table)
                    .col(Comments::ThreadId)
                    .col(Comments::Position)
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_comments_thread_created_at")
                    .table(Comments::Table)
                    .col(Comments::ThreadId)
                    .col(Comments::CreatedAt)
                    .to_owned(),
            )
            .await?;

        manager
            .create_table(
                Table::create()
                    .table(CommentBodies::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CommentBodies::Id)
                            .uuid()
                            .not_null()
                            .primary_key(),
                    )
                    .col(ColumnDef::new(CommentBodies::CommentId).uuid().not_null())
                    .col(
                        ColumnDef::new(CommentBodies::Locale)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(ColumnDef::new(CommentBodies::Body).text().not_null())
                    .col(
                        ColumnDef::new(CommentBodies::CreatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(CommentBodies::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_comment_bodies_comment")
                            .from(CommentBodies::Table, CommentBodies::CommentId)
                            .to(Comments::Table, Comments::Id)
                            .on_update(ForeignKeyAction::Cascade)
                            .on_delete(ForeignKeyAction::Cascade),
                    )
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_comment_bodies_comment_locale")
                    .table(CommentBodies::Table)
                    .col(CommentBodies::CommentId)
                    .col(CommentBodies::Locale)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(Table::drop().table(CommentBodies::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(Comments::Table).to_owned())
            .await?;
        manager
            .drop_table(Table::drop().table(CommentThreads::Table).to_owned())
            .await?;
        Ok(())
    }
}

#[derive(DeriveIden)]
enum CommentThreads {
    Table,
    Id,
    TenantId,
    TargetType,
    TargetId,
    Status,
    CommentCount,
    LastCommentedAt,
    CreatedAt,
    UpdatedAt,
}

#[derive(DeriveIden)]
enum Comments {
    Table,
    Id,
    TenantId,
    ThreadId,
    AuthorId,
    ParentCommentId,
    Status,
    Position,
    CreatedAt,
    UpdatedAt,
    DeletedAt,
}

#[derive(DeriveIden)]
enum CommentBodies {
    Table,
    Id,
    CommentId,
    Locale,
    Body,
    CreatedAt,
    UpdatedAt,
}
