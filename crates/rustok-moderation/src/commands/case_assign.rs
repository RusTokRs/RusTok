use chrono::Utc;
use rustok_api::PortContext;
use sea_orm::{ColumnTrait, EntityTrait, QueryFilter, prelude::DateTimeWithTimeZone};
use uuid::Uuid;

use crate::commands::finish;
use crate::domain::{AssignModerationCaseCommand, ModerationCaseRecord, ModerationCaseStatus};
use crate::entities::moderation_case;
use crate::error::{ModerationError, ModerationResult};
use crate::receipts::{
    ModerationReceiptAdmission, NewModerationReceipt, admit, replay, replay_existing, request_hash,
    required_idempotency_key,
};
use crate::service::{ModerationService, append_event, find_case, map_case, parse_tenant_id};

const OP_ASSIGN_CASE: &str = "assign_case";

impl ModerationService {
    pub async fn assign_case_replay_safe(
        &self,
        context: PortContext,
        command: AssignModerationCaseCommand,
    ) -> ModerationResult<ModerationCaseRecord> {
        context
            .require_write_semantics()
            .map_err(|error| ModerationError::Validation(error.message))?;
        let tenant_id = parse_tenant_id(&context)?;
        if command.expected_revision < 1 {
            return Err(ModerationError::Validation(
                "expected_revision must be at least 1".to_string(),
            ));
        }
        let key = required_idempotency_key(&context)?;
        let hash = request_hash(OP_ASSIGN_CASE, &context.actor, &command)?;
        if let Some(response) = replay_existing(
            self.database(),
            tenant_id,
            OP_ASSIGN_CASE,
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
            OP_ASSIGN_CASE,
            key,
            hash.as_str(),
        )
        .await?
        {
            ModerationReceiptAdmission::Replay(receipt) => {
                replay(receipt, OP_ASSIGN_CASE, hash.as_str())
            }
            ModerationReceiptAdmission::New(receipt) => {
                let result = assign_case_in_transaction(&receipt, tenant_id, command).await;
                finish(receipt, result).await
            }
        }
    }
}

async fn assign_case_in_transaction(
    receipt: &NewModerationReceipt,
    tenant_id: Uuid,
    command: AssignModerationCaseCommand,
) -> ModerationResult<ModerationCaseRecord> {
    let current = find_case(&receipt.transaction, tenant_id, command.case_id).await?;
    let status = ModerationCaseStatus::parse(current.status.as_str())
        .ok_or_else(|| ModerationError::Invariant("unknown stored case status".to_string()))?;
    if !status.accepts_assignment() {
        return Err(ModerationError::LifecycleConflict {
            from: current.status,
            to: ModerationCaseStatus::Assigned.as_str().to_string(),
        });
    }
    let next_revision = command
        .expected_revision
        .checked_add(1)
        .ok_or(ModerationError::RevisionConflict)?;
    let now: DateTimeWithTimeZone = Utc::now().into();
    let started_at = current.started_at.clone().or_else(|| Some(now.clone()));
    let updated = moderation_case::Entity::update_many()
        .col_expr(
            moderation_case::Column::AssignedModeratorId,
            sea_orm::sea_query::Expr::value(Some(command.moderator_id)),
        )
        .col_expr(
            moderation_case::Column::Status,
            sea_orm::sea_query::Expr::value(ModerationCaseStatus::Assigned.as_str()),
        )
        .col_expr(
            moderation_case::Column::Revision,
            sea_orm::sea_query::Expr::value(next_revision),
        )
        .col_expr(
            moderation_case::Column::StartedAt,
            sea_orm::sea_query::Expr::value(started_at),
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
    let stored = find_case(&receipt.transaction, tenant_id, command.case_id).await?;
    append_event(
        &receipt.transaction,
        tenant_id,
        "case",
        stored.id,
        "case_assigned",
        serde_json::json!({
            "moderator_id": command.moderator_id,
            "revision": stored.revision,
        }),
    )
    .await?;
    map_case(stored)
}
