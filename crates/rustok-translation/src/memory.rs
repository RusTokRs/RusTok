use std::collections::BTreeSet;

use chrono::{DateTime, FixedOffset, Utc};
use rustok_api::{
    Action, PortActorKind, PortCallPolicy, PortContext, Resource, TenantLocale,
    manifest_hash::hash_manifest,
};
use rustok_core::{PermissionScope, RetentionPolicy, SecurityContext, generate_id};
use rustok_translation_targets::{
    FieldKey, TranslationDataClassification, TranslationResourceIdentity,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder,
    QuerySelect, Set, TransactionTrait,
    sea_query::{Expr, OnConflict},
};
use serde::{Deserialize, Serialize};
use unicode_normalization::UnicodeNormalization;
use uuid::Uuid;

use crate::{
    TranslationError, TranslationResult,
    entities::{machine_memory_binding, memory_entry, memory_receipt},
    observability,
};

const MAX_LOOKUP_LIMIT: u16 = 50;
const MAX_LIST_LIMIT: u16 = 200;
const MAX_CANDIDATES: u64 = 500;
const MAX_SEGMENT_BYTES: usize = 32 * 1024;
const SEGMENTATION_VERSION: &str = "owner-field-v1";
const QUALITY_STATE: &str = "human_approved_applied";
const DEFAULT_RETENTION_POLICY: &str = "owner_lifecycle";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryLookupInput {
    pub source_locale: TenantLocale,
    pub target_locale: TenantLocale,
    pub identity: TranslationResourceIdentity,
    pub field_key: FieldKey,
    pub source_text: String,
    pub minimum_similarity_basis_points: u16,
    pub limit: u16,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryMatchKind {
    Exact,
    ContextualFuzzy,
    Fuzzy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryMatchEvidence {
    pub kind: MemoryMatchKind,
    pub source_exact: bool,
    pub context_match: bool,
    pub base_similarity_basis_points: u16,
    pub context_bonus_basis_points: u16,
    pub final_similarity_basis_points: u16,
    pub segmentation_version: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemorySuggestion {
    pub entry_id: Uuid,
    pub source_text: String,
    pub target_text: String,
    pub source_hash: String,
    pub owner_slug: String,
    pub resource_kind: String,
    pub resource_id: String,
    pub field_key: String,
    pub origin: String,
    pub proposal_id: Uuid,
    pub apply_receipt_id: Uuid,
    pub evidence: MemoryMatchEvidence,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryListInput {
    pub source_locale: Option<TenantLocale>,
    pub target_locale: Option<TenantLocale>,
    pub include_tombstoned: bool,
    pub limit: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetMemoryRetentionInput {
    pub entry_id: Uuid,
    pub expected_revision: i64,
    pub policy: RetentionPolicy,
    pub retain_until: Option<DateTime<FixedOffset>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TombstoneMemoryEntryInput {
    pub entry_id: Uuid,
    pub expected_revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PurgeMemoryEntryInput {
    pub entry_id: Uuid,
    pub expected_revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryEntryRecord {
    pub id: Uuid,
    pub tenant_id: Uuid,
    pub source_locale: String,
    pub target_locale: String,
    pub owner_slug: String,
    pub resource_kind: String,
    pub resource_id: String,
    pub subresource_id: Option<String>,
    pub field_key: String,
    pub source_text: String,
    pub target_text: String,
    pub source_hash: String,
    pub target_hash: String,
    pub context_fingerprint: String,
    pub segmentation_version: String,
    pub origin: String,
    pub quality_state: String,
    pub reviewer_actor_kind: String,
    pub reviewer_actor_id: String,
    pub proposal_id: Uuid,
    pub apply_receipt_id: Uuid,
    pub retention_policy: RetentionPolicy,
    pub retain_until: Option<DateTime<FixedOffset>>,
    pub tombstoned_at: Option<DateTime<FixedOffset>>,
    pub revision: i64,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MemoryMutationRecord {
    pub entry_id: Uuid,
    pub revision: i64,
    pub state: String,
    pub retention_policy: RetentionPolicy,
    pub retain_until: Option<DateTime<FixedOffset>>,
    pub tombstoned_at: Option<DateTime<FixedOffset>>,
}

pub struct TranslationMemoryService {
    database: DatabaseConnection,
}

impl TranslationMemoryService {
    pub fn new(database: DatabaseConnection) -> Self {
        Self { database }
    }

    pub async fn list_entries(
        &self,
        context: PortContext,
        input: MemoryListInput,
    ) -> TranslationResult<Vec<MemoryEntryRecord>> {
        let tenant_id = authorize(&context, &[Action::List], PortCallPolicy::read())?;
        if input.limit == 0 || input.limit > MAX_LIST_LIMIT {
            return Err(TranslationError::InvalidRequest(format!(
                "translation memory list limit must be between 1 and {MAX_LIST_LIMIT}"
            )));
        }
        let mut query =
            memory_entry::Entity::find().filter(memory_entry::Column::TenantId.eq(tenant_id));
        if let Some(source_locale) = input.source_locale {
            query = query.filter(memory_entry::Column::SourceLocale.eq(source_locale.as_str()));
        }
        if let Some(target_locale) = input.target_locale {
            query = query.filter(memory_entry::Column::TargetLocale.eq(target_locale.as_str()));
        }
        if !input.include_tombstoned {
            query = query.filter(memory_entry::Column::TombstonedAt.is_null());
        }
        query
            .order_by_desc(memory_entry::Column::UpdatedAt)
            .order_by_asc(memory_entry::Column::Id)
            .limit(u64::from(input.limit))
            .all(&self.database)
            .await?
            .into_iter()
            .map(entry_record)
            .collect()
    }

    pub async fn read_entry(
        &self,
        context: PortContext,
        entry_id: Uuid,
    ) -> TranslationResult<MemoryEntryRecord> {
        let tenant_id = authorize(&context, &[Action::Read], PortCallPolicy::read())?;
        find_entry(&self.database, tenant_id, entry_id)
            .await
            .and_then(entry_record)
    }

    pub async fn lookup(
        &self,
        context: PortContext,
        input: MemoryLookupInput,
    ) -> TranslationResult<Vec<MemorySuggestion>> {
        let tenant_id = authorize(&context, &[Action::Read], PortCallPolicy::read())?;
        self.lookup_for_authorized_tenant(tenant_id, input).await
    }

    pub(crate) async fn lookup_for_machine(
        &self,
        tenant_id: Uuid,
        input: MemoryLookupInput,
    ) -> TranslationResult<Vec<MemorySuggestion>> {
        self.lookup_for_authorized_tenant(tenant_id, input).await
    }

    async fn lookup_for_authorized_tenant(
        &self,
        tenant_id: Uuid,
        input: MemoryLookupInput,
    ) -> TranslationResult<Vec<MemorySuggestion>> {
        validate_lookup(&input)?;
        let normalized_source = normalize_segment(&input.source_text);
        let requested_context = context_fingerprint(
            input.identity.owner_slug.as_str(),
            input.identity.resource_kind.as_str(),
            input.field_key.as_str(),
        )?;
        let models = memory_entry::Entity::find()
            .filter(memory_entry::Column::TenantId.eq(tenant_id))
            .filter(memory_entry::Column::SourceLocale.eq(input.source_locale.as_str()))
            .filter(memory_entry::Column::TargetLocale.eq(input.target_locale.as_str()))
            .filter(memory_entry::Column::TombstonedAt.is_null())
            .order_by_desc(memory_entry::Column::CreatedAt)
            .order_by_asc(memory_entry::Column::Id)
            .limit(MAX_CANDIDATES)
            .all(&self.database)
            .await?;

        let mut suggestions = models
            .into_iter()
            .filter_map(|model| {
                suggestion(
                    model,
                    &normalized_source,
                    &requested_context,
                    input.minimum_similarity_basis_points,
                )
            })
            .collect::<Vec<_>>();
        suggestions.sort_by(|left, right| {
            right
                .evidence
                .final_similarity_basis_points
                .cmp(&left.evidence.final_similarity_basis_points)
                .then_with(|| right.evidence.source_exact.cmp(&left.evidence.source_exact))
                .then_with(|| {
                    right
                        .evidence
                        .context_match
                        .cmp(&left.evidence.context_match)
                })
                .then_with(|| left.entry_id.cmp(&right.entry_id))
        });
        suggestions.truncate(usize::from(input.limit));
        observability::record_memory_lookup(&suggestions);
        Ok(suggestions)
    }

    pub async fn set_retention(
        &self,
        context: PortContext,
        input: SetMemoryRetentionInput,
    ) -> TranslationResult<MemoryMutationRecord> {
        let tenant_id = authorize(&context, &[Action::Manage], PortCallPolicy::write())?;
        validate_expected_revision(input.expected_revision)?;
        validate_retention(input.policy, input.retain_until.as_ref())?;
        let request_hash = hash_manifest(&input)?;
        if let Some(receipt) =
            find_receipt(&self.database, tenant_id, idempotency_key(&context)).await?
        {
            return replay_receipt(receipt, &context, "set_retention", &request_hash);
        }
        let current = find_entry(&self.database, tenant_id, input.entry_id).await?;
        ensure_revision(&current, input.expected_revision)?;
        if current.tombstoned_at.is_some() {
            return Err(TranslationError::MemoryLifecycleConflict(
                "retention cannot change after tombstone".to_string(),
            ));
        }
        let revision = next_revision(current.revision)?;
        let now = Utc::now().fixed_offset();
        let transaction = self.database.begin().await?;
        let update = memory_entry::Entity::update_many()
            .col_expr(
                memory_entry::Column::RetentionPolicy,
                Expr::value(input.policy.as_str()),
            )
            .col_expr(
                memory_entry::Column::RetainUntil,
                Expr::value(input.retain_until),
            )
            .col_expr(memory_entry::Column::Revision, Expr::value(revision))
            .col_expr(memory_entry::Column::UpdatedAt, Expr::value(now))
            .filter(memory_entry::Column::TenantId.eq(tenant_id))
            .filter(memory_entry::Column::Id.eq(input.entry_id))
            .filter(memory_entry::Column::Revision.eq(input.expected_revision))
            .filter(memory_entry::Column::TombstonedAt.is_null())
            .exec(&transaction)
            .await?;
        if update.rows_affected != 1 {
            return Err(TranslationError::MemoryRevisionConflict {
                expected: input.expected_revision,
                actual: current.revision,
            });
        }
        let result = mutation_record(find_entry(&transaction, tenant_id, input.entry_id).await?)?;
        if let Some(replay) = insert_receipt(
            &transaction,
            tenant_id,
            &context,
            "set_retention",
            &request_hash,
            &result,
        )
        .await?
        {
            transaction.rollback().await?;
            return Ok(replay);
        }
        transaction.commit().await?;
        Ok(result)
    }

    pub async fn tombstone_entry(
        &self,
        context: PortContext,
        input: TombstoneMemoryEntryInput,
    ) -> TranslationResult<MemoryMutationRecord> {
        let tenant_id = authorize(&context, &[Action::Delete], PortCallPolicy::write())?;
        validate_expected_revision(input.expected_revision)?;
        let request_hash = hash_manifest(&input)?;
        if let Some(receipt) =
            find_receipt(&self.database, tenant_id, idempotency_key(&context)).await?
        {
            return replay_receipt(receipt, &context, "tombstone", &request_hash);
        }
        let current = find_entry(&self.database, tenant_id, input.entry_id).await?;
        ensure_revision(&current, input.expected_revision)?;
        if current.retention_policy == "legal_hold" {
            return Err(TranslationError::MemoryRetentionConflict(
                "legal-hold entries cannot be tombstoned".to_string(),
            ));
        }
        if current.tombstoned_at.is_some() {
            return Err(TranslationError::MemoryLifecycleConflict(
                "memory entry is already tombstoned".to_string(),
            ));
        }
        let revision = next_revision(current.revision)?;
        let now = Utc::now().fixed_offset();
        let transaction = self.database.begin().await?;
        let update = memory_entry::Entity::update_many()
            .col_expr(memory_entry::Column::TombstonedAt, Expr::value(Some(now)))
            .col_expr(memory_entry::Column::Revision, Expr::value(revision))
            .col_expr(memory_entry::Column::UpdatedAt, Expr::value(now))
            .filter(memory_entry::Column::TenantId.eq(tenant_id))
            .filter(memory_entry::Column::Id.eq(input.entry_id))
            .filter(memory_entry::Column::Revision.eq(input.expected_revision))
            .filter(memory_entry::Column::TombstonedAt.is_null())
            .exec(&transaction)
            .await?;
        if update.rows_affected != 1 {
            return Err(TranslationError::MemoryRevisionConflict {
                expected: input.expected_revision,
                actual: current.revision,
            });
        }
        let result = mutation_record(find_entry(&transaction, tenant_id, input.entry_id).await?)?;
        if let Some(replay) = insert_receipt(
            &transaction,
            tenant_id,
            &context,
            "tombstone",
            &request_hash,
            &result,
        )
        .await?
        {
            transaction.rollback().await?;
            return Ok(replay);
        }
        transaction.commit().await?;
        Ok(result)
    }

    pub async fn purge_entry(
        &self,
        context: PortContext,
        input: PurgeMemoryEntryInput,
    ) -> TranslationResult<MemoryMutationRecord> {
        let tenant_id = authorize(
            &context,
            &[Action::Delete, Action::Manage],
            PortCallPolicy::write(),
        )?;
        validate_expected_revision(input.expected_revision)?;
        let request_hash = hash_manifest(&input)?;
        if let Some(receipt) =
            find_receipt(&self.database, tenant_id, idempotency_key(&context)).await?
        {
            return replay_receipt(receipt, &context, "purge", &request_hash);
        }
        let current = find_entry(&self.database, tenant_id, input.entry_id).await?;
        ensure_revision(&current, input.expected_revision)?;
        if current.retention_policy == "legal_hold" {
            return Err(TranslationError::MemoryRetentionConflict(
                "legal-hold entries cannot be purged".to_string(),
            ));
        }
        if current.tombstoned_at.is_none() {
            return Err(TranslationError::MemoryLifecycleConflict(
                "memory entry must be tombstoned before purge".to_string(),
            ));
        }
        if current
            .retain_until
            .as_ref()
            .is_some_and(|retain_until| *retain_until > Utc::now().fixed_offset())
        {
            return Err(TranslationError::MemoryRetentionConflict(
                "memory entry retention window has not elapsed".to_string(),
            ));
        }
        if machine_memory_binding::Entity::find()
            .filter(machine_memory_binding::Column::TenantId.eq(tenant_id))
            .filter(machine_memory_binding::Column::MemoryEntryId.eq(input.entry_id))
            .one(&self.database)
            .await?
            .is_some()
        {
            return Err(TranslationError::MemoryRetentionConflict(
                "memory entry is pinned by a registered machine translation operation".to_string(),
            ));
        }
        let revision = next_revision(current.revision)?;
        let result = MemoryMutationRecord {
            entry_id: current.id,
            revision,
            state: "purged".to_string(),
            retention_policy: parse_retention_policy(&current.retention_policy)?,
            retain_until: current.retain_until,
            tombstoned_at: current.tombstoned_at,
        };
        let transaction = self.database.begin().await?;
        if let Some(replay) = insert_receipt(
            &transaction,
            tenant_id,
            &context,
            "purge",
            &request_hash,
            &result,
        )
        .await?
        {
            transaction.rollback().await?;
            return Ok(replay);
        }
        let delete = memory_entry::Entity::delete_many()
            .filter(memory_entry::Column::TenantId.eq(tenant_id))
            .filter(memory_entry::Column::Id.eq(input.entry_id))
            .filter(memory_entry::Column::Revision.eq(input.expected_revision))
            .filter(memory_entry::Column::TombstonedAt.is_not_null())
            .exec(&transaction)
            .await?;
        if delete.rows_affected != 1 {
            return Err(TranslationError::MemoryRevisionConflict {
                expected: input.expected_revision,
                actual: current.revision,
            });
        }
        transaction.commit().await?;
        Ok(result)
    }
}

pub(crate) struct AppliedMemorySegment {
    pub source_locale: TenantLocale,
    pub target_locale: TenantLocale,
    pub identity: TranslationResourceIdentity,
    pub field_key: FieldKey,
    pub classification: TranslationDataClassification,
    pub source_text: String,
    pub target_text: String,
    pub source_hash: String,
    pub origin: String,
    pub reviewer_actor_kind: String,
    pub reviewer_actor_id: String,
    pub proposal_id: Uuid,
    pub apply_receipt_id: Uuid,
}

pub(crate) async fn ingest_applied_segments<C>(
    database: &C,
    tenant_id: Uuid,
    created_at: DateTime<FixedOffset>,
    segments: Vec<AppliedMemorySegment>,
) -> TranslationResult<()>
where
    C: ConnectionTrait,
{
    let mut models = Vec::new();
    for segment in segments.into_iter().filter(memory_eligible) {
        let source_text = segment.source_text;
        let target_text = segment.target_text;
        if source_text.is_empty()
            || target_text.is_empty()
            || source_text.trim().is_empty()
            || target_text.trim().is_empty()
            || source_text.len() > MAX_SEGMENT_BYTES
            || target_text.len() > MAX_SEGMENT_BYTES
            || segment.source_locale.as_str() == "und"
            || segment.target_locale.as_str() == "und"
        {
            continue;
        }
        let normalized_source = normalize_segment(&source_text);
        if normalized_source.is_empty() {
            continue;
        }
        models.push(memory_entry::ActiveModel {
            id: Set(generate_id()),
            tenant_id: Set(tenant_id),
            source_locale: Set(segment.source_locale.as_str().to_string()),
            target_locale: Set(segment.target_locale.as_str().to_string()),
            owner_slug: Set(segment.identity.owner_slug.as_str().to_string()),
            resource_kind: Set(segment.identity.resource_kind.as_str().to_string()),
            resource_id: Set(segment.identity.resource_id.as_str().to_string()),
            subresource_id: Set(segment
                .identity
                .subresource_id
                .as_ref()
                .map(|value| value.as_str().to_string())),
            field_key: Set(segment.field_key.as_str().to_string()),
            source_text: Set(source_text),
            target_text: Set(target_text.clone()),
            source_key: Set(hash_manifest(&normalized_source)?),
            source_hash: Set(segment.source_hash),
            target_hash: Set(hash_manifest(&target_text)?),
            context_fingerprint: Set(context_fingerprint(
                segment.identity.owner_slug.as_str(),
                segment.identity.resource_kind.as_str(),
                segment.field_key.as_str(),
            )?),
            segmentation_version: Set(SEGMENTATION_VERSION.to_string()),
            origin: Set(segment.origin),
            quality_state: Set(QUALITY_STATE.to_string()),
            reviewer_actor_kind: Set(segment.reviewer_actor_kind),
            reviewer_actor_id: Set(segment.reviewer_actor_id),
            proposal_id: Set(segment.proposal_id),
            apply_receipt_id: Set(segment.apply_receipt_id),
            retention_policy: Set(DEFAULT_RETENTION_POLICY.to_string()),
            retain_until: Set(None),
            owner_lifecycle_revision: Set(None),
            owner_deleted_at: Set(None),
            tombstoned_at: Set(None),
            revision: Set(1),
            created_at: Set(created_at),
            updated_at: Set(created_at),
        });
    }
    if models.is_empty() {
        return Ok(());
    }
    memory_entry::Entity::insert_many(models)
        .on_conflict(
            OnConflict::columns([
                memory_entry::Column::TenantId,
                memory_entry::Column::ProposalId,
                memory_entry::Column::FieldKey,
            ])
            .do_nothing()
            .to_owned(),
        )
        .exec_without_returning(database)
        .await?;
    Ok(())
}

pub(crate) async fn record_owner_deletion<C>(
    database: &C,
    tenant_id: Uuid,
    identity: &TranslationResourceIdentity,
    resource_revision: &str,
    observed_at: DateTime<FixedOffset>,
) -> TranslationResult<u64>
where
    C: ConnectionTrait,
{
    if resource_revision.trim().is_empty() || resource_revision.len() > 256 {
        return Err(TranslationError::MemoryInvariant(
            "owner deletion revision is invalid".to_string(),
        ));
    }
    let mut update = memory_entry::Entity::update_many()
        .col_expr(
            memory_entry::Column::OwnerLifecycleRevision,
            Expr::value(Some(resource_revision.to_string())),
        )
        .col_expr(
            memory_entry::Column::OwnerDeletedAt,
            Expr::value(Some(observed_at)),
        )
        .col_expr(
            memory_entry::Column::Revision,
            sea_orm::sea_query::ExprTrait::add(Expr::col(memory_entry::Column::Revision), 1),
        )
        .col_expr(memory_entry::Column::UpdatedAt, Expr::value(observed_at))
        .filter(memory_entry::Column::TenantId.eq(tenant_id))
        .filter(memory_entry::Column::OwnerSlug.eq(identity.owner_slug.as_str()))
        .filter(memory_entry::Column::ResourceKind.eq(identity.resource_kind.as_str()))
        .filter(memory_entry::Column::ResourceId.eq(identity.resource_id.as_str()))
        .filter(memory_entry::Column::OwnerDeletedAt.is_null());
    update = match identity.subresource_id.as_ref() {
        Some(subresource_id) => {
            update.filter(memory_entry::Column::SubresourceId.eq(subresource_id.as_str()))
        }
        None => update.filter(memory_entry::Column::SubresourceId.is_null()),
    };
    Ok(update.exec(database).await?.rows_affected)
}

async fn find_entry<C>(
    database: &C,
    tenant_id: Uuid,
    entry_id: Uuid,
) -> TranslationResult<memory_entry::Model>
where
    C: ConnectionTrait,
{
    memory_entry::Entity::find_by_id(entry_id)
        .filter(memory_entry::Column::TenantId.eq(tenant_id))
        .one(database)
        .await?
        .ok_or(TranslationError::MemoryEntryNotFound)
}

fn entry_record(model: memory_entry::Model) -> TranslationResult<MemoryEntryRecord> {
    Ok(MemoryEntryRecord {
        id: model.id,
        tenant_id: model.tenant_id,
        source_locale: model.source_locale,
        target_locale: model.target_locale,
        owner_slug: model.owner_slug,
        resource_kind: model.resource_kind,
        resource_id: model.resource_id,
        subresource_id: model.subresource_id,
        field_key: model.field_key,
        source_text: model.source_text,
        target_text: model.target_text,
        source_hash: model.source_hash,
        target_hash: model.target_hash,
        context_fingerprint: model.context_fingerprint,
        segmentation_version: model.segmentation_version,
        origin: model.origin,
        quality_state: model.quality_state,
        reviewer_actor_kind: model.reviewer_actor_kind,
        reviewer_actor_id: model.reviewer_actor_id,
        proposal_id: model.proposal_id,
        apply_receipt_id: model.apply_receipt_id,
        retention_policy: parse_retention_policy(&model.retention_policy)?,
        retain_until: model.retain_until,
        tombstoned_at: model.tombstoned_at,
        revision: model.revision,
        created_at: model.created_at,
        updated_at: model.updated_at,
    })
}

fn mutation_record(model: memory_entry::Model) -> TranslationResult<MemoryMutationRecord> {
    Ok(MemoryMutationRecord {
        entry_id: model.id,
        revision: model.revision,
        state: if model.tombstoned_at.is_some() {
            "tombstoned".to_string()
        } else {
            "active".to_string()
        },
        retention_policy: parse_retention_policy(&model.retention_policy)?,
        retain_until: model.retain_until,
        tombstoned_at: model.tombstoned_at,
    })
}

fn parse_retention_policy(value: &str) -> TranslationResult<RetentionPolicy> {
    value.parse().map_err(|_| {
        TranslationError::MemoryInvariant(format!("unknown retention policy `{value}`"))
    })
}

fn validate_retention(
    policy: RetentionPolicy,
    retain_until: Option<&DateTime<FixedOffset>>,
) -> TranslationResult<()> {
    policy
        .validate(retain_until, Utc::now().fixed_offset())
        .map_err(|error| TranslationError::MemoryRetentionConflict(error.to_string()))
}

fn validate_expected_revision(revision: i64) -> TranslationResult<()> {
    if revision < 1 {
        return Err(TranslationError::MemoryRevisionConflict {
            expected: revision,
            actual: 1,
        });
    }
    Ok(())
}

fn ensure_revision(model: &memory_entry::Model, expected: i64) -> TranslationResult<()> {
    if model.revision != expected {
        return Err(TranslationError::MemoryRevisionConflict {
            expected,
            actual: model.revision,
        });
    }
    Ok(())
}

fn next_revision(revision: i64) -> TranslationResult<i64> {
    revision
        .checked_add(1)
        .ok_or_else(|| TranslationError::MemoryInvariant("revision overflow".to_string()))
}

async fn find_receipt<C>(
    database: &C,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> TranslationResult<Option<memory_receipt::Model>>
where
    C: ConnectionTrait,
{
    Ok(memory_receipt::Entity::find()
        .filter(memory_receipt::Column::TenantId.eq(tenant_id))
        .filter(memory_receipt::Column::IdempotencyKey.eq(idempotency_key))
        .one(database)
        .await?)
}

async fn insert_receipt<C>(
    database: &C,
    tenant_id: Uuid,
    context: &PortContext,
    operation: &str,
    request_hash: &str,
    response: &MemoryMutationRecord,
) -> TranslationResult<Option<MemoryMutationRecord>>
where
    C: ConnectionTrait,
{
    let receipt_id = generate_id();
    memory_receipt::Entity::insert(memory_receipt::ActiveModel {
        id: Set(receipt_id),
        tenant_id: Set(tenant_id),
        entry_id: Set(response.entry_id),
        operation: Set(operation.to_string()),
        idempotency_key: Set(idempotency_key(context).to_string()),
        request_hash: Set(request_hash.to_string()),
        requested_by_actor_kind: Set(actor_kind(context).to_string()),
        requested_by_actor_id: Set(context.actor.id.clone()),
        resulting_entry_revision: Set(response.revision),
        response: Set(serde_json::to_value(response)?),
        created_at: Set(Utc::now().fixed_offset()),
    })
    .on_conflict(
        OnConflict::columns([
            memory_receipt::Column::TenantId,
            memory_receipt::Column::IdempotencyKey,
        ])
        .do_nothing()
        .to_owned(),
    )
    .exec_without_returning(database)
    .await?;
    let receipt = find_receipt(database, tenant_id, idempotency_key(context))
        .await?
        .ok_or_else(|| {
            TranslationError::MemoryInvariant("memory receipt did not persist".to_string())
        })?;
    if receipt.id == receipt_id {
        Ok(None)
    } else {
        replay_receipt(receipt, context, operation, request_hash).map(Some)
    }
}

fn replay_receipt(
    receipt: memory_receipt::Model,
    context: &PortContext,
    operation: &str,
    request_hash: &str,
) -> TranslationResult<MemoryMutationRecord> {
    if receipt.requested_by_actor_kind != actor_kind(context)
        || receipt.requested_by_actor_id != context.actor.id
    {
        return Err(TranslationError::IdempotencyActorMismatch);
    }
    if receipt.operation != operation || receipt.request_hash != request_hash {
        return Err(TranslationError::IdempotencyConflict);
    }
    serde_json::from_value(receipt.response).map_err(Into::into)
}

fn validate_lookup(input: &MemoryLookupInput) -> TranslationResult<()> {
    if input.source_locale == input.target_locale {
        return Err(TranslationError::InvalidRequest(
            "translation memory locale pair must differ".to_string(),
        ));
    }
    if input.source_locale.as_str() == "und" || input.target_locale.as_str() == "und" {
        return Err(TranslationError::InvalidRequest(
            "translation memory lookup does not accept the und locale".to_string(),
        ));
    }
    if input.limit == 0 || input.limit > MAX_LOOKUP_LIMIT {
        return Err(TranslationError::InvalidRequest(format!(
            "translation memory lookup limit must be between 1 and {MAX_LOOKUP_LIMIT}"
        )));
    }
    if input.minimum_similarity_basis_points > 10_000 {
        return Err(TranslationError::InvalidRequest(
            "translation memory similarity must not exceed 10000 basis points".to_string(),
        ));
    }
    if input.source_text.trim().is_empty() || input.source_text.len() > MAX_SEGMENT_BYTES {
        return Err(TranslationError::InvalidRequest(
            "translation memory source segment is empty or exceeds the safety bound".to_string(),
        ));
    }
    Ok(())
}

fn suggestion(
    model: memory_entry::Model,
    normalized_source: &str,
    requested_context: &str,
    minimum_similarity_basis_points: u16,
) -> Option<MemorySuggestion> {
    let candidate_source = normalize_segment(&model.source_text);
    let source_exact = candidate_source == normalized_source;
    let base_similarity_basis_points = if source_exact {
        10_000
    } else {
        token_dice_similarity(normalized_source, &candidate_source)
    };
    let context_match = model.context_fingerprint == requested_context;
    let context_bonus_basis_points = if !source_exact && context_match {
        500
    } else {
        0
    };
    let final_similarity_basis_points = if source_exact {
        10_000
    } else {
        base_similarity_basis_points
            .saturating_add(context_bonus_basis_points)
            .min(9_999)
    };
    if final_similarity_basis_points < minimum_similarity_basis_points {
        return None;
    }
    let kind = if source_exact {
        MemoryMatchKind::Exact
    } else if context_match {
        MemoryMatchKind::ContextualFuzzy
    } else {
        MemoryMatchKind::Fuzzy
    };
    Some(MemorySuggestion {
        entry_id: model.id,
        source_text: model.source_text,
        target_text: model.target_text,
        source_hash: model.source_hash,
        owner_slug: model.owner_slug,
        resource_kind: model.resource_kind,
        resource_id: model.resource_id,
        field_key: model.field_key,
        origin: model.origin,
        proposal_id: model.proposal_id,
        apply_receipt_id: model.apply_receipt_id,
        evidence: MemoryMatchEvidence {
            kind,
            source_exact,
            context_match,
            base_similarity_basis_points,
            context_bonus_basis_points,
            final_similarity_basis_points,
            segmentation_version: model.segmentation_version,
        },
    })
}

fn memory_eligible(segment: &AppliedMemorySegment) -> bool {
    segment.reviewer_actor_kind == "user"
        && matches!(
            segment.classification,
            TranslationDataClassification::Public | TranslationDataClassification::TenantPrivate
        )
}

fn normalize_segment(value: &str) -> String {
    value
        .nfkc()
        .flat_map(char::to_lowercase)
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn token_dice_similarity(left: &str, right: &str) -> u16 {
    let left = tokens(left);
    let right = tokens(right);
    if left.is_empty() || right.is_empty() {
        return 0;
    }
    let intersection = left.intersection(&right).count() as u64;
    let denominator = (left.len() + right.len()) as u64;
    u16::try_from((2 * intersection * 10_000) / denominator).unwrap_or(10_000)
}

fn tokens(value: &str) -> BTreeSet<String> {
    value
        .split(|character: char| !character.is_alphanumeric())
        .filter(|token| !token.is_empty())
        .map(ToString::to_string)
        .collect()
}

fn context_fingerprint(
    owner_slug: &str,
    resource_kind: &str,
    field_key: &str,
) -> TranslationResult<String> {
    #[derive(Serialize)]
    struct Context<'a> {
        owner_slug: &'a str,
        resource_kind: &'a str,
        field_key: &'a str,
        segmentation_version: &'static str,
    }
    Ok(hash_manifest(&Context {
        owner_slug,
        resource_kind,
        field_key,
        segmentation_version: SEGMENTATION_VERSION,
    })?)
}

fn authorize(
    context: &PortContext,
    actions: &[Action],
    policy: PortCallPolicy,
) -> TranslationResult<Uuid> {
    context.require_policy(policy)?;
    let security = SecurityContext::try_from_port_context(context)?;
    for action in actions {
        if security.get_scope(Resource::TranslationMemory, *action) == PermissionScope::None {
            return Err(TranslationError::Forbidden);
        }
    }
    Uuid::parse_str(&context.tenant_id).map_err(|_| TranslationError::InvalidTenantId)
}

fn idempotency_key(context: &PortContext) -> &str {
    context.idempotency_key.as_deref().unwrap_or_default()
}

fn actor_kind(context: &PortContext) -> &'static str {
    match &context.actor.kind {
        PortActorKind::User => "user",
        PortActorKind::Service => "service",
        PortActorKind::System => "system",
    }
}

#[cfg(test)]
mod tests {
    use super::{normalize_segment, token_dice_similarity};

    #[test]
    fn segment_normalization_is_unicode_and_whitespace_stable() {
        assert_eq!(normalize_segment("  Cafe\u{301}\n HERO  "), "café hero");
        assert_eq!(normalize_segment("ＦＯＯ"), "foo");
    }

    #[test]
    fn token_similarity_is_deterministic() {
        assert_eq!(
            token_dice_similarity("hero returns", "hero returns"),
            10_000
        );
        assert_eq!(token_dice_similarity("hero returns", "hero arrives"), 5_000);
        assert_eq!(token_dice_similarity("hero", "villain"), 0);
    }
}
