//! Consolidated Blog schema migration baseline.
//!
//! Replaces the 32 unreleased development migrations with a single canonical schema
//! containing all current Blog tables, indexes, constraints, and triggers under
//! AGENTS.md §7.1 and the Zero-Legacy Policy.

use sea_orm_migration::prelude::*;

mod schema;

#[derive(DeriveMigrationName)]
pub struct Migration;

#[async_trait::async_trait]
impl MigrationTrait for Migration {
    async fn up(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        schema::up(manager).await
    }

    async fn down(&self, manager: &SchemaManager) -> Result<(), DbErr> {
        schema::down(manager).await
    }
}

#[derive(Iden)]
enum BlogPosts {
    #[iden = "blog_posts"]
    Table,
    Id,
    TenantId,
    AuthorId,
    CategoryId,
    Status,
    Slug,
    Metadata,
    FeaturedImageUrl,
    PublishedAt,
    CreatedAt,
    UpdatedAt,
    ArchivedAt,
    ScheduledAt,
    IsPinned,
    PinnedAt,
    CommentCount,
    Version,
}

#[derive(Iden)]
enum BlogPostTranslations {
    #[iden = "blog_post_translations"]
    Table,
    Id,
    PostId,
    Locale,
    Title,
    Excerpt,
    SeoTitle,
    SeoDescription,
    Body,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum BlogPostChannelVisibility {
    #[iden = "blog_post_channel_visibility"]
    Table,
    Id,
    PostId,
    TenantId,
    ChannelSlug,
    CreatedAt,
}

#[derive(Iden)]
enum BlogCategories {
    #[iden = "blog_categories"]
    Table,
    Id,
    TenantId,
    Settings,
    Revision,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum BlogPostTags {
    #[iden = "blog_post_tags"]
    Table,
    PostId,
    TagId,
    TenantId,
    CreatedAt,
}

#[derive(Iden)]
enum BlogTagUsage {
    #[iden = "blog_tag_usage"]
    Table,
    TenantId,
    TagId,
    CanonicalKey,
    UseCount,
}

#[derive(Iden)]
enum BlogPostRoutes {
    #[iden = "blog_post_routes"]
    Table,
    TenantId,
    SourceRoute,
    TargetKind,
    TargetId,
    CreatedAt,
    UpdatedAt,
}

#[derive(Iden)]
enum BlogPreviewTokens {
    #[iden = "blog_preview_tokens"]
    Table,
    Id,
    TenantId,
    PostId,
    Token,
    CreatedBy,
    ExpiresAt,
    CreatedAt,
}

#[derive(Iden)]
enum BlogCommentProjectionDeliveries {
    #[iden = "blog_comment_projection_deliveries"]
    Table,
    EventId,
    TenantId,
    CommentId,
    PostId,
    Delta,
    ProjectionRevision,
    ProcessedAt,
}

#[derive(Iden)]
enum BlogCommentsTcpDelegationScheduleState {
    #[iden = "blog_comments_tcp_delegation_schedule_state"]
    Table,
    #[iden = "state_key"]
    StateKey,
    #[iden = "schema_version"]
    SchemaVersion,
    #[iden = "source"]
    Source,
    #[iden = "generation"]
    Generation,
    #[iden = "schedule_digest_hex"]
    ScheduleDigestHex,
    #[iden = "updated_at"]
    UpdatedAt,
}

#[derive(Iden)]
enum BlogCommentsTcpDelegationScheduleAuditOutbox {
    #[iden = "blog_comments_tcp_delegation_schedule_audit_outbox"]
    Table,
    #[iden = "request_id"]
    RequestId,
    #[iden = "state_key"]
    StateKey,
    #[iden = "audit_schema_version"]
    AuditSchemaVersion,
    #[iden = "event_type"]
    EventType,
    #[iden = "occurred_at_unix_ms"]
    OccurredAtUnixMs,
    #[iden = "actor_id"]
    ActorId,
    #[iden = "principal_kind"]
    PrincipalKind,
    #[iden = "operation"]
    Operation,
    #[iden = "source"]
    Source,
    #[iden = "previous_generation"]
    PreviousGeneration,
    #[iden = "candidate_generation"]
    CandidateGeneration,
    #[iden = "outcome"]
    Outcome,
    #[iden = "created_at"]
    CreatedAt,
    #[iden = "published_at"]
    PublishedAt,
    #[iden = "canonical_envelope_id"]
    CanonicalEnvelopeId,
    #[iden = "handoff_claim_token"]
    HandoffClaimToken,
    #[iden = "handoff_claim_expires_at"]
    HandoffClaimExpiresAt,
    #[iden = "handoff_attempt_count"]
    HandoffAttemptCount,
    #[iden = "handoff_next_attempt_at"]
    HandoffNextAttemptAt,
    #[iden = "handoff_last_failure_at"]
    HandoffLastFailureAt,
    #[iden = "handoff_last_failure_code"]
    HandoffLastFailureCode,
    #[iden = "handoff_dead_lettered_at"]
    HandoffDeadLetteredAt,
    #[iden = "handoff_dead_letter_reason"]
    HandoffDeadLetterReason,
    #[iden = "handoff_recovery_epoch"]
    HandoffRecoveryEpoch,
}

#[derive(Iden)]
enum BlogCommentsTcpDelegationScheduleAuditRecoveryAudits {
    #[iden = "blog_comments_tcp_delegation_schedule_audit_recovery_audits"]
    Table,
    #[iden = "audit_id"]
    AuditId,
    #[iden = "control_plane_tenant_id"]
    ControlPlaneTenantId,
    #[iden = "request_id"]
    RequestId,
    #[iden = "actor_id"]
    ActorId,
    #[iden = "action"]
    Action,
    #[iden = "reason"]
    Reason,
    #[iden = "prior_attempt_count"]
    PriorAttemptCount,
    #[iden = "recovery_epoch"]
    RecoveryEpoch,
    #[iden = "created_at"]
    CreatedAt,
}
