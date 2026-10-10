use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use rustok_forum::{
    ForumModerationReportCommand, ForumModerationReportPort, SharedForumModerationReportPort,
};
use rustok_moderation::{
    ModerationCommandPort, ModerationReporterKind, ModerationService,
    SubmitModerationReportCommand,
};
use sea_orm::DatabaseConnection;
use serde_json::json;
use uuid::Uuid;

/// Host composition that files Forum user reports with the Moderation owner.
///
/// Forum owns the reporter audience and subject revision checks. This adapter only
/// maps the Forum command onto the Moderation owner's replay-safe `submit_report`
/// command, so the report is stored, queued, and decided by `rustok-moderation`.
#[derive(Clone)]
pub(crate) struct ServerForumModerationReportPort {
    db: DatabaseConnection,
}

impl ServerForumModerationReportPort {
    pub(crate) fn shared(db: DatabaseConnection) -> SharedForumModerationReportPort {
        Arc::new(Self { db })
    }
}

#[async_trait]
impl ForumModerationReportPort for ServerForumModerationReportPort {
    async fn submit_forum_report(
        &self,
        context: PortContext,
        command: ForumModerationReportCommand,
    ) -> Result<Uuid, PortError> {
        let service = ModerationService::new(self.db.clone());
        let report = ModerationCommandPort::submit_report(
            &service,
            context,
            SubmitModerationReportCommand {
                scope: command.scope,
                subject: command.subject,
                reporter_kind: ModerationReporterKind::User,
                reporter_id: Some(command.reporter_id),
                reason_code: command.reason_code,
                description_reference: None,
                metadata: json!({ "source": "forum" }),
            },
        )
        .await?;
        Ok(report.id)
    }
}
