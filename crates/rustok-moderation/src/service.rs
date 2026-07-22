use chrono::Utc;
use rustok_api::{PortActorKind, PortContext};
use rustok_core::generate_id;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseConnection, DatabaseTransaction,
    EntityTrait, QueryFilter, QueryOrder, QuerySelect, Set,
};
use serde::Serialize;
use serde_json::Value;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::domain::{
    ModerationCasePriority, ModerationCaseRecord, ModerationCaseStatus, ModerationDecisionKind,
    ModerationDecisionRecord, ModerationQueueFilter, ModerationReasonCode, ModerationReportRecord,
    ModerationReportStatus, ModerationReporterKind, ModerationScopeKind, ModerationScopeRef,
    ModerationSubjectKind, ModerationSubjectRef, OpenModerationCaseCommand,
    SubmitModerationReportCommand,
};
use crate::entities::{moderation_case, moderation_decision, moderation_event, moderation_report};
use crate::error::{ModerationError, ModerationResult};

const MAX_MODULE_BYTES: usize = 100;
const MAX_QUEUE_KEY_BYTES: usize = 100;
const MAX_DESCRIPTION_REFERENCE_BYTES: usize = 500;
const MAX_METADATA_BYTES: usize = 32 * 1024;
const MAX_REPORTS_PER_CASE: usize = 100;
const MAX_QUEUE_LIMIT: u32 = 100;

#[derive(Clone)]
pub struct ModerationService {
    db: DatabaseConnection,
}

impl ModerationService {
    pub fn new(db: DatabaseConnection) -> Self {
        Self { db }
    }

    pub fn database(&self) -> &DatabaseConnection {
        &self.db
    }

    pub async fn get_report(
        &self,
        tenant_id: Uuid,
        report_id: Uuid,
    ) -> ModerationResult<Option<ModerationReportRecord>> {
        moderation_report::Entity::find_by_id(report_id)
            .filter(moderation_report::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .map(map_report)
            .transpose()
    }

    pub async fn get_case(
        &self,
        tenant_id: Uuid,
        case_id: Uuid,
    ) -> ModerationResult<Option<ModerationCaseRecord>> {
        moderation_case::Entity::find_by_id(case_id)
            .filter(moderation_case::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .map(map_case)
            .transpose()
    }

    pub async fn get_decision(
        &self,
        tenant_id: Uuid,
        decision_id: Uuid,
    ) -> ModerationResult<Option<ModerationDecisionRecord>> {
        moderation_decision::Entity::find_by_id(decision_id)
            .filter(moderation_decision::Column::TenantId.eq(tenant_id))
            .one(&self.db)
            .await?
            .map(map_decision)
            .transpose()
    }

    pub async fn list_queue_records(
        &self,
        tenant_id: Uuid,
        filter: ModerationQueueFilter,
    ) -> ModerationResult<Vec<ModerationCaseRecord>> {
        if filter.cursor.is_some() {
            return Err(ModerationError::Validation(
                "moderation queue cursors are not supported by this owner slice".to_string(),
            ));
        }
        let mut query =
            moderation_case::Entity::find().filter(moderation_case::Column::TenantId.eq(tenant_id));
        if let Some(queue_key) = filter.queue_key {
            query = query.filter(moderation_case::Column::QueueKey.eq(normalize_identifier(
                queue_key,
                "queue_key",
                MAX_QUEUE_KEY_BYTES,
            )?));
        }
        if let Some(status) = filter.status {
            query = query.filter(moderation_case::Column::Status.eq(status.as_str()));
        }
        if let Some(priority) = filter.priority {
            query = query.filter(moderation_case::Column::Priority.eq(priority.as_str()));
        }
        if let Some(moderator_id) = filter.assigned_moderator_id {
            query = query.filter(moderation_case::Column::AssignedModeratorId.eq(moderator_id));
        }
        let limit = filter.limit.clamp(1, MAX_QUEUE_LIMIT) as u64;
        let models = query
            .order_by_asc(moderation_case::Column::OpenedAt)
            .order_by_asc(moderation_case::Column::Id)
            .limit(limit)
            .all(&self.db)
            .await?;
        models.into_iter().map(map_case).collect()
    }
}

pub(crate) fn normalize_report_command(
    mut command: SubmitModerationReportCommand,
) -> ModerationResult<SubmitModerationReportCommand> {
    validate_scope(&command.scope)?;
    command.subject = normalize_subject(command.subject)?;
    command.description_reference = normalize_optional_text(
        command.description_reference,
        "description_reference",
        MAX_DESCRIPTION_REFERENCE_BYTES,
    )?;
    command.metadata = normalize_object(command.metadata, "metadata", MAX_METADATA_BYTES)?;
    Ok(command)
}

pub(crate) fn normalize_case_command(
    mut command: OpenModerationCaseCommand,
) -> ModerationResult<OpenModerationCaseCommand> {
    validate_scope(&command.scope)?;
    command.subject = normalize_subject(command.subject)?;
    command.queue_key = normalize_identifier(command.queue_key, "queue_key", MAX_QUEUE_KEY_BYTES)?;
    if command.policy_version < 1 {
        return Err(ModerationError::Validation(
            "policy_version must be at least 1".to_string(),
        ));
    }
    command.metadata = normalize_object(command.metadata, "metadata", MAX_METADATA_BYTES)?;
    command.report_ids.sort_unstable();
    command.report_ids.dedup();
    if command.report_ids.is_empty() || command.report_ids.len() > MAX_REPORTS_PER_CASE {
        return Err(ModerationError::Validation(format!(
            "report_ids must contain 1 to {MAX_REPORTS_PER_CASE} unique reports"
        )));
    }
    Ok(command)
}

pub(crate) fn validate_reporter(
    context: &PortContext,
    command: &SubmitModerationReportCommand,
) -> ModerationResult<()> {
    match command.reporter_kind {
        ModerationReporterKind::User | ModerationReporterKind::Moderator => {
            let actor_id = actor_uuid(context)?;
            if command.reporter_id != Some(actor_id) {
                return Err(ModerationError::Validation(
                    "reporter_id must match PortContext.actor for user or moderator reports"
                        .to_string(),
                ));
            }
        }
        ModerationReporterKind::DomainModule | ModerationReporterKind::AutomatedProvider => {
            if context.actor.kind == PortActorKind::User {
                return Err(ModerationError::Validation(
                    "domain and automated reports require a service or system actor".to_string(),
                ));
            }
            if command.reporter_id.is_some() {
                return Err(ModerationError::Validation(
                    "domain and automated reports must not provide reporter_id".to_string(),
                ));
            }
        }
        ModerationReporterKind::System => {
            if context.actor.kind != PortActorKind::System || command.reporter_id.is_some() {
                return Err(ModerationError::Validation(
                    "system reports require the system actor and no reporter_id".to_string(),
                ));
            }
        }
    }
    Ok(())
}

pub(crate) fn actor_uuid(context: &PortContext) -> ModerationResult<Uuid> {
    Uuid::parse_str(context.actor.id.trim()).map_err(|_| {
        ModerationError::Validation(
            "PortContext.actor.id must be a UUID for this moderation command".to_string(),
        )
    })
}

pub(crate) fn parse_tenant_id(context: &PortContext) -> ModerationResult<Uuid> {
    Uuid::parse_str(context.tenant_id.trim()).map_err(|_| {
        ModerationError::Validation(
            "PortContext.tenant_id must be a UUID for moderation ports".to_string(),
        )
    })
}

pub(crate) fn validate_policy_snapshot(value: Value) -> ModerationResult<Value> {
    normalize_object(value, "policy_snapshot", MAX_METADATA_BYTES)
}

pub(crate) fn active_case_deduplication_key(
    command: &OpenModerationCaseCommand,
) -> ModerationResult<String> {
    hash_value(&serde_json::json!({
        "version": 1,
        "scope": command.scope,
        "subject": command.subject,
        "queue_key": command.queue_key,
        "policy_id": command.policy_id,
        "policy_version": command.policy_version,
    }))
}

pub(crate) fn immutable_decision_hash<T: Serialize>(value: &T) -> ModerationResult<String> {
    let value = serde_json::to_value(value).map_err(|_| {
        ModerationError::Invariant("decision snapshot could not be normalized".to_string())
    })?;
    hash_value(&value)
}

pub(crate) async fn append_event(
    transaction: &DatabaseTransaction,
    tenant_id: Uuid,
    aggregate_kind: &str,
    aggregate_id: Uuid,
    event_type: &str,
    payload: Value,
) -> ModerationResult<()> {
    let now = Utc::now();
    moderation_event::ActiveModel {
        id: Set(generate_id()),
        tenant_id: Set(tenant_id),
        aggregate_kind: Set(aggregate_kind.to_string()),
        aggregate_id: Set(aggregate_id),
        event_type: Set(event_type.to_string()),
        payload: Set(payload),
        occurred_at: Set(now.into()),
        created_at: Set(now.into()),
    }
    .insert(transaction)
    .await?;
    Ok(())
}

pub(crate) async fn find_case<C>(
    connection: &C,
    tenant_id: Uuid,
    case_id: Uuid,
) -> ModerationResult<moderation_case::Model>
where
    C: ConnectionTrait,
{
    moderation_case::Entity::find_by_id(case_id)
        .filter(moderation_case::Column::TenantId.eq(tenant_id))
        .one(connection)
        .await?
        .ok_or(ModerationError::CaseNotFound(case_id))
}

pub(crate) async fn find_active_case_by_key<C>(
    connection: &C,
    tenant_id: Uuid,
    key: &str,
) -> ModerationResult<Option<moderation_case::Model>>
where
    C: ConnectionTrait,
{
    moderation_case::Entity::find()
        .filter(moderation_case::Column::TenantId.eq(tenant_id))
        .filter(moderation_case::Column::ActiveDeduplicationKey.eq(key))
        .one(connection)
        .await
        .map_err(Into::into)
}

pub(crate) fn report_matches_case(
    report: &moderation_report::Model,
    command: &OpenModerationCaseCommand,
) -> bool {
    report.scope_kind == command.scope.kind.as_str()
        && report.scope_id == command.scope.id
        && report.subject_module == command.subject.module
        && report.subject_kind == command.subject.kind.as_str()
        && report.subject_id == command.subject.id
        && report.subject_revision == command.subject.revision
}

pub(crate) fn map_report(
    model: moderation_report::Model,
) -> ModerationResult<ModerationReportRecord> {
    Ok(ModerationReportRecord {
        id: model.id,
        tenant_id: model.tenant_id,
        scope: stored_scope(model.scope_kind.as_str(), model.scope_id)?,
        subject: stored_subject(
            model.subject_module,
            model.subject_kind.as_str(),
            model.subject_id,
            model.subject_revision,
        )?,
        reporter_kind: ModerationReporterKind::parse(model.reporter_kind.as_str()).ok_or_else(
            || ModerationError::Invariant("unknown stored reporter kind".to_string()),
        )?,
        reporter_id: model.reporter_id,
        reason_code: ModerationReasonCode::parse(model.reason_code.as_str()).ok_or_else(|| {
            ModerationError::Invariant("unknown stored moderation reason".to_string())
        })?,
        description_reference: model.description_reference,
        status: ModerationReportStatus::parse(model.status.as_str()).ok_or_else(|| {
            ModerationError::Invariant("unknown stored report status".to_string())
        })?,
        metadata: model.metadata,
        created_at: model.created_at.with_timezone(&Utc),
        updated_at: model.updated_at.with_timezone(&Utc),
    })
}

pub(crate) fn map_case(model: moderation_case::Model) -> ModerationResult<ModerationCaseRecord> {
    Ok(ModerationCaseRecord {
        id: model.id,
        tenant_id: model.tenant_id,
        scope: stored_scope(model.scope_kind.as_str(), model.scope_id)?,
        subject: stored_subject(
            model.subject_module,
            model.subject_kind.as_str(),
            model.subject_id,
            model.subject_revision,
        )?,
        queue_key: model.queue_key,
        policy_id: model.policy_id,
        policy_version: model.policy_version,
        priority: ModerationCasePriority::parse(model.priority.as_str()).ok_or_else(|| {
            ModerationError::Invariant("unknown stored case priority".to_string())
        })?,
        status: ModerationCaseStatus::parse(model.status.as_str())
            .ok_or_else(|| ModerationError::Invariant("unknown stored case status".to_string()))?,
        assigned_moderator_id: model.assigned_moderator_id,
        revision: model.revision,
        metadata: model.metadata,
        opened_at: model.opened_at.with_timezone(&Utc),
        started_at: model.started_at.map(|value| value.with_timezone(&Utc)),
        decided_at: model.decided_at.map(|value| value.with_timezone(&Utc)),
        closed_at: model.closed_at.map(|value| value.with_timezone(&Utc)),
        created_at: model.created_at.with_timezone(&Utc),
        updated_at: model.updated_at.with_timezone(&Utc),
    })
}

pub(crate) fn map_decision(
    model: moderation_decision::Model,
) -> ModerationResult<ModerationDecisionRecord> {
    Ok(ModerationDecisionRecord {
        id: model.id,
        tenant_id: model.tenant_id,
        case_id: model.case_id,
        decision_kind: ModerationDecisionKind::parse(model.decision_kind.as_str()).ok_or_else(
            || ModerationError::Invariant("unknown stored decision kind".to_string()),
        )?,
        reason_code: ModerationReasonCode::parse(model.reason_code.as_str()).ok_or_else(|| {
            ModerationError::Invariant("unknown stored decision reason".to_string())
        })?,
        policy_snapshot: model.policy_snapshot,
        subject_revision: model.subject_revision,
        decision_hash: model.decision_hash,
        decided_by: model.decided_by,
        decided_at: model.decided_at.with_timezone(&Utc),
    })
}

fn normalize_subject(mut subject: ModerationSubjectRef) -> ModerationResult<ModerationSubjectRef> {
    subject.module = normalize_identifier(subject.module, "subject.module", MAX_MODULE_BYTES)?;
    if subject.revision < 0 {
        return Err(ModerationError::Validation(
            "subject.revision must not be negative".to_string(),
        ));
    }
    Ok(subject)
}

fn validate_scope(scope: &ModerationScopeRef) -> ModerationResult<()> {
    match (scope.kind, scope.id) {
        (ModerationScopeKind::Platform, None) => Ok(()),
        (ModerationScopeKind::Platform, Some(_)) => Err(ModerationError::Validation(
            "platform moderation scope must not provide an id".to_string(),
        )),
        (_, Some(_)) => Ok(()),
        (_, None) => Err(ModerationError::Validation(
            "non-platform moderation scope requires an id".to_string(),
        )),
    }
}

fn stored_scope(kind: &str, id: Option<Uuid>) -> ModerationResult<ModerationScopeRef> {
    let kind = ModerationScopeKind::parse(kind)
        .ok_or_else(|| ModerationError::Invariant("unknown stored moderation scope".to_string()))?;
    let scope = ModerationScopeRef { kind, id };
    validate_scope(&scope)?;
    Ok(scope)
}

fn stored_subject(
    module: String,
    kind: &str,
    id: Uuid,
    revision: i64,
) -> ModerationResult<ModerationSubjectRef> {
    normalize_subject(ModerationSubjectRef {
        module,
        kind: ModerationSubjectKind::parse(kind).ok_or_else(|| {
            ModerationError::Invariant("unknown stored moderation subject kind".to_string())
        })?,
        id,
        revision,
    })
}

fn normalize_identifier(value: String, field: &str, max_bytes: usize) -> ModerationResult<String> {
    let value = value.trim().to_ascii_lowercase();
    if value.is_empty()
        || value.len() > max_bytes
        || !value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"_-.:".contains(&byte)
        })
    {
        return Err(ModerationError::Validation(format!(
            "{field} must contain 1 to {max_bytes} lowercase identifier bytes"
        )));
    }
    Ok(value)
}

fn normalize_optional_text(
    value: Option<String>,
    field: &str,
    max_bytes: usize,
) -> ModerationResult<Option<String>> {
    value
        .map(|value| {
            let value = value.trim().to_string();
            if value.is_empty() || value.len() > max_bytes {
                return Err(ModerationError::Validation(format!(
                    "{field} must contain 1 to {max_bytes} bytes when provided"
                )));
            }
            Ok(value)
        })
        .transpose()
}

fn normalize_object(value: Value, field: &str, max_bytes: usize) -> ModerationResult<Value> {
    if !value.is_object() {
        return Err(ModerationError::Validation(format!(
            "{field} must be a JSON object"
        )));
    }
    let encoded = serde_json::to_vec(&value)
        .map_err(|_| ModerationError::Validation(format!("{field} could not be serialized")))?;
    if encoded.len() > max_bytes {
        return Err(ModerationError::Validation(format!(
            "{field} exceeds {max_bytes} bytes"
        )));
    }
    Ok(value)
}

fn hash_value(value: &Value) -> ModerationResult<String> {
    let encoded = serde_json::to_vec(value)
        .map_err(|_| ModerationError::Invariant("hash input serialization failed".to_string()))?;
    Ok(hex::encode(Sha256::digest(encoded)))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn platform_scope_rejects_an_id() {
        assert!(
            validate_scope(&ModerationScopeRef {
                kind: ModerationScopeKind::Platform,
                id: Some(Uuid::new_v4()),
            })
            .is_err()
        );
    }

    #[test]
    fn active_case_key_changes_with_source_revision() {
        let base = OpenModerationCaseCommand {
            scope: ModerationScopeRef::platform(),
            subject: ModerationSubjectRef {
                module: "forum".to_string(),
                kind: ModerationSubjectKind::ForumPost,
                id: Uuid::nil(),
                revision: 1,
            },
            queue_key: "content".to_string(),
            priority: ModerationCasePriority::Normal,
            policy_id: None,
            policy_version: 1,
            report_ids: vec![Uuid::new_v4()],
            metadata: serde_json::json!({}),
        };
        let mut changed = base.clone();
        changed.subject.revision = 2;

        assert_ne!(
            active_case_deduplication_key(&base).unwrap(),
            active_case_deduplication_key(&changed).unwrap()
        );
    }
}
