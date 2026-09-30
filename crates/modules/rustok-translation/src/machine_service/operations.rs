use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, FixedOffset, Utc};
use rustok_api::{
    Action, PortCallPolicy, PortContext, Resource, manifest_hash::hash_manifest,
};
use rustok_core::{PermissionScope, SecurityContext, generate_id};
use rustok_translation_targets::{
    FieldKey, TranslationResourceSnapshot, protected_token_ledger_matches,
    protected_token_multiplicities_match, whitespace_shape_matches,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait,
    sea_query::{Expr, OnConflict},
};
use uuid::Uuid;

use crate::{
    GlossaryBinding, GlossaryTermPolicy, MachineTranslationBatchRequest,
    MachineTranslationBatchResult, MachineTranslationExecutionStatusEvidence,
    MachineTranslationGlossaryTerm, MachineTranslationMemorySuggestion,
    MachineTranslationProviderDescriptor, MachineTranslationUnit, MemoryLookupInput,
    TranslationError, TranslationMemoryService, TranslationResult,
    entities::{
        job, job_item, machine_memory_binding, machine_operation, memory_entry,
    },
    glossary::read_bound_glossary,
    qa::{glossary_concept_matches, glossary_scope_matches},
};

use super::cancellation::machine_execution_status;
use super::helpers::{
    actor_kind, find_operation, find_operation_by_idempotency, is_digest,
};
use super::types::{
    GenerateMachineProposalInput, MachineDiagnosticEvidence, MachineOperationStatusRecord,
    MachineProposalOutcome, MachineProposalRecord, MEMORY_SUGGESTIONS_PER_UNIT,
};

pub(crate) fn validate_generation_input(
    input: &GenerateMachineProposalInput,
) -> TranslationResult<()> {
    if input.field_keys.is_empty() {
        return Err(TranslationError::InvalidRequest(
            "machine translation requires an explicit non-empty field selection".to_string(),
        ));
    }
    let unique = input.field_keys.iter().collect::<BTreeSet<_>>();
    if unique.len() != input.field_keys.len() {
        return Err(TranslationError::InvalidRequest(
            "machine translation field selection contains duplicates".to_string(),
        ));
    }
    if input.minimum_memory_similarity_basis_points > 10_000 {
        return Err(TranslationError::InvalidRequest(
            "memory similarity must be between 0 and 10000 basis points".to_string(),
        ));
    }
    Ok(())
}

pub(crate) fn authorize_machine_generation(context: &PortContext) -> TranslationResult<Uuid> {
    context.require_policy(PortCallPolicy::write())?;
    let security = SecurityContext::try_from_port_context(context)?;
    for action in [Action::Run, Action::Update] {
        if security.get_scope(Resource::Translations, action) == PermissionScope::None {
            return Err(TranslationError::Forbidden);
        }
    }
    Uuid::parse_str(&context.tenant_id).map_err(|_| TranslationError::InvalidTenantId)
}

pub(crate) fn authorize_machine_recovery(context: &PortContext) -> TranslationResult<Uuid> {
    context.require_policy(PortCallPolicy::write())?;
    let security = SecurityContext::try_from_port_context(context)?;
    for action in [Action::Manage, Action::Update] {
        if security.get_scope(Resource::Translations, action) == PermissionScope::None {
            return Err(TranslationError::Forbidden);
        }
    }
    Uuid::parse_str(&context.tenant_id).map_err(|_| TranslationError::InvalidTenantId)
}

pub(crate) fn enforce_machine_assignment(
    item: &job_item::Model,
    context: &PortContext,
) -> TranslationResult<()> {
    match (&item.assigned_actor_kind, &item.assigned_actor_id) {
        (None, None) => Ok(()),
        (Some(kind), Some(id))
            if kind == actor_kind(context) && id.as_str() == context.actor.id.as_str() =>
        {
            Ok(())
        }
        (Some(_), Some(_)) => Err(TranslationError::ItemAssignedToAnotherActor),
        _ => Err(TranslationError::WorkflowRevisionConflict),
    }
}

pub(crate) async fn lookup_memory_suggestions(
    memory: &TranslationMemoryService,
    tenant_id: Uuid,
    snapshot: &TranslationResourceSnapshot,
    units: &[MachineTranslationUnit],
    minimum_similarity_basis_points: u16,
) -> TranslationResult<Vec<MachineTranslationMemorySuggestion>> {
    let mut memory_suggestions = Vec::new();
    for unit in units {
        let suggestions = memory
            .lookup_for_machine(
                tenant_id,
                MemoryLookupInput {
                    source_locale: snapshot.source_locale.clone(),
                    target_locale: snapshot.target_locale.clone(),
                    identity: snapshot.summary.identity.clone(),
                    field_key: FieldKey::new(unit.field_key.clone())
                        .map_err(|error| TranslationError::InvalidRequest(error.to_string()))?,
                    source_text: unit.source_value.clone(),
                    minimum_similarity_basis_points,
                    limit: MEMORY_SUGGESTIONS_PER_UNIT,
                },
            )
            .await?;
        memory_suggestions.extend(suggestions.into_iter().map(|suggestion| {
            MachineTranslationMemorySuggestion {
                unit_id: unit.unit_id.clone(),
                entry_id: suggestion.entry_id.to_string(),
                source_value: suggestion.source_text,
                target_value: suggestion.target_text,
                score_basis_points: suggestion.evidence.final_similarity_basis_points,
                source_hash: suggestion.source_hash,
            }
        }));
    }
    Ok(memory_suggestions)
}

pub(crate) async fn read_pinned_memory_suggestions(
    database: &DatabaseConnection,
    tenant_id: Uuid,
    operation: &machine_operation::Model,
    snapshot: &TranslationResourceSnapshot,
    units: &[MachineTranslationUnit],
) -> TranslationResult<Vec<MachineTranslationMemorySuggestion>> {
    let bindings = machine_memory_binding::Entity::find()
        .filter(machine_memory_binding::Column::TenantId.eq(tenant_id))
        .filter(machine_memory_binding::Column::OperationId.eq(operation.id))
        .order_by_asc(machine_memory_binding::Column::BatchOrdinal)
        .all(database)
        .await?;
    if bindings.is_empty() {
        if operation.memory_digest.is_some() {
            return Err(TranslationError::MachineMemoryProjectionUnavailable);
        }
        return Ok(Vec::new());
    }
    if operation.memory_digest.is_none() {
        return Err(TranslationError::MachineMemoryProjectionUnavailable);
    }

    let unit_ids = units
        .iter()
        .map(|unit| unit.unit_id.as_str())
        .collect::<BTreeSet<_>>();
    let entry_ids = bindings
        .iter()
        .map(|binding| binding.memory_entry_id)
        .collect::<BTreeSet<_>>();
    let expected_entry_count = entry_ids.len();
    let entries = memory_entry::Entity::find()
        .filter(memory_entry::Column::TenantId.eq(tenant_id))
        .filter(memory_entry::Column::Id.is_in(entry_ids))
        .all(database)
        .await?
        .into_iter()
        .map(|entry| (entry.id, entry))
        .collect::<BTreeMap<_, _>>();
    if entries.len() != expected_entry_count {
        return Err(TranslationError::MachineMemoryProjectionUnavailable);
    }

    bindings
        .into_iter()
        .map(|binding| {
            if !unit_ids.contains(binding.unit_id.as_str()) {
                return Err(TranslationError::MachineMemoryProjectionUnavailable);
            }
            let entry = entries
                .get(&binding.memory_entry_id)
                .ok_or(TranslationError::MachineMemoryProjectionUnavailable)?;
            if entry.source_locale != snapshot.source_locale.as_str()
                || entry.target_locale != snapshot.target_locale.as_str()
                || entry.field_key != binding.unit_id
            {
                return Err(TranslationError::MachineMemoryProjectionUnavailable);
            }
            let score_basis_points = u16::try_from(binding.score_basis_points)
                .map_err(|_| TranslationError::MachineMemoryProjectionUnavailable)?;
            Ok(MachineTranslationMemorySuggestion {
                unit_id: binding.unit_id,
                entry_id: entry.id.to_string(),
                source_value: entry.source_text.clone(),
                target_value: entry.target_text.clone(),
                score_basis_points,
                source_hash: entry.source_hash.clone(),
            })
        })
        .collect()
}

pub(crate) async fn project_glossary(
    database: &DatabaseConnection,
    tenant_id: Uuid,
    job: &job::Model,
    snapshot: &TranslationResourceSnapshot,
    selected: &BTreeSet<&FieldKey>,
) -> TranslationResult<(
    Option<String>,
    Option<String>,
    Vec<MachineTranslationGlossaryTerm>,
)> {
    let binding = match (job.glossary_id, job.glossary_revision) {
        (None, None) => return Ok((None, None, Vec::new())),
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
    if glossary.source_locale != snapshot.source_locale
        || glossary.target_locale != snapshot.target_locale
    {
        return Err(TranslationError::GlossaryLocaleMismatch);
    }
    if !glossary_scope_matches(&glossary, &snapshot.summary.identity) {
        return Ok((None, None, Vec::new()));
    }

    let applicable_fields = snapshot.fields.iter().filter(|field| {
        selected.contains(&field.descriptor.key)
            && glossary
                .scope
                .field_key
                .as_ref()
                .is_none_or(|key| key == &field.descriptor.key)
    });
    let source_values = applicable_fields
        .map(|field| field.source_value.as_str())
        .collect::<Vec<_>>();
    let terms = glossary
        .concepts
        .iter()
        .filter(|concept| {
            source_values
                .iter()
                .any(|source| glossary_concept_matches(source, concept))
        })
        .map(|concept| {
            let mut preferred_target_term = None;
            let mut allowed_target_terms = Vec::new();
            let mut forbidden_target_terms = Vec::new();
            let mut do_not_translate = false;
            for variant in &concept.variants {
                match variant.policy {
                    GlossaryTermPolicy::Preferred => {
                        preferred_target_term = Some(variant.value.clone());
                    }
                    GlossaryTermPolicy::Allowed => {
                        allowed_target_terms.push(variant.value.clone());
                    }
                    GlossaryTermPolicy::Forbidden => {
                        forbidden_target_terms.push(variant.value.clone());
                    }
                    GlossaryTermPolicy::DoNotTranslate => do_not_translate = true,
                }
            }
            MachineTranslationGlossaryTerm {
                concept_id: concept.concept_key.clone(),
                source_term: concept.source_term.clone(),
                preferred_target_term,
                allowed_target_terms,
                forbidden_target_terms,
                do_not_translate,
            }
        })
        .collect::<Vec<_>>();
    if terms.is_empty() {
        return Ok((None, None, terms));
    }
    let digest = hash_manifest(&terms)?;
    Ok((Some(binding.revision.to_string()), Some(digest), terms))
}

pub(crate) struct RegisterMachineOperation<'a> {
    pub(crate) tenant_id: Uuid,
    pub(crate) context: &'a PortContext,
    pub(crate) input: &'a GenerateMachineProposalInput,
    pub(crate) command_hash: &'a str,
    pub(crate) machine_request_digest: &'a str,
    pub(crate) request: &'a MachineTranslationBatchRequest,
    pub(crate) adapter_slug: &'a str,
    pub(crate) provider_policy_digest: &'a str,
}

pub(crate) async fn register_operation(
    database: &DatabaseConnection,
    registration: RegisterMachineOperation<'_>,
) -> TranslationResult<(machine_operation::Model, bool)> {
    let now = Utc::now().fixed_offset();
    let id = generate_id();
    let idempotency_key = registration
        .context
        .idempotency_key
        .clone()
        .unwrap_or_default();
    let transaction = database.begin().await?;
    machine_operation::Entity::insert(machine_operation::ActiveModel {
        id: Set(id),
        tenant_id: Set(registration.tenant_id),
        item_id: Set(registration.input.item_id),
        proposal_id: Set(None),
        status: Set("registered".to_string()),
        command_hash: Set(registration.command_hash.to_string()),
        machine_request_digest: Set(registration.machine_request_digest.to_string()),
        adapter_slug: Set(registration.adapter_slug.to_string()),
        provider_slug: Set(None),
        provider_policy_digest: Set(registration.provider_policy_digest.to_string()),
        glossary_revision: Set(registration.request.glossary_revision.clone()),
        glossary_digest: Set(registration.request.glossary_digest.clone()),
        memory_digest: Set(registration.request.memory_digest.clone()),
        execution_id: Set(None),
        execution_request_digest: Set(None),
        prompt_policy_digest: Set(None),
        attempts: Set(serde_json::json!([])),
        usage: Set(None),
        diagnostics: Set(serde_json::json!([])),
        review_required: Set(None),
        requested_by_actor_kind: Set(actor_kind(registration.context).to_string()),
        requested_by_actor_id: Set(registration.context.actor.id.clone()),
        idempotency_key: Set(idempotency_key.clone()),
        created_at: Set(now),
        updated_at: Set(now),
    })
    .on_conflict(
        OnConflict::columns([
            machine_operation::Column::TenantId,
            machine_operation::Column::IdempotencyKey,
        ])
        .do_nothing()
        .to_owned(),
    )
    .exec_without_returning(&transaction)
    .await?;
    let persisted =
        find_operation_by_idempotency(&transaction, registration.tenant_id, &idempotency_key)
            .await?
            .ok_or(TranslationError::WorkflowRevisionConflict)?;
    let created = persisted.id == id;
    if created {
        insert_memory_bindings(
            &transaction,
            registration.tenant_id,
            persisted.id,
            &registration.request.memory_suggestions,
            now,
        )
        .await?;
    }
    transaction.commit().await?;
    Ok((persisted, created))
}

pub(crate) async fn insert_memory_bindings<C>(
    database: &C,
    tenant_id: Uuid,
    operation_id: Uuid,
    suggestions: &[MachineTranslationMemorySuggestion],
    created_at: DateTime<FixedOffset>,
) -> TranslationResult<()>
where
    C: ConnectionTrait,
{
    if suggestions.is_empty() {
        return Ok(());
    }
    let mut unit_ordinals = BTreeMap::<&str, i16>::new();
    let mut models = Vec::with_capacity(suggestions.len());
    for (batch_ordinal, suggestion) in suggestions.iter().enumerate() {
        let unit_ordinal = unit_ordinals
            .entry(suggestion.unit_id.as_str())
            .or_insert(0);
        let model = machine_memory_binding::ActiveModel {
            id: Set(generate_id()),
            tenant_id: Set(tenant_id),
            operation_id: Set(operation_id),
            unit_id: Set(suggestion.unit_id.clone()),
            batch_ordinal: Set(i16::try_from(batch_ordinal)
                .map_err(|_| TranslationError::MachineMemoryProjectionUnavailable)?),
            unit_ordinal: Set(*unit_ordinal),
            memory_entry_id: Set(Uuid::parse_str(&suggestion.entry_id)
                .map_err(|_| TranslationError::MachineMemoryProjectionUnavailable)?),
            score_basis_points: Set(i32::from(suggestion.score_basis_points)),
            created_at: Set(created_at),
        };
        *unit_ordinal = unit_ordinal
            .checked_add(1)
            .ok_or(TranslationError::MachineMemoryProjectionUnavailable)?;
        models.push(model);
    }
    machine_memory_binding::Entity::insert_many(models)
        .exec_without_returning(database)
        .await?;
    Ok(())
}

pub(crate) async fn begin_machine_proposal_save(
    database: &DatabaseConnection,
    tenant_id: Uuid,
    operation_id: Uuid,
) -> TranslationResult<Option<MachineProposalRecord>> {
    machine_operation::Entity::update_many()
        .col_expr(machine_operation::Column::Status, Expr::value("saving"))
        .col_expr(
            machine_operation::Column::UpdatedAt,
            Expr::value(Utc::now().fixed_offset()),
        )
        .filter(machine_operation::Column::TenantId.eq(tenant_id))
        .filter(machine_operation::Column::Id.eq(operation_id))
        .filter(machine_operation::Column::Status.eq("registered"))
        .exec(database)
        .await?;
    let operation = find_operation(database, tenant_id, operation_id).await?;
    match operation.status.as_str() {
        "saving" => Ok(None),
        "completed" => machine_proposal_record(operation).map(Some),
        "cancelled" => Err(TranslationError::MachineOperationCancelled),
        _ => Err(TranslationError::WorkflowRevisionConflict),
    }
}

pub(crate) async fn complete_operation(
    database: &DatabaseConnection,
    tenant_id: Uuid,
    operation_id: Uuid,
    proposal_id: Uuid,
    result: &MachineTranslationBatchResult,
) -> TranslationResult<MachineProposalRecord> {
    let diagnostics = result
        .units
        .iter()
        .flat_map(|unit| {
            unit.diagnostics
                .iter()
                .map(|diagnostic| MachineDiagnosticEvidence {
                    code: diagnostic.code.clone(),
                    blocking: diagnostic.blocking,
                    unit_id: diagnostic.unit_id.clone(),
                })
        })
        .collect::<Vec<_>>();
    let transaction = database.begin().await?;
    let update = machine_operation::Entity::update_many()
        .col_expr(machine_operation::Column::Status, Expr::value("completed"))
        .col_expr(
            machine_operation::Column::ProposalId,
            Expr::value(Some(proposal_id)),
        )
        .col_expr(
            machine_operation::Column::ProviderSlug,
            Expr::value(Some(result.provider_slug.clone())),
        )
        .col_expr(
            machine_operation::Column::ExecutionId,
            Expr::value(Some(result.execution.execution_id.clone())),
        )
        .col_expr(
            machine_operation::Column::ExecutionRequestDigest,
            Expr::value(Some(result.execution.request_digest.clone())),
        )
        .col_expr(
            machine_operation::Column::PromptPolicyDigest,
            Expr::value(Some(result.execution.prompt_policy_digest.clone())),
        )
        .col_expr(
            machine_operation::Column::Attempts,
            Expr::value(serde_json::to_value(&result.execution.attempts)?),
        )
        .col_expr(
            machine_operation::Column::Usage,
            Expr::value(Some(serde_json::to_value(&result.execution.usage)?)),
        )
        .col_expr(
            machine_operation::Column::Diagnostics,
            Expr::value(serde_json::to_value(diagnostics)?),
        )
        .col_expr(
            machine_operation::Column::ReviewRequired,
            Expr::value(Some(result.review_required)),
        )
        .col_expr(
            machine_operation::Column::UpdatedAt,
            Expr::value(Utc::now().fixed_offset()),
        )
        .filter(machine_operation::Column::TenantId.eq(tenant_id))
        .filter(machine_operation::Column::Id.eq(operation_id))
        .filter(machine_operation::Column::Status.eq("saving"))
        .exec(&transaction)
        .await?;
    if update.rows_affected != 1 {
        transaction.rollback().await?;
        let operation = find_operation(database, tenant_id, operation_id).await?;
        return match operation.status.as_str() {
            "completed" => machine_proposal_record(operation),
            "cancelled" => Err(TranslationError::MachineOperationCancelled),
            _ => Err(TranslationError::WorkflowRevisionConflict),
        };
    }
    machine_memory_binding::Entity::delete_many()
        .filter(machine_memory_binding::Column::TenantId.eq(tenant_id))
        .filter(machine_memory_binding::Column::OperationId.eq(operation_id))
        .exec(&transaction)
        .await?;
    transaction.commit().await?;
    machine_proposal_record(
        machine_operation::Entity::find_by_id(operation_id)
            .filter(machine_operation::Column::TenantId.eq(tenant_id))
            .one(database)
            .await?
            .ok_or(TranslationError::WorkflowRevisionConflict)?,
    )
}

pub(crate) fn validate_machine_result(
    request: &MachineTranslationBatchRequest,
    result: &MachineTranslationBatchResult,
) -> TranslationResult<()> {
    if result.units.len() != request.units.len()
        || result.execution.execution_id.trim().is_empty()
        || !is_digest(&result.execution.request_digest)
        || !is_digest(&result.execution.prompt_policy_digest)
        || result.execution.prompt_policy_digest != request.adapter_policy_digest
        || result.provider_slug.trim().is_empty()
        || result.provider_slug.len() > 191
        || !result.review_required
        || result.execution.attempts.is_empty()
        || result.execution.attempts.len() > 16
        || result.execution.attempts.iter().any(|attempt| {
            attempt.attempt == 0
                || attempt.provider_profile_id.trim().is_empty()
                || attempt.provider_profile_id.len() > 256
                || attempt.provider_slug.trim().is_empty()
                || attempt.provider_slug.len() > 191
                || attempt.model.trim().is_empty()
                || attempt.model.len() > 256
        })
        || result.execution.usage.total_tokens
            != result
                .execution
                .usage
                .input_tokens
                .saturating_add(result.execution.usage.output_tokens)
        || result.execution.usage.currency_code.trim().is_empty()
        || result.execution.usage.currency_code.len() > 16
        || !is_digest(&result.execution.usage.price_snapshot_digest)
    {
        return Err(TranslationError::InvalidMachineTranslationResult);
    }
    let expected = request
        .units
        .iter()
        .map(|unit| (unit.unit_id.as_str(), unit))
        .collect::<BTreeMap<_, _>>();
    let mut actual = BTreeSet::new();
    for unit in &result.units {
        let Some(source) = expected.get(unit.unit_id.as_str()) else {
            return Err(TranslationError::InvalidMachineTranslationResult);
        };
        if !actual.insert(unit.unit_id.as_str())
            || unit.translated_value.is_empty()
            || source
                .max_characters
                .is_some_and(|max| unit.translated_value.chars().count() > max as usize)
            || !protected_token_ledger_matches(&source.protected_tokens, &unit.protected_tokens)
            || !protected_token_multiplicities_match(
                &source.source_value,
                &unit.translated_value,
                &source.protected_tokens,
            )
            || (source.preserves_whitespace
                && !whitespace_shape_matches(&source.source_value, &unit.translated_value))
            || unit.diagnostics.len() > 64
            || unit.diagnostics.iter().any(|diagnostic| {
                diagnostic.code.trim().is_empty()
                    || diagnostic.code.len() > 128
                    || diagnostic
                        .unit_id
                        .as_ref()
                        .is_some_and(|id| id != &unit.unit_id)
            })
        {
            return Err(TranslationError::InvalidMachineTranslationResult);
        }
    }
    Ok(())
}

pub(crate) fn validate_provider_compatibility(
    request: &MachineTranslationBatchRequest,
    descriptor: &MachineTranslationProviderDescriptor,
) -> TranslationResult<()> {
    let character_count = request
        .units
        .iter()
        .map(|unit| unit.source_value.chars().count())
        .sum::<usize>();
    if descriptor.slug.trim().is_empty()
        || descriptor.slug.len() > 191
        || descriptor.policy_digest != request.adapter_policy_digest
        || request.units.len() > usize::from(descriptor.max_batch_units)
        || character_count > descriptor.max_batch_characters as usize
        || request.units.iter().any(|unit| {
            !descriptor.supported_profiles.contains(&unit.profile)
                || !descriptor
                    .supported_classifications
                    .contains(&unit.classification)
        })
    {
        return Err(TranslationError::Provider {
            code: "translation.machine.provider_incompatible".to_string(),
            message: "machine translation provider cannot execute the requested batch".to_string(),
            retryable: false,
        });
    }
    Ok(())
}

pub(crate) fn machine_proposal_record(
    model: machine_operation::Model,
) -> TranslationResult<MachineProposalRecord> {
    if model.status != "completed" {
        return Err(TranslationError::WorkflowRevisionConflict);
    }
    Ok(MachineProposalRecord {
        operation_id: model.id,
        item_id: model.item_id,
        proposal_id: model
            .proposal_id
            .ok_or(TranslationError::InvalidMachineTranslationResult)?,
        adapter_slug: model.adapter_slug,
        provider_slug: model
            .provider_slug
            .ok_or(TranslationError::InvalidMachineTranslationResult)?,
        provider_policy_digest: model.provider_policy_digest,
        machine_request_digest: model.machine_request_digest,
        glossary_revision: model.glossary_revision,
        glossary_digest: model.glossary_digest,
        memory_digest: model.memory_digest,
        execution_id: model
            .execution_id
            .ok_or(TranslationError::InvalidMachineTranslationResult)?,
        execution_request_digest: model
            .execution_request_digest
            .ok_or(TranslationError::InvalidMachineTranslationResult)?,
        prompt_policy_digest: model
            .prompt_policy_digest
            .ok_or(TranslationError::InvalidMachineTranslationResult)?,
        attempts: serde_json::from_value(model.attempts)?,
        usage: serde_json::from_value(
            model
                .usage
                .ok_or(TranslationError::InvalidMachineTranslationResult)?,
        )?,
        diagnostics: serde_json::from_value(model.diagnostics)?,
        review_required: model
            .review_required
            .ok_or(TranslationError::InvalidMachineTranslationResult)?,
        created_at: model.created_at,
        updated_at: model.updated_at,
    })
}

pub(crate) fn machine_proposal_outcome(
    model: machine_operation::Model,
    evidence: MachineTranslationExecutionStatusEvidence,
) -> TranslationResult<MachineProposalOutcome> {
    match model.status.as_str() {
        "completed" => machine_proposal_record(model)
            .map(Box::new)
            .map(MachineProposalOutcome::Completed),
        "registered" | "saving" => {
            let provider_execution_id = evidence.execution_id;
            let provider_status = machine_execution_status(evidence.status).to_string();
            Ok(MachineProposalOutcome::InProgress(
                MachineOperationStatusRecord {
                    operation_id: model.id,
                    item_id: model.item_id,
                    status: model.status,
                    provider_execution_id,
                    provider_status,
                    provider_error_code: None,
                    updated_at: model.updated_at,
                },
            ))
        }
        "cancelled" => Err(TranslationError::MachineOperationCancelled),
        _ => Err(TranslationError::WorkflowRevisionConflict),
    }
}
