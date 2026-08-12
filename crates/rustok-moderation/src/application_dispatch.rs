use std::time::Duration as StdDuration;

use rustok_api::{PortActor, PortContext, PortError, PortErrorKind};
use rustok_moderation_api::{
    ApplyModerationDecisionCommand, ModerationDecisionApplication, ModerationSubjectAdapterRegistry,
};
use uuid::Uuid;

use crate::application::DEFAULT_APPLICATION_LEASE_SECONDS;
use crate::domain::ModerationApplicationOperationRecord;
use crate::error::{ModerationError, ModerationResult};
use crate::service::ModerationService;

pub const APPLICATION_ADAPTER_DEADLINE_SECONDS: u64 = 30;
pub const APPLICATION_RETRY_BASE_SECONDS: i64 = 5;
pub const APPLICATION_RETRY_MAX_SECONDS: i64 = 300;
const APPLICATION_DISPATCH_ACTOR: &str = "rustok-moderation";
const APPLICATION_DISPATCH_LOCALE: &str = "und";
const ADAPTER_MISSING_CODE: &str = "moderation.application_adapter_missing";
const COMMAND_INVALID_CODE: &str = "moderation.application_command_invalid";
const EVIDENCE_INVALID_CODE: &str = "moderation.application_evidence_invalid";

impl ModerationService {
    /// Claims and dispatches at most one due application operation.
    ///
    /// `None` means the operation was not claimable. A claimed attempt always owns a fresh
    /// lease token. Domain idempotency remains the immutable decision UUID so a lost-response
    /// retry replays the domain-owned receipt rather than applying the mutation twice.
    pub async fn dispatch_application_operation_once(
        &self,
        registry: &ModerationSubjectAdapterRegistry,
        tenant_id: Uuid,
        decision_id: Uuid,
        lease_owner: impl Into<String>,
    ) -> ModerationResult<Option<ModerationApplicationOperationRecord>> {
        let Some(operation) = self
            .claim_application_operation(
                tenant_id,
                decision_id,
                lease_owner,
                DEFAULT_APPLICATION_LEASE_SECONDS,
            )
            .await?
        else {
            return Ok(None);
        };
        let lease_token = operation.lease_token.ok_or_else(|| {
            ModerationError::Invariant(
                "claimed moderation application operation is missing its lease token".to_string(),
            )
        })?;
        if operation.attempt_count < 1 {
            return Err(ModerationError::Invariant(
                "claimed moderation application operation has an invalid attempt count".to_string(),
            ));
        }

        let command = match self
            .reconstruct_application_command(tenant_id, &operation)
            .await
        {
            Ok(command) => command,
            Err(error @ ModerationError::Database(_)) => return Err(error),
            Err(error) => {
                return self
                    .mark_application_operator_review(
                        tenant_id,
                        decision_id,
                        lease_token,
                        COMMAND_INVALID_CODE,
                        error.to_string(),
                    )
                    .await
                    .map(Some);
            }
        };

        let Some(adapter) = registry.get(&operation.subject.module, operation.subject.kind) else {
            return self
                .mark_application_retryable(
                    tenant_id,
                    decision_id,
                    lease_token,
                    ADAPTER_MISSING_CODE,
                    "the exact moderation subject adapter is not materialized",
                    application_retry_delay_seconds(operation.attempt_count),
                )
                .await
                .map(Some);
        };

        let context = application_port_context(tenant_id, decision_id, lease_token);
        match adapter.apply_moderation_decision(context, command).await {
            Ok(application) => self
                .finish_adapter_success(tenant_id, decision_id, lease_token, application)
                .await
                .map(Some),
            Err(error) => self
                .finish_adapter_error(
                    tenant_id,
                    decision_id,
                    lease_token,
                    operation.attempt_count,
                    error,
                )
                .await
                .map(Some),
        }
    }

    async fn reconstruct_application_command(
        &self,
        tenant_id: Uuid,
        operation: &ModerationApplicationOperationRecord,
    ) -> ModerationResult<ApplyModerationDecisionCommand> {
        let decision = self
            .get_decision(tenant_id, operation.decision_id)
            .await?
            .ok_or(ModerationError::DecisionNotFound(operation.decision_id))?;
        if decision.case_id != operation.case_id
            || decision.decision_hash != operation.decision_hash
            || decision.subject_revision != operation.subject.revision
        {
            return Err(ModerationError::Invariant(
                "application operation does not match its immutable moderation decision"
                    .to_string(),
            ));
        }

        let case = self
            .get_case(tenant_id, operation.case_id)
            .await?
            .ok_or(ModerationError::CaseNotFound(operation.case_id))?;
        if case.subject != operation.subject {
            return Err(ModerationError::Invariant(
                "application operation subject does not match its moderation case".to_string(),
            ));
        }

        let effect = decision.effect.ok_or_else(|| {
            ModerationError::Invariant(
                "moderation decision without a typed effect cannot be dispatched".to_string(),
            )
        })?;
        effect
            .validate_for_decision_kind(decision.decision_kind)
            .map_err(|error| ModerationError::Invariant(error.to_string()))?;

        Ok(ApplyModerationDecisionCommand {
            decision_id: decision.id,
            subject: operation.subject.clone(),
            decision_kind: decision.decision_kind,
            reason_code: decision.reason_code,
            effect,
            decision_hash: decision.decision_hash,
        })
    }

    async fn finish_adapter_success(
        &self,
        tenant_id: Uuid,
        decision_id: Uuid,
        lease_token: Uuid,
        application: ModerationDecisionApplication,
    ) -> ModerationResult<ModerationApplicationOperationRecord> {
        match self
            .mark_application_applied(tenant_id, decision_id, lease_token, application)
            .await
        {
            Ok(operation) => Ok(operation),
            Err(error @ ModerationError::ApplicationEvidenceMismatch(_)) => {
                self.mark_application_operator_review(
                    tenant_id,
                    decision_id,
                    lease_token,
                    EVIDENCE_INVALID_CODE,
                    error.to_string(),
                )
                .await
            }
            Err(error) => Err(error),
        }
    }

    async fn finish_adapter_error(
        &self,
        tenant_id: Uuid,
        decision_id: Uuid,
        lease_token: Uuid,
        attempt_count: i32,
        error: PortError,
    ) -> ModerationResult<ModerationApplicationOperationRecord> {
        if error.retryable {
            return self
                .mark_application_retryable(
                    tenant_id,
                    decision_id,
                    lease_token,
                    error.code,
                    error.message,
                    application_retry_delay_seconds(attempt_count),
                )
                .await;
        }
        if matches!(
            &error.kind,
            PortErrorKind::Conflict | PortErrorKind::InvariantViolation
        ) {
            return self
                .mark_application_operator_review(
                    tenant_id,
                    decision_id,
                    lease_token,
                    error.code,
                    error.message,
                )
                .await;
        }
        self.mark_application_rejected(
            tenant_id,
            decision_id,
            lease_token,
            error.code,
            error.message,
        )
        .await
    }
}

fn application_port_context(tenant_id: Uuid, decision_id: Uuid, lease_token: Uuid) -> PortContext {
    PortContext::new(
        tenant_id.to_string(),
        PortActor::service(APPLICATION_DISPATCH_ACTOR),
        APPLICATION_DISPATCH_LOCALE,
        format!("moderation-application:{decision_id}:{lease_token}"),
    )
    .with_causation_id(decision_id.to_string())
    .with_idempotency_key(decision_id.to_string())
    .with_deadline(StdDuration::from_secs(APPLICATION_ADAPTER_DEADLINE_SECONDS))
}

pub fn application_retry_delay_seconds(attempt_count: i32) -> i64 {
    let exponent = attempt_count.saturating_sub(1).clamp(0, 6) as u32;
    APPLICATION_RETRY_BASE_SECONDS
        .saturating_mul(1_i64 << exponent)
        .min(APPLICATION_RETRY_MAX_SECONDS)
}

#[cfg(test)]
mod tests {
    use super::*;
    use rustok_api::PortActorKind;

    #[test]
    fn retry_delay_is_bounded_exponential() {
        assert_eq!(application_retry_delay_seconds(1), 5);
        assert_eq!(application_retry_delay_seconds(2), 10);
        assert_eq!(application_retry_delay_seconds(3), 20);
        assert_eq!(application_retry_delay_seconds(6), 160);
        assert_eq!(application_retry_delay_seconds(7), 300);
        assert_eq!(application_retry_delay_seconds(100), 300);
    }

    #[test]
    fn domain_context_keeps_decision_idempotency_across_attempts() {
        let tenant_id = Uuid::new_v4();
        let decision_id = Uuid::new_v4();
        let decision_id_text = decision_id.to_string();
        let first = application_port_context(tenant_id, decision_id, Uuid::new_v4());
        let second = application_port_context(tenant_id, decision_id, Uuid::new_v4());

        assert_eq!(first.tenant_id, tenant_id.to_string());
        assert_eq!(first.actor.kind, PortActorKind::Service);
        assert_eq!(first.actor.id, APPLICATION_DISPATCH_ACTOR);
        assert_eq!(
            first.idempotency_key.as_deref(),
            Some(decision_id_text.as_str())
        );
        assert_eq!(second.idempotency_key, first.idempotency_key);
        assert_ne!(second.correlation_id, first.correlation_id);
        assert_eq!(
            first.deadline_ms,
            Some(APPLICATION_ADAPTER_DEADLINE_SECONDS * 1_000),
        );
    }
}
