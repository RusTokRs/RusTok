use chrono::Utc;
use rustok_api::PortContext;
use rustok_core::generate_id;
use sea_orm::{prelude::DateTimeWithTimeZone, ActiveModelTrait, Set};
use uuid::Uuid;

use crate::commands::finish;
use crate::domain::{
    ModerationReportRecord, ModerationReportStatus, SubmitModerationReportCommand,
};
use crate::entities::moderation_report;
use crate::error::ModerationResult;
use crate::receipts::{
    admit, replay, replay_existing, request_hash, required_idempotency_key,
    ModerationReceiptAdmission, NewModerationReceipt,
};
use crate::service::{
    append_event, map_report, normalize_report_command, parse_tenant_id, validate_reporter,
    ModerationService,
};

const OP_SUBMIT_REPORT: &str = "submit_report";

impl ModerationService {
    pub async fn submit_report_replay_safe(
        &self,
        context: PortContext,
        command: SubmitModerationReportCommand,
    ) -> ModerationResult<ModerationReportRecord> {
        context
            .require_write_semantics()
            .map_err(|error| crate::error::ModerationError::Validation(error.message))?;
        let tenant_id = parse_tenant_id(&context)?;
        let command = normalize_report_command(command)?;
        validate_reporter(&context, &command)?;
        let key = required_idempotency_key(&context)?;
        let hash = request_hash(OP_SUBMIT_REPORT, &context.actor, &command)?;
        if let Some(response) = replay_existing(
            self.database(),
            tenant_id,
            OP_SUBMIT_REPORT,
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
            OP_SUBMIT_REPORT,
            key,
            hash.as_str(),
        )
        .await?
        {
            ModerationReceiptAdmission::Replay(receipt) => {
                replay(receipt, OP_SUBMIT_REPORT, hash.as_str())
            }
            ModerationReceiptAdmission::New(receipt) => {
                let result = submit_report_in_transaction(&receipt, tenant_id, command).await;
                finish(receipt, result).await
            }
        }
    }
}

async fn submit_report_in_transaction(
    receipt: &NewModerationReceipt,
    tenant_id: Uuid,
    command: SubmitModerationReportCommand,
) -> ModerationResult<ModerationReportRecord> {
    let now: DateTimeWithTimeZone = Utc::now().into();
    let model = moderation_report::ActiveModel {
        id: Set(generate_id()),
        tenant_id: Set(tenant_id),
        scope_kind: Set(command.scope.kind.as_str().to_string()),
        scope_id: Set(command.scope.id),
        subject_module: Set(command.subject.module),
        subject_kind: Set(command.subject.kind.as_str().to_string()),
        subject_id: Set(command.subject.id),
        subject_revision: Set(command.subject.revision),
        reporter_kind: Set(command.reporter_kind.as_str().to_string()),
        reporter_id: Set(command.reporter_id),
        reason_code: Set(command.reason_code.as_str().to_string()),
        description_reference: Set(command.description_reference),
        status: Set(ModerationReportStatus::Submitted.as_str().to_string()),
        metadata: Set(command.metadata),
        created_at: Set(now),
        updated_at: Set(now),
    }
    .insert(&receipt.transaction)
    .await?;
    append_event(
        &receipt.transaction,
        tenant_id,
        "report",
        model.id,
        "report_submitted",
        serde_json::json!({
            "subject_module": model.subject_module,
            "subject_kind": model.subject_kind,
            "subject_id": model.subject_id,
            "subject_revision": model.subject_revision,
            "reason_code": model.reason_code,
        }),
    )
    .await?;
    map_report(model)
}
