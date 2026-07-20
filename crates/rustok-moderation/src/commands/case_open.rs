use chrono::Utc;
use rustok_api::PortContext;
use rustok_core::generate_id;
use sea_orm::{
    prelude::DateTimeWithTimeZone, sea_query::OnConflict, ColumnTrait, EntityTrait, QueryFilter,
    Set,
};
use uuid::Uuid;

use crate::commands::finish;
use crate::domain::{
    ModerationCaseRecord, ModerationCaseStatus, ModerationReportStatus, OpenModerationCaseCommand,
};
use crate::entities::{moderation_case, moderation_case_report, moderation_report};
use crate::error::{ModerationError, ModerationResult};
use crate::receipts::{
    admit, replay, replay_existing, request_hash, required_idempotency_key,
    ModerationReceiptAdmission, NewModerationReceipt,
};
use crate::service::{
    active_case_deduplication_key, append_event, find_active_case_by_key, map_case,
    normalize_case_command, parse_tenant_id, report_matches_case, ModerationService,
};

const OP_OPEN_CASE: &str = "open_case";

impl ModerationService {
    pub async fn open_case_replay_safe(
        &self,
        context: PortContext,
        command: OpenModerationCaseCommand,
    ) -> ModerationResult<ModerationCaseRecord> {
        context
            .require_write_semantics()
            .map_err(|error| ModerationError::Validation(error.message))?;
        let tenant_id = parse_tenant_id(&context)?;
        let command = normalize_case_command(command)?;
        let key = required_idempotency_key(&context)?;
        let hash = request_hash(OP_OPEN_CASE, &context.actor, &command)?;
        if let Some(response) = replay_existing(
            self.database(),
            tenant_id,
            OP_OPEN_CASE,
            key.as_str(),
            hash.as_str(),
        )
        .await?
        {
            return Ok(response);
        }

        match admit(self.database(), tenant_id, OP_OPEN_CASE, key, hash.as_str()).await? {
            ModerationReceiptAdmission::Replay(receipt) => {
                replay(receipt, OP_OPEN_CASE, hash.as_str())
            }
            ModerationReceiptAdmission::New(receipt) => {
                let result = open_case_in_transaction(&receipt, tenant_id, command).await;
                finish(receipt, result).await
            }
        }
    }
}

async fn open_case_in_transaction(
    receipt: &NewModerationReceipt,
    tenant_id: Uuid,
    command: OpenModerationCaseCommand,
) -> ModerationResult<ModerationCaseRecord> {
    let reports = moderation_report::Entity::find()
        .filter(moderation_report::Column::TenantId.eq(tenant_id))
        .filter(moderation_report::Column::Id.is_in(command.report_ids.clone()))
        .all(&receipt.transaction)
        .await?;
    if reports.len() != command.report_ids.len() {
        return Err(ModerationError::Validation(
            "one or more report_ids do not exist in this tenant".to_string(),
        ));
    }
    for report in &reports {
        if !report_matches_case(report, &command) {
            return Err(ModerationError::Validation(
                "all reports must reference the exact case scope and subject revision".to_string(),
            ));
        }
        let status = ModerationReportStatus::parse(report.status.as_str()).ok_or_else(|| {
            ModerationError::Invariant("unknown stored report status".to_string())
        })?;
        if !matches!(
            status,
            ModerationReportStatus::Submitted | ModerationReportStatus::Attached
        ) {
            return Err(ModerationError::LifecycleConflict {
                from: report.status.clone(),
                to: "attached".to_string(),
            });
        }
    }

    let deduplication_key = active_case_deduplication_key(&command)?;
    let case_id = generate_id();
    let now: DateTimeWithTimeZone = Utc::now().into();
    moderation_case::Entity::insert(moderation_case::ActiveModel {
        id: Set(case_id),
        tenant_id: Set(tenant_id),
        scope_kind: Set(command.scope.kind.as_str().to_string()),
        scope_id: Set(command.scope.id),
        subject_module: Set(command.subject.module.clone()),
        subject_kind: Set(command.subject.kind.as_str().to_string()),
        subject_id: Set(command.subject.id),
        subject_revision: Set(command.subject.revision),
        queue_key: Set(command.queue_key.clone()),
        policy_id: Set(command.policy_id),
        policy_version: Set(command.policy_version),
        priority: Set(command.priority.as_str().to_string()),
        status: Set(ModerationCaseStatus::Open.as_str().to_string()),
        assigned_moderator_id: Set(None),
        revision: Set(1),
        metadata: Set(command.metadata),
        deduplication_key: Set(deduplication_key.clone()),
        active_deduplication_key: Set(Some(deduplication_key.clone())),
        opened_at: Set(now),
        started_at: Set(None),
        decided_at: Set(None),
        closed_at: Set(None),
        created_at: Set(now),
        updated_at: Set(now),
    })
    .on_conflict(
        OnConflict::columns([
            moderation_case::Column::TenantId,
            moderation_case::Column::ActiveDeduplicationKey,
        ])
        .do_nothing()
        .to_owned(),
    )
    .exec_without_returning(&receipt.transaction)
    .await?;

    let stored =
        find_active_case_by_key(&receipt.transaction, tenant_id, deduplication_key.as_str())
            .await?
            .ok_or_else(|| {
                ModerationError::Invariant(
                    "active case admission completed without a readable case".to_string(),
                )
            })?;
    let created = stored.id == case_id;

    for report_id in &command.report_ids {
        moderation_case_report::Entity::insert(moderation_case_report::ActiveModel {
            tenant_id: Set(tenant_id),
            case_id: Set(stored.id),
            report_id: Set(*report_id),
            created_at: Set(now),
        })
        .on_conflict(
            OnConflict::columns([
                moderation_case_report::Column::TenantId,
                moderation_case_report::Column::CaseId,
                moderation_case_report::Column::ReportId,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(&receipt.transaction)
        .await?;
    }

    moderation_report::Entity::update_many()
        .col_expr(
            moderation_report::Column::Status,
            sea_orm::sea_query::Expr::value(ModerationReportStatus::Attached.as_str()),
        )
        .col_expr(
            moderation_report::Column::UpdatedAt,
            sea_orm::sea_query::Expr::value(now),
        )
        .filter(moderation_report::Column::TenantId.eq(tenant_id))
        .filter(moderation_report::Column::Id.is_in(command.report_ids.clone()))
        .filter(moderation_report::Column::Status.eq(ModerationReportStatus::Submitted.as_str()))
        .exec(&receipt.transaction)
        .await?;

    append_event(
        &receipt.transaction,
        tenant_id,
        "case",
        stored.id,
        if created {
            "case_opened"
        } else {
            "reports_attached"
        },
        serde_json::json!({
            "created": created,
            "report_ids": command.report_ids,
            "deduplication_key": deduplication_key,
            "subject_revision": stored.subject_revision,
        }),
    )
    .await?;
    map_case(stored)
}
