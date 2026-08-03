use sea_orm_migration::{
    prelude::*,
    sea_orm::{ConnectionTrait, DbBackend},
};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        add_column_if_missing(
            manager,
            ColumnDef::new(BlogCommentsDelegationScheduleAuditOutbox::HandoffNextAttemptAt)
                .timestamp_with_time_zone()
                .to_owned(),
        )
        .await?;
        add_column_if_missing(
            manager,
            ColumnDef::new(BlogCommentsDelegationScheduleAuditOutbox::HandoffLastFailureAt)
                .timestamp_with_time_zone()
                .to_owned(),
        )
        .await?;
        add_column_if_missing(
            manager,
            ColumnDef::new(BlogCommentsDelegationScheduleAuditOutbox::HandoffLastFailureCode)
                .string_len(32)
                .to_owned(),
        )
        .await?;
        add_column_if_missing(
            manager,
            ColumnDef::new(BlogCommentsDelegationScheduleAuditOutbox::HandoffDeadLetteredAt)
                .timestamp_with_time_zone()
                .to_owned(),
        )
        .await?;
        add_column_if_missing(
            manager,
            ColumnDef::new(BlogCommentsDelegationScheduleAuditOutbox::HandoffDeadLetterReason)
                .string_len(64)
                .to_owned(),
        )
        .await?;

        manager
            .create_index(
                Index::create()
                    .name("idx_blog_comments_delegation_audit_handoff_retry_ready")
                    .table(BlogCommentsDelegationScheduleAuditOutbox::Table)
                    .col(BlogCommentsDelegationScheduleAuditOutbox::PublishedAt)
                    .col(BlogCommentsDelegationScheduleAuditOutbox::HandoffDeadLetteredAt)
                    .col(BlogCommentsDelegationScheduleAuditOutbox::HandoffNextAttemptAt)
                    .col(BlogCommentsDelegationScheduleAuditOutbox::CreatedAt)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_blog_comments_delegation_audit_handoff_dead_letter")
                    .table(BlogCommentsDelegationScheduleAuditOutbox::Table)
                    .col(BlogCommentsDelegationScheduleAuditOutbox::HandoffDeadLetteredAt)
                    .col(BlogCommentsDelegationScheduleAuditOutbox::CreatedAt)
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;

        if manager.get_database_backend() == DbBackend::Postgres {
            manager
                .get_connection()
                .execute_unprepared(
                    r#"
ALTER TABLE blog_comments_tcp_delegation_schedule_audit_outbox
    ADD CONSTRAINT ck_blog_comments_delegation_audit_handoff_failure_pair
    CHECK ((handoff_last_failure_at IS NULL) = (handoff_last_failure_code IS NULL)),
    ADD CONSTRAINT ck_blog_comments_delegation_audit_handoff_failure_code
    CHECK (
        handoff_last_failure_code IS NULL
        OR handoff_last_failure_code IN ('conflict', 'unavailable')
    ),
    ADD CONSTRAINT ck_blog_comments_delegation_audit_handoff_dead_letter_pair
    CHECK ((handoff_dead_lettered_at IS NULL) = (handoff_dead_letter_reason IS NULL)),
    ADD CONSTRAINT ck_blog_comments_delegation_audit_handoff_dead_letter_reason
    CHECK (
        handoff_dead_letter_reason IS NULL
        OR handoff_dead_letter_reason = 'attempt_budget_exhausted'
    ),
    ADD CONSTRAINT ck_blog_comments_delegation_audit_handoff_retry_unclaimed
    CHECK (
        handoff_next_attempt_at IS NULL
        OR (
            published_at IS NULL
            AND canonical_envelope_id IS NULL
            AND handoff_dead_lettered_at IS NULL
            AND handoff_claim_token IS NULL
            AND handoff_claim_expires_at IS NULL
        )
    ),
    ADD CONSTRAINT ck_blog_comments_delegation_audit_handoff_dead_letter_terminal
    CHECK (
        handoff_dead_lettered_at IS NULL
        OR (
            published_at IS NULL
            AND canonical_envelope_id IS NULL
            AND handoff_next_attempt_at IS NULL
            AND handoff_claim_token IS NULL
            AND handoff_claim_expires_at IS NULL
        )
    ),
    ADD CONSTRAINT ck_blog_comments_delegation_audit_handoff_published_not_retrying
    CHECK (
        published_at IS NULL
        OR (
            handoff_next_attempt_at IS NULL
            AND handoff_dead_lettered_at IS NULL
            AND handoff_dead_letter_reason IS NULL
        )
    );
"#,
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "Comments schedule audit source retry and dead-letter metadata is security-sensitive and intentionally irreversible"
                .to_string(),
        ))
    }
}

async fn add_column_if_missing(
    manager: &SchemaManager<'_>,
    column: ColumnDef,
) -> Result<(), DbErr> {
    manager
        .alter_table(
            Table::alter()
                .table(BlogCommentsDelegationScheduleAuditOutbox::Table)
                .add_column_if_not_exists(column)
                .to_owned(),
        )
        .await
}

#[derive(DeriveIden)]
enum BlogCommentsDelegationScheduleAuditOutbox {
    #[sea_orm(iden = "blog_comments_tcp_delegation_schedule_audit_outbox")]
    Table,
    HandoffNextAttemptAt,
    HandoffLastFailureAt,
    HandoffLastFailureCode,
    HandoffDeadLetteredAt,
    HandoffDeadLetterReason,
    PublishedAt,
    CreatedAt,
}
