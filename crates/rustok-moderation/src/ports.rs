use async_trait::async_trait;
use rustok_api::{PortContext, PortError};
use uuid::Uuid;

use crate::domain::{
    ApplyModerationDecisionCommand, AssignModerationCaseCommand, DecideModerationCaseCommand,
    ModerationCaseRecord, ModerationDecisionApplication, ModerationDecisionRecord,
    ModerationQueueFilter, ModerationReportRecord, OpenModerationCaseCommand,
    SubmitModerationReportCommand,
};

#[async_trait]
pub trait ModerationCommandPort: Send + Sync {
    async fn submit_report(
        &self,
        context: PortContext,
        command: SubmitModerationReportCommand,
    ) -> Result<ModerationReportRecord, PortError>;

    async fn open_case(
        &self,
        context: PortContext,
        command: OpenModerationCaseCommand,
    ) -> Result<ModerationCaseRecord, PortError>;

    async fn assign_case(
        &self,
        context: PortContext,
        command: AssignModerationCaseCommand,
    ) -> Result<ModerationCaseRecord, PortError>;

    async fn decide_case(
        &self,
        context: PortContext,
        command: DecideModerationCaseCommand,
    ) -> Result<ModerationDecisionRecord, PortError>;
}

#[async_trait]
pub trait ModerationReadPort: Send + Sync {
    async fn read_report(
        &self,
        context: PortContext,
        report_id: Uuid,
    ) -> Result<Option<ModerationReportRecord>, PortError>;

    async fn read_case(
        &self,
        context: PortContext,
        case_id: Uuid,
    ) -> Result<Option<ModerationCaseRecord>, PortError>;

    async fn read_decision(
        &self,
        context: PortContext,
        decision_id: Uuid,
    ) -> Result<Option<ModerationDecisionRecord>, PortError>;

    async fn list_queue(
        &self,
        context: PortContext,
        filter: ModerationQueueFilter,
    ) -> Result<Vec<ModerationCaseRecord>, PortError>;
}

/// Implemented by each domain owner that accepts moderation decisions.
///
/// The moderation owner never updates forum, blog, comment, review, group,
/// listing, seller, media, message, or profile tables directly.
#[async_trait]
pub trait ModerationSubjectCommandPort: Send + Sync {
    async fn apply_moderation_decision(
        &self,
        context: PortContext,
        command: ApplyModerationDecisionCommand,
    ) -> Result<ModerationDecisionApplication, PortError>;
}
