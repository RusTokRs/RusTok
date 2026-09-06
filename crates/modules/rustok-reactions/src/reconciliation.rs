use std::collections::{BTreeMap, BTreeSet};

use chrono::Utc;
use rustok_api::{PortCallPolicy, PortContext, PortError};
use rustok_events::{MAX_REACTIONS_EVENT_KEYS, ReactionsEvent};
use rustok_outbox::{ContractEventWriteOnceError, TransactionalEventBus, idempotency};
use rustok_reactions_api::{ReactionCatalog, ReactionKey, ReactionSubjectRef};
use sea_orm::sea_query::Expr;
use sea_orm::{
    ActiveModelTrait, ColumnTrait, ConnectionTrait, DatabaseTransaction, EntityTrait,
    PaginatorTrait, QueryFilter, QueryOrder, Set, TransactionTrait,
};
use serde::{Deserialize, Serialize, de::DeserializeOwned};
use uuid::Uuid;

use crate::ReactionsService;
use crate::entities::{actor_state, aggregate, catalog, subject};

pub const MAX_REACTION_RECONCILIATION_ACTOR_STATES: u32 = 1_000;
pub const MAX_REACTION_RECONCILIATION_ISSUES: usize = 64;
const MAX_REACTION_RECONCILIATION_AGGREGATE_ROWS: u64 = 128;
const REACTION_OWNER_SLUG: &str = "reactions";
const RECONCILE_REACTION_SUBJECT_OPERATION: &str = "reconcile_subject";
const RECONCILE_REACTIONS_CLAIM: &str = "reactions:reconcile";

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ReactionReconciliationStatus {
    Clean,
    Drift,
    Blocked,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReactionReconciliationRequest {
    pub subject: ReactionSubjectRef,
    pub max_actor_states: u32,
}

impl ReactionReconciliationRequest {
    pub fn new(subject: ReactionSubjectRef, max_actor_states: u32) -> Result<Self, PortError> {
        let request = Self {
            subject,
            max_actor_states,
        };
        request.validate()?;
        Ok(request)
    }

    fn validate(&self) -> Result<(), PortError> {
        if self.max_actor_states == 0
            || self.max_actor_states > MAX_REACTION_RECONCILIATION_ACTOR_STATES
        {
            return Err(PortError::validation(
                "reactions.reconciliation_scope_invalid",
                format!(
                    "max_actor_states must be between 1 and {MAX_REACTION_RECONCILIATION_ACTOR_STATES}"
                ),
            ));
        }
        Ok(())
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct RepairReactionSubjectCommand {
    pub command_id: Uuid,
    pub request: ReactionReconciliationRequest,
}

impl RepairReactionSubjectCommand {
    pub fn new(
        command_id: Uuid,
        request: ReactionReconciliationRequest,
    ) -> Result<Self, PortError> {
        if command_id.is_nil() {
            return Err(PortError::validation(
                "reactions.reconciliation_command_id_invalid",
                "reaction reconciliation command UUID must not be nil",
            ));
        }
        request.validate()?;
        Ok(Self {
            command_id,
            request,
        })
    }

    fn validate(&self) -> Result<(), PortError> {
        if self.command_id.is_nil() {
            return Err(PortError::validation(
                "reactions.reconciliation_command_id_invalid",
                "reaction reconciliation command UUID must not be nil",
            ));
        }
        self.request.validate()
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReactionReconciliationIssue {
    pub code: String,
    pub actor_id: Option<Uuid>,
    pub reaction: Option<ReactionKey>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReactionAggregateComparison {
    pub reaction: ReactionKey,
    pub expected: u64,
    pub actual: i64,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReactionReconciliationReport {
    pub subject: ReactionSubjectRef,
    pub catalog_revision: u64,
    pub status: ReactionReconciliationStatus,
    pub actor_states_scanned: u64,
    pub aggregate_rows_scanned: u64,
    pub comparisons: Vec<ReactionAggregateComparison>,
    pub issues: Vec<ReactionReconciliationIssue>,
    pub issues_truncated: bool,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct ReactionReconciliationReceipt {
    pub command_id: Uuid,
    pub subject: ReactionSubjectRef,
    pub changed: bool,
    pub actor_states_scanned: u64,
    pub aggregate_rows_before: u64,
    pub aggregate_rows_after: u64,
    pub changed_key_count: u64,
}

struct ReconciliationAnalysis {
    report: ReactionReconciliationReport,
    reaction_subject_id: Uuid,
    expected: BTreeMap<ReactionKey, u64>,
    changed_key_count: u64,
    changed_key_sample: Vec<String>,
}

impl ReactionsService {
    pub async fn inspect_reconciliation(
        &self,
        context: PortContext,
        request: ReactionReconciliationRequest,
    ) -> Result<ReactionReconciliationReport, PortError> {
        context.require_policy(PortCallPolicy::read())?;
        authorize_reconciliation(&context, &request)?;
        request.validate()?;
        Ok(analyze_reconciliation(self.database(), &request)
            .await?
            .report)
    }

    pub async fn repair_reconciliation(
        &self,
        context: PortContext,
        command: RepairReactionSubjectCommand,
    ) -> Result<ReactionReconciliationReceipt, PortError> {
        context.require_policy(PortCallPolicy::write())?;
        command.validate()?;
        authorize_reconciliation(&context, &command.request)?;

        let expected_key = command.command_id.to_string();
        if context.idempotency_key.as_deref() != Some(expected_key.as_str()) {
            return Err(PortError::validation(
                "reactions.reconciliation_command_idempotency_mismatch",
                "reaction reconciliation command UUID must equal the port idempotency key",
            ));
        }

        let tenant_id = command.request.subject.tenant_id();
        let lease = match idempotency::admit(
            self.database(),
            idempotency::OwnerOperationScope::Tenant(tenant_id),
            REACTION_OWNER_SLUG,
            expected_key.as_str(),
            RECONCILE_REACTION_SUBJECT_OPERATION,
            &command,
        )
        .await?
        {
            idempotency::Admission::Run(lease) => lease,
            idempotency::Admission::Replay(value) => return decode_replay(value),
            idempotency::Admission::ReplayError(error) => return Err(error),
        };

        let actor_id = Uuid::parse_str(&context.actor.id).ok();
        let result = self.execute_repair(lease, actor_id, &command).await;
        if let Err(error) = &result
            && let Err(receipt_error) = idempotency::fail(self.database(), lease, error).await
        {
            tracing::error!(
                operation_id = %lease.operation_id,
                error = %receipt_error.message,
                "failed to persist Reactions reconciliation failure receipt"
            );
        }
        result
    }

    async fn execute_repair(
        &self,
        lease: idempotency::Lease,
        actor_id: Option<Uuid>,
        command: &RepairReactionSubjectCommand,
    ) -> Result<ReactionReconciliationReceipt, PortError> {
        let transaction = self.database().begin().await.map_err(database_error)?;
        let receipt =
            repair_inside_transaction(&transaction, lease.operation_id, actor_id, command).await?;
        idempotency::complete(&transaction, lease, &receipt).await?;
        transaction.commit().await.map_err(database_error)?;
        Ok(receipt)
    }
}

async fn repair_inside_transaction(
    transaction: &DatabaseTransaction,
    event_envelope_id: Uuid,
    actor_id: Option<Uuid>,
    command: &RepairReactionSubjectCommand,
) -> Result<ReactionReconciliationReceipt, PortError> {
    serialize_subject(transaction, &command.request.subject).await?;
    let analysis = analyze_reconciliation(transaction, &command.request).await?;

    if analysis.report.status == ReactionReconciliationStatus::Blocked {
        return Err(PortError::conflict(
            "reactions.reconciliation_blocked",
            "reaction aggregate repair is blocked by catalog or actor-state corruption",
        ));
    }

    let aggregate_rows_before = analysis.report.aggregate_rows_scanned;
    if analysis.report.status == ReactionReconciliationStatus::Clean {
        return Ok(ReactionReconciliationReceipt {
            command_id: command.command_id,
            subject: command.request.subject.clone(),
            changed: false,
            actor_states_scanned: analysis.report.actor_states_scanned,
            aggregate_rows_before,
            aggregate_rows_after: aggregate_rows_before,
            changed_key_count: 0,
        });
    }

    aggregate::Entity::delete_many()
        .filter(aggregate::Column::TenantId.eq(command.request.subject.tenant_id()))
        .filter(aggregate::Column::ReactionSubjectId.eq(analysis.reaction_subject_id))
        .exec(transaction)
        .await
        .map_err(database_error)?;

    let now = Utc::now().fixed_offset();
    for (reaction, count) in &analysis.expected {
        if *count == 0 {
            continue;
        }
        aggregate::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(command.request.subject.tenant_id()),
            reaction_subject_id: Set(analysis.reaction_subject_id),
            reaction_key: Set(reaction.to_string()),
            count: Set(u64_to_i64(*count, "reconciliation aggregate count")?),
            created_at: Set(now),
            updated_at: Set(now),
        }
        .insert(transaction)
        .await
        .map_err(database_error)?;
    }

    let aggregate_rows_after = u64::try_from(
        analysis
            .expected
            .values()
            .filter(|count| **count > 0)
            .count(),
    )
    .map_err(|_| {
        PortError::invariant_violation(
            "reactions.reconciliation_count_out_of_range",
            "reaction reconciliation aggregate row count exceeds u64",
        )
    })?;

    let event = ReactionsEvent::SubjectReconciled {
        repair_command_id: command.command_id,
        source_slug: command.request.subject.source().to_string(),
        subject_kind: command.request.subject.kind().to_string(),
        subject_id: command.request.subject.subject_id(),
        subject_revision: u64_to_i64(
            command.request.subject.subject_revision(),
            "reconciliation subject revision",
        )?,
        catalog_revision: u64_to_i64(
            analysis.report.catalog_revision,
            "reconciliation catalog revision",
        )?,
        actor_states_scanned: u64_to_i64(
            analysis.report.actor_states_scanned,
            "reconciliation actor-state count",
        )?,
        aggregate_rows_before: u64_to_i64(
            aggregate_rows_before,
            "reconciliation aggregate rows before",
        )?,
        aggregate_rows_after: u64_to_i64(
            aggregate_rows_after,
            "reconciliation aggregate rows after",
        )?,
        changed_key_count: u64_to_i64(
            analysis.changed_key_count,
            "reconciliation changed-key count",
        )?,
        changed_keys: analysis.changed_key_sample.clone(),
        changed_keys_truncated: analysis.changed_key_count
            > u64::try_from(analysis.changed_key_sample.len()).unwrap_or(u64::MAX),
    };
    publish_event_once(
        transaction,
        event_envelope_id,
        command.request.subject.tenant_id(),
        actor_id,
        event,
    )
    .await?;

    Ok(ReactionReconciliationReceipt {
        command_id: command.command_id,
        subject: command.request.subject.clone(),
        changed: true,
        actor_states_scanned: analysis.report.actor_states_scanned,
        aggregate_rows_before,
        aggregate_rows_after,
        changed_key_count: analysis.changed_key_count,
    })
}

async fn serialize_subject(
    transaction: &DatabaseTransaction,
    subject_ref: &ReactionSubjectRef,
) -> Result<(), PortError> {
    let update = subject::Entity::update_many()
        .col_expr(
            subject::Column::UpdatedAt,
            Expr::value(Utc::now().fixed_offset()),
        )
        .filter(subject::Column::TenantId.eq(subject_ref.tenant_id()))
        .filter(subject::Column::SourceSlug.eq(subject_ref.source().as_str()))
        .filter(subject::Column::SubjectKind.eq(subject_ref.kind().as_str()))
        .filter(subject::Column::SubjectId.eq(subject_ref.subject_id()))
        .exec(transaction)
        .await
        .map_err(database_error)?;
    if update.rows_affected != 1 {
        return Err(PortError::not_found(
            "reactions.reconciliation_subject_missing",
            "reaction subject is not present in owner storage",
        ));
    }
    Ok(())
}

async fn analyze_reconciliation<C>(
    connection: &C,
    request: &ReactionReconciliationRequest,
) -> Result<ReconciliationAnalysis, PortError>
where
    C: ConnectionTrait,
{
    request.validate()?;
    let stored_subject = subject::Entity::find()
        .filter(subject::Column::TenantId.eq(request.subject.tenant_id()))
        .filter(subject::Column::SourceSlug.eq(request.subject.source().as_str()))
        .filter(subject::Column::SubjectKind.eq(request.subject.kind().as_str()))
        .filter(subject::Column::SubjectId.eq(request.subject.subject_id()))
        .one(connection)
        .await
        .map_err(database_error)?
        .ok_or_else(|| {
            PortError::not_found(
                "reactions.reconciliation_subject_missing",
                "reaction subject is not present in owner storage",
            )
        })?;

    let stored_subject_revision = i64_to_u64(
        stored_subject.subject_revision,
        "stored reaction subject revision",
    )?;
    if stored_subject_revision != request.subject.subject_revision() {
        return Err(PortError::conflict(
            "reactions.reconciliation_subject_revision_stale",
            "reaction reconciliation targets a stale subject revision",
        ));
    }
    let catalog_revision = i64_to_u64(
        stored_subject.current_catalog_revision,
        "stored reaction catalog revision",
    )?;

    let actor_count = actor_state::Entity::find()
        .filter(actor_state::Column::TenantId.eq(request.subject.tenant_id()))
        .filter(actor_state::Column::ReactionSubjectId.eq(stored_subject.id))
        .count(connection)
        .await
        .map_err(database_error)?;
    if actor_count > u64::from(request.max_actor_states) {
        return Err(PortError::conflict(
            "reactions.reconciliation_scope_exceeded",
            "reaction subject contains more actor states than the requested reconciliation bound",
        ));
    }

    let aggregate_count = aggregate::Entity::find()
        .filter(aggregate::Column::TenantId.eq(request.subject.tenant_id()))
        .filter(aggregate::Column::ReactionSubjectId.eq(stored_subject.id))
        .count(connection)
        .await
        .map_err(database_error)?;
    if aggregate_count > MAX_REACTION_RECONCILIATION_AGGREGATE_ROWS {
        return Err(PortError::conflict(
            "reactions.reconciliation_scope_exceeded",
            "reaction subject contains more aggregate rows than the reconciliation bound",
        ));
    }

    let mut issues = Vec::new();
    let mut issues_truncated = false;
    let stored_catalog = catalog::Entity::find()
        .filter(catalog::Column::TenantId.eq(request.subject.tenant_id()))
        .filter(catalog::Column::ReactionSubjectId.eq(stored_subject.id))
        .filter(catalog::Column::CatalogRevision.eq(stored_subject.current_catalog_revision))
        .one(connection)
        .await
        .map_err(database_error)?;
    let catalog_value = match stored_catalog {
        None => {
            push_issue(
                &mut issues,
                &mut issues_truncated,
                ReactionReconciliationIssue {
                    code: "current_catalog_missing".to_string(),
                    actor_id: None,
                    reaction: None,
                },
            );
            return Ok(ReconciliationAnalysis {
                report: ReactionReconciliationReport {
                    subject: request.subject.clone(),
                    catalog_revision,
                    status: ReactionReconciliationStatus::Blocked,
                    actor_states_scanned: 0,
                    aggregate_rows_scanned: aggregate_count,
                    comparisons: Vec::new(),
                    issues,
                    issues_truncated,
                },
                reaction_subject_id: stored_subject.id,
                expected: BTreeMap::new(),
                changed_key_count: 0,
                changed_key_sample: Vec::new(),
            });
        }
        Some(row) => match serde_json::from_value::<ReactionCatalog>(row.catalog_json) {
            Ok(catalog) => catalog,
            Err(_) => {
                push_issue(
                    &mut issues,
                    &mut issues_truncated,
                    ReactionReconciliationIssue {
                        code: "current_catalog_corrupt".to_string(),
                        actor_id: None,
                        reaction: None,
                    },
                );
                return Ok(ReconciliationAnalysis {
                    report: ReactionReconciliationReport {
                        subject: request.subject.clone(),
                        catalog_revision,
                        status: ReactionReconciliationStatus::Blocked,
                        actor_states_scanned: 0,
                        aggregate_rows_scanned: aggregate_count,
                        comparisons: Vec::new(),
                        issues,
                        issues_truncated,
                    },
                    reaction_subject_id: stored_subject.id,
                    expected: BTreeMap::new(),
                    changed_key_count: 0,
                    changed_key_sample: Vec::new(),
                });
            }
        },
    };

    let actor_rows = actor_state::Entity::find()
        .filter(actor_state::Column::TenantId.eq(request.subject.tenant_id()))
        .filter(actor_state::Column::ReactionSubjectId.eq(stored_subject.id))
        .order_by_asc(actor_state::Column::ActorId)
        .all(connection)
        .await
        .map_err(database_error)?;
    let mut expected = BTreeMap::<ReactionKey, u64>::new();
    let mut blocked = false;
    for row in actor_rows {
        if row.revision <= 0 {
            blocked = true;
            push_issue(
                &mut issues,
                &mut issues_truncated,
                issue("actor_revision_invalid", Some(row.actor_id), None),
            );
            continue;
        }
        let selected = match serde_json::from_value::<Vec<ReactionKey>>(row.selected_json) {
            Ok(selected) => selected,
            Err(_) => {
                blocked = true;
                push_issue(
                    &mut issues,
                    &mut issues_truncated,
                    issue("actor_state_corrupt", Some(row.actor_id), None),
                );
                continue;
            }
        };
        if selected.iter().collect::<BTreeSet<_>>().len() != selected.len() {
            blocked = true;
            push_issue(
                &mut issues,
                &mut issues_truncated,
                issue("actor_state_duplicate_keys", Some(row.actor_id), None),
            );
            continue;
        }
        if selected.len() > catalog_value.selection().maximum_selected() {
            blocked = true;
            push_issue(
                &mut issues,
                &mut issues_truncated,
                issue("actor_selection_limit_exceeded", Some(row.actor_id), None),
            );
            continue;
        }
        if let Some(reaction) = selected
            .iter()
            .find(|reaction| !catalog_value.contains(reaction))
        {
            blocked = true;
            push_issue(
                &mut issues,
                &mut issues_truncated,
                issue(
                    "actor_selection_outside_catalog",
                    Some(row.actor_id),
                    Some(reaction.clone()),
                ),
            );
            continue;
        }
        for reaction in selected {
            let count = expected.entry(reaction).or_default();
            *count = count.checked_add(1).ok_or_else(|| {
                PortError::invariant_violation(
                    "reactions.reconciliation_count_exhausted",
                    "reaction reconciliation expected aggregate count overflowed",
                )
            })?;
        }
    }

    let aggregate_rows = aggregate::Entity::find()
        .filter(aggregate::Column::TenantId.eq(request.subject.tenant_id()))
        .filter(aggregate::Column::ReactionSubjectId.eq(stored_subject.id))
        .order_by_asc(aggregate::Column::ReactionKey)
        .all(connection)
        .await
        .map_err(database_error)?;
    let mut actual = BTreeMap::<ReactionKey, i64>::new();
    let mut changed_keys = BTreeSet::<String>::new();
    let mut invalid_key_rows = 0_u64;
    let mut repairable_drift = false;
    for row in aggregate_rows {
        let reaction = match ReactionKey::new(row.reaction_key) {
            Ok(reaction) => reaction,
            Err(_) => {
                repairable_drift = true;
                invalid_key_rows = invalid_key_rows.checked_add(1).ok_or_else(|| {
                    PortError::invariant_violation(
                        "reactions.reconciliation_count_exhausted",
                        "reaction reconciliation invalid-key count overflowed",
                    )
                })?;
                changed_keys.insert("invalid".to_string());
                push_issue(
                    &mut issues,
                    &mut issues_truncated,
                    issue("aggregate_key_invalid", None, None),
                );
                continue;
            }
        };
        if row.count < 0 {
            repairable_drift = true;
            changed_keys.insert(reaction.to_string());
            push_issue(
                &mut issues,
                &mut issues_truncated,
                issue("aggregate_count_negative", None, Some(reaction.clone())),
            );
        }
        if !catalog_value.contains(&reaction) {
            repairable_drift = true;
            changed_keys.insert(reaction.to_string());
            push_issue(
                &mut issues,
                &mut issues_truncated,
                issue("aggregate_outside_catalog", None, Some(reaction.clone())),
            );
        }
        actual.insert(reaction, row.count);
    }

    let mut comparisons = Vec::with_capacity(catalog_value.keys().len());
    for reaction in catalog_value.keys() {
        let expected_count = expected.get(reaction).copied().unwrap_or(0);
        let actual_count = actual.get(reaction).copied().unwrap_or(0);
        if actual_count < 0 || u64::try_from(actual_count).ok() != Some(expected_count) {
            repairable_drift = true;
            changed_keys.insert(reaction.to_string());
            push_issue(
                &mut issues,
                &mut issues_truncated,
                issue("aggregate_count_mismatch", None, Some(reaction.clone())),
            );
        }
        comparisons.push(ReactionAggregateComparison {
            reaction: reaction.clone(),
            expected: expected_count,
            actual: actual_count,
        });
    }

    let unique_changed_key_count = u64::try_from(changed_keys.len()).map_err(|_| {
        PortError::invariant_violation(
            "reactions.reconciliation_count_out_of_range",
            "reaction reconciliation changed-key count exceeds u64",
        )
    })?;
    let changed_key_count = unique_changed_key_count
        .checked_add(invalid_key_rows.saturating_sub(u64::from(changed_keys.contains("invalid"))))
        .ok_or_else(|| {
            PortError::invariant_violation(
                "reactions.reconciliation_count_exhausted",
                "reaction reconciliation changed-key count overflowed",
            )
        })?;
    let changed_key_sample = changed_keys
        .into_iter()
        .take(MAX_REACTIONS_EVENT_KEYS)
        .collect::<Vec<_>>();
    let status = if blocked {
        ReactionReconciliationStatus::Blocked
    } else if repairable_drift {
        ReactionReconciliationStatus::Drift
    } else {
        ReactionReconciliationStatus::Clean
    };

    Ok(ReconciliationAnalysis {
        report: ReactionReconciliationReport {
            subject: request.subject.clone(),
            catalog_revision,
            status,
            actor_states_scanned: actor_count,
            aggregate_rows_scanned: aggregate_count,
            comparisons,
            issues,
            issues_truncated,
        },
        reaction_subject_id: stored_subject.id,
        expected,
        changed_key_count,
        changed_key_sample,
    })
}

fn authorize_reconciliation(
    context: &PortContext,
    request: &ReactionReconciliationRequest,
) -> Result<(), PortError> {
    let tenant_id = Uuid::parse_str(&context.tenant_id).map_err(|_| {
        PortError::validation(
            "reactions.tenant_id_invalid",
            "reaction port context must carry a UUID tenant_id",
        )
    })?;
    if tenant_id != request.subject.tenant_id() {
        return Err(PortError::forbidden(
            "reactions.tenant_mismatch",
            "reaction reconciliation subject does not belong to the port tenant",
        ));
    }
    if !context
        .claims
        .iter()
        .any(|claim| claim == RECONCILE_REACTIONS_CLAIM)
    {
        return Err(PortError::forbidden(
            "reactions.reconciliation_forbidden",
            "reaction reconciliation requires the reactions:reconcile claim",
        ));
    }
    Ok(())
}

fn issue(
    code: &str,
    actor_id: Option<Uuid>,
    reaction: Option<ReactionKey>,
) -> ReactionReconciliationIssue {
    ReactionReconciliationIssue {
        code: code.to_string(),
        actor_id,
        reaction,
    }
}

fn push_issue(
    issues: &mut Vec<ReactionReconciliationIssue>,
    truncated: &mut bool,
    issue: ReactionReconciliationIssue,
) {
    if issues.len() < MAX_REACTION_RECONCILIATION_ISSUES {
        issues.push(issue);
    } else {
        *truncated = true;
    }
}

async fn publish_event_once(
    transaction: &DatabaseTransaction,
    envelope_id: Uuid,
    tenant_id: Uuid,
    actor_id: Option<Uuid>,
    event: ReactionsEvent,
) -> Result<(), PortError> {
    match TransactionalEventBus::publish_contract_once_direct_in_tx_with_envelope_id(
        transaction,
        envelope_id,
        tenant_id,
        actor_id,
        event,
    )
    .await
    {
        Ok(_) => Ok(()),
        Err(ContractEventWriteOnceError::Conflict) => Err(PortError::conflict(
            "reactions.event_identity_conflict",
            "reaction semantic event identity is already bound to different facts",
        )),
        Err(ContractEventWriteOnceError::Unavailable) => Err(PortError::unavailable(
            "reactions.event_unavailable",
            "reaction semantic event could not be persisted",
        )),
    }
}

fn database_error(error: sea_orm::DbErr) -> PortError {
    PortError::unavailable("reactions.database", error.to_string())
}

fn u64_to_i64(value: u64, label: &'static str) -> Result<i64, PortError> {
    i64::try_from(value).map_err(|_| {
        PortError::invariant_violation(
            "reactions.revision_out_of_range",
            format!("{label} exceeds the persistence range"),
        )
    })
}

fn i64_to_u64(value: i64, label: &'static str) -> Result<u64, PortError> {
    u64::try_from(value).map_err(|_| {
        PortError::invariant_violation("reactions.revision_invalid", format!("{label} is negative"))
    })
}

fn decode_replay<T: DeserializeOwned>(value: serde_json::Value) -> Result<T, PortError> {
    serde_json::from_value(value).map_err(|error| {
        PortError::invariant_violation("outbox.operation_receipt_corrupt", error.to_string())
    })
}
