use std::str::FromStr;

use async_trait::async_trait;
use rustok_api::{Permission, PortCallPolicy, PortContext, PortError, PortErrorKind};
use uuid::Uuid;

pub use rustok_moderation_api::ModerationSubjectCommandPort;

use crate::domain::{
    AssignModerationCaseCommand, DecideModerationCaseCommand, ModerationApplicationRecoveryRecord,
    ModerationCaseRecord, ModerationDecisionRecord, ModerationQueueFilter, ModerationReportRecord,
    OpenModerationCaseCommand, ReconcileLegacyModerationApplicationCommand,
    RequeueModerationApplicationCommand, SubmitModerationReportCommand,
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

/// Administrative recovery boundary for already-terminal moderation applications.
///
/// These operations are intentionally separate from ordinary moderation commands. A caller
/// must carry the dedicated `moderation_cases:override` permission (or
/// `moderation_cases:manage`) in its trusted `PortContext` claim snapshot before the owner
/// command is entered. The owner still enforces human-user identity, replay safety, expected
/// case revision and terminal-state invariants.
#[async_trait]
pub trait ModerationRecoveryCommandPort: Send + Sync {
    async fn requeue_application(
        &self,
        context: PortContext,
        command: RequeueModerationApplicationCommand,
    ) -> Result<ModerationApplicationRecoveryRecord, PortError>;

    async fn reconcile_legacy_application(
        &self,
        context: PortContext,
        command: ReconcileLegacyModerationApplicationCommand,
    ) -> Result<ModerationApplicationRecoveryRecord, PortError>;
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
impl ModerationRecoveryCommandPort for ModerationService {
    async fn requeue_application(
        &self,
        context: PortContext,
        command: RequeueModerationApplicationCommand,
    ) -> Result<ModerationApplicationRecoveryRecord, PortError> {
        require_recovery_override(&context)?;
        self.operator_requeue_application_replay_safe(context, command)
            .await
            .map_err(map_owner_error)
    }

    async fn reconcile_legacy_application(
        &self,
        context: PortContext,
        command: ReconcileLegacyModerationApplicationCommand,
    ) -> Result<ModerationApplicationRecoveryRecord, PortError> {
        require_recovery_override(&context)?;
        self.operator_reconcile_legacy_application_replay_safe(context, command)
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

fn require_recovery_override(context: &PortContext) -> Result<(), PortError> {
    let permissions = context
        .claims
        .iter()
        .map(|claim| {
            Permission::from_str(claim).map_err(|_| {
                PortError::validation(
                    "moderation.permission_claim_invalid",
                    format!("invalid moderation permission claim: {claim}"),
                )
            })
        })
        .collect::<Result<Vec<_>, _>>()?;

    if permissions.contains(&Permission::MODERATION_CASES_OVERRIDE)
        || permissions.contains(&Permission::MODERATION_CASES_MANAGE)
    {
        return Ok(());
    }

    Err(PortError::forbidden(
        "moderation.application_recovery_forbidden",
        "moderation application recovery requires moderation_cases:override",
    ))
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
        ModerationError::ApplicationOperationNotFound(id) => PortError::not_found(
            "moderation.application_operation_not_found",
            format!("moderation application operation for decision {id} not found"),
        ),
        ModerationError::ApplicationLeaseConflict(id) => PortError::conflict(
            "moderation.application_lease_conflict",
            format!("moderation application lease conflict for decision {id}"),
        ),
        ModerationError::ApplicationRecoveryConflict(id) => PortError::conflict(
            "moderation.application_recovery_conflict",
            format!("moderation application recovery conflict for decision {id}"),
        ),
        ModerationError::ApplicationEvidenceMismatch(id) => PortError::invariant_violation(
            "moderation.application_evidence_mismatch",
            format!("moderation application evidence does not match decision {id}"),
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

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::PortActor;

    fn recovery_context() -> PortContext {
        PortContext::new(
            Uuid::new_v4().to_string(),
            PortActor::user(Uuid::new_v4().to_string()),
            "en",
            "moderation-recovery-test",
        )
        .with_claim(Permission::MODERATION_CASES_OVERRIDE.to_string())
    }

    #[test]
    fn recovery_boundary_requires_dedicated_override_permission() {
        assert!(require_recovery_override(&recovery_context()).is_ok());
        assert!(
            require_recovery_override(
                &PortContext::new(
                    Uuid::new_v4().to_string(),
                    PortActor::user(Uuid::new_v4().to_string()),
                    "en",
                    "moderation-recovery-test",
                )
                .with_claim(Permission::FORUM_TOPICS_MODERATE.to_string()),
            )
            .is_err()
        );
    }

    #[test]
    fn recovery_boundary_accepts_resource_manage_permission() {
        let context = PortContext::new(
            Uuid::new_v4().to_string(),
            PortActor::user(Uuid::new_v4().to_string()),
            "en",
            "moderation-recovery-test",
        )
        .with_claim(Permission::MODERATION_CASES_MANAGE.to_string());
        assert!(require_recovery_override(&context).is_ok());
    }
}
