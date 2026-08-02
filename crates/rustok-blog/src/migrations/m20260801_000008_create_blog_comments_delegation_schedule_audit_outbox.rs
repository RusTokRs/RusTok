use sea_orm_migration::prelude::*;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        manager
            .create_table(
                Table::create()
                    .table(BlogCommentsDelegationScheduleAuditOutbox::Table)
                    .if_not_exists()
                    .col(
                        ColumnDef::new(
                            BlogCommentsDelegationScheduleAuditOutbox::RequestId,
                        )
                        .uuid()
                        .not_null()
                        .primary_key(),
                    )
                    .col(
                        ColumnDef::new(
                            BlogCommentsDelegationScheduleAuditOutbox::StateKey,
                        )
                        .string_len(64)
                        .not_null(),
                    )
                    .col(
                        ColumnDef::new(
                            BlogCommentsDelegationScheduleAuditOutbox::AuditSchemaVersion,
                        )
                        .small_integer()
                        .not_null(),
                    )
                    .col(
                        ColumnDef::new(
                            BlogCommentsDelegationScheduleAuditOutbox::EventType,
                        )
                        .string_len(64)
                        .not_null(),
                    )
                    .col(
                        ColumnDef::new(
                            BlogCommentsDelegationScheduleAuditOutbox::OccurredAtUnixMs,
                        )
                        .big_integer()
                        .not_null(),
                    )
                    .col(
                        ColumnDef::new(
                            BlogCommentsDelegationScheduleAuditOutbox::ActorId,
                        )
                        .uuid()
                        .not_null(),
                    )
                    .col(
                        ColumnDef::new(
                            BlogCommentsDelegationScheduleAuditOutbox::PrincipalKind,
                        )
                        .string_len(16)
                        .not_null(),
                    )
                    .col(
                        ColumnDef::new(
                            BlogCommentsDelegationScheduleAuditOutbox::Operation,
                        )
                        .string_len(32)
                        .not_null(),
                    )
                    .col(
                        ColumnDef::new(
                            BlogCommentsDelegationScheduleAuditOutbox::Source,
                        )
                        .string_len(16)
                        .not_null(),
                    )
                    .col(
                        ColumnDef::new(
                            BlogCommentsDelegationScheduleAuditOutbox::PreviousGeneration,
                        )
                        .big_integer()
                        .not_null(),
                    )
                    .col(
                        ColumnDef::new(
                            BlogCommentsDelegationScheduleAuditOutbox::CandidateGeneration,
                        )
                        .big_integer()
                        .not_null(),
                    )
                    .col(
                        ColumnDef::new(
                            BlogCommentsDelegationScheduleAuditOutbox::Outcome,
                        )
                        .string_len(32)
                        .not_null(),
                    )
                    .col(
                        ColumnDef::new(
                            BlogCommentsDelegationScheduleAuditOutbox::CreatedAt,
                        )
                        .timestamp_with_time_zone()
                        .not_null()
                        .default(Expr::current_timestamp()),
                    )
                    .col(
                        ColumnDef::new(
                            BlogCommentsDelegationScheduleAuditOutbox::PublishedAt,
                        )
                        .timestamp_with_time_zone(),
                    )
                    .foreign_key(
                        ForeignKey::create()
                            .name("fk_blog_comments_delegation_audit_state")
                            .from(
                                BlogCommentsDelegationScheduleAuditOutbox::Table,
                                BlogCommentsDelegationScheduleAuditOutbox::StateKey,
                            )
                            .to(
                                BlogCommentsDelegationScheduleState::Table,
                                BlogCommentsDelegationScheduleState::StateKey,
                            )
                            .on_delete(ForeignKeyAction::Restrict),
                    )
                    .check(Expr::cust("audit_schema_version = 1"))
                    .check(Expr::cust(
                        "state_key = 'comments_tcp_delegation_schedule'",
                    ))
                    .check(Expr::cust(
                        "event_type = 'comments_tcp_delegation_schedule_replaced'",
                    ))
                    .check(Expr::cust("occurred_at_unix_ms > 0"))
                    .check(Expr::cust(
                        "principal_kind IN ('direct_user', 'service')",
                    ))
                    .check(Expr::cust(
                        "operation IN ('reload_file', 'replace_host_schedule')",
                    ))
                    .check(Expr::cust(
                        "source IN ('host_provided', 'file')",
                    ))
                    .check(Expr::cust("previous_generation > 0"))
                    .check(Expr::cust(
                        "candidate_generation > previous_generation",
                    ))
                    .check(Expr::cust("outcome = 'replacement_succeeded'"))
                    .to_owned(),
            )
            .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_blog_comments_delegation_audit_generation")
                    .table(BlogCommentsDelegationScheduleAuditOutbox::Table)
                    .col(BlogCommentsDelegationScheduleAuditOutbox::StateKey)
                    .col(
                        BlogCommentsDelegationScheduleAuditOutbox::CandidateGeneration,
                    )
                    .unique()
                    .to_owned(),
            )
            .await
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "Comments delegation schedule durable audit outbox is security-sensitive and intentionally irreversible"
                .to_string(),
        ))
    }
}

#[derive(DeriveIden)]
enum BlogCommentsDelegationScheduleAuditOutbox {
    #[sea_orm(iden = "blog_comments_tcp_delegation_schedule_audit_outbox")]
    Table,
    RequestId,
    StateKey,
    AuditSchemaVersion,
    EventType,
    OccurredAtUnixMs,
    ActorId,
    PrincipalKind,
    Operation,
    Source,
    PreviousGeneration,
    CandidateGeneration,
    Outcome,
    CreatedAt,
    PublishedAt,
}

#[derive(DeriveIden)]
enum BlogCommentsDelegationScheduleState {
    #[sea_orm(iden = "blog_comments_tcp_delegation_schedule_state")]
    Table,
    StateKey,
}
