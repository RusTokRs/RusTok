//! Forum-owned intake contract for user reports handed to the Moderation owner.
//!
//! Forum never depends on the `rustok-moderation` owner crate. The forum service
//! validates the reporter's audience and the subject revision, then calls this port.
//! The host composes an implementation that forwards to the Moderation owner's
//! replay-safe `submit_report` command. When no implementation is composed, reports
//! fail closed with a capability failure; they are never dropped silently.

use std::sync::Arc;

use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use rustok_moderation_api::{ModerationReasonCode, ModerationScopeRef, ModerationSubjectRef};
use uuid::Uuid;

/// Capability name used in forum error payloads for report intake failures.
pub const FORUM_MODERATION_REPORT_CAPABILITY: &str = "forum.moderation_report";

/// Command handed from Forum to the Moderation intake port.
///
/// The `context.idempotency_key` is derived by Forum from the reporter, subject,
/// subject revision and reason, so a retried request replays the same report.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ForumModerationReportCommand {
    pub scope: ModerationScopeRef,
    pub subject: ModerationSubjectRef,
    pub reporter_id: Uuid,
    pub reason_code: ModerationReasonCode,
}

#[async_trait]
pub trait ForumModerationReportPort: Send + Sync {
    /// Submit one user report and return the Moderation report id.
    async fn submit_forum_report(
        &self,
        context: PortContext,
        command: ForumModerationReportCommand,
    ) -> Result<Uuid, PortError>;
}

pub type SharedForumModerationReportPort = Arc<dyn ForumModerationReportPort>;
