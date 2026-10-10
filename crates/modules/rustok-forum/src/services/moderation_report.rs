use rustok_api::{Action, PortContext, Resource};
use rustok_core::SecurityContext;
use rustok_moderation_api::{
    ModerationReasonCode, ModerationScopeRef, ModerationSubjectKind, ModerationSubjectRef,
};
use sea_orm::{ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, TransactionTrait};
use uuid::Uuid;

use crate::audience::SharedForumAudienceFactsPort;
use crate::entities::{forum_reply, forum_topic};
use crate::error::{ForumError, ForumResult};
use crate::moderation_report::{
    FORUM_MODERATION_REPORT_CAPABILITY, ForumModerationReportCommand,
    SharedForumModerationReportPort,
};
use crate::moderation_subject::{FORUM_MODERATION_MODULE, current_subject_revision};
use crate::services::rbac::enforce_scope;
use crate::services::topic_write_audience::topic_write_audience_allows;
use crate::state_machine::ReplyStatus;

/// Files user reports about forum topics and replies with the Moderation owner.
///
/// The reporter must be an authenticated user who can read the subject through the
/// owner audience contract. Missing and audience-hidden subjects return the same
/// not-found error. Authors cannot report their own content. The subject revision is
/// read from the Forum moderation revision table, the same source that the moderation
/// decision adapter checks, so a later decision on a changed subject is rejected.
///
/// The Moderation owner stores the report and decides on it. Forum does not open
/// cases, apply decisions, or count reports for automatic escalation.
pub struct ForumModerationReportService {
    db: DatabaseConnection,
    audience_facts: Option<SharedForumAudienceFactsPort>,
    intake: Option<SharedForumModerationReportPort>,
}

impl ForumModerationReportService {
    pub fn new(
        db: DatabaseConnection,
        audience_facts: Option<SharedForumAudienceFactsPort>,
        intake: Option<SharedForumModerationReportPort>,
    ) -> Self {
        Self {
            db,
            audience_facts,
            intake,
        }
    }

    pub async fn report_topic(
        &self,
        tenant_id: Uuid,
        topic_id: Uuid,
        security: SecurityContext,
        context: PortContext,
        reason_code: ModerationReasonCode,
    ) -> ForumResult<Uuid> {
        enforce_scope(&security, Resource::ForumTopics, Action::Read)?;
        let reporter_id = require_reporter(&security)?;

        let topic = forum_topic::Entity::find_by_id(topic_id)
            .filter(forum_topic::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(ForumError::TopicNotFound(topic_id))?;
        if !topic_write_audience_allows(
            &self.db,
            self.audience_facts.clone(),
            tenant_id,
            topic_id,
            &security,
            context.clone(),
        )
        .await?
        {
            return Err(ForumError::TopicNotFound(topic_id));
        }
        if topic.author_id == Some(reporter_id) {
            return Err(ForumError::Validation(
                "Forum authors cannot report their own topic".to_string(),
            ));
        }

        let revision = self
            .subject_revision(tenant_id, ModerationSubjectKind::ForumTopic, topic_id)
            .await?;
        self.submit(
            ModerationSubjectRef {
                module: FORUM_MODERATION_MODULE.to_string(),
                kind: ModerationSubjectKind::ForumTopic,
                id: topic_id,
                revision,
            },
            reporter_id,
            reason_code,
            context,
        )
        .await
    }

    pub async fn report_reply(
        &self,
        tenant_id: Uuid,
        reply_id: Uuid,
        security: SecurityContext,
        context: PortContext,
        reason_code: ModerationReasonCode,
    ) -> ForumResult<Uuid> {
        enforce_scope(&security, Resource::ForumReplies, Action::Read)?;
        let reporter_id = require_reporter(&security)?;

        let reply = forum_reply::Entity::find_by_id(reply_id)
            .filter(forum_reply::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .ok_or(ForumError::ReplyNotFound(reply_id))?;
        // Only approved replies are visible to other readers, so only they can be reported.
        if reply.status != ReplyStatus::Approved {
            return Err(ForumError::ReplyNotFound(reply_id));
        }
        if !topic_write_audience_allows(
            &self.db,
            self.audience_facts.clone(),
            tenant_id,
            reply.topic_id,
            &security,
            context.clone(),
        )
        .await?
        {
            return Err(ForumError::ReplyNotFound(reply_id));
        }
        if reply.author_id == Some(reporter_id) {
            return Err(ForumError::Validation(
                "Forum authors cannot report their own reply".to_string(),
            ));
        }

        let revision = self
            .subject_revision(tenant_id, ModerationSubjectKind::ForumPost, reply_id)
            .await?;
        self.submit(
            ModerationSubjectRef {
                module: FORUM_MODERATION_MODULE.to_string(),
                kind: ModerationSubjectKind::ForumPost,
                id: reply_id,
                revision,
            },
            reporter_id,
            reason_code,
            context,
        )
        .await
    }

    async fn subject_revision(
        &self,
        tenant_id: Uuid,
        kind: ModerationSubjectKind,
        subject_id: Uuid,
    ) -> ForumResult<i64> {
        let transaction = self.db.begin().await?;
        let revision = current_subject_revision(&transaction, tenant_id, kind, subject_id)
            .await
            .map_err(map_intake_port_error)?;
        transaction.commit().await?;
        Ok(revision)
    }

    async fn submit(
        &self,
        subject: ModerationSubjectRef,
        reporter_id: Uuid,
        reason_code: ModerationReasonCode,
        context: PortContext,
    ) -> ForumResult<Uuid> {
        let intake = self.intake.clone().ok_or_else(|| {
            ForumError::capability_failure(
                FORUM_MODERATION_REPORT_CAPABILITY,
                "forum.moderation_report.unavailable",
                "Forum user reports are not composed on this host",
                false,
            )
        })?;
        // The key is stable for one reporter, subject revision and reason, so a retried
        // request replays the same report instead of creating a duplicate.
        let idempotency_key = format!(
            "forum-report:{}:{}:{}:{}:{}",
            reporter_id,
            subject.kind.as_str(),
            subject.id,
            subject.revision,
            reason_code.as_str()
        );
        let command = ForumModerationReportCommand {
            scope: ModerationScopeRef::platform(),
            subject,
            reporter_id,
            reason_code,
        };
        intake
            .submit_forum_report(context.with_idempotency_key(idempotency_key), command)
            .await
            .map_err(map_intake_port_error)
    }
}

fn require_reporter(security: &SecurityContext) -> ForumResult<Uuid> {
    security.user_id.ok_or_else(|| {
        ForumError::forbidden("Authenticated user context is required to report content")
    })
}

fn map_intake_port_error(error: rustok_api::PortError) -> ForumError {
    ForumError::capability_failure(
        FORUM_MODERATION_REPORT_CAPABILITY,
        error.code,
        error.message,
        error.retryable,
    )
}
