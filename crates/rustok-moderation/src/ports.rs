use async_trait::async_trait;
use rustok_api::{PortCallPolicy, PortContext, PortError, PortErrorKind};
use uuid::Uuid;

use crate::domain::{
    ApplyModerationDecisionCommand, AssignModerationCaseCommand, DecideModerationCaseCommand,
    ModerationCaseRecord, ModerationDecisionApplication, ModerationDecisionRecord,
    ModerationQueueFilter, ModerationReportRecord, OpenModerationCaseCommand,
    SubmitModerationReportCommand,
};
use crate::error::ModerationError;
use crate::service::{ModerationService, parse_tenant_id};

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

#[async_trait]
impl ModerationCommandPort for ModerationService {
    async fn submit_report(
        &self,
        context: PortContext,
        command: SubmitModerationReportCommand,
    ) -> Result<ModerationReportRecord, PortError> {
        self.submit_report_replay_safe(context, command)
            .await
            .map_err(map_owner_error)
    }

    async fn open_case(
        &self,
        context: PortContext,
        command: OpenModerationCaseCommand,
    ) -> Result<ModerationCaseRecord, PortError> {
        self.open_case_replay_safe(context, command)
            .await
            .map_err(map_owner_error)
    }

    async fn assign_case(
        &self,
        context: PortContext,
        command: AssignModerationCaseCommand,
    ) -> Result<ModerationCaseRecord, PortError> {
        self.assign_case_replay_safe(context, command)
            .await
            .map_err(map_owner_error)
    }

    async fn decide_case(
        &self,
        context: PortContext,
        command: DecideModerationCaseCommand,
    ) -> Result<ModerationDecisionRecord, PortError> {
        self.decide_case_replay_safe(context, command)
            .await
            .map_err(map_owner_error)
    }
}

#[async_trait]
impl ModerationReadPort for ModerationService {
    async fn read_report(
        &self,
        context: PortContext,
        report_id: Uuid,
    ) -> Result<Option<ModerationReportRecord>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        self.get_report(
            parse_tenant_id(&context).map_err(map_owner_error)?,
            report_id,
        )
        .await
        .map_err(map_owner_error)
    }

    async fn read_case(
        &self,
        context: PortContext,
        case_id: Uuid,
    ) -> Result<Option<ModerationCaseRecord>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        self.get_case(parse_tenant_id(&context).map_err(map_owner_error)?, case_id)
            .await
            .map_err(map_owner_error)
    }

    async fn read_decision(
        &self,
        context: PortContext,
        decision_id: Uuid,
    ) -> Result<Option<ModerationDecisionRecord>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        self.get_decision(
            parse_tenant_id(&context).map_err(map_owner_error)?,
            decision_id,
        )
        .await
        .map_err(map_owner_error)
    }

    async fn list_queue(
        &self,
        context: PortContext,
        filter: ModerationQueueFilter,
    ) -> Result<Vec<ModerationCaseRecord>, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        self.list_queue_records(parse_tenant_id(&context).map_err(map_owner_error)?, filter)
            .await
            .map_err(map_owner_error)
    }
}

fn map_owner_error(error: ModerationError) -> PortError {
    match error {
        ModerationError::ReportNotFound(id) => PortError::not_found(
            "moderation.report_not_found",
            format!("moderation report {id} not found"),
        ),
        ModerationError::CaseNotFound(id) => PortError::not_found(
            "moderation.case_not_found",
            format!("moderation case {id} not found"),
        ),
        ModerationError::DecisionNotFound(id) => PortError::not_found(
            "moderation.decision_not_found",
            format!("moderation decision {id} not found"),
        ),
        ModerationError::Validation(message) => {
            PortError::validation("moderation.validation", message)
        }
        ModerationError::RevisionConflict => PortError::conflict(
            "moderation.revision_conflict",
            "moderation case revision changed before the command was applied",
        ),
        ModerationError::LifecycleConflict { from, to } => PortError::conflict(
            "moderation.lifecycle_conflict",
            format!("moderation transition from `{from}` to `{to}` is not allowed"),
        ),
        ModerationError::IdempotencyConflict => PortError::conflict(
            "moderation.idempotency_conflict",
            "moderation idempotency key is already bound to another command",
        ),
        ModerationError::CommandReceiptCorrupt => PortError::invariant_violation(
            "moderation.command_receipt_corrupt",
            "moderation command receipt requires operator review",
        ),
        ModerationError::Invariant(message) => {
            PortError::invariant_violation("moderation.invariant", message)
        }
        ModerationError::Database(_) => PortError::new(
            PortErrorKind::Unavailable,
            "moderation.storage_unavailable",
            "moderation storage is temporarily unavailable",
            true,
        ),
    }
}
