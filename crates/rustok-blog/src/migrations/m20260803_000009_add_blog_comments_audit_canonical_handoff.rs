use sea_orm_migration::{prelude::*, sea_orm::ConnectionTrait};

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        add_column_if_missing(
            manager,
            ColumnDef::new(BlogCommentsDelegationScheduleAuditOutbox::CanonicalEnvelopeId)
                .uuid()
                .to_owned(),
        )
        .await?;
        add_column_if_missing(
            manager,
            ColumnDef::new(BlogCommentsDelegationScheduleAuditOutbox::HandoffClaimToken)
                .uuid()
                .to_owned(),
        )
        .await?;
        add_column_if_missing(
            manager,
            ColumnDef::new(BlogCommentsDelegationScheduleAuditOutbox::HandoffClaimExpiresAt)
                .timestamp_with_time_zone()
                .to_owned(),
        )
        .await?;
        add_column_if_missing(
            manager,
            ColumnDef::new(BlogCommentsDelegationScheduleAuditOutbox::HandoffAttemptCount)
                .big_integer()
                .not_null()
                .default(0)
                .to_owned(),
        )
        .await?;

        manager
            .create_index(
                Index::create()
                    .name("uq_blog_comments_delegation_audit_canonical_envelope")
                    .table(BlogCommentsDelegationScheduleAuditOutbox::Table)
                    .col(BlogCommentsDelegationScheduleAuditOutbox::CanonicalEnvelopeId)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("uq_blog_comments_delegation_audit_handoff_claim_token")
                    .table(BlogCommentsDelegationScheduleAuditOutbox::Table)
                    .col(BlogCommentsDelegationScheduleAuditOutbox::HandoffClaimToken)
                    .unique()
                    .if_not_exists()
                    .to_owned(),
            )
            .await?;
        manager
            .create_index(
                Index::create()
                    .name("idx_blog_comments_delegation_audit_handoff_pending")
                    .table(BlogCommentsDelegationScheduleAuditOutbox::Table)
                    .col(BlogCommentsDelegationScheduleAuditOutbox::PublishedAt)
                    .col(BlogCommentsDelegationScheduleAuditOutbox::HandoffClaimExpiresAt)
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
DO $$
BEGIN
    IF EXISTS (
        SELECT 1
        FROM blog_comments_tcp_delegation_schedule_audit_outbox
        WHERE published_at IS NOT NULL
    ) THEN
        RAISE EXCEPTION 'legacy Comments schedule audit rows already use published_at without canonical identity';
    END IF;
END
$$;

ALTER TABLE blog_comments_tcp_delegation_schedule_audit_outbox
    ADD CONSTRAINT ck_blog_comments_delegation_audit_handoff_attempt_count
    CHECK (handoff_attempt_count >= 0),
    ADD CONSTRAINT ck_blog_comments_delegation_audit_handoff_claim_pair
    CHECK ((handoff_claim_token IS NULL) = (handoff_claim_expires_at IS NULL)),
    ADD CONSTRAINT ck_blog_comments_delegation_audit_handoff_claim_token_non_nil
    CHECK (
        handoff_claim_token IS NULL
        OR handoff_claim_token <> '00000000-0000-0000-0000-000000000000'::uuid
    ),
    ADD CONSTRAINT ck_blog_comments_delegation_audit_handoff_publication_pair
    CHECK (
        (canonical_envelope_id IS NULL AND published_at IS NULL)
        OR (canonical_envelope_id = request_id AND published_at IS NOT NULL)
    ),
    ADD CONSTRAINT ck_blog_comments_delegation_audit_handoff_terminal_unclaimed
    CHECK (
        published_at IS NULL
        OR (handoff_claim_token IS NULL AND handoff_claim_expires_at IS NULL)
    );
"#,
                )
                .await?;
        }

        Ok(())
    }

    async fn down(&self, _manager: &SchemaManager) -> Result<(), DbErr> {
        Err(DbErr::Migration(
            "Comments schedule audit canonical handoff metadata is security-sensitive and intentionally irreversible"
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
    CanonicalEnvelopeId,
    HandoffClaimToken,
    HandoffClaimExpiresAt,
    HandoffAttemptCount,
    PublishedAt,
    CreatedAt,
}
