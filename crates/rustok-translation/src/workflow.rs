use std::{collections::BTreeSet, sync::Arc};

use chrono::Utc;
use rustok_api::{
    Action, PortActorKind, PortCallPolicy, PortContext, Resource, TenantLocale,
    manifest_hash::hash_manifest,
};
use rustok_core::{PermissionScope, SecurityContext, generate_id};
use rustok_translation_targets::{
    FieldKey, OpaqueRevision, ReadTranslationResourceRequest, TranslationFieldPatch,
    TranslationPatchIssue, TranslationPatchRequest, TranslationResourceIdentity,
    TranslationResourceSnapshot, TranslationTargetCapability, TranslationTargetProvider,
    TranslationTargetRegistry,
};
use sea_orm::{
    ColumnTrait, DatabaseConnection, EntityTrait, QueryFilter, Set, TransactionTrait,
    sea_query::{Expr, OnConflict},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    TranslationError, TranslationResult,
    entities::{job, job_item, proposal},
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateJobInput {
    pub source_locale: TenantLocale,
    pub target_locale: TenantLocale,
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

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobRecord {
    pub id: Uuid,
    pub source_locale: TenantLocale,
    pub target_locale: TenantLocale,
    pub status: String,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobItemRecord {
    pub id: Uuid,
    pub job_id: Uuid,
    pub identity: TranslationResourceIdentity,
    pub status: String,
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
    pub status: String,
    pub approval_receipt_id: Option<String>,
}

pub struct TranslationWorkflowService {
    database: DatabaseConnection,
    providers: Arc<TranslationTargetRegistry>,
}

impl TranslationWorkflowService {
    pub fn new(database: DatabaseConnection, providers: Arc<TranslationTargetRegistry>) -> Self {
        Self {
            database,
            providers,
        }
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
        job::Entity::insert(job::ActiveModel {
            id: Set(generate_id()),
            tenant_id: Set(tenant_id),
            source_locale: Set(input.source_locale.as_str().to_string()),
            target_locale: Set(input.target_locale.as_str().to_string()),
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
        .exec_without_returning(&self.database)
        .await?;
        let persisted = find_job_by_idempotency(&self.database, tenant_id, &idempotency_key)
            .await?
            .ok_or(TranslationError::WorkflowRevisionConflict)?;
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
        let tenant_id = authorize_write(&context, Action::Update)?;
        let idempotency_key = operation_idempotency_key(&context);
        let request_hash = hash_manifest(&input)?;
        if let Some(existing) =
            find_proposal_by_idempotency(&self.database, tenant_id, &idempotency_key).await?
        {
            return replay_proposal(existing, &request_hash);
        }

        let item = find_item(&self.database, tenant_id, input.item_id).await?;
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
        let validation = provider
            .validate_patch(context.clone(), patch.clone())
            .await?;
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
        let validation = provider.validate_patch(context, patch).await?;
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
        let validation = provider.validate_patch(context.clone(), patch).await?;
        if !validation.accepted {
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
                Expr::value(Some(context.actor.id)),
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
        transaction.commit().await?;
        proposal_record(persisted)
    }
}

fn authorize_write(context: &PortContext, action: Action) -> TranslationResult<Uuid> {
    context.require_policy(PortCallPolicy::write())?;
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::Translations, action) == PermissionScope::None {
        return Err(TranslationError::Forbidden);
    }
    Uuid::parse_str(&context.tenant_id).map_err(|_| TranslationError::InvalidTenantId)
}

fn actor_kind(context: &PortContext) -> &'static str {
    match context.actor.kind {
        PortActorKind::User => "user",
        PortActorKind::Service => "service",
        PortActorKind::System => "system",
    }
}

fn operation_idempotency_key(context: &PortContext) -> String {
    context.idempotency_key.clone().unwrap_or_default()
}

fn next_revision(revision: i64) -> TranslationResult<i64> {
    revision
        .checked_add(1)
        .ok_or(TranslationError::WorkflowRevisionConflict)
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
        status: model.status,
        revision: model.revision,
    })
}

fn item_record(model: job_item::Model) -> TranslationResult<JobItemRecord> {
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
    Ok(ProposalRecord {
        id: model.id,
        item_id: model.item_id,
        proposal_revision: model.proposal_revision,
        origin,
        values: serde_json::from_value(model.values)?,
        qa_issues: serde_json::from_value(model.qa_issues)?,
        status: status.to_string(),
        approval_receipt_id: model.approval_receipt_id,
    })
}
