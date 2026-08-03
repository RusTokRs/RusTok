use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BlogCommentsDelegationScheduleState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BlogCommentsDelegationScheduleState::StateKey)
                            .string_len(64)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(BlogCommentsDelegationScheduleState::SchemaVersion)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCommentsDelegationScheduleState::Source)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCommentsDelegationScheduleState::Generation)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCommentsDelegationScheduleState::ScheduleDigestHex)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCommentsDelegationScheduleState::UpdatedAt)
                            .timestamp_with_time_zone()
                            .not_null()
                            .default(Expr::current_timestamp()),
                    )
                    .check(Expr::cust("schema_version = 1"))
                    .check(Expr::cust("source IN ('host_provided', 'file')"))
                    .check(Expr::cust("generation > 0"))
                    .check(Expr::cust("length(schedule_digest_hex) = 64"))
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "Comments delegation schedule persistence state is security-sensitive and intentionally irreversible"
                .to_string(),
        ))
    }
}

#[derive(DeriveIden)]
enum BlogCommentsDelegationScheduleState {
    #[sea_orm(iden = "blog_comments_tcp_delegation_schedule_state")]
    Table,
    StateKey,
    SchemaVersion,
    Source,
    Generation,
    ScheduleDigestHex,
    UpdatedAt,
}
