use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use chrono::{DateTime, FixedOffset, Utc};
use rustok_api::{
    Action, PortActor, PortCallPolicy, PortContext, Resource, TenantLocale,
    manifest_hash::hash_manifest,
};
use rustok_core::{PermissionScope, SecurityContext, generate_id};
use rustok_tenant::TenantLocalePolicyPort;
use rustok_translation_targets::{
    FieldKey, OpaqueCursor, OwnerSlug, ResourceKind, TranslationFieldPatch,
    TranslationResourceSnapshot, TranslationTargetCapability, TranslationTargetProgressFacts,
    TranslationTargetProgressRequest, TranslationTargetRegistry,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, TransactionTrait,
    sea_query::{Expr, OnConflict},
};
use serde::Serialize;
use uuid::Uuid;

use crate::{
    TranslationError, TranslationPolicyFreshness, TranslationPolicyService, TranslationResult,
    entities::{apply_receipt, job, job_item, job_progress, proposal, provider_checkpoint},
    workflow::{
        JobItemRecord, actor_kind_value, assignment_actor, item_record, validate_workflow_actor,
    },
};

const MAX_REVIEWER_QUEUE_LIMIT: u16 = 200;
const MAX_REVIEWER_WORKLOAD_ITEMS: usize = 500;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JobProgressRecord {
    pub job_id: Uuid,
    pub source_digest: String,
    pub total_items: u64,
    pub assigned_items: u64,
    pub terminal_items: u64,
    pub missing_items: u64,
    pub draft_items: u64,
    pub in_review_items: u64,
    pub approved_items: u64,
    pub applying_items: u64,
    pub applied_items: u64,
    pub stale_items: u64,
    pub conflict_items: u64,
    pub blocked_items: u64,
    pub excluded_items: u64,
    pub cancelled_items: u64,
    pub required_units: u64,
    pub optional_units: u64,
    pub applied_required_units: u64,
    pub applied_optional_units: u64,
    pub approved_required_units: u64,
    pub approved_optional_units: u64,
    pub complete_resources: u64,
    pub source_characters: u64,
    pub translated_characters: u64,
    pub revision: i64,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewerQueueInput {
    pub job_id: Uuid,
    pub assignee: Option<PortActor>,
    pub include_unassigned: bool,
    pub limit: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewerQueueRecord {
    pub item: JobItemRecord,
    pub proposal_id: Uuid,
    pub proposal_revision: i64,
    pub submitted_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewerWorkloadInput {
    pub job_id: Uuid,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReviewerWorkloadRecord {
    pub job_id: Uuid,
    pub assignee: Option<PortActor>,
    pub open_items: u64,
    pub missing_items: u64,
    pub draft_items: u64,
    pub in_review_items: u64,
    pub approved_items: u64,
    pub applying_items: u64,
    pub rebase_required_items: u64,
    pub blocked_items: u64,
    pub source_characters: u64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderProjectionFreshness {
    Current,
    Behind,
    Unknown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderProgressRecord {
    pub owner_slug: OwnerSlug,
    pub resource_kind: ResourceKind,
    pub source_locale: TenantLocale,
    pub target_locale: TenantLocale,
    pub facts: TranslationTargetProgressFacts,
    pub projected_cursor: Option<OpaqueCursor>,
    pub checkpoint_revision: Option<i64>,
    pub checkpoint_updated_at: Option<DateTime<FixedOffset>>,
    pub freshness: ProviderProjectionFreshness,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RequiredProviderProgressRecord {
    pub owner_slug: OwnerSlug,
    pub resource_kind: ResourceKind,
    pub source_locale: TenantLocale,
    pub required_target_locales: Vec<TenantLocale>,
    pub translation_policy_revision: i64,
    pub tenant_locale_policy_revision: i64,
    pub required_units: u64,
    pub exact_required_units: u64,
    pub optional_units: u64,
    pub exact_optional_units: u64,
    pub resource_locale_pairs: u64,
    pub complete_resource_locale_pairs: u64,
    pub freshness: ProviderProjectionFreshness,
    pub targets: Vec<ProviderProgressRecord>,
}

pub struct TranslationProgressService {
    database: DatabaseConnection,
    providers: Arc<TranslationTargetRegistry>,
    tenant_locale_policies: Arc<dyn TenantLocalePolicyPort>,
}

impl TranslationProgressService {
    pub fn new(
        database: DatabaseConnection,
        providers: Arc<TranslationTargetRegistry>,
        tenant_locale_policies: Arc<dyn TenantLocalePolicyPort>,
    ) -> Self {
        Self {
            database,
            providers,
            tenant_locale_policies,
        }
    }

    pub async fn read_job_progress(
        &self,
        context: PortContext,
        job_id: Uuid,
    ) -> TranslationResult<JobProgressRecord> {
        let tenant_id = authorize_progress(&context, PortCallPolicy::read(), Action::Read)?;
        ensure_job(&self.database, tenant_id, job_id).await?;
        let model = job_progress::Entity::find()
            .filter(job_progress::Column::TenantId.eq(tenant_id))
            .filter(job_progress::Column::JobId.eq(job_id))
            .one(&self.database)
            .await?
            .ok_or(TranslationError::JobProgressNotFound)?;
        progress_record(model)
    }

    pub async fn list_reviewer_queue(
        &self,
        context: PortContext,
        input: ReviewerQueueInput,
    ) -> TranslationResult<Vec<ReviewerQueueRecord>> {
        let tenant_id = authorize_progress(&context, PortCallPolicy::read(), Action::Read)?;
        if input.limit == 0 || input.limit > MAX_REVIEWER_QUEUE_LIMIT {
            return Err(TranslationError::InvalidRequest(format!(
                "reviewer queue limit must be between 1 and {MAX_REVIEWER_QUEUE_LIMIT}"
            )));
        }
        if let Some(assignee) = input.assignee.as_ref() {
            validate_workflow_actor(assignee)?;
        }
        ensure_job(&self.database, tenant_id, input.job_id).await?;

        let mut query = job_item::Entity::find()
            .filter(job_item::Column::TenantId.eq(tenant_id))
            .filter(job_item::Column::JobId.eq(input.job_id))
            .filter(job_item::Column::Status.eq("in_review"));
        if let Some(assignee) = input.assignee.as_ref() {
            query = query
                .filter(job_item::Column::AssignedActorKind.eq(actor_kind_value(&assignee.kind)))
                .filter(job_item::Column::AssignedActorId.eq(&assignee.id));
        } else if !input.include_unassigned {
            query = query
                .filter(job_item::Column::AssignedActorKind.is_not_null())
                .filter(job_item::Column::AssignedActorId.is_not_null());
        }
        let items = query
            .order_by_asc(job_item::Column::UpdatedAt)
            .order_by_asc(job_item::Column::Id)
            .limit(u64::from(input.limit))
            .all(&self.database)
            .await?;
        let proposal_ids = items
            .iter()
            .filter_map(|item| item.current_proposal_id)
            .collect::<Vec<_>>();
        let proposals = if proposal_ids.is_empty() {
            BTreeMap::new()
        } else {
            proposal::Entity::find()
                .filter(proposal::Column::TenantId.eq(tenant_id))
                .filter(proposal::Column::Id.is_in(proposal_ids))
                .all(&self.database)
                .await?
                .into_iter()
                .map(|proposal| (proposal.id, proposal))
                .collect()
        };

        items
            .into_iter()
            .map(|item| {
                let proposal_id = item.current_proposal_id.ok_or_else(|| {
                    TranslationError::InvalidProgressSource(
                        "in-review job item has no current proposal".to_string(),
                    )
                })?;
                let proposal = proposals.get(&proposal_id).ok_or_else(|| {
                    TranslationError::InvalidProgressSource(
                        "in-review job item references a missing current proposal".to_string(),
                    )
                })?;
                let submitted_at = proposal.submitted_at.ok_or_else(|| {
                    TranslationError::InvalidProgressSource(
                        "in-review job item current proposal has not been submitted".to_string(),
                    )
                })?;
                if proposal.item_id != item.id || proposal.approved_at.is_some() {
                    return Err(TranslationError::InvalidProgressSource(
                        "in-review job item current proposal does not match review state"
                            .to_string(),
                    ));
                }
                Ok(ReviewerQueueRecord {
                    proposal_id,
                    proposal_revision: proposal.proposal_revision,
                    submitted_at,
                    item: item_record(item)?,
                })
            })
            .collect()
    }

    pub async fn list_reviewer_workload(
        &self,
        context: PortContext,
        input: ReviewerWorkloadInput,
    ) -> TranslationResult<Vec<ReviewerWorkloadRecord>> {
        let tenant_id = authorize_progress(&context, PortCallPolicy::read(), Action::Read)?;
        ensure_job(&self.database, tenant_id, input.job_id).await?;
        let items = job_item::Entity::find()
            .filter(job_item::Column::TenantId.eq(tenant_id))
            .filter(job_item::Column::JobId.eq(input.job_id))
            .order_by_asc(job_item::Column::Id)
            .limit((MAX_REVIEWER_WORKLOAD_ITEMS + 1) as u64)
            .all(&self.database)
            .await?;
        if items.len() > MAX_REVIEWER_WORKLOAD_ITEMS {
            return Err(TranslationError::InvalidRequest(format!(
                "reviewer workload is bounded to {MAX_REVIEWER_WORKLOAD_ITEMS} job items"
            )));
        }

        let mut workloads = BTreeMap::<(String, String), ReviewerWorkloadRecord>::new();
        for item in items {
            let assignee = assignment_actor(&item)?;
            let Some(open) = reviewer_workload_open(&item)? else {
                continue;
            };
            let workload = workloads
                .entry(reviewer_workload_key(&assignee))
                .or_insert_with(|| ReviewerWorkloadRecord {
                    job_id: input.job_id,
                    assignee,
                    open_items: 0,
                    missing_items: 0,
                    draft_items: 0,
                    in_review_items: 0,
                    approved_items: 0,
                    applying_items: 0,
                    rebase_required_items: 0,
                    blocked_items: 0,
                    source_characters: 0,
                });
            reviewer_workload_count(workload, open)?;
            let source_characters = source_character_count(&validated_source_snapshot(&item)?)?;
            workload.source_characters =
                checked_progress_add(workload.source_characters, source_characters)?;
        }
        Ok(workloads.into_values().collect())
    }

    pub async fn rebuild_job_progress(
        &self,
        context: PortContext,
        job_id: Uuid,
    ) -> TranslationResult<JobProgressRecord> {
        let tenant_id = authorize_progress(&context, PortCallPolicy::write(), Action::Manage)?;
        let transaction = self.database.begin().await?;
        let progress = refresh_job_progress(&transaction, tenant_id, job_id).await?;
        transaction.commit().await?;
        Ok(progress)
    }

    pub async fn read_provider_progress(
        &self,
        context: PortContext,
        owner_slug: OwnerSlug,
        resource_kind: ResourceKind,
        source_locale: TenantLocale,
        target_locale: TenantLocale,
    ) -> TranslationResult<ProviderProgressRecord> {
        let tenant_id = authorize_progress(&context, PortCallPolicy::read(), Action::Read)?;
        let request = TranslationTargetProgressRequest {
            source_locale: source_locale.clone(),
            target_locale: target_locale.clone(),
        };
        request
            .validate()
            .map_err(|error| TranslationError::InvalidRequest(error.to_string()))?;
        let provider = self
            .providers
            .get(&owner_slug, &resource_kind)
            .ok_or_else(|| TranslationError::ProviderNotFound {
                owner_slug: owner_slug.as_str().to_string(),
                resource_kind: resource_kind.as_str().to_string(),
            })?;
        if !provider
            .descriptor()
            .capabilities
            .contains(&TranslationTargetCapability::AggregateProgress)
        {
            return Err(TranslationError::AggregateProgressUnavailable);
        }

        let facts = provider.read_progress(context, request).await?;
        facts
            .validate()
            .map_err(|error| TranslationError::InvalidProviderProgress(error.to_string()))?;
        let checkpoint = provider_checkpoint::Entity::find()
            .filter(provider_checkpoint::Column::TenantId.eq(tenant_id))
            .filter(provider_checkpoint::Column::OwnerSlug.eq(owner_slug.as_str()))
            .filter(provider_checkpoint::Column::ResourceKind.eq(resource_kind.as_str()))
            .one(&self.database)
            .await?;
        let projected_cursor = checkpoint
            .as_ref()
            .and_then(|checkpoint| checkpoint.cursor.as_deref())
            .map(OpaqueCursor::new)
            .transpose()
            .map_err(|error| TranslationError::InvalidProviderCheckpoint(error.to_string()))?;
        let freshness = match checkpoint.as_ref() {
            None => ProviderProjectionFreshness::Unknown,
            Some(_) if projected_cursor == facts.owner_change_cursor => {
                ProviderProjectionFreshness::Current
            }
            Some(_) => ProviderProjectionFreshness::Behind,
        };

        Ok(ProviderProgressRecord {
            owner_slug,
            resource_kind,
            source_locale,
            target_locale,
            facts,
            projected_cursor,
            checkpoint_revision: checkpoint.as_ref().map(|checkpoint| checkpoint.revision),
            checkpoint_updated_at: checkpoint.map(|checkpoint| checkpoint.updated_at),
            freshness,
        })
    }

    pub async fn read_required_provider_progress(
        &self,
        context: PortContext,
        owner_slug: OwnerSlug,
        resource_kind: ResourceKind,
        source_locale: TenantLocale,
    ) -> TranslationResult<RequiredProviderProgressRecord> {
        let policy = TranslationPolicyService::new(
            self.database.clone(),
            Arc::clone(&self.tenant_locale_policies),
        )
        .read_policy(context.clone())
        .await?;
        if policy.freshness != TranslationPolicyFreshness::Current {
            return Err(TranslationError::TranslationPolicyStale(
                "required-target progress requires a policy validated against the current tenant locale revision"
                    .to_string(),
            ));
        }
        let required_target_locales = policy
            .required_target_locales
            .into_iter()
            .filter(|locale| locale != &source_locale)
            .collect::<Vec<_>>();
        let mut targets = Vec::with_capacity(required_target_locales.len());
        let mut required_units = 0_u64;
        let mut exact_required_units = 0_u64;
        let mut optional_units = 0_u64;
        let mut exact_optional_units = 0_u64;
        let mut resource_locale_pairs = 0_u64;
        let mut complete_resource_locale_pairs = 0_u64;

        for target_locale in &required_target_locales {
            let progress = self
                .read_provider_progress(
                    context.clone(),
                    owner_slug.clone(),
                    resource_kind.clone(),
                    source_locale.clone(),
                    target_locale.clone(),
                )
                .await?;
            required_units = checked_progress_add(required_units, progress.facts.required_units)?;
            exact_required_units =
                checked_progress_add(exact_required_units, progress.facts.exact_required_units)?;
            optional_units = checked_progress_add(optional_units, progress.facts.optional_units)?;
            exact_optional_units =
                checked_progress_add(exact_optional_units, progress.facts.exact_optional_units)?;
            resource_locale_pairs =
                checked_progress_add(resource_locale_pairs, progress.facts.resources)?;
            complete_resource_locale_pairs = checked_progress_add(
                complete_resource_locale_pairs,
                progress.facts.complete_resources,
            )?;
            targets.push(progress);
        }
        let freshness = aggregate_freshness(&targets);

        Ok(RequiredProviderProgressRecord {
            owner_slug,
            resource_kind,
            source_locale,
            required_target_locales,
            translation_policy_revision: policy.revision,
            tenant_locale_policy_revision: policy.tenant_locale_policy_revision,
            required_units,
            exact_required_units,
            optional_units,
            exact_optional_units,
            resource_locale_pairs,
            complete_resource_locale_pairs,
            freshness,
            targets,
        })
    }
}

fn checked_progress_add(left: u64, right: u64) -> TranslationResult<u64> {
    left.checked_add(right)
        .ok_or(TranslationError::ProgressOverflow)
}

fn aggregate_freshness(targets: &[ProviderProgressRecord]) -> ProviderProjectionFreshness {
    if targets
        .iter()
        .any(|target| target.freshness == ProviderProjectionFreshness::Behind)
    {
        ProviderProjectionFreshness::Behind
    } else if targets
        .iter()
        .any(|target| target.freshness == ProviderProjectionFreshness::Unknown)
    {
        ProviderProjectionFreshness::Unknown
    } else {
        ProviderProjectionFreshness::Current
    }
}

#[derive(Clone, Copy)]
enum ReviewerWorkloadState {
    Missing,
    Draft,
    InReview,
    Approved,
    Applying,
    RebaseRequired,
    Blocked,
}

fn reviewer_workload_open(
    item: &job_item::Model,
) -> TranslationResult<Option<ReviewerWorkloadState>> {
    match item.status.as_str() {
        "missing" => Ok(Some(ReviewerWorkloadState::Missing)),
        "draft" => Ok(Some(ReviewerWorkloadState::Draft)),
        "in_review" => Ok(Some(ReviewerWorkloadState::InReview)),
        "approved" => Ok(Some(ReviewerWorkloadState::Approved)),
        "applying" => Ok(Some(ReviewerWorkloadState::Applying)),
        "stale" | "conflict" => Ok(Some(ReviewerWorkloadState::RebaseRequired)),
        "blocked" => Ok(Some(ReviewerWorkloadState::Blocked)),
        "applied" | "excluded" | "cancelled" => Ok(None),
        status => Err(TranslationError::InvalidProgressSource(format!(
            "unknown job item status `{status}`"
        ))),
    }
}

fn reviewer_workload_count(
    workload: &mut ReviewerWorkloadRecord,
    state: ReviewerWorkloadState,
) -> TranslationResult<()> {
    workload.open_items = checked_progress_add(workload.open_items, 1)?;
    match state {
        ReviewerWorkloadState::Missing => {
            workload.missing_items = checked_progress_add(workload.missing_items, 1)?;
        }
        ReviewerWorkloadState::Draft => {
            workload.draft_items = checked_progress_add(workload.draft_items, 1)?;
        }
        ReviewerWorkloadState::InReview => {
            workload.in_review_items = checked_progress_add(workload.in_review_items, 1)?;
        }
        ReviewerWorkloadState::Approved => {
            workload.approved_items = checked_progress_add(workload.approved_items, 1)?;
        }
        ReviewerWorkloadState::Applying => {
            workload.applying_items = checked_progress_add(workload.applying_items, 1)?;
        }
        ReviewerWorkloadState::RebaseRequired => {
            workload.rebase_required_items =
                checked_progress_add(workload.rebase_required_items, 1)?;
        }
        ReviewerWorkloadState::Blocked => {
            workload.blocked_items = checked_progress_add(workload.blocked_items, 1)?;
        }
    }
    Ok(())
}

fn reviewer_workload_key(assignee: &Option<PortActor>) -> (String, String) {
    match assignee {
        None => (String::new(), String::new()),
        Some(actor) => (actor_kind_value(&actor.kind).to_string(), actor.id.clone()),
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct ProgressCounts {
    total_items: i64,
    assigned_items: i64,
    terminal_items: i64,
    missing_items: i64,
    draft_items: i64,
    in_review_items: i64,
    approved_items: i64,
    applying_items: i64,
    applied_items: i64,
    stale_items: i64,
    conflict_items: i64,
    blocked_items: i64,
    excluded_items: i64,
    cancelled_items: i64,
    required_units: i64,
    optional_units: i64,
    applied_required_units: i64,
    applied_optional_units: i64,
    approved_required_units: i64,
    approved_optional_units: i64,
    complete_resources: i64,
    source_characters: i64,
    translated_characters: i64,
}

#[derive(Serialize)]
struct ProgressSource<'a> {
    job_id: Uuid,
    job_status: &'a str,
    job_revision: i64,
    items: Vec<ProgressItemSource<'a>>,
}

#[derive(Serialize)]
struct ProgressItemSource<'a> {
    id: Uuid,
    revision: i64,
    status: &'a str,
    source_digest: &'a str,
    current_proposal_id: Option<Uuid>,
    proposal_values_digest: Option<&'a str>,
    receipt_id: Option<Uuid>,
    receipt_target_revision: Option<&'a str>,
    receipt_applied_field_keys: Option<&'a serde_json::Value>,
    assigned_actor_kind: Option<&'a str>,
    assigned_actor_id: Option<&'a str>,
}

pub(crate) async fn refresh_job_progress<C>(
    database: &C,
    tenant_id: Uuid,
    job_id: Uuid,
) -> TranslationResult<JobProgressRecord>
where
    C: ConnectionTrait,
{
    let job_model = ensure_job(database, tenant_id, job_id).await?;
    let items = job_item::Entity::find()
        .filter(job_item::Column::TenantId.eq(tenant_id))
        .filter(job_item::Column::JobId.eq(job_id))
        .order_by_asc(job_item::Column::Id)
        .all(database)
        .await?;
    let proposal_ids = items
        .iter()
        .filter_map(|item| item.current_proposal_id)
        .collect::<Vec<_>>();
    let proposals = if proposal_ids.is_empty() {
        Vec::new()
    } else {
        proposal::Entity::find()
            .filter(proposal::Column::TenantId.eq(tenant_id))
            .filter(proposal::Column::Id.is_in(proposal_ids))
            .all(database)
            .await?
    };
    let proposals = proposals
        .into_iter()
        .map(|proposal| (proposal.id, proposal))
        .collect::<BTreeMap<_, _>>();
    let item_ids = items.iter().map(|item| item.id).collect::<Vec<_>>();
    let receipts = if item_ids.is_empty() {
        Vec::new()
    } else {
        apply_receipt::Entity::find()
            .filter(apply_receipt::Column::TenantId.eq(tenant_id))
            .filter(apply_receipt::Column::ItemId.is_in(item_ids))
            .order_by_asc(apply_receipt::Column::CreatedAt)
            .all(database)
            .await?
    };
    let receipts = receipts
        .into_iter()
        .map(|receipt| (receipt.item_id, receipt))
        .collect::<BTreeMap<_, _>>();

    let mut counts = ProgressCounts::default();
    let mut source_items = Vec::with_capacity(items.len());
    for item in &items {
        let snapshot = validated_source_snapshot(item)?;
        let current_proposal = item
            .current_proposal_id
            .and_then(|proposal_id| proposals.get(&proposal_id));
        let receipt = receipts.get(&item.id);
        count_item(&mut counts, item, &snapshot, current_proposal, receipt)?;
        source_items.push(ProgressItemSource {
            id: item.id,
            revision: item.revision,
            status: &item.status,
            source_digest: &item.source_digest,
            current_proposal_id: item.current_proposal_id,
            proposal_values_digest: current_proposal
                .map(|proposal| proposal.values_digest.as_str()),
            receipt_id: receipt.map(|receipt| receipt.id),
            receipt_target_revision: receipt.map(|receipt| receipt.target_revision.as_str()),
            receipt_applied_field_keys: receipt.map(|receipt| &receipt.applied_field_keys),
            assigned_actor_kind: item.assigned_actor_kind.as_deref(),
            assigned_actor_id: item.assigned_actor_id.as_deref(),
        });
    }
    let source_digest = rustok_api::manifest_hash::hash_manifest(&ProgressSource {
        job_id,
        job_status: &job_model.status,
        job_revision: job_model.revision,
        items: source_items,
    })?;
    persist_progress(database, tenant_id, job_id, source_digest, counts).await
}

fn count_item(
    counts: &mut ProgressCounts,
    item: &job_item::Model,
    snapshot: &TranslationResourceSnapshot,
    proposal: Option<&proposal::Model>,
    receipt: Option<&apply_receipt::Model>,
) -> TranslationResult<()> {
    add(&mut counts.total_items, 1)?;
    match (&item.assigned_actor_kind, &item.assigned_actor_id) {
        (None, None) => {}
        (Some(_), Some(_)) => add(&mut counts.assigned_items, 1)?,
        _ => {
            return Err(TranslationError::InvalidProgressSource(
                "job item assignment is incomplete".to_string(),
            ));
        }
    }
    match item.status.as_str() {
        "missing" => add(&mut counts.missing_items, 1)?,
        "draft" => add(&mut counts.draft_items, 1)?,
        "in_review" => add(&mut counts.in_review_items, 1)?,
        "approved" => add(&mut counts.approved_items, 1)?,
        "applying" => add(&mut counts.applying_items, 1)?,
        "applied" => add(&mut counts.applied_items, 1)?,
        "stale" => add(&mut counts.stale_items, 1)?,
        "conflict" => add(&mut counts.conflict_items, 1)?,
        "blocked" => add(&mut counts.blocked_items, 1)?,
        "excluded" => add(&mut counts.excluded_items, 1)?,
        "cancelled" => add(&mut counts.cancelled_items, 1)?,
        status => {
            return Err(TranslationError::InvalidProgressSource(format!(
                "unknown job item status `{status}`"
            )));
        }
    }
    if matches!(item.status.as_str(), "applied" | "excluded" | "cancelled") {
        add(&mut counts.terminal_items, 1)?;
    }

    let applied_keys = match (item.status.as_str(), receipt) {
        ("applied", Some(receipt)) => {
            serde_json::from_value::<Vec<FieldKey>>(receipt.applied_field_keys.clone())?
                .into_iter()
                .collect::<BTreeSet<_>>()
        }
        ("applied", None) => {
            return Err(TranslationError::InvalidProgressSource(
                "applied job item has no owner receipt".to_string(),
            ));
        }
        (_, _) => BTreeSet::new(),
    };
    let proposal_values = proposal
        .map(|proposal| {
            serde_json::from_value::<Vec<TranslationFieldPatch>>(proposal.values.clone())
        })
        .transpose()?
        .unwrap_or_default();
    if let Some(proposal) = proposal
        && rustok_api::manifest_hash::hash_manifest(&proposal_values)? != proposal.values_digest
    {
        return Err(TranslationError::InvalidProgressSource(
            "current proposal values digest does not match".to_string(),
        ));
    }
    if matches!(
        item.status.as_str(),
        "draft" | "in_review" | "approved" | "applying" | "applied" | "conflict" | "blocked"
    ) && proposal.is_none()
    {
        return Err(TranslationError::InvalidProgressSource(
            "workflow item state requires a current proposal".to_string(),
        ));
    }
    let proposal_by_key = proposal_values
        .iter()
        .map(|field| (&field.key, field))
        .collect::<BTreeMap<_, _>>();
    let approved_keys = if matches!(item.status.as_str(), "approved" | "applying") {
        proposal_by_key.keys().copied().collect::<BTreeSet<_>>()
    } else {
        BTreeSet::new()
    };

    let known_keys = snapshot
        .fields
        .iter()
        .map(|field| field.descriptor.key.clone())
        .collect::<BTreeSet<_>>();
    if !applied_keys.is_subset(&known_keys)
        || !approved_keys.iter().all(|key| known_keys.contains(*key))
    {
        return Err(TranslationError::InvalidProgressSource(
            "progress evidence contains an unknown field key".to_string(),
        ));
    }
    let source_characters = source_character_count(snapshot)?;
    add(
        &mut counts.source_characters,
        i64::try_from(source_characters).map_err(|_| TranslationError::ProgressOverflow)?,
    )?;
    let mut required_complete = true;
    for field in &snapshot.fields {
        let (total, applied, approved) = if field.descriptor.required {
            (
                &mut counts.required_units,
                &mut counts.applied_required_units,
                &mut counts.approved_required_units,
            )
        } else {
            (
                &mut counts.optional_units,
                &mut counts.applied_optional_units,
                &mut counts.approved_optional_units,
            )
        };
        add(total, 1)?;
        if applied_keys.contains(&field.descriptor.key) {
            add(applied, 1)?;
            let translated = proposal_by_key
                .get(&field.descriptor.key)
                .ok_or_else(|| {
                    TranslationError::InvalidProgressSource(
                        "owner receipt field is absent from the current proposal".to_string(),
                    )
                })?
                .value
                .chars()
                .count();
            add(
                &mut counts.translated_characters,
                i64::try_from(translated).map_err(|_| TranslationError::ProgressOverflow)?,
            )?;
        } else if field.descriptor.required {
            required_complete = false;
        }
        if approved_keys.contains(&field.descriptor.key) {
            add(approved, 1)?;
        }
    }
    if item.status == "applied" && required_complete {
        add(&mut counts.complete_resources, 1)?;
    }
    Ok(())
}

pub(crate) fn validated_source_snapshot(
    item: &job_item::Model,
) -> TranslationResult<TranslationResourceSnapshot> {
    let snapshot: TranslationResourceSnapshot =
        serde_json::from_value(item.source_snapshot.clone())?;
    snapshot
        .validate()
        .map_err(|error| TranslationError::InvalidProgressSource(error.to_string()))?;
    if hash_manifest(&snapshot)? != item.source_digest {
        return Err(TranslationError::InvalidProgressSource(
            "job item source snapshot digest does not match".to_string(),
        ));
    }
    Ok(snapshot)
}

fn source_character_count(snapshot: &TranslationResourceSnapshot) -> TranslationResult<u64> {
    snapshot.fields.iter().try_fold(0_u64, |total, field| {
        let count = u64::try_from(field.source_value.chars().count())
            .map_err(|_| TranslationError::ProgressOverflow)?;
        checked_progress_add(total, count)
    })
}

async fn persist_progress<C>(
    database: &C,
    tenant_id: Uuid,
    job_id: Uuid,
    source_digest: String,
    counts: ProgressCounts,
) -> TranslationResult<JobProgressRecord>
where
    C: ConnectionTrait,
{
    let existing = job_progress::Entity::find()
        .filter(job_progress::Column::TenantId.eq(tenant_id))
        .filter(job_progress::Column::JobId.eq(job_id))
        .one(database)
        .await?;
    let now = Utc::now().fixed_offset();
    let Some(existing) = existing else {
        let model = job_progress::Model {
            id: generate_id(),
            tenant_id,
            job_id,
            source_digest,
            total_items: counts.total_items,
            assigned_items: counts.assigned_items,
            terminal_items: counts.terminal_items,
            missing_items: counts.missing_items,
            draft_items: counts.draft_items,
            in_review_items: counts.in_review_items,
            approved_items: counts.approved_items,
            applying_items: counts.applying_items,
            applied_items: counts.applied_items,
            stale_items: counts.stale_items,
            conflict_items: counts.conflict_items,
            blocked_items: counts.blocked_items,
            excluded_items: counts.excluded_items,
            cancelled_items: counts.cancelled_items,
            required_units: counts.required_units,
            optional_units: counts.optional_units,
            applied_required_units: counts.applied_required_units,
            applied_optional_units: counts.applied_optional_units,
            approved_required_units: counts.approved_required_units,
            approved_optional_units: counts.approved_optional_units,
            complete_resources: counts.complete_resources,
            source_characters: counts.source_characters,
            translated_characters: counts.translated_characters,
            revision: 0,
            updated_at: now,
        };
        job_progress::Entity::insert(job_progress::ActiveModel {
            id: Set(model.id),
            tenant_id: Set(model.tenant_id),
            job_id: Set(model.job_id),
            source_digest: Set(model.source_digest.clone()),
            total_items: Set(model.total_items),
            assigned_items: Set(model.assigned_items),
            terminal_items: Set(model.terminal_items),
            missing_items: Set(model.missing_items),
            draft_items: Set(model.draft_items),
            in_review_items: Set(model.in_review_items),
            approved_items: Set(model.approved_items),
            applying_items: Set(model.applying_items),
            applied_items: Set(model.applied_items),
            stale_items: Set(model.stale_items),
            conflict_items: Set(model.conflict_items),
            blocked_items: Set(model.blocked_items),
            excluded_items: Set(model.excluded_items),
            cancelled_items: Set(model.cancelled_items),
            required_units: Set(model.required_units),
            optional_units: Set(model.optional_units),
            applied_required_units: Set(model.applied_required_units),
            applied_optional_units: Set(model.applied_optional_units),
            approved_required_units: Set(model.approved_required_units),
            approved_optional_units: Set(model.approved_optional_units),
            complete_resources: Set(model.complete_resources),
            source_characters: Set(model.source_characters),
            translated_characters: Set(model.translated_characters),
            revision: Set(model.revision),
            updated_at: Set(model.updated_at),
        })
        .on_conflict(
            OnConflict::columns([job_progress::Column::TenantId, job_progress::Column::JobId])
                .do_nothing()
                .to_owned(),
        )
        .exec_without_returning(database)
        .await?;
        let persisted = job_progress::Entity::find()
            .filter(job_progress::Column::TenantId.eq(tenant_id))
            .filter(job_progress::Column::JobId.eq(job_id))
            .one(database)
            .await?
            .ok_or(TranslationError::ProgressRevisionConflict)?;
        if persisted.id != model.id
            && !projection_matches(&persisted, &model.source_digest, &counts)
        {
            return Err(TranslationError::ProgressRevisionConflict);
        }
        return progress_record(persisted);
    };

    if projection_matches(&existing, &source_digest, &counts) {
        return progress_record(existing);
    }
    let revision = existing
        .revision
        .checked_add(1)
        .ok_or(TranslationError::ProgressOverflow)?;
    let update = job_progress::Entity::update_many()
        .col_expr(
            job_progress::Column::SourceDigest,
            Expr::value(source_digest),
        )
        .col_expr(
            job_progress::Column::TotalItems,
            Expr::value(counts.total_items),
        )
        .col_expr(
            job_progress::Column::AssignedItems,
            Expr::value(counts.assigned_items),
        )
        .col_expr(
            job_progress::Column::TerminalItems,
            Expr::value(counts.terminal_items),
        )
        .col_expr(
            job_progress::Column::MissingItems,
            Expr::value(counts.missing_items),
        )
        .col_expr(
            job_progress::Column::DraftItems,
            Expr::value(counts.draft_items),
        )
        .col_expr(
            job_progress::Column::InReviewItems,
            Expr::value(counts.in_review_items),
        )
        .col_expr(
            job_progress::Column::ApprovedItems,
            Expr::value(counts.approved_items),
        )
        .col_expr(
            job_progress::Column::ApplyingItems,
            Expr::value(counts.applying_items),
        )
        .col_expr(
            job_progress::Column::AppliedItems,
            Expr::value(counts.applied_items),
        )
        .col_expr(
            job_progress::Column::StaleItems,
            Expr::value(counts.stale_items),
        )
        .col_expr(
            job_progress::Column::ConflictItems,
            Expr::value(counts.conflict_items),
        )
        .col_expr(
            job_progress::Column::BlockedItems,
            Expr::value(counts.blocked_items),
        )
        .col_expr(
            job_progress::Column::ExcludedItems,
            Expr::value(counts.excluded_items),
        )
        .col_expr(
            job_progress::Column::CancelledItems,
            Expr::value(counts.cancelled_items),
        )
        .col_expr(
            job_progress::Column::RequiredUnits,
            Expr::value(counts.required_units),
        )
        .col_expr(
            job_progress::Column::OptionalUnits,
            Expr::value(counts.optional_units),
        )
        .col_expr(
            job_progress::Column::AppliedRequiredUnits,
            Expr::value(counts.applied_required_units),
        )
        .col_expr(
            job_progress::Column::AppliedOptionalUnits,
            Expr::value(counts.applied_optional_units),
        )
        .col_expr(
            job_progress::Column::ApprovedRequiredUnits,
            Expr::value(counts.approved_required_units),
        )
        .col_expr(
            job_progress::Column::ApprovedOptionalUnits,
            Expr::value(counts.approved_optional_units),
        )
        .col_expr(
            job_progress::Column::CompleteResources,
            Expr::value(counts.complete_resources),
        )
        .col_expr(
            job_progress::Column::SourceCharacters,
            Expr::value(counts.source_characters),
        )
        .col_expr(
            job_progress::Column::TranslatedCharacters,
            Expr::value(counts.translated_characters),
        )
        .col_expr(job_progress::Column::Revision, Expr::value(revision))
        .col_expr(job_progress::Column::UpdatedAt, Expr::value(now))
        .filter(job_progress::Column::Id.eq(existing.id))
        .filter(job_progress::Column::TenantId.eq(tenant_id))
        .filter(job_progress::Column::Revision.eq(existing.revision))
        .exec(database)
        .await?;
    if update.rows_affected != 1 {
        return Err(TranslationError::ProgressRevisionConflict);
    }
    let persisted = job_progress::Entity::find_by_id(existing.id)
        .filter(job_progress::Column::TenantId.eq(tenant_id))
        .one(database)
        .await?
        .ok_or(TranslationError::JobProgressNotFound)?;
    progress_record(persisted)
}

fn projection_matches(
    model: &job_progress::Model,
    source_digest: &str,
    counts: &ProgressCounts,
) -> bool {
    model.source_digest == source_digest
        && model.total_items == counts.total_items
        && model.assigned_items == counts.assigned_items
        && model.terminal_items == counts.terminal_items
        && model.missing_items == counts.missing_items
        && model.draft_items == counts.draft_items
        && model.in_review_items == counts.in_review_items
        && model.approved_items == counts.approved_items
        && model.applying_items == counts.applying_items
        && model.applied_items == counts.applied_items
        && model.stale_items == counts.stale_items
        && model.conflict_items == counts.conflict_items
        && model.blocked_items == counts.blocked_items
        && model.excluded_items == counts.excluded_items
        && model.cancelled_items == counts.cancelled_items
        && model.required_units == counts.required_units
        && model.optional_units == counts.optional_units
        && model.applied_required_units == counts.applied_required_units
        && model.applied_optional_units == counts.applied_optional_units
        && model.approved_required_units == counts.approved_required_units
        && model.approved_optional_units == counts.approved_optional_units
        && model.complete_resources == counts.complete_resources
        && model.source_characters == counts.source_characters
        && model.translated_characters == counts.translated_characters
}

fn progress_record(model: job_progress::Model) -> TranslationResult<JobProgressRecord> {
    Ok(JobProgressRecord {
        job_id: model.job_id,
        source_digest: model.source_digest,
        total_items: unsigned(model.total_items)?,
        assigned_items: unsigned(model.assigned_items)?,
        terminal_items: unsigned(model.terminal_items)?,
        missing_items: unsigned(model.missing_items)?,
        draft_items: unsigned(model.draft_items)?,
        in_review_items: unsigned(model.in_review_items)?,
        approved_items: unsigned(model.approved_items)?,
        applying_items: unsigned(model.applying_items)?,
        applied_items: unsigned(model.applied_items)?,
        stale_items: unsigned(model.stale_items)?,
        conflict_items: unsigned(model.conflict_items)?,
        blocked_items: unsigned(model.blocked_items)?,
        excluded_items: unsigned(model.excluded_items)?,
        cancelled_items: unsigned(model.cancelled_items)?,
        required_units: unsigned(model.required_units)?,
        optional_units: unsigned(model.optional_units)?,
        applied_required_units: unsigned(model.applied_required_units)?,
        applied_optional_units: unsigned(model.applied_optional_units)?,
        approved_required_units: unsigned(model.approved_required_units)?,
        approved_optional_units: unsigned(model.approved_optional_units)?,
        complete_resources: unsigned(model.complete_resources)?,
        source_characters: unsigned(model.source_characters)?,
        translated_characters: unsigned(model.translated_characters)?,
        revision: model.revision,
        updated_at: model.updated_at,
    })
}

async fn ensure_job<C>(database: &C, tenant_id: Uuid, job_id: Uuid) -> TranslationResult<job::Model>
where
    C: ConnectionTrait,
{
    job::Entity::find_by_id(job_id)
        .filter(job::Column::TenantId.eq(tenant_id))
        .one(database)
        .await?
        .ok_or(TranslationError::JobNotFound)
}

fn authorize_progress(
    context: &PortContext,
    policy: PortCallPolicy,
    action: Action,
) -> TranslationResult<Uuid> {
    context.require_policy(policy)?;
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::Translations, action) == PermissionScope::None {
        return Err(TranslationError::Forbidden);
    }
    Uuid::parse_str(&context.tenant_id).map_err(|_| TranslationError::InvalidTenantId)
}

fn add(value: &mut i64, increment: i64) -> TranslationResult<()> {
    *value = value
        .checked_add(increment)
        .ok_or(TranslationError::ProgressOverflow)?;
    Ok(())
}

fn unsigned(value: i64) -> TranslationResult<u64> {
    u64::try_from(value).map_err(|_| {
        TranslationError::InvalidProgressSource("persisted progress count is negative".to_string())
    })
}
