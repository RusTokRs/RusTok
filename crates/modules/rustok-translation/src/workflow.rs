use std::{collections::BTreeSet, sync::Arc, time::Instant};

use chrono::{Duration as ChronoDuration, Utc};
use rustok_api::{
    Action, PortActorKind, PortCallPolicy, PortContext, PortError, PortErrorKind, Resource,
    TenantLocale, manifest_hash::hash_manifest,
};
use rustok_core::{PermissionScope, SecurityContext, generate_id};
use rustok_events::TranslationWorkflowEvent;
use rustok_outbox::TransactionalEventBus;
use rustok_tenant::TenantLocalePolicyPort;
use rustok_translation_targets::{
    FieldKey, OpaqueRevision, ReadTranslationResourceRequest, TranslationApplicationReceipt,
    TranslationFieldPatch, TranslationPatchIssue, TranslationPatchIssueSeverity,
    TranslationPatchRequest, TranslationResourceIdentity, TranslationResourceSnapshot,
    TranslationTargetCapability, TranslationTargetProvider, TranslationTargetRegistry,
};
use sea_orm::{
    ColumnTrait, Condition, DatabaseConnection, EntityTrait, QueryFilter, Set, TransactionTrait,
    sea_query::{Expr, OnConflict},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    TranslationError, TranslationResult,
    entities::{
        apply_operation, apply_receipt, apply_recovery, assignment, cancellation, job, job_item,
        proposal, retry,
    },
    glossary::{GlossaryBinding, GlossaryRecord, read_bound_glossary, validate_glossary_binding},
    memory::{AppliedMemorySegment, ingest_applied_segments},
    observability::{self, WorkflowOperation},
    policy::{read_validated_tenant_locale_policy, validate_job_locales},
    progress::refresh_job_progress,
    qa::evaluate_patch_qa,
};

const MIN_APPLY_LEASE_SECONDS: i64 = 30;
const MAX_APPLY_LEASE_SECONDS: i64 = 15 * 60;
const APPLY_LEASE_SAFETY_SECONDS: i64 = 5;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateJobInput {
    pub source_locale: TenantLocale,
    pub target_locale: TenantLocale,
    pub glossary: Option<GlossaryBinding>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AddItemInput {
    pub job_id: Uuid,
    pub identity: TranslationResourceIdentity,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProposalOrigin {
    Manual,
    Import,
    Memory,
    Ai,
}

impl ProposalOrigin {
    fn as_str(self) -> &'static str {
        match self {
            Self::Manual => "manual",
            Self::Import => "import",
            Self::Memory => "memory",
            Self::Ai => "ai",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProposalValue {
    pub key: FieldKey,
    pub value: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SaveProposalInput {
    pub item_id: Uuid,
    pub origin: ProposalOrigin,
    pub values: Vec<ProposalValue>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SubmitProposalInput {
    pub item_id: Uuid,
    pub proposal_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApproveProposalInput {
    pub item_id: Uuid,
    pub proposal_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ApplyProposalInput {
    pub item_id: Uuid,
    pub proposal_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoverApplyInput {
    pub operation_id: Uuid,
    pub expected_attempt_count: i64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssignItemInput {
    pub item_id: Uuid,
    pub expected_revision: i64,
    pub assignee: rustok_api::PortActor,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UnassignItemInput {
    pub item_id: Uuid,
    pub expected_revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelJobInput {
    pub job_id: Uuid,
    pub expected_revision: i64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetryItemInput {
    pub item_id: Uuid,
    pub expected_revision: i64,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobRecord {
    pub id: Uuid,
    pub source_locale: TenantLocale,
    pub target_locale: TenantLocale,
    pub glossary: Option<GlossaryBinding>,
    pub status: String,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobItemRecord {
    pub id: Uuid,
    pub job_id: Uuid,
    pub identity: TranslationResourceIdentity,
    pub status: String,
    pub assignee: Option<rustok_api::PortActor>,
    pub source_digest: String,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProposalRecord {
    pub id: Uuid,
    pub item_id: Uuid,
    pub proposal_revision: i64,
    pub origin: ProposalOrigin,
    pub values: Vec<TranslationFieldPatch>,
    pub qa_issues: Vec<TranslationPatchIssue>,
    pub qa_accepted: bool,
    pub status: String,
    pub approval_receipt_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApplyRecord {
    pub operation_id: Uuid,
    pub item_id: Uuid,
    pub proposal_id: Uuid,
    pub provider_receipt_id: String,
    pub resource_revision: OpaqueRevision,
    pub target_revision: OpaqueRevision,
    pub applied_field_keys: Vec<FieldKey>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssignmentRecord {
    pub operation_id: Uuid,
    pub item_id: Uuid,
    pub assignee: Option<rustok_api::PortActor>,
    pub item_revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CancellationRecord {
    pub cancellation_id: Uuid,
    pub job_id: Uuid,
    pub job_revision: i64,
    pub cancelled_item_count: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryRecord {
    pub retry_id: Uuid,
    pub item_id: Uuid,
    pub item_revision: i64,
    pub status: String,
}

pub struct TranslationWorkflowService {
    database: DatabaseConnection,
    providers: Arc<TranslationTargetRegistry>,
    tenant_locale_policies: Arc<dyn TenantLocalePolicyPort>,
    event_bus: TransactionalEventBus,
}

impl TranslationWorkflowService {
    pub fn new(
        database: DatabaseConnection,
        providers: Arc<TranslationTargetRegistry>,
        tenant_locale_policies: Arc<dyn TenantLocalePolicyPort>,
        event_bus: TransactionalEventBus,
    ) -> Self {
        Self {
            database,
            providers,
            tenant_locale_policies,
            event_bus,
        }
    }

    pub fn interchange_service(&self) -> crate::TranslationInterchangeService {
        crate::TranslationInterchangeService::new(
            self.database.clone(),
            Arc::clone(&self.providers),
            Arc::clone(&self.tenant_locale_policies),
            self.event_bus.clone(),
        )
    }

    pub fn collaboration_service(&self) -> crate::TranslationCollaborationService {
        crate::TranslationCollaborationService::new(self.database.clone(), self.event_bus.clone())
    }

    pub async fn create_job(
        &self,
        context: PortContext,
        input: CreateJobInput,
    ) -> TranslationResult<JobRecord> {
        let tenant_id = authorize_write(&context, Action::Create)?;
        if input.source_locale == input.target_locale {
            return Err(TranslationError::InvalidRequest(
                "source and target locale must differ".to_string(),
            ));
        }
        let tenant_locale_policy = read_validated_tenant_locale_policy(
            self.tenant_locale_policies.as_ref(),
            context.clone(),
            tenant_id,
        )
        .await?;
        validate_job_locales(
            &tenant_locale_policy,
            &input.source_locale,
            &input.target_locale,
        )?;
        if let Some(binding) = input.glossary.as_ref() {
            validate_glossary_binding(
                &self.database,
                &context,
                binding,
                &input.source_locale,
                &input.target_locale,
            )
            .await?;
        }
        let idempotency_key = context
            .idempotency_key
            .as_deref()
            .unwrap_or_default()
            .to_string();
        let request_hash = hash_manifest(&input)?;
        if let Some(existing) =
            find_job_by_idempotency(&self.database, tenant_id, &idempotency_key).await?
        {
            return replay_job(existing, &request_hash);
        }

        let now = Utc::now().fixed_offset();
        let job_id = generate_id();
        let transaction = self.database.begin().await?;
        job::Entity::insert(job::ActiveModel {
            id: Set(job_id),
            tenant_id: Set(tenant_id),
            source_locale: Set(input.source_locale.as_str().to_string()),
            target_locale: Set(input.target_locale.as_str().to_string()),
            glossary_id: Set(input.glossary.as_ref().map(|binding| binding.glossary_id)),
            glossary_revision: Set(input.glossary.as_ref().map(|binding| binding.revision)),
            status: Set("open".to_string()),
            created_by_actor_kind: Set(actor_kind(&context).to_string()),
            created_by_actor_id: Set(context.actor.id.clone()),
            idempotency_key: Set(idempotency_key.clone()),
            request_hash: Set(request_hash.clone()),
            revision: Set(0),
            created_at: Set(now),
            updated_at: Set(now),
        })
        .on_conflict(
            OnConflict::columns([job::Column::TenantId, job::Column::IdempotencyKey])
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(&transaction)
        .await?;
        let persisted = find_job_by_idempotency(&transaction, tenant_id, &idempotency_key)
            .await?
            .ok_or(TranslationError::WorkflowRevisionConflict)?;
        if persisted.id != job_id {
            transaction.rollback().await?;
            return replay_job(persisted, &request_hash);
        }
        refresh_job_progress(&transaction, tenant_id, job_id).await?;
        self.event_bus
            .publish_contract_in_tx(
                &transaction,
                tenant_id,
                event_actor_id(&context),
                TranslationWorkflowEvent::JobCreated {
                    job_id,
                    source_locale: input.source_locale.as_str().to_string(),
                    target_locale: input.target_locale.as_str().to_string(),
                    revision: 0,
                },
            )
            .await?;
        transaction.commit().await?;
        replay_job(persisted, &request_hash)
    }

    pub async fn add_item(
        &self,
        context: PortContext,
        input: AddItemInput,
    ) -> TranslationResult<JobItemRecord> {
        let tenant_id = authorize_write(&context, Action::Update)?;
        let idempotency_key = context
            .idempotency_key
            .as_deref()
            .unwrap_or_default()
            .to_string();
        let request_hash = hash_manifest(&input)?;
        if let Some(existing) =
            find_item_by_idempotency(&self.database, tenant_id, &idempotency_key).await?
        {
            if existing.request_hash != request_hash {
                return Err(TranslationError::IdempotencyConflict);
            }
            return item_record(existing);
        }

        let job_model = job::Entity::find_by_id(input.job_id)
            .filter(job::Column::TenantId.eq(tenant_id))
            .one(&self.database)
            .await?
            .ok_or(TranslationError::JobNotFound)?;
        if !matches!(job_model.status.as_str(), "open" | "in_progress") {
            return Err(TranslationError::JobNotWritable(job_model.status));
        }
        let source_locale = TenantLocale::new(&job_model.source_locale)
            .map_err(|error| TranslationError::InvalidRequest(error.to_string()))?;
        let target_locale = TenantLocale::new(&job_model.target_locale)
            .map_err(|error| TranslationError::InvalidRequest(error.to_string()))?;
        let provider = self
            .providers
            .get(&input.identity.owner_slug, &input.identity.resource_kind)
            .ok_or_else(|| TranslationError::ProviderNotFound {
                owner_slug: input.identity.owner_slug.as_str().to_string(),
                resource_kind: input.identity.resource_kind.as_str().to_string(),
            })?;
        if !provider
            .descriptor()
            .capabilities
            .contains(&TranslationTargetCapability::ReadExactResource)
        {
            return Err(TranslationError::InvalidRequest(
                "translation provider does not expose exact resource reads".to_string(),
            ));
        }
        let snapshot = provider
            .read_resource(
                context.clone(),
                ReadTranslationResourceRequest {
                    identity: input.identity.clone(),
                    source_locale,
                    target_locale,
                },
            )
            .await?;
        snapshot
            .validate()
            .map_err(|error| TranslationError::InvalidRequest(error.to_string()))?;
        if snapshot.summary.identity != input.identity {
            return Err(TranslationError::ProviderIdentityMismatch);
        }
        let source_snapshot = serde_json::to_value(&snapshot)?;
        let source_digest = hash_manifest(&snapshot)?;
        if let Some(existing) =
            find_item_by_identity(&self.database, tenant_id, input.job_id, &input.identity).await?
        {
            if existing.source_digest == source_digest {
                return item_record(existing);
            }
            return Err(TranslationError::WorkflowRevisionConflict);
        }

        let next_job_revision = job_model
            .revision
            .checked_add(1)
            .ok_or(TranslationError::WorkflowRevisionConflict)?;
        let now = Utc::now().fixed_offset();
        let transaction = self.database.begin().await?;
        job_item::Entity::insert(job_item::ActiveModel {
            id: Set(generate_id()),
            tenant_id: Set(tenant_id),
            job_id: Set(input.job_id),
            owner_slug: Set(input.identity.owner_slug.as_str().to_string()),
            resource_kind: Set(input.identity.resource_kind.as_str().to_string()),
            resource_id: Set(input.identity.resource_id.as_str().to_string()),
            subresource_key: Set(input
                .identity
                .subresource_id
                .as_ref()
                .map(|value| value.as_str().to_string())
                .unwrap_or_default()),
            resource_revision: Set(snapshot.summary.resource_revision.as_str().to_string()),
            source_revision: Set(snapshot.source_revision.as_str().to_string()),
            target_revision: Set(snapshot
                .target_revision
                .as_ref()
                .map(|value| value.as_str().to_string())),
            source_snapshot: Set(source_snapshot),
            source_digest: Set(source_digest.clone()),
            status: Set("missing".to_string()),
            current_proposal_id: Set(None),
            active_apply_operation_id: Set(None),
            assigned_actor_kind: Set(None),
            assigned_actor_id: Set(None),
            idempotency_key: Set(idempotency_key.clone()),
            request_hash: Set(request_hash.clone()),
            revision: Set(0),
            created_at: Set(now),
            updated_at: Set(now),
        })
        .on_conflict(
            OnConflict::columns([job_item::Column::TenantId, job_item::Column::IdempotencyKey])
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(&transaction)
        .await?;
        let update = job::Entity::update_many()
            .col_expr(
                job::Column::Status,
                sea_orm::sea_query::Expr::value("in_progress"),
            )
            .col_expr(
                job::Column::Revision,
                sea_orm::sea_query::Expr::value(next_job_revision),
            )
            .col_expr(job::Column::UpdatedAt, sea_orm::sea_query::Expr::value(now))
            .filter(job::Column::Id.eq(job_model.id))
            .filter(job::Column::TenantId.eq(tenant_id))
            .filter(job::Column::Revision.eq(job_model.revision))
            .exec(&transaction)
            .await?;
        if update.rows_affected != 1 {
            return Err(TranslationError::WorkflowRevisionConflict);
        }
        let persisted = find_item_by_idempotency(&transaction, tenant_id, &idempotency_key)
            .await?
            .ok_or(TranslationError::WorkflowRevisionConflict)?;
        refresh_job_progress(&transaction, tenant_id, input.job_id).await?;
        transaction.commit().await?;
        if persisted.request_hash != request_hash {
            return Err(TranslationError::IdempotencyConflict);
        }
        item_record(persisted)
    }

    pub async fn save_proposal(
        &self,
        context: PortContext,
        input: SaveProposalInput,
    ) -> TranslationResult<ProposalRecord> {
        self.save_proposal_with_assignment(context, input, true)
            .await
    }

    pub(crate) async fn save_recovered_machine_proposal(
        &self,
        context: PortContext,
        input: SaveProposalInput,
    ) -> TranslationResult<ProposalRecord> {
        self.save_proposal_with_assignment(context, input, false)
            .await
    }

    async fn save_proposal_with_assignment(
        &self,
        context: PortContext,
        input: SaveProposalInput,
        enforce_current_assignment: bool,
    ) -> TranslationResult<ProposalRecord> {
        let action = if input.origin == ProposalOrigin::Import {
            Action::Import
        } else {
            Action::Update
        };
        let tenant_id = authorize_write(&context, action)?;
        let idempotency_key = operation_idempotency_key(&context);
        let request_hash = hash_manifest(&input)?;
        if let Some(existing) =
            find_proposal_by_idempotency(&self.database, tenant_id, &idempotency_key).await?
        {
            return replay_proposal(existing, &request_hash);
        }

        let item = find_item(&self.database, tenant_id, input.item_id).await?;
        if enforce_current_assignment {
            enforce_assignment(&item, &context)?;
        }
        if !matches!(
            item.status.as_str(),
            "missing" | "draft" | "stale" | "conflict"
        ) {
            return Err(TranslationError::ItemNotWritable(item.status));
        }
        let snapshot: TranslationResourceSnapshot =
            serde_json::from_value(item.source_snapshot.clone())?;
        let proposal_id = generate_id();
        let validation_receipt = format!("validation:{proposal_id}");
        let patch = build_patch(&snapshot, &input.values, proposal_id, &validation_receipt)?;
        let provider = proposal_provider(&self.providers, &patch.identity)?;
        let owner_validation = provider
            .validate_patch(context.clone(), patch.clone())
            .await?;
        let glossary = job_glossary_snapshot(&self.database, tenant_id, item.job_id).await?;
        let validation = evaluate_patch_qa(&snapshot, &patch, owner_validation, glossary.as_ref())?;
        let values_digest = hash_manifest(&patch.fields)?;
        let values = serde_json::to_value(&patch.fields)?;
        let qa_issues = serde_json::to_value(&validation.issues)?;
        let next_item_revision = next_revision(item.revision)?;
        let now = Utc::now().fixed_offset();
        let transaction = self.database.begin().await?;
        proposal::Entity::insert(proposal::ActiveModel {
            id: Set(proposal_id),
            tenant_id: Set(tenant_id),
            item_id: Set(item.id),
            proposal_revision: Set(next_item_revision),
            origin: Set(input.origin.as_str().to_string()),
            values: Set(values),
            values_digest: Set(values_digest),
            qa_issues: Set(qa_issues),
            created_by_actor_kind: Set(actor_kind(&context).to_string()),
            created_by_actor_id: Set(context.actor.id.clone()),
            idempotency_key: Set(idempotency_key.clone()),
            request_hash: Set(request_hash.clone()),
            submitted_at: Set(None),
            submission_idempotency_key: Set(None),
            submission_request_hash: Set(None),
            approved_by_actor_kind: Set(None),
            approved_by_actor_id: Set(None),
            approved_at: Set(None),
            approval_receipt_id: Set(None),
            approval_idempotency_key: Set(None),
            approval_request_hash: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
        })
        .on_conflict(
            OnConflict::columns([proposal::Column::TenantId, proposal::Column::IdempotencyKey])
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(&transaction)
        .await?;
        let persisted = find_proposal_by_idempotency(&transaction, tenant_id, &idempotency_key)
            .await?
            .ok_or(TranslationError::WorkflowRevisionConflict)?;
        if persisted.id != proposal_id {
            transaction.rollback().await?;
            return replay_proposal(persisted, &request_hash);
        }
        let update = job_item::Entity::update_many()
            .col_expr(job_item::Column::Status, Expr::value("draft"))
            .col_expr(
                job_item::Column::CurrentProposalId,
                Expr::value(Some(proposal_id)),
            )
            .col_expr(job_item::Column::Revision, Expr::value(next_item_revision))
            .col_expr(job_item::Column::UpdatedAt, Expr::value(now))
            .filter(job_item::Column::Id.eq(item.id))
            .filter(job_item::Column::TenantId.eq(tenant_id))
            .filter(job_item::Column::Revision.eq(item.revision))
            .exec(&transaction)
            .await?;
        if update.rows_affected != 1 {
            return Err(TranslationError::WorkflowRevisionConflict);
        }
        refresh_job_progress(&transaction, tenant_id, item.job_id).await?;
        transaction.commit().await?;
        proposal_record(persisted)
    }

    pub async fn submit_proposal(
        &self,
        context: PortContext,
        input: SubmitProposalInput,
    ) -> TranslationResult<ProposalRecord> {
        let tenant_id = authorize_write(&context, Action::Update)?;
        let idempotency_key = operation_idempotency_key(&context);
        let request_hash = hash_manifest(&input)?;
        if let Some(existing) =
            find_proposal_by_submission_idempotency(&self.database, tenant_id, &idempotency_key)
                .await?
        {
            return replay_submission(existing, &request_hash);
        }

        let item = find_item(&self.database, tenant_id, input.item_id).await?;
        enforce_assignment(&item, &context)?;
        require_current_proposal(&item, input.proposal_id)?;
        if item.status != "draft" {
            return Err(TranslationError::ItemNotWritable(item.status));
        }
        let proposal =
            find_proposal(&self.database, tenant_id, input.item_id, input.proposal_id).await?;
        if proposal.submitted_at.is_some() {
            return Err(TranslationError::WorkflowRevisionConflict);
        }
        let snapshot: TranslationResourceSnapshot =
            serde_json::from_value(item.source_snapshot.clone())?;
        let values: Vec<TranslationFieldPatch> = serde_json::from_value(proposal.values.clone())?;
        let patch = patch_from_persisted(
            &snapshot,
            values,
            proposal.id,
            &format!("validation:{}", proposal.id),
        )?;
        let provider = proposal_provider(&self.providers, &patch.identity)?;
        let owner_validation = provider
            .validate_patch(context.clone(), patch.clone())
            .await?;
        let glossary = job_glossary_snapshot(&self.database, tenant_id, item.job_id).await?;
        let validation = evaluate_patch_qa(&snapshot, &patch, owner_validation, glossary.as_ref())?;
        if !validation.accepted {
            proposal::Entity::update_many()
                .col_expr(
                    proposal::Column::QaIssues,
                    Expr::value(serde_json::to_value(validation.issues)?),
                )
                .col_expr(
                    proposal::Column::UpdatedAt,
                    Expr::value(Utc::now().fixed_offset()),
                )
                .filter(proposal::Column::Id.eq(proposal.id))
                .filter(proposal::Column::TenantId.eq(tenant_id))
                .exec(&self.database)
                .await?;
            return Err(TranslationError::ProposalValidationFailed);
        }

        let next_item_revision = next_revision(item.revision)?;
        let now = Utc::now().fixed_offset();
        let transaction = self.database.begin().await?;
        let proposal_update = proposal::Entity::update_many()
            .col_expr(proposal::Column::SubmittedAt, Expr::value(Some(now)))
            .col_expr(
                proposal::Column::SubmissionIdempotencyKey,
                Expr::value(Some(idempotency_key)),
            )
            .col_expr(
                proposal::Column::SubmissionRequestHash,
                Expr::value(Some(request_hash.clone())),
            )
            .col_expr(
                proposal::Column::QaIssues,
                Expr::value(serde_json::to_value(validation.issues)?),
            )
            .col_expr(proposal::Column::UpdatedAt, Expr::value(now))
            .filter(proposal::Column::Id.eq(proposal.id))
            .filter(proposal::Column::TenantId.eq(tenant_id))
            .filter(proposal::Column::SubmittedAt.is_null())
            .exec(&transaction)
            .await?;
        if proposal_update.rows_affected != 1 {
            return Err(TranslationError::WorkflowRevisionConflict);
        }
        update_item_state(
            &transaction,
            &item,
            tenant_id,
            "in_review",
            next_item_revision,
            now,
        )
        .await?;
        let persisted = find_proposal(&transaction, tenant_id, item.id, proposal.id).await?;
        refresh_job_progress(&transaction, tenant_id, item.job_id).await?;
        self.event_bus
            .publish_contract_in_tx(
                &transaction,
                tenant_id,
                event_actor_id(&context),
                TranslationWorkflowEvent::ProposalSubmitted {
                    item_id: item.id,
                    proposal_id: proposal.id,
                    item_revision: next_item_revision,
                },
            )
            .await?;
        transaction.commit().await?;
        proposal_record(persisted)
    }

    pub async fn approve_proposal(
        &self,
        context: PortContext,
        input: ApproveProposalInput,
    ) -> TranslationResult<ProposalRecord> {
        let tenant_id = authorize_write(&context, Action::Resolve)?;
        let idempotency_key = operation_idempotency_key(&context);
        let request_hash = hash_manifest(&input)?;
        if let Some(existing) =
            find_proposal_by_approval_idempotency(&self.database, tenant_id, &idempotency_key)
                .await?
        {
            return replay_approval(existing, &request_hash);
        }

        let item = find_item(&self.database, tenant_id, input.item_id).await?;
        require_current_proposal(&item, input.proposal_id)?;
        if item.status != "in_review" {
            return Err(TranslationError::ItemNotWritable(item.status));
        }
        let proposal =
            find_proposal(&self.database, tenant_id, input.item_id, input.proposal_id).await?;
        if proposal.submitted_at.is_none() || proposal.approved_at.is_some() {
            return Err(TranslationError::WorkflowRevisionConflict);
        }
        if proposal.created_by_actor_kind == actor_kind(&context)
            && proposal.created_by_actor_id == context.actor.id
        {
            return Err(TranslationError::ReviewerSeparationRequired);
        }

        let approval_receipt_id = format!("translation-approval:{}", generate_id());
        let snapshot: TranslationResourceSnapshot =
            serde_json::from_value(item.source_snapshot.clone())?;
        let values: Vec<TranslationFieldPatch> = serde_json::from_value(proposal.values.clone())?;
        let patch = patch_from_persisted(&snapshot, values, proposal.id, &approval_receipt_id)?;
        let provider = proposal_provider(&self.providers, &patch.identity)?;
        let owner_validation = provider
            .validate_patch(context.clone(), patch.clone())
            .await?;
        let glossary = job_glossary_snapshot(&self.database, tenant_id, item.job_id).await?;
        let validation = evaluate_patch_qa(&snapshot, &patch, owner_validation, glossary.as_ref())?;
        if !validation.accepted {
            proposal::Entity::update_many()
                .col_expr(
                    proposal::Column::QaIssues,
                    Expr::value(serde_json::to_value(validation.issues)?),
                )
                .col_expr(
                    proposal::Column::UpdatedAt,
                    Expr::value(Utc::now().fixed_offset()),
                )
                .filter(proposal::Column::Id.eq(proposal.id))
                .filter(proposal::Column::TenantId.eq(tenant_id))
                .exec(&self.database)
                .await?;
            return Err(TranslationError::ProposalValidationFailed);
        }

        let next_item_revision = next_revision(item.revision)?;
        let now = Utc::now().fixed_offset();
        let transaction = self.database.begin().await?;
        let proposal_update = proposal::Entity::update_many()
            .col_expr(
                proposal::Column::ApprovedByActorKind,
                Expr::value(Some(actor_kind(&context).to_string())),
            )
            .col_expr(
                proposal::Column::ApprovedByActorId,
                Expr::value(Some(context.actor.id.clone())),
            )
            .col_expr(proposal::Column::ApprovedAt, Expr::value(Some(now)))
            .col_expr(
                proposal::Column::ApprovalReceiptId,
                Expr::value(Some(approval_receipt_id)),
            )
            .col_expr(
                proposal::Column::ApprovalIdempotencyKey,
                Expr::value(Some(idempotency_key)),
            )
            .col_expr(
                proposal::Column::ApprovalRequestHash,
                Expr::value(Some(request_hash.clone())),
            )
            .col_expr(
                proposal::Column::QaIssues,
                Expr::value(serde_json::to_value(validation.issues)?),
            )
            .col_expr(proposal::Column::UpdatedAt, Expr::value(now))
            .filter(proposal::Column::Id.eq(proposal.id))
            .filter(proposal::Column::TenantId.eq(tenant_id))
            .filter(proposal::Column::ApprovedAt.is_null())
            .exec(&transaction)
            .await?;
        if proposal_update.rows_affected != 1 {
            return Err(TranslationError::WorkflowRevisionConflict);
        }
        update_item_state(
            &transaction,
            &item,
            tenant_id,
            "approved",
            next_item_revision,
            now,
        )
        .await?;
        let persisted = find_proposal(&transaction, tenant_id, item.id, proposal.id).await?;
        refresh_job_progress(&transaction, tenant_id, item.job_id).await?;
        self.event_bus
            .publish_contract_in_tx(
                &transaction,
                tenant_id,
                event_actor_id(&context),
                TranslationWorkflowEvent::ProposalApproved {
                    item_id: item.id,
                    proposal_id: proposal.id,
                    item_revision: next_item_revision,
                },
            )
            .await?;
        transaction.commit().await?;
        proposal_record(persisted)
    }

    pub async fn assign_item(
        &self,
        context: PortContext,
        input: AssignItemInput,
    ) -> TranslationResult<AssignmentRecord> {
        let tenant_id = authorize_write(&context, Action::Manage)?;
        validate_expected_revision(input.expected_revision)?;
        validate_workflow_actor(&input.assignee)?;
        let request_hash = hash_manifest(&input)?;
        self.change_assignment(
            context,
            tenant_id,
            input.item_id,
            input.expected_revision,
            Some(input.assignee),
            request_hash,
        )
        .await
    }

    pub async fn unassign_item(
        &self,
        context: PortContext,
        input: UnassignItemInput,
    ) -> TranslationResult<AssignmentRecord> {
        let tenant_id = authorize_write(&context, Action::Manage)?;
        validate_expected_revision(input.expected_revision)?;
        let request_hash = hash_manifest(&input)?;
        self.change_assignment(
            context,
            tenant_id,
            input.item_id,
            input.expected_revision,
            None,
            request_hash,
        )
        .await
    }

    async fn change_assignment(
        &self,
        context: PortContext,
        tenant_id: Uuid,
        item_id: Uuid,
        expected_revision: i64,
        assignee: Option<rustok_api::PortActor>,
        request_hash: String,
    ) -> TranslationResult<AssignmentRecord> {
        let idempotency_key = operation_idempotency_key(&context);
        if let Some(existing) =
            find_assignment_by_idempotency(&self.database, tenant_id, &idempotency_key).await?
        {
            return replay_assignment(existing, &context, &request_hash);
        }

        let item = find_item(&self.database, tenant_id, item_id).await?;
        if !matches!(
            item.status.as_str(),
            "missing" | "draft" | "in_review" | "approved" | "stale" | "conflict" | "blocked"
        ) {
            return Err(TranslationError::ItemNotWritable(item.status));
        }
        if item.revision != expected_revision {
            return Err(TranslationError::WorkflowRevisionConflict);
        }
        let current_assignee = assignment_actor(&item)?;
        if current_assignee == assignee {
            return Err(TranslationError::AssignmentUnchanged);
        }

        let operation_id = generate_id();
        let operation = if assignee.is_some() {
            "assign"
        } else {
            "unassign"
        };
        let next_item_revision = next_revision(item.revision)?;
        let now = Utc::now().fixed_offset();
        let transaction = self.database.begin().await?;
        assignment::Entity::insert(assignment::ActiveModel {
            id: Set(operation_id),
            tenant_id: Set(tenant_id),
            item_id: Set(item.id),
            operation: Set(operation.to_string()),
            assignee_actor_kind: Set(assignee
                .as_ref()
                .map(|actor| actor_kind_value(&actor.kind).to_string())),
            assignee_actor_id: Set(assignee.as_ref().map(|actor| actor.id.clone())),
            requested_by_actor_kind: Set(actor_kind(&context).to_string()),
            requested_by_actor_id: Set(context.actor.id.clone()),
            idempotency_key: Set(idempotency_key.clone()),
            request_hash: Set(request_hash.clone()),
            resulting_item_revision: Set(next_item_revision),
            created_at: Set(now),
        })
        .on_conflict(
            OnConflict::columns([
                assignment::Column::TenantId,
                assignment::Column::IdempotencyKey,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(&transaction)
        .await?;
        let persisted = find_assignment_by_idempotency(&transaction, tenant_id, &idempotency_key)
            .await?
            .ok_or(TranslationError::WorkflowRevisionConflict)?;
        if persisted.id != operation_id {
            transaction.rollback().await?;
            return replay_assignment(persisted, &context, &request_hash);
        }

        let update = job_item::Entity::update_many()
            .col_expr(
                job_item::Column::AssignedActorKind,
                Expr::value(
                    assignee
                        .as_ref()
                        .map(|actor| actor_kind_value(&actor.kind).to_string()),
                ),
            )
            .col_expr(
                job_item::Column::AssignedActorId,
                Expr::value(assignee.as_ref().map(|actor| actor.id.clone())),
            )
            .col_expr(job_item::Column::Revision, Expr::value(next_item_revision))
            .col_expr(job_item::Column::UpdatedAt, Expr::value(now))
            .filter(job_item::Column::Id.eq(item.id))
            .filter(job_item::Column::TenantId.eq(tenant_id))
            .filter(job_item::Column::Revision.eq(item.revision))
            .exec(&transaction)
            .await?;
        if update.rows_affected != 1 {
            return Err(TranslationError::WorkflowRevisionConflict);
        }

        let event = match (&assignee, current_assignee) {
            (Some(actor), _) => TranslationWorkflowEvent::ItemAssigned {
                job_id: item.job_id,
                item_id: item.id,
                assignee_actor_kind: actor_kind_value(&actor.kind).to_string(),
                assignee_actor_id: actor.id.clone(),
                item_revision: next_item_revision,
            },
            (None, Some(previous)) => TranslationWorkflowEvent::ItemUnassigned {
                job_id: item.job_id,
                item_id: item.id,
                previous_actor_kind: actor_kind_value(&previous.kind).to_string(),
                previous_actor_id: previous.id,
                item_revision: next_item_revision,
            },
            (None, None) => return Err(TranslationError::AssignmentUnchanged),
        };
        refresh_job_progress(&transaction, tenant_id, item.job_id).await?;
        self.event_bus
            .publish_contract_in_tx(&transaction, tenant_id, event_actor_id(&context), event)
            .await?;
        transaction.commit().await?;
        assignment_record(persisted)
    }

    pub async fn cancel_job(
        &self,
        context: PortContext,
        input: CancelJobInput,
    ) -> TranslationResult<CancellationRecord> {
        let tenant_id = authorize_write(&context, Action::Manage)?;
        validate_expected_revision(input.expected_revision)?;
        validate_cancellation_reason(&input.reason)?;
        let idempotency_key = operation_idempotency_key(&context);
        let request_hash = hash_manifest(&input)?;
        if let Some(existing) =
            find_cancellation_by_idempotency(&self.database, tenant_id, &idempotency_key).await?
        {
            return replay_cancellation(existing, &context, &request_hash);
        }

        let job_model = find_job(&self.database, tenant_id, input.job_id).await?;
        if !matches!(job_model.status.as_str(), "open" | "in_progress") {
            return Err(TranslationError::JobNotCancellable(job_model.status));
        }
        if job_model.revision != input.expected_revision {
            return Err(TranslationError::WorkflowRevisionConflict);
        }
        let items = job_item::Entity::find()
            .filter(job_item::Column::TenantId.eq(tenant_id))
            .filter(job_item::Column::JobId.eq(job_model.id))
            .all(&self.database)
            .await?;
        if items.iter().any(|item| item.status == "applying") {
            return Err(TranslationError::JobCancellationInProgress);
        }
        let cancellable = items
            .iter()
            .filter(|item| !matches!(item.status.as_str(), "applied" | "excluded" | "cancelled"))
            .collect::<Vec<_>>();
        let cancelled_item_count = u64::try_from(cancellable.len())
            .map_err(|_| TranslationError::WorkflowRevisionConflict)?;
        let next_job_revision = next_revision(job_model.revision)?;
        let cancellation_id = generate_id();
        let now = Utc::now().fixed_offset();
        let transaction = self.database.begin().await?;
        let job_update = job::Entity::update_many()
            .col_expr(job::Column::Status, Expr::value("cancelled"))
            .col_expr(job::Column::Revision, Expr::value(next_job_revision))
            .col_expr(job::Column::UpdatedAt, Expr::value(now))
            .filter(job::Column::Id.eq(job_model.id))
            .filter(job::Column::TenantId.eq(tenant_id))
            .filter(job::Column::Revision.eq(job_model.revision))
            .filter(job::Column::Status.is_in(["open", "in_progress"]))
            .exec(&transaction)
            .await?;
        if job_update.rows_affected != 1 {
            transaction.rollback().await?;
            if let Some(existing) =
                find_cancellation_by_idempotency(&self.database, tenant_id, &idempotency_key)
                    .await?
            {
                return replay_cancellation(existing, &context, &request_hash);
            }
            return Err(TranslationError::WorkflowRevisionConflict);
        }
        for item in cancellable {
            let next_item_revision = next_revision(item.revision)?;
            let update = job_item::Entity::update_many()
                .col_expr(job_item::Column::Status, Expr::value("cancelled"))
                .col_expr(
                    job_item::Column::AssignedActorKind,
                    Expr::value(Option::<String>::None),
                )
                .col_expr(
                    job_item::Column::AssignedActorId,
                    Expr::value(Option::<String>::None),
                )
                .col_expr(job_item::Column::Revision, Expr::value(next_item_revision))
                .col_expr(job_item::Column::UpdatedAt, Expr::value(now))
                .filter(job_item::Column::Id.eq(item.id))
                .filter(job_item::Column::TenantId.eq(tenant_id))
                .filter(job_item::Column::Revision.eq(item.revision))
                .filter(job_item::Column::Status.eq(item.status.as_str()))
                .exec(&transaction)
                .await?;
            if update.rows_affected != 1 {
                return Err(TranslationError::WorkflowRevisionConflict);
            }
        }
        cancellation::Entity::insert(cancellation::ActiveModel {
            id: Set(cancellation_id),
            tenant_id: Set(tenant_id),
            job_id: Set(job_model.id),
            idempotency_key: Set(idempotency_key),
            request_hash: Set(request_hash.clone()),
            requested_by_actor_kind: Set(actor_kind(&context).to_string()),
            requested_by_actor_id: Set(context.actor.id.clone()),
            reason: Set(input.reason),
            resulting_job_revision: Set(next_job_revision),
            cancelled_item_count: Set(i64::try_from(cancelled_item_count)
                .map_err(|_| TranslationError::WorkflowRevisionConflict)?),
            created_at: Set(now),
        })
        .exec_without_returning(&transaction)
        .await?;
        refresh_job_progress(&transaction, tenant_id, job_model.id).await?;
        self.event_bus
            .publish_contract_in_tx(
                &transaction,
                tenant_id,
                event_actor_id(&context),
                TranslationWorkflowEvent::JobCancelled {
                    job_id: job_model.id,
                    revision: next_job_revision,
                    cancelled_item_count,
                },
            )
            .await?;
        transaction.commit().await?;
        Ok(CancellationRecord {
            cancellation_id,
            job_id: job_model.id,
            job_revision: next_job_revision,
            cancelled_item_count,
        })
    }

    pub async fn retry_item(
        &self,
        context: PortContext,
        input: RetryItemInput,
    ) -> TranslationResult<RetryRecord> {
        let tenant_id = authorize_write(&context, Action::Manage)?;
        validate_expected_revision(input.expected_revision)?;
        validate_retry_reason(&input.reason)?;
        let idempotency_key = operation_idempotency_key(&context);
        let request_hash = hash_manifest(&input)?;
        if let Some(existing) =
            find_retry_by_idempotency(&self.database, tenant_id, &idempotency_key).await?
        {
            return replay_retry(existing, &context, &request_hash);
        }

        let item = find_item(&self.database, tenant_id, input.item_id).await?;
        if item.status != "blocked" {
            return Err(TranslationError::ItemNotRetryable(item.status));
        }
        if item.revision != input.expected_revision || item.active_apply_operation_id.is_some() {
            return Err(TranslationError::WorkflowRevisionConflict);
        }
        let job_model = find_job(&self.database, tenant_id, item.job_id).await?;
        if !matches!(job_model.status.as_str(), "open" | "in_progress") {
            return Err(TranslationError::JobNotWritable(job_model.status));
        }
        let proposal_id = item
            .current_proposal_id
            .ok_or(TranslationError::RetryProposalNotApproved)?;
        let proposal = find_proposal(&self.database, tenant_id, item.id, proposal_id).await?;
        if proposal.approved_at.is_none() || proposal.approval_receipt_id.is_none() {
            return Err(TranslationError::RetryProposalNotApproved);
        }

        let retry_id = generate_id();
        let next_item_revision = next_revision(item.revision)?;
        let now = Utc::now().fixed_offset();
        let transaction = self.database.begin().await?;
        retry::Entity::insert(retry::ActiveModel {
            id: Set(retry_id),
            tenant_id: Set(tenant_id),
            item_id: Set(item.id),
            prior_status: Set(item.status.clone()),
            resulting_status: Set("approved".to_string()),
            idempotency_key: Set(idempotency_key.clone()),
            request_hash: Set(request_hash.clone()),
            requested_by_actor_kind: Set(actor_kind(&context).to_string()),
            requested_by_actor_id: Set(context.actor.id.clone()),
            reason: Set(input.reason),
            resulting_item_revision: Set(next_item_revision),
            created_at: Set(now),
        })
        .on_conflict(
            OnConflict::columns([retry::Column::TenantId, retry::Column::IdempotencyKey])
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(&transaction)
        .await?;
        let persisted = find_retry_by_idempotency(&transaction, tenant_id, &idempotency_key)
            .await?
            .ok_or(TranslationError::WorkflowRevisionConflict)?;
        if persisted.id != retry_id {
            transaction.rollback().await?;
            return replay_retry(persisted, &context, &request_hash);
        }
        let update = job_item::Entity::update_many()
            .col_expr(job_item::Column::Status, Expr::value("approved"))
            .col_expr(job_item::Column::Revision, Expr::value(next_item_revision))
            .col_expr(job_item::Column::UpdatedAt, Expr::value(now))
            .filter(job_item::Column::Id.eq(item.id))
            .filter(job_item::Column::TenantId.eq(tenant_id))
            .filter(job_item::Column::Status.eq("blocked"))
            .filter(job_item::Column::ActiveApplyOperationId.is_null())
            .filter(job_item::Column::Revision.eq(item.revision))
            .exec(&transaction)
            .await?;
        if update.rows_affected != 1 {
            return Err(TranslationError::WorkflowRevisionConflict);
        }
        refresh_job_progress(&transaction, tenant_id, item.job_id).await?;
        self.event_bus
            .publish_contract_in_tx(
                &transaction,
                tenant_id,
                event_actor_id(&context),
                TranslationWorkflowEvent::ItemRetryRequested {
                    job_id: item.job_id,
                    item_id: item.id,
                    prior_status: item.status,
                    item_revision: next_item_revision,
                },
            )
            .await?;
        transaction.commit().await?;
        retry_record(persisted)
    }

    pub async fn apply_proposal(
        &self,
        context: PortContext,
        input: ApplyProposalInput,
    ) -> TranslationResult<ApplyRecord> {
        observability::observe_workflow_operation(
            WorkflowOperation::ApplyProposal,
            self.apply_proposal_inner(context, input),
        )
        .await
    }

    async fn apply_proposal_inner(
        &self,
        context: PortContext,
        input: ApplyProposalInput,
    ) -> TranslationResult<ApplyRecord> {
        let tenant_id = authorize_write(&context, Action::Publish)?;
        let idempotency_key = operation_idempotency_key(&context);
        let request_hash = hash_manifest(&input)?;
        if let Some(existing) =
            find_apply_operation_by_idempotency(&self.database, tenant_id, &idempotency_key).await?
        {
            observability::record_apply_replay();
            validate_apply_replay(&existing, &context, &request_hash)?;
            return self.resume_apply_operation(context, existing).await;
        }

        let item = find_item(&self.database, tenant_id, input.item_id).await?;
        require_current_proposal(&item, input.proposal_id)?;
        if item.status != "approved" || item.active_apply_operation_id.is_some() {
            return Err(TranslationError::ItemNotWritable(item.status));
        }
        let proposal =
            find_proposal(&self.database, tenant_id, input.item_id, input.proposal_id).await?;
        let approval_receipt_id = proposal
            .approval_receipt_id
            .as_deref()
            .ok_or(TranslationError::WorkflowRevisionConflict)?;
        let snapshot: TranslationResourceSnapshot =
            serde_json::from_value(item.source_snapshot.clone())?;
        let values: Vec<TranslationFieldPatch> = serde_json::from_value(proposal.values.clone())?;
        let patch = patch_from_persisted(&snapshot, values, proposal.id, approval_receipt_id)?;
        apply_provider(&self.providers, &patch.identity)?;

        let operation_id = generate_id();
        let patch_digest = hash_manifest(&patch)?;
        let applying_item_revision = next_revision(item.revision)?;
        let now = Utc::now().fixed_offset();
        let transaction = self.database.begin().await?;
        apply_operation::Entity::insert(apply_operation::ActiveModel {
            id: Set(operation_id),
            tenant_id: Set(tenant_id),
            item_id: Set(item.id),
            proposal_id: Set(proposal.id),
            idempotency_key: Set(idempotency_key.clone()),
            request_hash: Set(request_hash.clone()),
            patch: Set(serde_json::to_value(&patch)?),
            patch_digest: Set(patch_digest),
            status: Set("pending".to_string()),
            created_by_actor_kind: Set(actor_kind(&context).to_string()),
            created_by_actor_id: Set(context.actor.id.clone()),
            applying_item_revision: Set(applying_item_revision),
            attempt_count: Set(0),
            last_error_kind: Set(None),
            last_error_code: Set(None),
            last_error_retryable: Set(None),
            lease_token: Set(None),
            lease_owner_actor_kind: Set(None),
            lease_owner_actor_id: Set(None),
            lease_expires_at: Set(None),
            created_at: Set(now),
            updated_at: Set(now),
            completed_at: Set(None),
        })
        .on_conflict(
            OnConflict::columns([
                apply_operation::Column::TenantId,
                apply_operation::Column::IdempotencyKey,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(&transaction)
        .await?;
        let persisted =
            find_apply_operation_by_idempotency(&transaction, tenant_id, &idempotency_key)
                .await?
                .ok_or(TranslationError::WorkflowRevisionConflict)?;
        if persisted.id != operation_id {
            transaction.rollback().await?;
            observability::record_apply_replay();
            validate_apply_replay(&persisted, &context, &request_hash)?;
            return self.resume_apply_operation(context, persisted).await;
        }
        let item_update = job_item::Entity::update_many()
            .col_expr(job_item::Column::Status, Expr::value("applying"))
            .col_expr(
                job_item::Column::ActiveApplyOperationId,
                Expr::value(Some(operation_id)),
            )
            .col_expr(
                job_item::Column::Revision,
                Expr::value(applying_item_revision),
            )
            .col_expr(job_item::Column::UpdatedAt, Expr::value(now))
            .filter(job_item::Column::Id.eq(item.id))
            .filter(job_item::Column::TenantId.eq(tenant_id))
            .filter(job_item::Column::Status.eq("approved"))
            .filter(job_item::Column::ActiveApplyOperationId.is_null())
            .filter(job_item::Column::Revision.eq(item.revision))
            .exec(&transaction)
            .await?;
        if item_update.rows_affected != 1 {
            return Err(TranslationError::WorkflowRevisionConflict);
        }
        refresh_job_progress(&transaction, tenant_id, item.job_id).await?;
        self.event_bus
            .publish_contract_in_tx(
                &transaction,
                tenant_id,
                event_actor_id(&context),
                TranslationWorkflowEvent::ApplyRequested {
                    operation_id,
                    item_id: item.id,
                    proposal_id: proposal.id,
                    item_revision: applying_item_revision,
                },
            )
            .await?;
        transaction.commit().await?;
        self.resume_apply_operation(context, persisted).await
    }

    pub async fn recover_apply(
        &self,
        context: PortContext,
        input: RecoverApplyInput,
    ) -> TranslationResult<ApplyRecord> {
        observability::observe_workflow_operation(
            WorkflowOperation::ApplyRecovery,
            self.recover_apply_inner(context, input),
        )
        .await
    }

    async fn recover_apply_inner(
        &self,
        context: PortContext,
        input: RecoverApplyInput,
    ) -> TranslationResult<ApplyRecord> {
        let tenant_id = authorize_write_actions(&context, &[Action::Manage, Action::Publish])?;
        validate_recovery_reason(&input.reason)?;
        if input.expected_attempt_count < 0 {
            return Err(TranslationError::ApplyRecoveryAttemptMismatch);
        }
        let idempotency_key = operation_idempotency_key(&context);
        let request_hash = hash_manifest(&input)?;
        if let Some(existing) =
            find_apply_recovery_by_idempotency(&self.database, tenant_id, &idempotency_key).await?
        {
            observability::record_apply_replay();
            validate_recovery_replay(&existing, &context, &request_hash)?;
            let operation =
                find_apply_operation(&self.database, tenant_id, existing.operation_id).await?;
            return self.resume_recovered_apply(context, operation).await;
        }

        let operation = find_apply_operation(&self.database, tenant_id, input.operation_id).await?;
        match operation.status.as_str() {
            "completed" => {
                let receipt = find_apply_receipt_by_idempotency(
                    &self.database,
                    tenant_id,
                    &operation.idempotency_key,
                )
                .await?
                .ok_or(TranslationError::WorkflowRevisionConflict)?;
                return apply_record(&operation, receipt);
            }
            "conflict" | "failed" => return Err(apply_terminal_error(&operation)),
            "pending" => {}
            _ => return Err(TranslationError::WorkflowRevisionConflict),
        }
        if operation.attempt_count != input.expected_attempt_count {
            return Err(TranslationError::ApplyRecoveryAttemptMismatch);
        }
        let item = find_item(&self.database, tenant_id, operation.item_id).await?;
        if item.status != "applying"
            || item.active_apply_operation_id != Some(operation.id)
            || item.revision != operation.applying_item_revision
        {
            return Err(TranslationError::WorkflowRevisionConflict);
        }
        let (patch, provider) = self.prepare_apply_execution(&operation)?;
        let recovery_id = generate_id();
        let now = Utc::now().fixed_offset();
        let transaction = self.database.begin().await?;
        apply_recovery::Entity::insert(apply_recovery::ActiveModel {
            id: Set(recovery_id),
            tenant_id: Set(tenant_id),
            operation_id: Set(operation.id),
            idempotency_key: Set(idempotency_key.clone()),
            request_hash: Set(request_hash.clone()),
            requested_by_actor_kind: Set(actor_kind(&context).to_string()),
            requested_by_actor_id: Set(context.actor.id.clone()),
            reason: Set(input.reason),
            observed_attempt_count: Set(operation.attempt_count),
            created_at: Set(now),
        })
        .on_conflict(
            OnConflict::columns([
                apply_recovery::Column::TenantId,
                apply_recovery::Column::IdempotencyKey,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(&transaction)
        .await?;
        let persisted_recovery =
            find_apply_recovery_by_idempotency(&transaction, tenant_id, &idempotency_key)
                .await?
                .ok_or(TranslationError::WorkflowRevisionConflict)?;
        if persisted_recovery.id != recovery_id {
            transaction.rollback().await?;
            observability::record_apply_replay();
            validate_recovery_replay(&persisted_recovery, &context, &request_hash)?;
            let current =
                find_apply_operation(&self.database, tenant_id, persisted_recovery.operation_id)
                    .await?;
            return self.resume_recovered_apply(context, current).await;
        }
        let lease_token = claim_apply_attempt(&transaction, &operation, &context).await?;
        self.event_bus
            .publish_contract_in_tx(
                &transaction,
                tenant_id,
                event_actor_id(&context),
                TranslationWorkflowEvent::ApplyRecoveryRequested {
                    operation_id: operation.id,
                    item_id: operation.item_id,
                    recovery_id,
                    observed_attempt_count: operation.attempt_count,
                },
            )
            .await?;
        transaction.commit().await?;

        let mut provider_context = context;
        provider_context.idempotency_key = Some(operation.idempotency_key.clone());
        self.execute_claimed_apply_operation(
            provider_context,
            operation,
            patch,
            provider,
            lease_token,
        )
        .await
    }

    async fn resume_recovered_apply(
        &self,
        context: PortContext,
        operation: apply_operation::Model,
    ) -> TranslationResult<ApplyRecord> {
        match operation.status.as_str() {
            "completed" => {
                let receipt = find_apply_receipt_by_idempotency(
                    &self.database,
                    operation.tenant_id,
                    &operation.idempotency_key,
                )
                .await?
                .ok_or(TranslationError::WorkflowRevisionConflict)?;
                return apply_record(&operation, receipt);
            }
            "conflict" | "failed" => return Err(apply_terminal_error(&operation)),
            "pending" => {}
            _ => return Err(TranslationError::WorkflowRevisionConflict),
        }
        let (patch, provider) = self.prepare_apply_execution(&operation)?;
        let lease_token = claim_apply_attempt(&self.database, &operation, &context).await?;
        let mut provider_context = context;
        provider_context.idempotency_key = Some(operation.idempotency_key.clone());
        self.execute_claimed_apply_operation(
            provider_context,
            operation,
            patch,
            provider,
            lease_token,
        )
        .await
    }

    async fn resume_apply_operation(
        &self,
        context: PortContext,
        operation: apply_operation::Model,
    ) -> TranslationResult<ApplyRecord> {
        match operation.status.as_str() {
            "completed" => {
                let receipt = find_apply_receipt_by_idempotency(
                    &self.database,
                    operation.tenant_id,
                    &operation.idempotency_key,
                )
                .await?
                .ok_or(TranslationError::WorkflowRevisionConflict)?;
                return apply_record(&operation, receipt);
            }
            "conflict" | "failed" => return Err(apply_terminal_error(&operation)),
            "pending" => {}
            _ => return Err(TranslationError::WorkflowRevisionConflict),
        }
        validate_apply_actor(&operation, &context)?;
        let (patch, provider) = self.prepare_apply_execution(&operation)?;
        let lease_token = match claim_apply_attempt(&self.database, &operation, &context).await {
            Ok(lease_token) => lease_token,
            Err(TranslationError::ApplyInProgress) => {
                let current =
                    find_apply_operation(&self.database, operation.tenant_id, operation.id).await?;
                return match current.status.as_str() {
                    "completed" => {
                        let receipt = find_apply_receipt_by_idempotency(
                            &self.database,
                            current.tenant_id,
                            &current.idempotency_key,
                        )
                        .await?
                        .ok_or(TranslationError::WorkflowRevisionConflict)?;
                        apply_record(&current, receipt)
                    }
                    "conflict" | "failed" => Err(apply_terminal_error(&current)),
                    "pending" => Err(TranslationError::ApplyInProgress),
                    _ => Err(TranslationError::WorkflowRevisionConflict),
                };
            }
            Err(error) => return Err(error),
        };
        self.execute_claimed_apply_operation(context, operation, patch, provider, lease_token)
            .await
    }

    fn prepare_apply_execution(
        &self,
        operation: &apply_operation::Model,
    ) -> TranslationResult<(TranslationPatchRequest, Arc<dyn TranslationTargetProvider>)> {
        let patch: TranslationPatchRequest = serde_json::from_value(operation.patch.clone())?;
        patch
            .validate()
            .map_err(|error| TranslationError::InvalidRequest(error.to_string()))?;
        if hash_manifest(&patch)? != operation.patch_digest {
            return Err(TranslationError::WorkflowRevisionConflict);
        }
        let provider = apply_provider(&self.providers, &patch.identity)?;
        Ok((patch, provider))
    }

    async fn execute_claimed_apply_operation(
        &self,
        context: PortContext,
        operation: apply_operation::Model,
        patch: TranslationPatchRequest,
        provider: Arc<dyn TranslationTargetProvider>,
        lease_token: Uuid,
    ) -> TranslationResult<ApplyRecord> {
        let event_actor_id = event_actor_id(&context);
        observability::record_apply_attempt_started();
        let provider_started_at = Instant::now();
        match provider.apply_patch(context, patch.clone()).await {
            Ok(receipt) => {
                observability::record_owner_apply_success(provider_started_at.elapsed());
                if let Err(error) = validate_provider_receipt(&patch, &receipt) {
                    observability::record_owner_apply_invalid_receipt();
                    record_pending_apply_error(
                        &self.database,
                        &self.event_bus,
                        &operation,
                        PendingApplyError {
                            kind: "invariant_violation",
                            code: "translation.invalid_provider_receipt",
                            retryable: true,
                            lease_token,
                            event_actor_id,
                        },
                    )
                    .await?;
                    return Err(error);
                }
                let result = self
                    .finalize_apply_operation(operation, receipt, lease_token, event_actor_id)
                    .await;
                if result.is_ok() {
                    observability::record_owner_apply_completed();
                } else {
                    observability::record_owner_apply_finalization_failure();
                }
                result
            }
            Err(error) => {
                observability::record_owner_apply_failure(
                    port_error_kind(error.kind.clone()),
                    error.retryable,
                    provider_started_at.elapsed(),
                );
                self.record_owner_apply_error(&operation, &error, lease_token, event_actor_id)
                    .await?;
                Err(error.into())
            }
        }
    }

    async fn record_owner_apply_error(
        &self,
        operation: &apply_operation::Model,
        error: &PortError,
        lease_token: Uuid,
        event_actor_id: Option<Uuid>,
    ) -> TranslationResult<()> {
        let kind = port_error_kind(error.kind.clone());
        let code = bounded_error_code(&error.code);
        if error.retryable {
            return record_pending_apply_error(
                &self.database,
                &self.event_bus,
                operation,
                PendingApplyError {
                    kind,
                    code: &code,
                    retryable: true,
                    lease_token,
                    event_actor_id,
                },
            )
            .await;
        }

        let (operation_status, item_status) = if error.kind == PortErrorKind::Conflict {
            ("conflict", "conflict")
        } else {
            ("failed", "blocked")
        };
        let terminal_item_revision = next_revision(operation.applying_item_revision)?;
        let now = Utc::now().fixed_offset();
        let transaction = self.database.begin().await?;
        let operation_update = apply_operation::Entity::update_many()
            .col_expr(
                apply_operation::Column::Status,
                Expr::value(operation_status),
            )
            .col_expr(
                apply_operation::Column::LastErrorKind,
                Expr::value(Some(kind.to_string())),
            )
            .col_expr(
                apply_operation::Column::LastErrorCode,
                Expr::value(Some(code)),
            )
            .col_expr(
                apply_operation::Column::LastErrorRetryable,
                Expr::value(Some(false)),
            )
            .col_expr(
                apply_operation::Column::LeaseToken,
                Expr::value(Option::<Uuid>::None),
            )
            .col_expr(
                apply_operation::Column::LeaseOwnerActorKind,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                apply_operation::Column::LeaseOwnerActorId,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                apply_operation::Column::LeaseExpiresAt,
                Expr::value(Option::<chrono::DateTime<chrono::FixedOffset>>::None),
            )
            .col_expr(apply_operation::Column::UpdatedAt, Expr::value(now))
            .filter(apply_operation::Column::Id.eq(operation.id))
            .filter(apply_operation::Column::TenantId.eq(operation.tenant_id))
            .filter(apply_operation::Column::Status.eq("pending"))
            .filter(apply_operation::Column::LeaseToken.eq(lease_token))
            .exec(&transaction)
            .await?;
        if operation_update.rows_affected != 1 {
            return Err(TranslationError::WorkflowRevisionConflict);
        }
        let item_update = job_item::Entity::update_many()
            .col_expr(job_item::Column::Status, Expr::value(item_status))
            .col_expr(
                job_item::Column::ActiveApplyOperationId,
                Expr::value(Option::<Uuid>::None),
            )
            .col_expr(
                job_item::Column::Revision,
                Expr::value(terminal_item_revision),
            )
            .col_expr(job_item::Column::UpdatedAt, Expr::value(now))
            .filter(job_item::Column::Id.eq(operation.item_id))
            .filter(job_item::Column::TenantId.eq(operation.tenant_id))
            .filter(job_item::Column::Status.eq("applying"))
            .filter(job_item::Column::ActiveApplyOperationId.eq(operation.id))
            .filter(job_item::Column::Revision.eq(operation.applying_item_revision))
            .exec(&transaction)
            .await?;
        if item_update.rows_affected != 1 {
            return Err(TranslationError::WorkflowRevisionConflict);
        }
        let item = find_item(&transaction, operation.tenant_id, operation.item_id).await?;
        refresh_job_progress(&transaction, operation.tenant_id, item.job_id).await?;
        self.event_bus
            .publish_contract_in_tx(
                &transaction,
                operation.tenant_id,
                event_actor_id,
                TranslationWorkflowEvent::ApplyFailed {
                    operation_id: operation.id,
                    item_id: operation.item_id,
                    proposal_id: operation.proposal_id,
                    status: operation_status.to_string(),
                    error_code: bounded_error_code(&error.code),
                    retryable: false,
                    attempt_count: next_revision(operation.attempt_count)?,
                },
            )
            .await?;
        transaction.commit().await?;
        Ok(())
    }

    async fn complete_job_if_terminal<C>(
        &self,
        database: &C,
        tenant_id: Uuid,
        job_id: Uuid,
        event_actor_id: Option<Uuid>,
    ) -> TranslationResult<bool>
    where
        C: sea_orm::ConnectionTrait,
    {
        let items = job_item::Entity::find()
            .filter(job_item::Column::TenantId.eq(tenant_id))
            .filter(job_item::Column::JobId.eq(job_id))
            .all(database)
            .await?;
        if items.is_empty()
            || items
                .iter()
                .any(|item| !matches!(item.status.as_str(), "applied" | "excluded" | "cancelled"))
        {
            return Ok(false);
        }
        let job_model = find_job(database, tenant_id, job_id).await?;
        if job_model.status == "completed" {
            return Ok(false);
        }
        if !matches!(job_model.status.as_str(), "open" | "in_progress") {
            return Ok(false);
        }
        let revision = next_revision(job_model.revision)?;
        let update = job::Entity::update_many()
            .col_expr(job::Column::Status, Expr::value("completed"))
            .col_expr(job::Column::Revision, Expr::value(revision))
            .col_expr(
                job::Column::UpdatedAt,
                Expr::value(Utc::now().fixed_offset()),
            )
            .filter(job::Column::Id.eq(job_id))
            .filter(job::Column::TenantId.eq(tenant_id))
            .filter(job::Column::Status.eq(job_model.status))
            .filter(job::Column::Revision.eq(job_model.revision))
            .exec(database)
            .await?;
        if update.rows_affected != 1 {
            return Err(TranslationError::WorkflowRevisionConflict);
        }
        let total_item_count =
            u64::try_from(items.len()).map_err(|_| TranslationError::WorkflowRevisionConflict)?;
        self.event_bus
            .publish_contract_in_tx(
                database,
                tenant_id,
                event_actor_id,
                TranslationWorkflowEvent::JobCompleted {
                    job_id,
                    revision,
                    total_item_count,
                },
            )
            .await?;
        Ok(true)
    }

    async fn finalize_apply_operation(
        &self,
        operation: apply_operation::Model,
        receipt: TranslationApplicationReceipt,
        lease_token: Uuid,
        event_actor_id: Option<Uuid>,
    ) -> TranslationResult<ApplyRecord> {
        let transaction = self.database.begin().await?;
        if let Some(existing) = find_apply_receipt_by_idempotency(
            &transaction,
            operation.tenant_id,
            &operation.idempotency_key,
        )
        .await?
        {
            transaction.rollback().await?;
            ensure_receipt_matches(&operation, &receipt, &existing)?;
            return apply_record(&operation, existing);
        }

        let patch: TranslationPatchRequest = serde_json::from_value(operation.patch.clone())?;
        let now = Utc::now().fixed_offset();
        apply_receipt::Entity::insert(apply_receipt::ActiveModel {
            id: Set(generate_id()),
            tenant_id: Set(operation.tenant_id),
            item_id: Set(operation.item_id),
            proposal_id: Set(operation.proposal_id),
            idempotency_key: Set(operation.idempotency_key.clone()),
            request_hash: Set(operation.request_hash.clone()),
            approval_receipt_id: Set(patch.approval_receipt_id.clone()),
            provider_receipt_id: Set(receipt.provider_receipt_id.clone()),
            resource_revision: Set(receipt.resource_revision.as_str().to_string()),
            target_revision: Set(receipt.target_revision.as_str().to_string()),
            applied_field_keys: Set(serde_json::to_value(&receipt.applied_field_keys)?),
            created_at: Set(now),
        })
        .on_conflict(
            OnConflict::columns([
                apply_receipt::Column::TenantId,
                apply_receipt::Column::IdempotencyKey,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(&transaction)
        .await?;
        let persisted = find_apply_receipt_by_idempotency(
            &transaction,
            operation.tenant_id,
            &operation.idempotency_key,
        )
        .await?
        .ok_or(TranslationError::WorkflowRevisionConflict)?;
        ensure_receipt_matches(&operation, &receipt, &persisted)?;

        let operation_update = apply_operation::Entity::update_many()
            .col_expr(apply_operation::Column::Status, Expr::value("completed"))
            .col_expr(
                apply_operation::Column::LastErrorKind,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                apply_operation::Column::LastErrorCode,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                apply_operation::Column::LastErrorRetryable,
                Expr::value(Option::<bool>::None),
            )
            .col_expr(
                apply_operation::Column::LeaseToken,
                Expr::value(Option::<Uuid>::None),
            )
            .col_expr(
                apply_operation::Column::LeaseOwnerActorKind,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                apply_operation::Column::LeaseOwnerActorId,
                Expr::value(Option::<String>::None),
            )
            .col_expr(
                apply_operation::Column::LeaseExpiresAt,
                Expr::value(Option::<chrono::DateTime<chrono::FixedOffset>>::None),
            )
            .col_expr(apply_operation::Column::UpdatedAt, Expr::value(now))
            .col_expr(apply_operation::Column::CompletedAt, Expr::value(Some(now)))
            .filter(apply_operation::Column::Id.eq(operation.id))
            .filter(apply_operation::Column::TenantId.eq(operation.tenant_id))
            .filter(apply_operation::Column::Status.eq("pending"))
            .filter(apply_operation::Column::LeaseToken.eq(lease_token))
            .exec(&transaction)
            .await?;
        if operation_update.rows_affected != 1 {
            return Err(TranslationError::WorkflowRevisionConflict);
        }
        let applied_item_revision = next_revision(operation.applying_item_revision)?;
        let item_update = job_item::Entity::update_many()
            .col_expr(job_item::Column::Status, Expr::value("applied"))
            .col_expr(
                job_item::Column::ActiveApplyOperationId,
                Expr::value(Option::<Uuid>::None),
            )
            .col_expr(
                job_item::Column::ResourceRevision,
                Expr::value(receipt.resource_revision.as_str()),
            )
            .col_expr(
                job_item::Column::TargetRevision,
                Expr::value(Some(receipt.target_revision.as_str().to_string())),
            )
            .col_expr(
                job_item::Column::Revision,
                Expr::value(applied_item_revision),
            )
            .col_expr(job_item::Column::UpdatedAt, Expr::value(now))
            .filter(job_item::Column::Id.eq(operation.item_id))
            .filter(job_item::Column::TenantId.eq(operation.tenant_id))
            .filter(job_item::Column::Status.eq("applying"))
            .filter(job_item::Column::ActiveApplyOperationId.eq(operation.id))
            .filter(job_item::Column::Revision.eq(operation.applying_item_revision))
            .exec(&transaction)
            .await?;
        if item_update.rows_affected != 1 {
            return Err(TranslationError::WorkflowRevisionConflict);
        }
        let item = find_item(&transaction, operation.tenant_id, operation.item_id).await?;
        let proposal = find_proposal(
            &transaction,
            operation.tenant_id,
            operation.item_id,
            operation.proposal_id,
        )
        .await?;
        let snapshot: TranslationResourceSnapshot =
            serde_json::from_value(item.source_snapshot.clone())?;
        let reviewer_actor_kind = proposal
            .approved_by_actor_kind
            .clone()
            .ok_or(TranslationError::WorkflowRevisionConflict)?;
        let reviewer_actor_id = proposal
            .approved_by_actor_id
            .clone()
            .ok_or(TranslationError::WorkflowRevisionConflict)?;
        let applied_field_keys = receipt
            .applied_field_keys
            .iter()
            .map(FieldKey::as_str)
            .collect::<BTreeSet<_>>();
        let patch_fields = patch
            .fields
            .iter()
            .map(|field| (field.key.as_str(), field))
            .collect::<std::collections::BTreeMap<_, _>>();
        let memory_segments = snapshot
            .fields
            .iter()
            .filter(|field| applied_field_keys.contains(field.descriptor.key.as_str()))
            .filter_map(|field| {
                patch_fields
                    .get(field.descriptor.key.as_str())
                    .map(|target| AppliedMemorySegment {
                        source_locale: patch.source_locale.clone(),
                        target_locale: patch.target_locale.clone(),
                        identity: patch.identity.clone(),
                        field_key: field.descriptor.key.clone(),
                        classification: field.descriptor.classification,
                        source_text: field.source_value.clone(),
                        target_text: target.value.clone(),
                        source_hash: field.source_hash.clone(),
                        origin: proposal.origin.clone(),
                        reviewer_actor_kind: reviewer_actor_kind.clone(),
                        reviewer_actor_id: reviewer_actor_id.clone(),
                        proposal_id: proposal.id,
                        apply_receipt_id: persisted.id,
                    })
            })
            .collect();
        ingest_applied_segments(&transaction, operation.tenant_id, now, memory_segments).await?;
        self.event_bus
            .publish_contract_in_tx(
                &transaction,
                operation.tenant_id,
                event_actor_id,
                TranslationWorkflowEvent::ApplyCompleted {
                    operation_id: operation.id,
                    item_id: operation.item_id,
                    proposal_id: operation.proposal_id,
                    item_revision: applied_item_revision,
                },
            )
            .await?;
        self.complete_job_if_terminal(
            &transaction,
            operation.tenant_id,
            item.job_id,
            event_actor_id,
        )
        .await?;
        refresh_job_progress(&transaction, operation.tenant_id, item.job_id).await?;
        transaction.commit().await?;
        apply_record(&operation, persisted)
    }
}

fn authorize_write(context: &PortContext, action: Action) -> TranslationResult<Uuid> {
    authorize_write_actions(context, &[action])
}

fn authorize_write_actions(context: &PortContext, actions: &[Action]) -> TranslationResult<Uuid> {
    context.require_policy(PortCallPolicy::write())?;
    let security = SecurityContext::try_from_port_context(context)?;
    for action in actions {
        if security.get_scope(Resource::Translations, *action) == PermissionScope::None {
            return Err(TranslationError::Forbidden);
        }
    }
    Uuid::parse_str(&context.tenant_id).map_err(|_| TranslationError::InvalidTenantId)
}

pub(crate) fn actor_kind(context: &PortContext) -> &'static str {
    actor_kind_value(&context.actor.kind)
}

pub(crate) fn actor_kind_value(kind: &PortActorKind) -> &'static str {
    match kind {
        PortActorKind::User => "user",
        PortActorKind::Service => "service",
        PortActorKind::System => "system",
    }
}

pub(crate) fn event_actor_id(context: &PortContext) -> Option<Uuid> {
    Uuid::parse_str(&context.actor.id)
        .ok()
        .filter(|actor_id| !actor_id.is_nil())
}

fn operation_idempotency_key(context: &PortContext) -> String {
    context.idempotency_key.clone().unwrap_or_default()
}

fn validate_expected_revision(revision: i64) -> TranslationResult<()> {
    if revision < 0 {
        return Err(TranslationError::WorkflowRevisionConflict);
    }
    Ok(())
}

pub(crate) fn validate_workflow_actor(actor: &rustok_api::PortActor) -> TranslationResult<()> {
    let id = actor.id.trim();
    if id.is_empty() || id.len() > 191 || id != actor.id {
        return Err(TranslationError::InvalidWorkflowActor);
    }
    match actor.kind {
        PortActorKind::User => {
            let id = Uuid::parse_str(id).map_err(|_| TranslationError::InvalidWorkflowActor)?;
            if id.is_nil() {
                return Err(TranslationError::InvalidWorkflowActor);
            }
        }
        PortActorKind::Service => {}
        PortActorKind::System if id == "system" => {}
        PortActorKind::System => return Err(TranslationError::InvalidWorkflowActor),
    }
    Ok(())
}

fn validate_cancellation_reason(reason: &str) -> TranslationResult<()> {
    let trimmed = reason.trim();
    if trimmed.is_empty() || trimmed.len() > 500 || trimmed != reason {
        return Err(TranslationError::InvalidCancellationReason);
    }
    Ok(())
}

fn validate_retry_reason(reason: &str) -> TranslationResult<()> {
    let trimmed = reason.trim();
    if trimmed.is_empty() || trimmed.len() > 500 || trimmed != reason {
        return Err(TranslationError::InvalidRetryReason);
    }
    Ok(())
}

pub(crate) fn assignment_actor(
    item: &job_item::Model,
) -> TranslationResult<Option<rustok_api::PortActor>> {
    match (&item.assigned_actor_kind, &item.assigned_actor_id) {
        (None, None) => Ok(None),
        (Some(kind), Some(id)) => Ok(Some(workflow_actor(kind, id)?)),
        _ => Err(TranslationError::WorkflowRevisionConflict),
    }
}

fn enforce_assignment(item: &job_item::Model, context: &PortContext) -> TranslationResult<()> {
    let Some(assignee) = assignment_actor(item)? else {
        return Ok(());
    };
    if assignee == context.actor {
        return Ok(());
    }
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::Translations, Action::Manage) != PermissionScope::None {
        return Ok(());
    }
    Err(TranslationError::ItemAssignedToAnotherActor)
}

pub(crate) fn workflow_actor(kind: &str, id: &str) -> TranslationResult<rustok_api::PortActor> {
    let actor = match kind {
        "user" => rustok_api::PortActor::user(id),
        "service" => rustok_api::PortActor::service(id),
        "system" => rustok_api::PortActor::system(),
        _ => return Err(TranslationError::InvalidWorkflowActor),
    };
    if actor.id != id {
        return Err(TranslationError::InvalidWorkflowActor);
    }
    validate_workflow_actor(&actor)?;
    Ok(actor)
}

pub(crate) fn next_revision(revision: i64) -> TranslationResult<i64> {
    revision
        .checked_add(1)
        .ok_or(TranslationError::WorkflowRevisionConflict)
}

fn replay_assignment(
    model: assignment::Model,
    context: &PortContext,
    request_hash: &str,
) -> TranslationResult<AssignmentRecord> {
    if model.request_hash != request_hash {
        return Err(TranslationError::IdempotencyConflict);
    }
    if model.requested_by_actor_kind != actor_kind(context)
        || model.requested_by_actor_id != context.actor.id
    {
        return Err(TranslationError::IdempotencyActorMismatch);
    }
    assignment_record(model)
}

fn assignment_record(model: assignment::Model) -> TranslationResult<AssignmentRecord> {
    let assignee = match (model.assignee_actor_kind, model.assignee_actor_id) {
        (None, None) => None,
        (Some(kind), Some(id)) => Some(workflow_actor(&kind, &id)?),
        _ => return Err(TranslationError::WorkflowRevisionConflict),
    };
    Ok(AssignmentRecord {
        operation_id: model.id,
        item_id: model.item_id,
        assignee,
        item_revision: model.resulting_item_revision,
    })
}

fn replay_cancellation(
    model: cancellation::Model,
    context: &PortContext,
    request_hash: &str,
) -> TranslationResult<CancellationRecord> {
    if model.request_hash != request_hash {
        return Err(TranslationError::IdempotencyConflict);
    }
    if model.requested_by_actor_kind != actor_kind(context)
        || model.requested_by_actor_id != context.actor.id
    {
        return Err(TranslationError::IdempotencyActorMismatch);
    }
    let cancelled_item_count = u64::try_from(model.cancelled_item_count)
        .map_err(|_| TranslationError::WorkflowRevisionConflict)?;
    Ok(CancellationRecord {
        cancellation_id: model.id,
        job_id: model.job_id,
        job_revision: model.resulting_job_revision,
        cancelled_item_count,
    })
}

fn replay_retry(
    model: retry::Model,
    context: &PortContext,
    request_hash: &str,
) -> TranslationResult<RetryRecord> {
    if model.request_hash != request_hash {
        return Err(TranslationError::IdempotencyConflict);
    }
    if model.requested_by_actor_kind != actor_kind(context)
        || model.requested_by_actor_id != context.actor.id
    {
        return Err(TranslationError::IdempotencyActorMismatch);
    }
    retry_record(model)
}

fn retry_record(model: retry::Model) -> TranslationResult<RetryRecord> {
    if model.prior_status != "blocked" || model.resulting_status != "approved" {
        return Err(TranslationError::WorkflowRevisionConflict);
    }
    Ok(RetryRecord {
        retry_id: model.id,
        item_id: model.item_id,
        item_revision: model.resulting_item_revision,
        status: model.resulting_status,
    })
}

fn proposal_provider(
    providers: &TranslationTargetRegistry,
    identity: &TranslationResourceIdentity,
) -> TranslationResult<Arc<dyn TranslationTargetProvider>> {
    let provider = providers
        .get(&identity.owner_slug, &identity.resource_kind)
        .ok_or_else(|| TranslationError::ProviderNotFound {
            owner_slug: identity.owner_slug.as_str().to_string(),
            resource_kind: identity.resource_kind.as_str().to_string(),
        })?;
    if !provider
        .descriptor()
        .capabilities
        .contains(&TranslationTargetCapability::ValidatePatch)
    {
        return Err(TranslationError::InvalidRequest(
            "translation provider does not expose patch validation".to_string(),
        ));
    }
    Ok(provider)
}

fn apply_provider(
    providers: &TranslationTargetRegistry,
    identity: &TranslationResourceIdentity,
) -> TranslationResult<Arc<dyn TranslationTargetProvider>> {
    let provider = providers
        .get(&identity.owner_slug, &identity.resource_kind)
        .ok_or_else(|| TranslationError::ProviderNotFound {
            owner_slug: identity.owner_slug.as_str().to_string(),
            resource_kind: identity.resource_kind.as_str().to_string(),
        })?;
    if !provider
        .descriptor()
        .capabilities
        .contains(&TranslationTargetCapability::ApplyPatch)
    {
        return Err(TranslationError::InvalidRequest(
            "translation provider does not expose patch application".to_string(),
        ));
    }
    Ok(provider)
}

fn validate_apply_actor(
    operation: &apply_operation::Model,
    context: &PortContext,
) -> TranslationResult<()> {
    if operation.created_by_actor_kind != actor_kind(context)
        || operation.created_by_actor_id != context.actor.id
    {
        return Err(TranslationError::IdempotencyActorMismatch);
    }
    Ok(())
}

fn validate_apply_replay(
    operation: &apply_operation::Model,
    context: &PortContext,
    request_hash: &str,
) -> TranslationResult<()> {
    if operation.request_hash != request_hash {
        return Err(TranslationError::IdempotencyConflict);
    }
    validate_apply_actor(operation, context)
}

fn validate_recovery_reason(reason: &str) -> TranslationResult<()> {
    let trimmed = reason.trim();
    if trimmed.is_empty() || trimmed.len() > 500 || trimmed != reason {
        return Err(TranslationError::InvalidRecoveryReason);
    }
    Ok(())
}

fn validate_recovery_replay(
    recovery: &apply_recovery::Model,
    context: &PortContext,
    request_hash: &str,
) -> TranslationResult<()> {
    if recovery.request_hash != request_hash {
        return Err(TranslationError::IdempotencyConflict);
    }
    if recovery.requested_by_actor_kind != actor_kind(context)
        || recovery.requested_by_actor_id != context.actor.id
    {
        return Err(TranslationError::IdempotencyActorMismatch);
    }
    Ok(())
}

fn apply_terminal_error(operation: &apply_operation::Model) -> TranslationError {
    TranslationError::ApplyOperationTerminal {
        status: operation.status.clone(),
        code: operation
            .last_error_code
            .clone()
            .unwrap_or_else(|| "translation.apply_terminal_without_error".to_string()),
    }
}

fn port_error_kind(kind: PortErrorKind) -> &'static str {
    match kind {
        PortErrorKind::Validation => "validation",
        PortErrorKind::Timeout => "timeout",
        PortErrorKind::Unavailable => "unavailable",
        PortErrorKind::NotFound => "not_found",
        PortErrorKind::Conflict => "conflict",
        PortErrorKind::Forbidden => "forbidden",
        PortErrorKind::InvariantViolation => "invariant_violation",
    }
}

fn bounded_error_code(code: &str) -> String {
    if code.len() <= 191 {
        code.to_string()
    } else {
        "translation.provider_error_code_too_long".to_string()
    }
}

fn apply_lease_duration(context: &PortContext) -> ChronoDuration {
    let deadline_seconds = context.deadline_ms.unwrap_or_default().saturating_add(999) / 1_000;
    let deadline_seconds = i64::try_from(deadline_seconds).unwrap_or(MAX_APPLY_LEASE_SECONDS);
    ChronoDuration::seconds(
        deadline_seconds
            .saturating_add(APPLY_LEASE_SAFETY_SECONDS)
            .clamp(MIN_APPLY_LEASE_SECONDS, MAX_APPLY_LEASE_SECONDS),
    )
}

async fn claim_apply_attempt<C>(
    database: &C,
    operation: &apply_operation::Model,
    context: &PortContext,
) -> TranslationResult<Uuid>
where
    C: sea_orm::ConnectionTrait,
{
    let lease_token = generate_id();
    let now = Utc::now().fixed_offset();
    let lease_expires_at = now + apply_lease_duration(context);
    let update = apply_operation::Entity::update_many()
        .col_expr(
            apply_operation::Column::AttemptCount,
            sea_orm::sea_query::ExprTrait::add(Expr::col(apply_operation::Column::AttemptCount), 1),
        )
        .col_expr(
            apply_operation::Column::LastErrorKind,
            Expr::value(Option::<String>::None),
        )
        .col_expr(
            apply_operation::Column::LastErrorCode,
            Expr::value(Option::<String>::None),
        )
        .col_expr(
            apply_operation::Column::LastErrorRetryable,
            Expr::value(Option::<bool>::None),
        )
        .col_expr(
            apply_operation::Column::LeaseToken,
            Expr::value(Some(lease_token)),
        )
        .col_expr(
            apply_operation::Column::LeaseOwnerActorKind,
            Expr::value(Some(actor_kind(context).to_string())),
        )
        .col_expr(
            apply_operation::Column::LeaseOwnerActorId,
            Expr::value(Some(context.actor.id.clone())),
        )
        .col_expr(
            apply_operation::Column::LeaseExpiresAt,
            Expr::value(Some(lease_expires_at)),
        )
        .col_expr(apply_operation::Column::UpdatedAt, Expr::value(now))
        .filter(apply_operation::Column::Id.eq(operation.id))
        .filter(apply_operation::Column::TenantId.eq(operation.tenant_id))
        .filter(apply_operation::Column::Status.eq("pending"))
        .filter(
            Condition::any()
                .add(apply_operation::Column::LeaseToken.is_null())
                .add(apply_operation::Column::LeaseExpiresAt.lte(now)),
        )
        .exec(database)
        .await?;
    if update.rows_affected != 1 {
        return Err(TranslationError::ApplyInProgress);
    }
    Ok(lease_token)
}

struct PendingApplyError<'a> {
    kind: &'a str,
    code: &'a str,
    retryable: bool,
    lease_token: Uuid,
    event_actor_id: Option<Uuid>,
}

async fn record_pending_apply_error(
    database: &DatabaseConnection,
    event_bus: &TransactionalEventBus,
    operation: &apply_operation::Model,
    error: PendingApplyError<'_>,
) -> TranslationResult<()> {
    let transaction = database.begin().await?;
    let update = apply_operation::Entity::update_many()
        .col_expr(
            apply_operation::Column::LastErrorKind,
            Expr::value(Some(error.kind.to_string())),
        )
        .col_expr(
            apply_operation::Column::LastErrorCode,
            Expr::value(Some(bounded_error_code(error.code))),
        )
        .col_expr(
            apply_operation::Column::LastErrorRetryable,
            Expr::value(Some(error.retryable)),
        )
        .col_expr(
            apply_operation::Column::LeaseToken,
            Expr::value(Option::<Uuid>::None),
        )
        .col_expr(
            apply_operation::Column::LeaseOwnerActorKind,
            Expr::value(Option::<String>::None),
        )
        .col_expr(
            apply_operation::Column::LeaseOwnerActorId,
            Expr::value(Option::<String>::None),
        )
        .col_expr(
            apply_operation::Column::LeaseExpiresAt,
            Expr::value(Option::<chrono::DateTime<chrono::FixedOffset>>::None),
        )
        .col_expr(
            apply_operation::Column::UpdatedAt,
            Expr::value(Utc::now().fixed_offset()),
        )
        .filter(apply_operation::Column::Id.eq(operation.id))
        .filter(apply_operation::Column::TenantId.eq(operation.tenant_id))
        .filter(apply_operation::Column::Status.eq("pending"))
        .filter(apply_operation::Column::LeaseToken.eq(error.lease_token))
        .exec(&transaction)
        .await?;
    if update.rows_affected != 1 {
        return Err(TranslationError::WorkflowRevisionConflict);
    }
    event_bus
        .publish_contract_in_tx(
            &transaction,
            operation.tenant_id,
            error.event_actor_id,
            TranslationWorkflowEvent::ApplyFailed {
                operation_id: operation.id,
                item_id: operation.item_id,
                proposal_id: operation.proposal_id,
                status: "pending".to_string(),
                error_code: bounded_error_code(error.code),
                retryable: error.retryable,
                attempt_count: next_revision(operation.attempt_count)?,
            },
        )
        .await?;
    transaction.commit().await?;
    Ok(())
}

fn validate_provider_receipt(
    request: &TranslationPatchRequest,
    receipt: &TranslationApplicationReceipt,
) -> TranslationResult<()> {
    let provider_receipt_id = receipt.provider_receipt_id.trim();
    if provider_receipt_id.is_empty()
        || provider_receipt_id.len() > 191
        || provider_receipt_id != receipt.provider_receipt_id
    {
        return Err(TranslationError::InvalidProviderReceipt(
            "provider_receipt_id must contain 1..=191 trimmed non-whitespace bytes".to_string(),
        ));
    }
    let requested = request
        .fields
        .iter()
        .map(|field| field.key.as_str())
        .collect::<BTreeSet<_>>();
    let applied = receipt
        .applied_field_keys
        .iter()
        .map(FieldKey::as_str)
        .collect::<BTreeSet<_>>();
    if applied.len() != receipt.applied_field_keys.len() || applied != requested {
        return Err(TranslationError::InvalidProviderReceipt(
            "applied_field_keys must exactly match the requested field set".to_string(),
        ));
    }
    Ok(())
}

fn ensure_receipt_matches(
    operation: &apply_operation::Model,
    owner_receipt: &TranslationApplicationReceipt,
    persisted: &apply_receipt::Model,
) -> TranslationResult<()> {
    let persisted_keys: Vec<FieldKey> =
        serde_json::from_value(persisted.applied_field_keys.clone())?;
    let patch: TranslationPatchRequest = serde_json::from_value(operation.patch.clone())?;
    if persisted.tenant_id != operation.tenant_id
        || persisted.item_id != operation.item_id
        || persisted.proposal_id != operation.proposal_id
        || persisted.idempotency_key != operation.idempotency_key
        || persisted.request_hash != operation.request_hash
        || persisted.approval_receipt_id != patch.approval_receipt_id
        || persisted.provider_receipt_id != owner_receipt.provider_receipt_id
        || persisted.resource_revision != owner_receipt.resource_revision.as_str()
        || persisted.target_revision != owner_receipt.target_revision.as_str()
        || persisted_keys != owner_receipt.applied_field_keys
    {
        return Err(TranslationError::ProviderReceiptMismatch);
    }
    Ok(())
}

fn apply_record(
    operation: &apply_operation::Model,
    receipt: apply_receipt::Model,
) -> TranslationResult<ApplyRecord> {
    let resource_revision = OpaqueRevision::new(receipt.resource_revision.clone())
        .map_err(|error| TranslationError::InvalidProviderReceipt(error.to_string()))?;
    let target_revision = OpaqueRevision::new(receipt.target_revision.clone())
        .map_err(|error| TranslationError::InvalidProviderReceipt(error.to_string()))?;
    let applied_field_keys: Vec<FieldKey> =
        serde_json::from_value(receipt.applied_field_keys.clone())?;
    let persisted_owner_receipt = TranslationApplicationReceipt {
        provider_receipt_id: receipt.provider_receipt_id.clone(),
        resource_revision: resource_revision.clone(),
        target_revision: target_revision.clone(),
        applied_field_keys: applied_field_keys.clone(),
    };
    let patch: TranslationPatchRequest = serde_json::from_value(operation.patch.clone())?;
    validate_provider_receipt(&patch, &persisted_owner_receipt)?;
    ensure_receipt_matches(operation, &persisted_owner_receipt, &receipt)?;
    Ok(ApplyRecord {
        operation_id: operation.id,
        item_id: receipt.item_id,
        proposal_id: receipt.proposal_id,
        provider_receipt_id: receipt.provider_receipt_id,
        resource_revision,
        target_revision,
        applied_field_keys,
    })
}

fn build_patch(
    snapshot: &TranslationResourceSnapshot,
    values: &[ProposalValue],
    proposal_id: Uuid,
    approval_receipt_id: &str,
) -> TranslationResult<TranslationPatchRequest> {
    if values.is_empty() {
        return Err(TranslationError::InvalidRequest(
            "translation proposal must contain at least one field".to_string(),
        ));
    }
    let mut keys = BTreeSet::new();
    let mut fields = Vec::with_capacity(values.len());
    for value in values {
        if !keys.insert(value.key.as_str()) {
            return Err(TranslationError::InvalidRequest(
                "translation proposal contains a duplicate field key".to_string(),
            ));
        }
        let source = snapshot
            .fields
            .iter()
            .find(|field| field.descriptor.key == value.key)
            .ok_or_else(|| {
                TranslationError::InvalidRequest(format!(
                    "translation proposal contains unknown field `{}`",
                    value.key
                ))
            })?;
        fields.push(TranslationFieldPatch {
            key: value.key.clone(),
            value: value.value.clone(),
            expected_source_hash: source.source_hash.clone(),
        });
    }
    patch_from_persisted(snapshot, fields, proposal_id, approval_receipt_id)
}

fn patch_from_persisted(
    snapshot: &TranslationResourceSnapshot,
    fields: Vec<TranslationFieldPatch>,
    proposal_id: Uuid,
    approval_receipt_id: &str,
) -> TranslationResult<TranslationPatchRequest> {
    let request = TranslationPatchRequest {
        identity: snapshot.summary.identity.clone(),
        source_locale: snapshot.source_locale.clone(),
        target_locale: snapshot.target_locale.clone(),
        expected_resource_revision: OpaqueRevision::new(
            snapshot.summary.resource_revision.as_str(),
        )
        .map_err(|error| TranslationError::InvalidRequest(error.to_string()))?,
        expected_source_revision: OpaqueRevision::new(snapshot.source_revision.as_str())
            .map_err(|error| TranslationError::InvalidRequest(error.to_string()))?,
        expected_target_revision: snapshot.target_revision.clone(),
        fields,
        proposal_id: proposal_id.to_string(),
        approval_receipt_id: approval_receipt_id.to_string(),
    };
    request
        .validate()
        .map_err(|error| TranslationError::InvalidRequest(error.to_string()))?;
    Ok(request)
}

fn require_current_proposal(item: &job_item::Model, proposal_id: Uuid) -> TranslationResult<()> {
    if item.current_proposal_id != Some(proposal_id) {
        return Err(TranslationError::ProposalNotCurrent);
    }
    Ok(())
}

async fn update_item_state<C>(
    database: &C,
    item: &job_item::Model,
    tenant_id: Uuid,
    status: &str,
    next_revision: i64,
    now: chrono::DateTime<chrono::FixedOffset>,
) -> TranslationResult<()>
where
    C: sea_orm::ConnectionTrait,
{
    let update = job_item::Entity::update_many()
        .col_expr(job_item::Column::Status, Expr::value(status.to_string()))
        .col_expr(job_item::Column::Revision, Expr::value(next_revision))
        .col_expr(job_item::Column::UpdatedAt, Expr::value(now))
        .filter(job_item::Column::Id.eq(item.id))
        .filter(job_item::Column::TenantId.eq(tenant_id))
        .filter(job_item::Column::Revision.eq(item.revision))
        .exec(database)
        .await?;
    if update.rows_affected != 1 {
        return Err(TranslationError::WorkflowRevisionConflict);
    }
    Ok(())
}

async fn find_item<C>(
    database: &C,
    tenant_id: Uuid,
    item_id: Uuid,
) -> TranslationResult<job_item::Model>
where
    C: sea_orm::ConnectionTrait,
{
    job_item::Entity::find_by_id(item_id)
        .filter(job_item::Column::TenantId.eq(tenant_id))
        .one(database)
        .await?
        .ok_or(TranslationError::ItemNotFound)
}

async fn find_job<C>(database: &C, tenant_id: Uuid, job_id: Uuid) -> TranslationResult<job::Model>
where
    C: sea_orm::ConnectionTrait,
{
    job::Entity::find_by_id(job_id)
        .filter(job::Column::TenantId.eq(tenant_id))
        .one(database)
        .await?
        .ok_or(TranslationError::JobNotFound)
}

async fn job_glossary_snapshot<C>(
    database: &C,
    tenant_id: Uuid,
    job_id: Uuid,
) -> TranslationResult<Option<GlossaryRecord>>
where
    C: sea_orm::ConnectionTrait,
{
    let job = find_job(database, tenant_id, job_id).await?;
    let binding = match (job.glossary_id, job.glossary_revision) {
        (None, None) => return Ok(None),
        (Some(glossary_id), Some(revision)) => GlossaryBinding {
            glossary_id,
            revision,
        },
        _ => {
            return Err(TranslationError::GlossaryInvariant(
                "translation job contains a partial glossary binding".to_string(),
            ));
        }
    };
    let glossary = read_bound_glossary(database, tenant_id, &binding).await?;
    if glossary.source_locale.as_str() != job.source_locale
        || glossary.target_locale.as_str() != job.target_locale
    {
        return Err(TranslationError::GlossaryInvariant(
            "bound glossary locales do not match the translation job locales".to_string(),
        ));
    }
    Ok(Some(glossary))
}

async fn find_proposal<C>(
    database: &C,
    tenant_id: Uuid,
    item_id: Uuid,
    proposal_id: Uuid,
) -> TranslationResult<proposal::Model>
where
    C: sea_orm::ConnectionTrait,
{
    proposal::Entity::find_by_id(proposal_id)
        .filter(proposal::Column::TenantId.eq(tenant_id))
        .filter(proposal::Column::ItemId.eq(item_id))
        .one(database)
        .await?
        .ok_or(TranslationError::ProposalNotFound)
}

async fn find_assignment_by_idempotency<C>(
    database: &C,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> TranslationResult<Option<assignment::Model>>
where
    C: sea_orm::ConnectionTrait,
{
    Ok(assignment::Entity::find()
        .filter(assignment::Column::TenantId.eq(tenant_id))
        .filter(assignment::Column::IdempotencyKey.eq(idempotency_key))
        .one(database)
        .await?)
}

async fn find_cancellation_by_idempotency<C>(
    database: &C,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> TranslationResult<Option<cancellation::Model>>
where
    C: sea_orm::ConnectionTrait,
{
    Ok(cancellation::Entity::find()
        .filter(cancellation::Column::TenantId.eq(tenant_id))
        .filter(cancellation::Column::IdempotencyKey.eq(idempotency_key))
        .one(database)
        .await?)
}

async fn find_retry_by_idempotency<C>(
    database: &C,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> TranslationResult<Option<retry::Model>>
where
    C: sea_orm::ConnectionTrait,
{
    Ok(retry::Entity::find()
        .filter(retry::Column::TenantId.eq(tenant_id))
        .filter(retry::Column::IdempotencyKey.eq(idempotency_key))
        .one(database)
        .await?)
}

async fn find_proposal_by_idempotency<C>(
    database: &C,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> TranslationResult<Option<proposal::Model>>
where
    C: sea_orm::ConnectionTrait,
{
    Ok(proposal::Entity::find()
        .filter(proposal::Column::TenantId.eq(tenant_id))
        .filter(proposal::Column::IdempotencyKey.eq(idempotency_key))
        .one(database)
        .await?)
}

async fn find_proposal_by_submission_idempotency<C>(
    database: &C,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> TranslationResult<Option<proposal::Model>>
where
    C: sea_orm::ConnectionTrait,
{
    Ok(proposal::Entity::find()
        .filter(proposal::Column::TenantId.eq(tenant_id))
        .filter(proposal::Column::SubmissionIdempotencyKey.eq(idempotency_key))
        .one(database)
        .await?)
}

async fn find_proposal_by_approval_idempotency<C>(
    database: &C,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> TranslationResult<Option<proposal::Model>>
where
    C: sea_orm::ConnectionTrait,
{
    Ok(proposal::Entity::find()
        .filter(proposal::Column::TenantId.eq(tenant_id))
        .filter(proposal::Column::ApprovalIdempotencyKey.eq(idempotency_key))
        .one(database)
        .await?)
}

async fn find_apply_operation<C>(
    database: &C,
    tenant_id: Uuid,
    operation_id: Uuid,
) -> TranslationResult<apply_operation::Model>
where
    C: sea_orm::ConnectionTrait,
{
    apply_operation::Entity::find_by_id(operation_id)
        .filter(apply_operation::Column::TenantId.eq(tenant_id))
        .one(database)
        .await?
        .ok_or(TranslationError::WorkflowRevisionConflict)
}

async fn find_apply_operation_by_idempotency<C>(
    database: &C,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> TranslationResult<Option<apply_operation::Model>>
where
    C: sea_orm::ConnectionTrait,
{
    Ok(apply_operation::Entity::find()
        .filter(apply_operation::Column::TenantId.eq(tenant_id))
        .filter(apply_operation::Column::IdempotencyKey.eq(idempotency_key))
        .one(database)
        .await?)
}

async fn find_apply_receipt_by_idempotency<C>(
    database: &C,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> TranslationResult<Option<apply_receipt::Model>>
where
    C: sea_orm::ConnectionTrait,
{
    Ok(apply_receipt::Entity::find()
        .filter(apply_receipt::Column::TenantId.eq(tenant_id))
        .filter(apply_receipt::Column::IdempotencyKey.eq(idempotency_key))
        .one(database)
        .await?)
}

async fn find_apply_recovery_by_idempotency<C>(
    database: &C,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> TranslationResult<Option<apply_recovery::Model>>
where
    C: sea_orm::ConnectionTrait,
{
    Ok(apply_recovery::Entity::find()
        .filter(apply_recovery::Column::TenantId.eq(tenant_id))
        .filter(apply_recovery::Column::IdempotencyKey.eq(idempotency_key))
        .one(database)
        .await?)
}

async fn find_job_by_idempotency<C>(
    database: &C,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> TranslationResult<Option<job::Model>>
where
    C: sea_orm::ConnectionTrait,
{
    Ok(job::Entity::find()
        .filter(job::Column::TenantId.eq(tenant_id))
        .filter(job::Column::IdempotencyKey.eq(idempotency_key))
        .one(database)
        .await?)
}

async fn find_item_by_idempotency<C>(
    database: &C,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> TranslationResult<Option<job_item::Model>>
where
    C: sea_orm::ConnectionTrait,
{
    Ok(job_item::Entity::find()
        .filter(job_item::Column::TenantId.eq(tenant_id))
        .filter(job_item::Column::IdempotencyKey.eq(idempotency_key))
        .one(database)
        .await?)
}

async fn find_item_by_identity<C>(
    database: &C,
    tenant_id: Uuid,
    job_id: Uuid,
    identity: &TranslationResourceIdentity,
) -> TranslationResult<Option<job_item::Model>>
where
    C: sea_orm::ConnectionTrait,
{
    Ok(job_item::Entity::find()
        .filter(job_item::Column::TenantId.eq(tenant_id))
        .filter(job_item::Column::JobId.eq(job_id))
        .filter(job_item::Column::OwnerSlug.eq(identity.owner_slug.as_str()))
        .filter(job_item::Column::ResourceKind.eq(identity.resource_kind.as_str()))
        .filter(job_item::Column::ResourceId.eq(identity.resource_id.as_str()))
        .filter(
            job_item::Column::SubresourceKey.eq(identity
                .subresource_id
                .as_ref()
                .map(|value| value.as_str())
                .unwrap_or_default()),
        )
        .one(database)
        .await?)
}

fn replay_job(model: job::Model, request_hash: &str) -> TranslationResult<JobRecord> {
    if model.request_hash != request_hash {
        return Err(TranslationError::IdempotencyConflict);
    }
    Ok(JobRecord {
        id: model.id,
        source_locale: TenantLocale::new(model.source_locale)
            .map_err(|error| TranslationError::InvalidRequest(error.to_string()))?,
        target_locale: TenantLocale::new(model.target_locale)
            .map_err(|error| TranslationError::InvalidRequest(error.to_string()))?,
        glossary: match (model.glossary_id, model.glossary_revision) {
            (Some(glossary_id), Some(revision)) => Some(GlossaryBinding {
                glossary_id,
                revision,
            }),
            (None, None) => None,
            _ => {
                return Err(TranslationError::GlossaryInvariant(
                    "translation job contains a partial glossary binding".to_string(),
                ));
            }
        },
        status: model.status,
        revision: model.revision,
    })
}

pub(crate) fn item_record(model: job_item::Model) -> TranslationResult<JobItemRecord> {
    let assignee = assignment_actor(&model)?;
    Ok(JobItemRecord {
        id: model.id,
        job_id: model.job_id,
        identity: TranslationResourceIdentity {
            owner_slug: rustok_translation_targets::OwnerSlug::new(model.owner_slug)
                .map_err(|error| TranslationError::InvalidRequest(error.to_string()))?,
            resource_kind: rustok_translation_targets::ResourceKind::new(model.resource_kind)
                .map_err(|error| TranslationError::InvalidRequest(error.to_string()))?,
            resource_id: rustok_translation_targets::ResourceId::new(model.resource_id)
                .map_err(|error| TranslationError::InvalidRequest(error.to_string()))?,
            subresource_id: if model.subresource_key.is_empty() {
                None
            } else {
                Some(
                    rustok_translation_targets::ResourceId::new(model.subresource_key)
                        .map_err(|error| TranslationError::InvalidRequest(error.to_string()))?,
                )
            },
        },
        status: model.status,
        assignee,
        source_digest: model.source_digest,
        revision: model.revision,
    })
}

fn replay_proposal(
    model: proposal::Model,
    request_hash: &str,
) -> TranslationResult<ProposalRecord> {
    if model.request_hash != request_hash {
        return Err(TranslationError::IdempotencyConflict);
    }
    proposal_record(model)
}

fn replay_submission(
    model: proposal::Model,
    request_hash: &str,
) -> TranslationResult<ProposalRecord> {
    if model.submission_request_hash.as_deref() != Some(request_hash) {
        return Err(TranslationError::IdempotencyConflict);
    }
    proposal_record(model)
}

fn replay_approval(
    model: proposal::Model,
    request_hash: &str,
) -> TranslationResult<ProposalRecord> {
    if model.approval_request_hash.as_deref() != Some(request_hash) {
        return Err(TranslationError::IdempotencyConflict);
    }
    proposal_record(model)
}

fn proposal_record(model: proposal::Model) -> TranslationResult<ProposalRecord> {
    let origin = match model.origin.as_str() {
        "manual" => ProposalOrigin::Manual,
        "import" => ProposalOrigin::Import,
        "memory" => ProposalOrigin::Memory,
        "ai" => ProposalOrigin::Ai,
        value => {
            return Err(TranslationError::InvalidRequest(format!(
                "unknown translation proposal origin `{value}`"
            )));
        }
    };
    let status = if model.approved_at.is_some() {
        "approved"
    } else if model.submitted_at.is_some() {
        "in_review"
    } else {
        "draft"
    };
    let qa_issues: Vec<TranslationPatchIssue> = serde_json::from_value(model.qa_issues)?;
    let qa_accepted = !qa_issues
        .iter()
        .any(|issue| issue.severity == TranslationPatchIssueSeverity::Error);
    Ok(ProposalRecord {
        id: model.id,
        item_id: model.item_id,
        proposal_revision: model.proposal_revision,
        origin,
        values: serde_json::from_value(model.values)?,
        qa_issues,
        qa_accepted,
        status: status.to_string(),
        approval_receipt_id: model.approval_receipt_id,
    })
}
