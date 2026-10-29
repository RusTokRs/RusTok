use super::super::*;
use sea_orm_migration::prelude::*;

pub(super) async fn create(manager: &SchemaManager<'_>) -> Result<(), DbErr> {
// 10. Create blog_comments_tcp_delegation_schedule_state table
        manager
            .create_table(
                Table::create()
                    .table(BlogCommentsTcpDelegationScheduleState::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(BlogCommentsTcpDelegationScheduleState::StateKey)
                            .string_len(64)
                            .not_null()
                            .primary_key(),
                    )
                    .col(
                        ColumnDef::new(BlogCommentsTcpDelegationScheduleState::SchemaVersion)
                            .small_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCommentsTcpDelegationScheduleState::Source)
                            .string_len(16)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCommentsTcpDelegationScheduleState::Generation)
                            .big_integer()
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCommentsTcpDelegationScheduleState::ScheduleDigestHex)
                            .string_len(64)
                            .not_null(),
                    )
                    .col(
                        ColumnDef::new(BlogCommentsTcpDelegationScheduleState::UpdatedAt)
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
            .await?;
    Ok(())
}
