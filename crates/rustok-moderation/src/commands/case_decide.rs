use chrono::Utc;
use rustok_api::PortContext;
use rustok_core::generate_id;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, EntityTrait, QueryFilter, Set, prelude::DateTimeWithTimeZone,
};
use uuid::Uuid;

use crate::commands::finish;
use crate::domain::{DecideModerationCaseCommand, ModerationCaseStatus, ModerationDecisionRecord};
use crate::entities::{moderation_case, moderation_decision, moderation_decision_effect};
use crate::error::{ModerationError, ModerationResult};
use crate::receipts::{
    ModerationReceiptAdmission, NewModerationReceipt, admit, replay, replay_existing, request_hash,
    required_idempotency_key,
};
use crate::service::{
    ModerationService, actor_uuid, append_event, find_case, immutable_decision_hash, map_decision,
    parse_tenant_id, validate_policy_snapshot,
};

const OP_DECIDE_CASE: &str = "decide_case";

impl ModerationService {
    pub async fn decide_case_replay_safe(
        &self,
        context: PortContext,
        mut command: DecideModerationCaseCommand,
    ) -> ModerationResult<ModerationDecisionRecord> {
        context
            .require_write_semantics()
            .map_err(|error| ModerationError::Validation(error.message))?;
        let tenant_id = parse_tenant_id(&context)?;
        let decided_by = actor_uuid(&context)?;
        if command.expected_revision < 1 {
            return Err(ModerationError::Validation(
                "expected_revision must be at least 1".to_string(),
            ));
        }
        command
            .effect
            .validate_for_decision_kind(command.decision_kind)
            .map_err(|error| ModerationError::Validation(error.to_string()))?;
        command.policy_snapshot = validate_policy_snapshot(command.policy_snapshot)?;
        let key = required_idempotency_key(&context)?;
        let hash = request_hash(OP_DECIDE_CASE, &context.actor, &command)?;
        if let Some(response) = replay_existing(
            self.database(),
            tenant_id,
            OP_DECIDE_CASE,
            key.as_str(),
            hash.as_str(),
        )
        .await?
        {
            return Ok(response);
        }

        match admit(
            self.database(),
            tenant_id,
            OP_DECIDE_CASE,
            key,
            hash.as_str(),
        )
        .await?
        {
            ModerationReceiptAdmission::Replay(receipt) => {
                replay(receipt, OP_DECIDE_CASE, hash.as_str())
            }
            ModerationReceiptAdmission::New(receipt) => {
                let result =
                    decide_case_in_transaction(&receipt, tenant_id, decided_by, command).await;
                finish(receipt, result).await
            }
        }
    }
}

async fn decide_case_in_transaction(
    receipt: &NewModerationReceipt,
    tenant_id: Uuid,
    decided_by: Uuid,
    command: DecideModerationCaseCommand,
) -> ModerationResult<ModerationDecisionRecord> {
    let current = find_case(&receipt.transaction, tenant_id, command.case_id).await?;
    let status = ModerationCaseStatus::parse(current.status.as_str())
        .ok_or_else(|| ModerationError::Invariant("unknown stored case status".to_string()))?;
    if !status.accepts_decision() {
        return Err(ModerationError::LifecycleConflict {
            from: current.status,
            to: ModerationCaseStatus::Decided.as_str().to_string(),
        });
    }
    let next_revision = command
        .expected_revision
        .checked_add(1)
        .ok_or(ModerationError::RevisionConflict)?;
    let decision_hash = immutable_decision_hash(&serde_json::json!({
        "version": 2,
        "case_id": current.id,
        "case_revision": command.expected_revision,
        "subject_module": current.subject_module.clone(),
        "subject_kind": current.subject_kind.clone(),
        "subject_id": current.subject_id,
        "subject_revision": current.subject_revision,
        "decision_kind": command.decision_kind,
        "reason_code": command.reason_code,
        "effect": &command.effect,
        "policy_snapshot": command.policy_snapshot.clone(),
        "decided_by": decided_by,
    }))?;
    let effect_payload = serde_json::to_value(&command.effect).map_err(|_| {
        ModerationError::Invariant("moderation decision effect could not be serialized".to_string())
    })?;
    let now: DateTimeWithTimeZone = Utc::now().into();
    let updated = moderation_case::Entity::update_many()
        .col_expr(
            moderation_case::Column::Status,
            sea_orm::sea_query::Expr::value(ModerationCaseStatus::Decided.as_str()),
        )
        .col_expr(
            moderation_case::Column::Revision,
            sea_orm::sea_query::Expr::value(next_revision),
        )
        .col_expr(
            moderation_case::Column::DecidedAt,
            sea_orm::sea_query::Expr::value(Some(now)),
        )
        .col_expr(
            moderation_case::Column::UpdatedAt,
            sea_orm::sea_query::Expr::value(now),
        )
        .filter(moderation_case::Column::TenantId.eq(tenant_id))
        .filter(moderation_case::Column::Id.eq(command.case_id))
        .filter(moderation_case::Column::Revision.eq(command.expected_revision))
        .exec(&receipt.transaction)
        .await?;
    if updated.rows_affected != 1 {
        return Err(ModerationError::RevisionConflict);
    }

    let decision = moderation_decision::ActiveModel {
        id: Set(generate_id()),
        tenant_id: Set(tenant_id),
        case_id: Set(command.case_id),
        decision_kind: Set(command.decision_kind.as_str().to_string()),
        reason_code: Set(command.reason_code.as_str().to_string()),
        policy_snapshot: Set(command.policy_snapshot),
        subject_revision: Set(current.subject_revision),
        decision_hash: Set(decision_hash.clone()),
        decided_by: Set(decided_by),
        decided_at: Set(now),
        created_at: Set(now),
    }
    .insert(&receipt.transaction)
    .await?;
    moderation_decision_effect::ActiveModel {
        decision_id: Set(decision.id),
        tenant_id: Set(tenant_id),
        schema_version: Set(i32::from(command.effect.schema_version)),
        effect_kind: Set(command.decision_kind.as_str().to_string()),
        effect_payload: Set(effect_payload),
        created_at: Set(now),
    }
    .insert(&receipt.transaction)
    .await?;
    append_event(
        &receipt.transaction,
        tenant_id,
        "case",
        command.case_id,
        "case_decided",
        serde_json::json!({
            "decision_id": decision.id,
            "decision_kind": decision.decision_kind,
            "reason_code": decision.reason_code,
            "effect_schema_version": command.effect.schema_version,
            "decision_hash": decision_hash,
            "case_revision": next_revision,
        }),
    )
    .await?;
    map_decision(decision, Some(command.effect))
}
