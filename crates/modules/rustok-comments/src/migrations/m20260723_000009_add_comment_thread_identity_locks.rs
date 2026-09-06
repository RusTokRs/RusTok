use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(CommentThreadIdentityLocks::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(CommentThreadIdentityLocks::TenantId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CommentThreadIdentityLocks::TargetType)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CommentThreadIdentityLocks::TargetId)
                            .uuid()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(CommentThreadIdentityLocks::CreatedAt)
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
                    .name("idx_comment_thread_identity_locks_identity")
                    .table(CommentThreadIdentityLocks::Table)
                    .col(CommentThreadIdentityLocks::TenantId)
                    .col(CommentThreadIdentityLocks::TargetType)
                    .col(CommentThreadIdentityLocks::TargetId)
                    .unique()
                    .to_owned(),
            )
            .await?;

        Ok(())
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .drop_table(
                Table::drop()
                    .table(CommentThreadIdentityLocks::Table)
                    .if_exists()
                    .to_owned(),
            )
            .await
    }
}

#[derive(Iden)]
enum CommentThreadIdentityLocks {
    Table,
    TenantId,
    TargetType,
    TargetId,
    CreatedAt,
}
