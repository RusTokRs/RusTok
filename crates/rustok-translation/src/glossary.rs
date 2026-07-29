use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use chrono::Utc;
use rustok_api::{
    Action, PortActorKind, PortCallPolicy, PortContext, Resource, TenantLocale,
    manifest_hash::hash_manifest,
};
use rustok_core::{PermissionScope, SecurityContext, generate_id};
use rustok_tenant::TenantLocalePolicyPort;
use rustok_translation_targets::{FieldKey, OwnerSlug, ResourceKind};
use sea_orm::{
    ColumnTrait, Condition, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter,
    QueryOrder, QuerySelect, Set, TransactionTrait,
    sea_query::{Expr, OnConflict, SimpleExpr},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    TranslationError, TranslationResult,
    entities::{glossary, glossary_receipt, glossary_term},
    policy::{read_validated_tenant_locale_policy, validate_job_locales},
};

const MAX_GLOSSARIES_PER_READ: u16 = 200;
const MAX_CONCEPTS: usize = 1_000;
const MAX_VARIANTS_PER_CONCEPT: usize = 32;
const MAX_TOTAL_VARIANTS: usize = 5_000;
const MAX_NAME_BYTES: usize = 191;
const MAX_DESCRIPTION_BYTES: usize = 4_096;
const MAX_CONCEPT_KEY_BYTES: usize = 191;
const MAX_TERM_BYTES: usize = 2_048;
const MAX_NOTES_BYTES: usize = 4_096;

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlossaryScope {
    pub owner_slug: Option<OwnerSlug>,
    pub resource_kind: Option<ResourceKind>,
    pub field_key: Option<FieldKey>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GlossaryTermPolicy {
    Preferred,
    Allowed,
    Forbidden,
    DoNotTranslate,
}

impl GlossaryTermPolicy {
    fn as_str(self) -> &'static str {
        match self {
            Self::Preferred => "preferred",
            Self::Allowed => "allowed",
            Self::Forbidden => "forbidden",
            Self::DoNotTranslate => "do_not_translate",
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GlossaryMatchKind {
    Exact,
    WholeWord,
    Substring,
}

impl GlossaryMatchKind {
    fn as_str(self) -> &'static str {
        match self {
            Self::Exact => "exact",
            Self::WholeWord => "whole_word",
            Self::Substring => "substring",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlossaryVariant {
    pub value: String,
    pub policy: GlossaryTermPolicy,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlossaryConcept {
    pub concept_key: String,
    pub source_term: String,
    pub variants: Vec<GlossaryVariant>,
    pub match_kind: GlossaryMatchKind,
    pub case_sensitive: bool,
    pub notes: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CreateGlossaryInput {
    pub name: String,
    pub description: String,
    pub source_locale: TenantLocale,
    pub target_locale: TenantLocale,
    pub scope: GlossaryScope,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UpdateGlossaryInput {
    pub glossary_id: Uuid,
    pub expected_revision: i64,
    pub name: String,
    pub description: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ReplaceGlossaryTermsInput {
    pub glossary_id: Uuid,
    pub expected_revision: i64,
    pub concepts: Vec<GlossaryConcept>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SetGlossaryActiveInput {
    pub glossary_id: Uuid,
    pub expected_revision: i64,
    pub is_active: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlossaryBinding {
    pub glossary_id: Uuid,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlossarySummaryRecord {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub source_locale: TenantLocale,
    pub target_locale: TenantLocale,
    pub scope: GlossaryScope,
    pub is_active: bool,
    pub revision: i64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GlossaryRecord {
    pub id: Uuid,
    pub name: String,
    pub description: String,
    pub source_locale: TenantLocale,
    pub target_locale: TenantLocale,
    pub scope: GlossaryScope,
    pub is_active: bool,
    pub revision: i64,
    pub concepts: Vec<GlossaryConcept>,
}

pub struct TranslationGlossaryService {
    database: DatabaseConnection,
    tenant_locale_policies: Arc<dyn TenantLocalePolicyPort>,
}

impl TranslationGlossaryService {
    pub fn new(
        database: DatabaseConnection,
        tenant_locale_policies: Arc<dyn TenantLocalePolicyPort>,
    ) -> Self {
        Self {
            database,
            tenant_locale_policies,
        }
    }

    pub async fn list_glossaries(
        &self,
        context: PortContext,
        limit: u16,
    ) -> TranslationResult<Vec<GlossarySummaryRecord>> {
        let tenant_id = authorize(&context, Action::List, PortCallPolicy::read())?;
        if limit == 0 || limit > MAX_GLOSSARIES_PER_READ {
            return Err(TranslationError::InvalidRequest(format!(
                "glossary list limit must be between 1 and {MAX_GLOSSARIES_PER_READ}"
            )));
        }
        glossary::Entity::find()
            .filter(glossary::Column::TenantId.eq(tenant_id))
            .order_by_asc(glossary::Column::NameKey)
            .order_by_asc(glossary::Column::Id)
            .limit(u64::from(limit))
            .all(&self.database)
            .await?
            .into_iter()
            .map(summary_record)
            .collect()
    }

    pub async fn read_glossary(
        &self,
        context: PortContext,
        glossary_id: Uuid,
        revision: Option<i64>,
    ) -> TranslationResult<GlossaryRecord> {
        let tenant_id = authorize(&context, Action::Read, PortCallPolicy::read())?;
        read_record(&self.database, tenant_id, glossary_id, revision).await
    }

    pub async fn create_glossary(
        &self,
        context: PortContext,
        mut input: CreateGlossaryInput,
    ) -> TranslationResult<GlossaryRecord> {
        let tenant_id = authorize(&context, Action::Create, PortCallPolicy::write())?;
        normalize_create_input(&mut input)?;
        let tenant_policy = read_validated_tenant_locale_policy(
            self.tenant_locale_policies.as_ref(),
            context.clone(),
            tenant_id,
        )
        .await?;
        validate_job_locales(&tenant_policy, &input.source_locale, &input.target_locale)?;
        let request_hash = operation_hash("create", &input)?;
        if let Some(receipt) =
            find_receipt(&self.database, tenant_id, idempotency_key(&context)).await?
        {
            return replay_receipt(receipt, &context, "create", &request_hash);
        }

        let glossary_id = generate_id();
        let now = Utc::now().fixed_offset();
        let name_key = name_key(&input.name);
        let actor_kind = actor_kind(&context).to_string();
        let transaction = self.database.begin().await?;
        glossary::Entity::insert(glossary::ActiveModel {
            id: Set(glossary_id),
            tenant_id: Set(tenant_id),
            name: Set(input.name),
            name_key: Set(name_key.clone()),
            description: Set(input.description),
            source_locale: Set(input.source_locale.as_str().to_string()),
            target_locale: Set(input.target_locale.as_str().to_string()),
            owner_slug: Set(scope_value(input.scope.owner_slug.as_ref())),
            resource_kind: Set(scope_value(input.scope.resource_kind.as_ref())),
            field_key: Set(scope_value(input.scope.field_key.as_ref())),
            is_active: Set(true),
            revision: Set(1),
            last_idempotency_key: Set(idempotency_key(&context).to_string()),
            last_request_hash: Set(request_hash.clone()),
            created_by_actor_kind: Set(actor_kind.clone()),
            created_by_actor_id: Set(context.actor.id.clone()),
            updated_by_actor_kind: Set(actor_kind),
            updated_by_actor_id: Set(context.actor.id.clone()),
            created_at: Set(now),
            updated_at: Set(now),
        })
        .on_conflict(OnConflict::new().do_nothing().to_owned())
        .exec_without_returning(&transaction)
        .await?;
        let Some(persisted) = glossary::Entity::find_by_id(glossary_id)
            .filter(glossary::Column::TenantId.eq(tenant_id))
            .one(&transaction)
            .await?
        else {
            let duplicate = glossary::Entity::find()
                .filter(glossary::Column::TenantId.eq(tenant_id))
                .filter(glossary::Column::NameKey.eq(name_key))
                .one(&transaction)
                .await?;
            transaction.rollback().await?;
            return if duplicate.is_some() {
                Err(TranslationError::GlossaryNameConflict)
            } else {
                Err(TranslationError::GlossaryInvariant(
                    "glossary insert did not persist".to_string(),
                ))
            };
        };
        let record = record_from_models(persisted, 1, Vec::new())?;
        if let Some(replay) = insert_receipt(
            &transaction,
            tenant_id,
            &context,
            "create",
            &request_hash,
            &record,
        )
        .await?
        {
            transaction.rollback().await?;
            return Ok(replay);
        }
        transaction.commit().await?;
        Ok(record)
    }

    pub async fn update_glossary(
        &self,
        context: PortContext,
        mut input: UpdateGlossaryInput,
    ) -> TranslationResult<GlossaryRecord> {
        let tenant_id = authorize(&context, Action::Update, PortCallPolicy::write())?;
        normalize_update_input(&mut input)?;
        let request_hash = operation_hash("update", &input)?;
        if let Some(receipt) =
            find_receipt(&self.database, tenant_id, idempotency_key(&context)).await?
        {
            return replay_receipt(receipt, &context, "update", &request_hash);
        }
        let current = current_model(&self.database, tenant_id, input.glossary_id).await?;
        ensure_revision(&current, input.expected_revision)?;
        let new_name_key = name_key(&input.name);
        if glossary::Entity::find()
            .filter(glossary::Column::TenantId.eq(tenant_id))
            .filter(glossary::Column::NameKey.eq(&new_name_key))
            .filter(glossary::Column::Id.ne(input.glossary_id))
            .one(&self.database)
            .await?
            .is_some()
        {
            return Err(TranslationError::GlossaryNameConflict);
        }
        let next_revision = next_revision(current.revision)?;
        let transaction = self.database.begin().await?;
        update_model(
            &transaction,
            tenant_id,
            input.glossary_id,
            input.expected_revision,
            next_revision,
            &context,
            &request_hash,
            [
                (glossary::Column::Name, Expr::value(input.name)),
                (glossary::Column::NameKey, Expr::value(new_name_key)),
                (
                    glossary::Column::Description,
                    Expr::value(input.description),
                ),
            ],
        )
        .await?;
        let record = read_record(
            &transaction,
            tenant_id,
            input.glossary_id,
            Some(next_revision),
        )
        .await?;
        if let Some(replay) = insert_receipt(
            &transaction,
            tenant_id,
            &context,
            "update",
            &request_hash,
            &record,
        )
        .await?
        {
            transaction.rollback().await?;
            return Ok(replay);
        }
        transaction.commit().await?;
        Ok(record)
    }

    pub async fn replace_terms(
        &self,
        context: PortContext,
        mut input: ReplaceGlossaryTermsInput,
    ) -> TranslationResult<GlossaryRecord> {
        let tenant_id = authorize(&context, Action::Update, PortCallPolicy::write())?;
        normalize_concepts(&mut input.concepts)?;
        let request_hash = operation_hash("replace_terms", &input)?;
        if let Some(receipt) =
            find_receipt(&self.database, tenant_id, idempotency_key(&context)).await?
        {
            return replay_receipt(receipt, &context, "replace_terms", &request_hash);
        }
        let current = current_model(&self.database, tenant_id, input.glossary_id).await?;
        ensure_revision(&current, input.expected_revision)?;
        if !current.is_active {
            return Err(TranslationError::GlossaryInactive);
        }
        let next_revision = next_revision(current.revision)?;
        let now = Utc::now().fixed_offset();
        let transaction = self.database.begin().await?;
        update_model(
            &transaction,
            tenant_id,
            input.glossary_id,
            input.expected_revision,
            next_revision,
            &context,
            &request_hash,
            [],
        )
        .await?;
        glossary_term::Entity::update_many()
            .col_expr(
                glossary_term::Column::ValidToRevision,
                Expr::value(next_revision),
            )
            .filter(glossary_term::Column::TenantId.eq(tenant_id))
            .filter(glossary_term::Column::GlossaryId.eq(input.glossary_id))
            .filter(glossary_term::Column::ValidToRevision.is_null())
            .exec(&transaction)
            .await?;
        let terms = term_models(
            tenant_id,
            input.glossary_id,
            next_revision,
            &context,
            now,
            &input.concepts,
        );
        if !terms.is_empty() {
            glossary_term::Entity::insert_many(terms)
                .exec_without_returning(&transaction)
                .await?;
        }
        let record = read_record(
            &transaction,
            tenant_id,
            input.glossary_id,
            Some(next_revision),
        )
        .await?;
        if let Some(replay) = insert_receipt(
            &transaction,
            tenant_id,
            &context,
            "replace_terms",
            &request_hash,
            &record,
        )
        .await?
        {
            transaction.rollback().await?;
            return Ok(replay);
        }
        transaction.commit().await?;
        Ok(record)
    }

    pub async fn set_active(
        &self,
        context: PortContext,
        input: SetGlossaryActiveInput,
    ) -> TranslationResult<GlossaryRecord> {
        let tenant_id = authorize(&context, Action::Manage, PortCallPolicy::write())?;
        let request_hash = operation_hash("set_active", &input)?;
        if let Some(receipt) =
            find_receipt(&self.database, tenant_id, idempotency_key(&context)).await?
        {
            return replay_receipt(receipt, &context, "set_active", &request_hash);
        }
        let current = current_model(&self.database, tenant_id, input.glossary_id).await?;
        ensure_revision(&current, input.expected_revision)?;
        if current.is_active == input.is_active {
            return Err(TranslationError::GlossaryActiveStateUnchanged);
        }
        let next_revision = next_revision(current.revision)?;
        let transaction = self.database.begin().await?;
        update_model(
            &transaction,
            tenant_id,
            input.glossary_id,
            input.expected_revision,
            next_revision,
            &context,
            &request_hash,
            [(glossary::Column::IsActive, Expr::value(input.is_active))],
        )
        .await?;
        let record = read_record(
            &transaction,
            tenant_id,
            input.glossary_id,
            Some(next_revision),
        )
        .await?;
        if let Some(replay) = insert_receipt(
            &transaction,
            tenant_id,
            &context,
            "set_active",
            &request_hash,
            &record,
        )
        .await?
        {
            transaction.rollback().await?;
            return Ok(replay);
        }
        transaction.commit().await?;
        Ok(record)
    }
}

pub(crate) async fn validate_glossary_binding(
    database: &DatabaseConnection,
    context: &PortContext,
    binding: &GlossaryBinding,
    source_locale: &TenantLocale,
    target_locale: &TenantLocale,
) -> TranslationResult<()> {
    let tenant_id = authorize(context, Action::Read, PortCallPolicy::read())?;
    let model = current_model(database, tenant_id, binding.glossary_id).await?;
    if !model.is_active {
        return Err(TranslationError::GlossaryInactive);
    }
    ensure_revision(&model, binding.revision)?;
    if model.source_locale != source_locale.as_str()
        || model.target_locale != target_locale.as_str()
    {
        return Err(TranslationError::GlossaryLocaleMismatch);
    }
    Ok(())
}

pub(crate) async fn read_bound_glossary<C>(
    database: &C,
    tenant_id: Uuid,
    binding: &GlossaryBinding,
) -> TranslationResult<GlossaryRecord>
where
    C: ConnectionTrait,
{
    read_record(
        database,
        tenant_id,
        binding.glossary_id,
        Some(binding.revision),
    )
    .await
}

async fn current_model<C>(
    database: &C,
    tenant_id: Uuid,
    glossary_id: Uuid,
) -> TranslationResult<glossary::Model>
where
    C: ConnectionTrait,
{
    glossary::Entity::find_by_id(glossary_id)
        .filter(glossary::Column::TenantId.eq(tenant_id))
        .one(database)
        .await?
        .ok_or(TranslationError::GlossaryNotFound)
}

async fn read_record<C>(
    database: &C,
    tenant_id: Uuid,
    glossary_id: Uuid,
    revision: Option<i64>,
) -> TranslationResult<GlossaryRecord>
where
    C: ConnectionTrait,
{
    let model = current_model(database, tenant_id, glossary_id).await?;
    let selected_revision = revision.unwrap_or(model.revision);
    if selected_revision < 1 || selected_revision > model.revision {
        return Err(TranslationError::GlossaryRevisionUnavailable {
            requested: selected_revision,
            current: model.revision,
        });
    }
    let term_models = glossary_term::Entity::find()
        .filter(glossary_term::Column::TenantId.eq(tenant_id))
        .filter(glossary_term::Column::GlossaryId.eq(glossary_id))
        .filter(glossary_term::Column::ValidFromRevision.lte(selected_revision))
        .filter(
            Condition::any()
                .add(glossary_term::Column::ValidToRevision.is_null())
                .add(glossary_term::Column::ValidToRevision.gt(selected_revision)),
        )
        .order_by_asc(glossary_term::Column::ConceptKey)
        .order_by_asc(glossary_term::Column::Policy)
        .order_by_asc(glossary_term::Column::TargetTerm)
        .all(database)
        .await?;
    record_from_models(model, selected_revision, term_models)
}

fn record_from_models(
    model: glossary::Model,
    revision: i64,
    terms: Vec<glossary_term::Model>,
) -> TranslationResult<GlossaryRecord> {
    let mut concepts = BTreeMap::<String, GlossaryConcept>::new();
    for term in terms {
        let policy = parse_policy(&term.policy)?;
        let match_kind = parse_match_kind(&term.match_kind)?;
        let entry = concepts
            .entry(term.concept_key.clone())
            .or_insert_with(|| GlossaryConcept {
                concept_key: term.concept_key.clone(),
                source_term: term.source_term.clone(),
                variants: Vec::new(),
                match_kind,
                case_sensitive: term.case_sensitive,
                notes: term.notes.clone(),
            });
        if entry.source_term != term.source_term
            || entry.match_kind != match_kind
            || entry.case_sensitive != term.case_sensitive
            || entry.notes != term.notes
        {
            return Err(TranslationError::GlossaryInvariant(format!(
                "concept `{}` contains inconsistent revision rows",
                term.concept_key
            )));
        }
        entry.variants.push(GlossaryVariant {
            value: term.target_term,
            policy,
        });
    }
    for concept in concepts.values_mut() {
        concept
            .variants
            .sort_by(|left, right| (left.policy, &left.value).cmp(&(right.policy, &right.value)));
    }
    Ok(GlossaryRecord {
        id: model.id,
        name: model.name,
        description: model.description,
        source_locale: parse_locale(model.source_locale)?,
        target_locale: parse_locale(model.target_locale)?,
        scope: parse_scope(model.owner_slug, model.resource_kind, model.field_key)?,
        is_active: model.is_active,
        revision,
        concepts: concepts.into_values().collect(),
    })
}

fn summary_record(model: glossary::Model) -> TranslationResult<GlossarySummaryRecord> {
    Ok(GlossarySummaryRecord {
        id: model.id,
        name: model.name,
        description: model.description,
        source_locale: parse_locale(model.source_locale)?,
        target_locale: parse_locale(model.target_locale)?,
        scope: parse_scope(model.owner_slug, model.resource_kind, model.field_key)?,
        is_active: model.is_active,
        revision: model.revision,
    })
}

fn parse_scope(
    owner_slug: String,
    resource_kind: String,
    field_key: String,
) -> TranslationResult<GlossaryScope> {
    Ok(GlossaryScope {
        owner_slug: (!owner_slug.is_empty())
            .then(|| OwnerSlug::new(owner_slug))
            .transpose()
            .map_err(|error| TranslationError::GlossaryInvariant(error.to_string()))?,
        resource_kind: (!resource_kind.is_empty())
            .then(|| ResourceKind::new(resource_kind))
            .transpose()
            .map_err(|error| TranslationError::GlossaryInvariant(error.to_string()))?,
        field_key: (!field_key.is_empty())
            .then(|| FieldKey::new(field_key))
            .transpose()
            .map_err(|error| TranslationError::GlossaryInvariant(error.to_string()))?,
    })
}

fn parse_locale(value: String) -> TranslationResult<TenantLocale> {
    TenantLocale::new(value).map_err(|error| TranslationError::GlossaryInvariant(error.to_string()))
}

fn parse_policy(value: &str) -> TranslationResult<GlossaryTermPolicy> {
    match value {
        "preferred" => Ok(GlossaryTermPolicy::Preferred),
        "allowed" => Ok(GlossaryTermPolicy::Allowed),
        "forbidden" => Ok(GlossaryTermPolicy::Forbidden),
        "do_not_translate" => Ok(GlossaryTermPolicy::DoNotTranslate),
        _ => Err(TranslationError::GlossaryInvariant(format!(
            "unknown glossary term policy `{value}`"
        ))),
    }
}

fn parse_match_kind(value: &str) -> TranslationResult<GlossaryMatchKind> {
    match value {
        "exact" => Ok(GlossaryMatchKind::Exact),
        "whole_word" => Ok(GlossaryMatchKind::WholeWord),
        "substring" => Ok(GlossaryMatchKind::Substring),
        _ => Err(TranslationError::GlossaryInvariant(format!(
            "unknown glossary match kind `{value}`"
        ))),
    }
}

fn normalize_create_input(input: &mut CreateGlossaryInput) -> TranslationResult<()> {
    input.name = bounded_text(&input.name, "name", MAX_NAME_BYTES, false)?;
    input.description = bounded_text(
        &input.description,
        "description",
        MAX_DESCRIPTION_BYTES,
        true,
    )?;
    validate_scope(&input.scope)
}

fn normalize_update_input(input: &mut UpdateGlossaryInput) -> TranslationResult<()> {
    if input.expected_revision < 1 {
        return Err(TranslationError::GlossaryRevisionConflict {
            expected: input.expected_revision,
            actual: 0,
        });
    }
    input.name = bounded_text(&input.name, "name", MAX_NAME_BYTES, false)?;
    input.description = bounded_text(
        &input.description,
        "description",
        MAX_DESCRIPTION_BYTES,
        true,
    )?;
    Ok(())
}

fn validate_scope(scope: &GlossaryScope) -> TranslationResult<()> {
    if scope.resource_kind.is_some() && scope.owner_slug.is_none() {
        return Err(TranslationError::GlossaryTermConflict(
            "resource_kind scope requires owner_slug".to_string(),
        ));
    }
    if scope.field_key.is_some() && scope.resource_kind.is_none() {
        return Err(TranslationError::GlossaryTermConflict(
            "field_key scope requires resource_kind".to_string(),
        ));
    }
    Ok(())
}

fn normalize_concepts(concepts: &mut Vec<GlossaryConcept>) -> TranslationResult<()> {
    if concepts.len() > MAX_CONCEPTS {
        return Err(TranslationError::GlossaryTermConflict(format!(
            "glossary exceeds the {MAX_CONCEPTS}-concept safety bound"
        )));
    }
    let mut concept_keys = BTreeSet::new();
    let mut source_terms = BTreeSet::new();
    let mut total_variants = 0usize;
    for concept in concepts.iter_mut() {
        concept.concept_key = normalize_concept_key(&concept.concept_key)?;
        if !concept_keys.insert(concept.concept_key.clone()) {
            return Err(TranslationError::GlossaryTermConflict(format!(
                "duplicate concept key `{}`",
                concept.concept_key
            )));
        }
        concept.source_term =
            bounded_text(&concept.source_term, "source_term", MAX_TERM_BYTES, false)?;
        let canonical_source = concept.source_term.to_lowercase();
        if !source_terms.insert(canonical_source) {
            return Err(TranslationError::GlossaryTermConflict(format!(
                "source term `{}` belongs to more than one concept",
                concept.source_term
            )));
        }
        concept.notes = bounded_text(&concept.notes, "notes", MAX_NOTES_BYTES, true)?;
        if concept.variants.is_empty() || concept.variants.len() > MAX_VARIANTS_PER_CONCEPT {
            return Err(TranslationError::GlossaryTermConflict(format!(
                "concept `{}` must contain between 1 and {MAX_VARIANTS_PER_CONCEPT} variants",
                concept.concept_key
            )));
        }
        total_variants = total_variants
            .checked_add(concept.variants.len())
            .ok_or_else(|| {
                TranslationError::GlossaryTermConflict(
                    "glossary variant count overflow".to_string(),
                )
            })?;
        let mut target_terms = BTreeSet::new();
        let mut preferred_count = 0usize;
        let mut do_not_translate_count = 0usize;
        for variant in &mut concept.variants {
            variant.value = bounded_text(&variant.value, "target_term", MAX_TERM_BYTES, false)?;
            if !target_terms.insert(variant.value.to_lowercase()) {
                return Err(TranslationError::GlossaryTermConflict(format!(
                    "concept `{}` contains a duplicate target term",
                    concept.concept_key
                )));
            }
            preferred_count += usize::from(variant.policy == GlossaryTermPolicy::Preferred);
            do_not_translate_count +=
                usize::from(variant.policy == GlossaryTermPolicy::DoNotTranslate);
        }
        if preferred_count > 1 {
            return Err(TranslationError::GlossaryTermConflict(format!(
                "concept `{}` has more than one preferred term",
                concept.concept_key
            )));
        }
        if do_not_translate_count > 0
            && (concept.variants.len() != 1 || concept.variants[0].value != concept.source_term)
        {
            return Err(TranslationError::GlossaryTermConflict(format!(
                "do-not-translate concept `{}` must contain only its exact source value",
                concept.concept_key
            )));
        }
        concept
            .variants
            .sort_by(|left, right| (left.policy, &left.value).cmp(&(right.policy, &right.value)));
    }
    if total_variants > MAX_TOTAL_VARIANTS {
        return Err(TranslationError::GlossaryTermConflict(format!(
            "glossary exceeds the {MAX_TOTAL_VARIANTS}-variant safety bound"
        )));
    }
    concepts.sort_by(|left, right| left.concept_key.cmp(&right.concept_key));
    Ok(())
}

fn term_models(
    tenant_id: Uuid,
    glossary_id: Uuid,
    revision: i64,
    context: &PortContext,
    now: chrono::DateTime<chrono::FixedOffset>,
    concepts: &[GlossaryConcept],
) -> Vec<glossary_term::ActiveModel> {
    concepts
        .iter()
        .flat_map(|concept| {
            concept
                .variants
                .iter()
                .map(move |variant| glossary_term::ActiveModel {
                    id: Set(generate_id()),
                    tenant_id: Set(tenant_id),
                    glossary_id: Set(glossary_id),
                    concept_key: Set(concept.concept_key.clone()),
                    source_term: Set(concept.source_term.clone()),
                    target_term: Set(variant.value.clone()),
                    policy: Set(variant.policy.as_str().to_string()),
                    match_kind: Set(concept.match_kind.as_str().to_string()),
                    case_sensitive: Set(concept.case_sensitive),
                    notes: Set(concept.notes.clone()),
                    valid_from_revision: Set(revision),
                    valid_to_revision: Set(None),
                    created_by_actor_kind: Set(actor_kind(context).to_string()),
                    created_by_actor_id: Set(context.actor.id.clone()),
                    created_at: Set(now),
                })
        })
        .collect()
}

async fn update_model<C, const N: usize>(
    database: &C,
    tenant_id: Uuid,
    glossary_id: Uuid,
    expected_revision: i64,
    next_revision: i64,
    context: &PortContext,
    request_hash: &str,
    changes: [(glossary::Column, SimpleExpr); N],
) -> TranslationResult<()>
where
    C: ConnectionTrait,
{
    let now = Utc::now().fixed_offset();
    let mut update = glossary::Entity::update_many()
        .col_expr(glossary::Column::Revision, Expr::value(next_revision))
        .col_expr(
            glossary::Column::LastIdempotencyKey,
            Expr::value(idempotency_key(context).to_string()),
        )
        .col_expr(
            glossary::Column::LastRequestHash,
            Expr::value(request_hash.to_string()),
        )
        .col_expr(
            glossary::Column::UpdatedByActorKind,
            Expr::value(actor_kind(context).to_string()),
        )
        .col_expr(
            glossary::Column::UpdatedByActorId,
            Expr::value(context.actor.id.clone()),
        )
        .col_expr(glossary::Column::UpdatedAt, Expr::value(now));
    for (column, value) in changes {
        update = update.col_expr(column, value);
    }
    let result = update
        .filter(glossary::Column::TenantId.eq(tenant_id))
        .filter(glossary::Column::Id.eq(glossary_id))
        .filter(glossary::Column::Revision.eq(expected_revision))
        .exec(database)
        .await?;
    if result.rows_affected != 1 {
        return Err(TranslationError::GlossaryRevisionConflict {
            expected: expected_revision,
            actual: expected_revision.saturating_add(1),
        });
    }
    Ok(())
}

async fn insert_receipt<C>(
    database: &C,
    tenant_id: Uuid,
    context: &PortContext,
    operation: &str,
    request_hash: &str,
    record: &GlossaryRecord,
) -> TranslationResult<Option<GlossaryRecord>>
where
    C: ConnectionTrait,
{
    let receipt_id = generate_id();
    glossary_receipt::Entity::insert(glossary_receipt::ActiveModel {
        id: Set(receipt_id),
        tenant_id: Set(tenant_id),
        glossary_id: Set(record.id),
        operation: Set(operation.to_string()),
        idempotency_key: Set(idempotency_key(context).to_string()),
        request_hash: Set(request_hash.to_string()),
        requested_by_actor_kind: Set(actor_kind(context).to_string()),
        requested_by_actor_id: Set(context.actor.id.clone()),
        resulting_glossary_revision: Set(record.revision),
        response: Set(serde_json::to_value(record)?),
        created_at: Set(Utc::now().fixed_offset()),
    })
    .on_conflict(
        OnConflict::columns([
            glossary_receipt::Column::TenantId,
            glossary_receipt::Column::IdempotencyKey,
        ])
        .do_nothing()
        .to_owned(),
    )
    .exec_without_returning(database)
    .await?;
    let receipt = find_receipt(database, tenant_id, idempotency_key(context))
        .await?
        .ok_or_else(|| {
            TranslationError::GlossaryInvariant("glossary receipt did not persist".to_string())
        })?;
    if receipt.id == receipt_id {
        Ok(None)
    } else {
        replay_receipt(receipt, context, operation, request_hash).map(Some)
    }
}

async fn find_receipt<C>(
    database: &C,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> TranslationResult<Option<glossary_receipt::Model>>
where
    C: ConnectionTrait,
{
    Ok(glossary_receipt::Entity::find()
        .filter(glossary_receipt::Column::TenantId.eq(tenant_id))
        .filter(glossary_receipt::Column::IdempotencyKey.eq(idempotency_key))
        .one(database)
        .await?)
}

fn replay_receipt(
    receipt: glossary_receipt::Model,
    context: &PortContext,
    operation: &str,
    request_hash: &str,
) -> TranslationResult<GlossaryRecord> {
    if receipt.requested_by_actor_kind != actor_kind(context)
        || receipt.requested_by_actor_id != context.actor.id
    {
        return Err(TranslationError::IdempotencyActorMismatch);
    }
    if receipt.operation != operation || receipt.request_hash != request_hash {
        return Err(TranslationError::IdempotencyConflict);
    }
    let record: GlossaryRecord = serde_json::from_value(receipt.response)?;
    if record.id != receipt.glossary_id || record.revision != receipt.resulting_glossary_revision {
        return Err(TranslationError::GlossaryInvariant(
            "glossary receipt does not match its response".to_string(),
        ));
    }
    Ok(record)
}

fn operation_hash<T: Serialize>(operation: &str, input: &T) -> TranslationResult<String> {
    hash_manifest(&(operation, input)).map_err(Into::into)
}

fn authorize(
    context: &PortContext,
    action: Action,
    call_policy: PortCallPolicy,
) -> TranslationResult<Uuid> {
    context.require_policy(call_policy)?;
    let security = SecurityContext::try_from_port_context(context)?;
    if security.get_scope(Resource::TranslationGlossaries, action) == PermissionScope::None {
        return Err(TranslationError::Forbidden);
    }
    Uuid::parse_str(&context.tenant_id).map_err(|_| TranslationError::InvalidTenantId)
}

fn ensure_revision(model: &glossary::Model, expected: i64) -> TranslationResult<()> {
    if model.revision != expected {
        return Err(TranslationError::GlossaryRevisionConflict {
            expected,
            actual: model.revision,
        });
    }
    Ok(())
}

fn next_revision(current: i64) -> TranslationResult<i64> {
    current.checked_add(1).ok_or_else(|| {
        TranslationError::GlossaryInvariant("glossary revision overflow".to_string())
    })
}

fn bounded_text(
    value: &str,
    field: &str,
    max_bytes: usize,
    allow_empty: bool,
) -> TranslationResult<String> {
    let normalized = value.trim();
    if !allow_empty && normalized.is_empty() {
        return Err(TranslationError::GlossaryTermConflict(format!(
            "{field} must not be empty"
        )));
    }
    if normalized.len() > max_bytes {
        return Err(TranslationError::GlossaryTermConflict(format!(
            "{field} exceeds the {max_bytes}-byte safety bound"
        )));
    }
    Ok(normalized.to_string())
}

fn normalize_concept_key(value: &str) -> TranslationResult<String> {
    let key = bounded_text(value, "concept_key", MAX_CONCEPT_KEY_BYTES, false)?;
    if !key
        .bytes()
        .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || b"._-".contains(&byte))
    {
        return Err(TranslationError::GlossaryTermConflict(
            "concept_key must use lowercase ASCII letters, digits, dot, underscore, or hyphen"
                .to_string(),
        ));
    }
    Ok(key)
}

fn name_key(value: &str) -> String {
    value.to_lowercase()
}

fn scope_value<T: ToString>(value: Option<&T>) -> String {
    value.map(ToString::to_string).unwrap_or_default()
}

fn idempotency_key(context: &PortContext) -> &str {
    context.idempotency_key.as_deref().unwrap_or_default()
}

fn actor_kind(context: &PortContext) -> &'static str {
    match context.actor.kind {
        PortActorKind::User => "user",
        PortActorKind::Service => "service",
        PortActorKind::System => "system",
    }
}
