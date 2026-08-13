use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
};

use chrono::{DateTime, FixedOffset, Utc};
use rustok_api::{
    Action, PortActorKind, PortCallPolicy, PortContext, Resource, manifest_hash::hash_manifest,
};
use rustok_core::{PermissionScope, SecurityContext, generate_id};
use rustok_outbox::TransactionalEventBus;
use rustok_tenant::TenantLocalePolicyPort;
use rustok_translation_targets::{
    FieldKey, TranslationResourceSnapshot, TranslationTargetRegistry,
    protected_token_ledger_matches, protected_token_multiplicities_match, whitespace_shape_matches,
};
use sea_orm::{
    ColumnTrait, ConnectionTrait, DatabaseConnection, EntityTrait, QueryFilter, QueryOrder, Set,
    TransactionTrait,
    sea_query::{Expr, OnConflict},
};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::{
    GlossaryBinding, GlossaryTermPolicy, MachineTranslationAttemptEvidence,
    MachineTranslationBatchRequest, MachineTranslationBatchResult, MachineTranslationEstimate,
    MachineTranslationExecutionStatus, MachineTranslationGlossaryTerm,
    MachineTranslationMemorySuggestion, MachineTranslationPort, MachineTranslationProviderState,
    MachineTranslationResourceContext, MachineTranslationUnit, MachineTranslationUsage,
    MemoryLookupInput, ProposalOrigin, ProposalValue, SaveProposalInput, TranslationError,
    TranslationMemoryService, TranslationResult, TranslationWorkflowService,
    entities::{
        job, job_item, machine_cancellation, machine_memory_binding, machine_operation,
        machine_recovery, memory_entry,
    },
    glossary::read_bound_glossary,
    qa::{glossary_concept_matches, glossary_scope_matches},
};

const MEMORY_SUGGESTIONS_PER_UNIT: u16 = 5;

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct GenerateMachineProposalInput {
    pub item_id: Uuid,
    pub field_keys: Vec<FieldKey>,
    pub minimum_memory_similarity_basis_points: u16,
    pub tone: Option<String>,
    pub domain: Option<String>,
    pub style: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CancelMachineOperationInput {
    pub operation_id: Uuid,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RecoverMachineOperationInput {
    pub operation_id: Uuid,
    pub expected_updated_at: DateTime<FixedOffset>,
    pub proposal: GenerateMachineProposalInput,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineCancellationRecord {
    pub cancellation_id: Uuid,
    pub operation_id: Uuid,
    pub status: String,
    pub provider_execution_id: Option<String>,
    pub provider_status: String,
    pub provider_error_code: Option<String>,
    pub provider_observed_at: DateTime<FixedOffset>,
    pub created_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineOperationStatusRecord {
    pub operation_id: Uuid,
    pub item_id: Uuid,
    pub status: String,
    pub provider_execution_id: Option<String>,
    pub provider_status: String,
    pub provider_error_code: Option<String>,
    pub updated_at: DateTime<FixedOffset>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MachineDiagnosticEvidence {
    pub code: String,
    pub blocking: bool,
    pub unit_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MachineProposalRecord {
    pub operation_id: Uuid,
    pub item_id: Uuid,
    pub proposal_id: Uuid,
    pub adapter_slug: String,
    pub provider_slug: String,
    pub provider_policy_digest: String,
    pub machine_request_digest: String,
    pub glossary_revision: Option<String>,
    pub glossary_digest: Option<String>,
    pub memory_digest: Option<String>,
    pub execution_id: String,
    pub execution_request_digest: String,
    pub prompt_policy_digest: String,
    pub attempts: Vec<MachineTranslationAttemptEvidence>,
    pub usage: MachineTranslationUsage,
    pub diagnostics: Vec<MachineDiagnosticEvidence>,
    pub review_required: bool,
    pub created_at: DateTime<FixedOffset>,
    pub updated_at: DateTime<FixedOffset>,
}

pub struct TranslationMachineService {
    database: DatabaseConnection,
    workflow: TranslationWorkflowService,
    memory: TranslationMemoryService,
    machine_port: Arc<dyn MachineTranslationPort>,
}

pub struct TranslationMachineControlService {
    database: DatabaseConnection,
    machine_port: Option<Arc<dyn MachineTranslationPort>>,
}

impl TranslationMachineControlService {
    pub fn new(
        database: DatabaseConnection,
        machine_port: Option<Arc<dyn MachineTranslationPort>>,
    ) -> Self {
        Self {
            database,
            machine_port,
        }
    }

    pub async fn cancel_operation(
        &self,
        context: PortContext,
        input: CancelMachineOperationInput,
    ) -> TranslationResult<MachineCancellationRecord> {
        cancel_machine_operation(&self.database, self.machine_port.as_deref(), context, input).await
    }

    pub async fn operation_status(
        &self,
        context: PortContext,
        operation_id: Uuid,
    ) -> TranslationResult<MachineOperationStatusRecord> {
        read_machine_operation_status(
            &self.database,
            self.machine_port.as_deref(),
            context,
            operation_id,
        )
        .await
    }
}

impl TranslationMachineService {
    pub fn new(
        database: DatabaseConnection,
        providers: Arc<TranslationTargetRegistry>,
        tenant_locale_policies: Arc<dyn TenantLocalePolicyPort>,
        event_bus: TransactionalEventBus,
        machine_port: Arc<dyn MachineTranslationPort>,
    ) -> Self {
        Self {
            workflow: TranslationWorkflowService::new(
                database.clone(),
                providers,
                tenant_locale_policies,
                event_bus,
            ),
            memory: TranslationMemoryService::new(database.clone()),
            database,
            machine_port,
        }
    }

    pub async fn generate_proposal(
        &self,
        context: PortContext,
        input: GenerateMachineProposalInput,
    ) -> TranslationResult<MachineProposalRecord> {
        let tenant_id = authorize_machine_generation(&context)?;
        validate_generation_input(&input)?;
        let idempotency_key = context.idempotency_key.clone().unwrap_or_default();
        let command_hash = hash_manifest(&input)?;

        let existing_operation =
            find_operation_by_idempotency(&self.database, tenant_id, &idempotency_key).await?;
        if let Some(existing) = existing_operation.as_ref() {
            validate_operation_replay(existing, &context, &command_hash)?;
            if existing.status == "completed" {
                return machine_proposal_record(existing.clone());
            }
            if existing.status == "cancelled" {
                return Err(TranslationError::MachineOperationCancelled);
            }
        }

        let item = find_item(&self.database, tenant_id, input.item_id).await?;
        enforce_machine_assignment(&item, &context)?;
        if !matches!(
            item.status.as_str(),
            "missing" | "draft" | "stale" | "conflict"
        ) {
            return Err(TranslationError::ItemNotWritable(item.status));
        }
        let snapshot: TranslationResourceSnapshot =
            serde_json::from_value(item.source_snapshot.clone())?;
        let mut request = self
            .build_request(
                tenant_id,
                &item,
                &snapshot,
                &input,
                existing_operation.as_ref(),
            )
            .await?;
        request.validate(&context)?;
        validate_provider_compatibility(&request, self.machine_port.descriptor())?;
        let mut machine_request_digest = hash_manifest(&request)?;
        let descriptor = self.machine_port.descriptor();

        let (operation, created) = register_operation(
            &self.database,
            RegisterMachineOperation {
                tenant_id,
                context: &context,
                input: &input,
                command_hash: &command_hash,
                machine_request_digest: &machine_request_digest,
                request: &request,
                adapter_slug: descriptor.slug.as_str(),
                provider_policy_digest: descriptor.policy_digest.as_str(),
            },
        )
        .await?;
        validate_operation_replay(&operation, &context, &command_hash)?;
        if !created && operation.machine_request_digest != machine_request_digest {
            request = self
                .build_request(tenant_id, &item, &snapshot, &input, Some(&operation))
                .await?;
            request.validate(&context)?;
            validate_provider_compatibility(&request, self.machine_port.descriptor())?;
            machine_request_digest = hash_manifest(&request)?;
        }
        if operation.machine_request_digest != machine_request_digest {
            return Err(TranslationError::IdempotencyConflict);
        }
        if operation.status == "completed" {
            return machine_proposal_record(operation);
        }
        if operation.status == "cancelled" {
            return Err(TranslationError::MachineOperationCancelled);
        }

        if created {
            let health = self.machine_port.health(context.clone()).await?;
            if health.state == MachineTranslationProviderState::Unavailable {
                return Err(TranslationError::Provider {
                    code: health
                        .reason_code
                        .unwrap_or_else(|| "translation.machine.provider_unavailable".to_string()),
                    message: "machine translation provider is unavailable".to_string(),
                    retryable: true,
                });
            }
        }
        let current = find_operation(&self.database, tenant_id, operation.id).await?;
        match current.status.as_str() {
            "registered" | "saving" => {}
            "completed" => return machine_proposal_record(current),
            "cancelled" => return Err(TranslationError::MachineOperationCancelled),
            _ => return Err(TranslationError::WorkflowRevisionConflict),
        }
        let machine_context = child_write_context(&context, "machine-port")?;
        let result = self
            .machine_port
            .translate_batch(machine_context, request.clone())
            .await?;
        validate_machine_result(&request, &result)?;
        if let Some(completed) =
            begin_machine_proposal_save(&self.database, tenant_id, operation.id).await?
        {
            return Ok(completed);
        }

        let values = result
            .units
            .iter()
            .map(|unit| {
                Ok(ProposalValue {
                    key: FieldKey::new(unit.unit_id.clone()).map_err(|error| {
                        TranslationError::InvalidRequest(format!(
                            "machine translation returned an invalid field key: {error}"
                        ))
                    })?,
                    value: unit.translated_value.clone(),
                })
            })
            .collect::<TranslationResult<Vec<_>>>()?;
        let proposal = self
            .workflow
            .save_proposal(
                child_write_context(&context, "save-proposal")?,
                SaveProposalInput {
                    item_id: input.item_id,
                    origin: ProposalOrigin::Ai,
                    values,
                },
            )
            .await?;

        complete_operation(
            &self.database,
            tenant_id,
            operation.id,
            proposal.id,
            &result,
        )
        .await
    }

    pub async fn estimate_proposal(
        &self,
        context: PortContext,
        input: GenerateMachineProposalInput,
    ) -> TranslationResult<MachineTranslationEstimate> {
        let tenant_id = authorize_machine_generation(&context)?;
        validate_generation_input(&input)?;
        let item = find_item(&self.database, tenant_id, input.item_id).await?;
        enforce_machine_assignment(&item, &context)?;
        if !matches!(
            item.status.as_str(),
            "missing" | "draft" | "stale" | "conflict"
        ) {
            return Err(TranslationError::ItemNotWritable(item.status));
        }
        let snapshot: TranslationResourceSnapshot =
            serde_json::from_value(item.source_snapshot.clone())?;
        let request = self
            .build_request(tenant_id, &item, &snapshot, &input, None)
            .await?;
        request.validate(&context)?;
        validate_provider_compatibility(&request, self.machine_port.descriptor())?;
        self.machine_port
            .estimate_batch(child_write_context(&context, "machine-estimate")?, request)
            .await
            .map_err(Into::into)
    }

    pub async fn recover_operation(
        &self,
        context: PortContext,
        input: RecoverMachineOperationInput,
    ) -> TranslationResult<MachineProposalRecord> {
        let tenant_id = authorize_machine_recovery(&context)?;
        validate_machine_recovery_reason(&input.reason)?;
        validate_generation_input(&input.proposal)?;
        let idempotency_key = context.idempotency_key.clone().unwrap_or_default();
        let request_hash = hash_manifest(&input)?;

        if let Some(existing) =
            find_machine_recovery_by_idempotency(&self.database, tenant_id, &idempotency_key)
                .await?
        {
            validate_machine_recovery_replay(&existing, &context, &request_hash)?;
            return self
                .resume_machine_recovery(context, input, existing.operation_id)
                .await;
        }

        let operation = find_operation(&self.database, tenant_id, input.operation_id).await?;
        if operation.status == "completed" {
            return machine_proposal_record(operation);
        }
        if operation.status != "saving" {
            return Err(TranslationError::MachineOperationTerminal(operation.status));
        }
        if operation.updated_at != input.expected_updated_at {
            return Err(TranslationError::MachineRecoveryRevisionMismatch);
        }
        validate_recovery_proposal_input(&operation, &input.proposal)?;
        self.rebuild_recovery_request(context.clone(), tenant_id, &operation, &input.proposal)
            .await?;

        let recovery_id = generate_id();
        let now = Utc::now().fixed_offset();
        let transaction = self.database.begin().await?;
        machine_recovery::Entity::insert(machine_recovery::ActiveModel {
            id: Set(recovery_id),
            tenant_id: Set(tenant_id),
            operation_id: Set(operation.id),
            idempotency_key: Set(idempotency_key.clone()),
            request_hash: Set(request_hash.clone()),
            requested_by_actor_kind: Set(actor_kind(&context).to_string()),
            requested_by_actor_id: Set(context.actor.id.clone()),
            reason: Set(input.reason.clone()),
            observed_updated_at: Set(operation.updated_at),
            created_at: Set(now),
        })
        .on_conflict(OnConflict::new().do_nothing().to_owned())
        .exec_without_returning(&transaction)
        .await?;
        let persisted =
            find_machine_recovery_by_idempotency(&transaction, tenant_id, &idempotency_key).await?;
        let Some(persisted) = persisted else {
            transaction.rollback().await?;
            return Err(TranslationError::MachineRecoveryAlreadyRequested);
        };
        if persisted.id != recovery_id {
            transaction.rollback().await?;
            validate_machine_recovery_replay(&persisted, &context, &request_hash)?;
            return self
                .resume_machine_recovery(context, input, persisted.operation_id)
                .await;
        }
        transaction.commit().await?;
        self.resume_machine_recovery(context, input, operation.id)
            .await
    }

    async fn resume_machine_recovery(
        &self,
        context: PortContext,
        input: RecoverMachineOperationInput,
        operation_id: Uuid,
    ) -> TranslationResult<MachineProposalRecord> {
        let tenant_id =
            Uuid::parse_str(&context.tenant_id).map_err(|_| TranslationError::InvalidTenantId)?;
        let operation = find_operation(&self.database, tenant_id, operation_id).await?;
        if operation.status == "completed" {
            return machine_proposal_record(operation);
        }
        if operation.status != "saving" {
            return Err(TranslationError::MachineOperationTerminal(operation.status));
        }
        validate_recovery_proposal_input(&operation, &input.proposal)?;
        let request = self
            .rebuild_recovery_request(context.clone(), tenant_id, &operation, &input.proposal)
            .await?;
        let execution_idempotency_key =
            child_idempotency_key(&operation.idempotency_key, "machine-port")?;
        let result = self
            .machine_port
            .recover_batch(context.clone(), execution_idempotency_key, request.clone())
            .await?
            .ok_or(TranslationError::MachineRecoveryResultUnavailable)?;
        validate_machine_result(&request, &result)?;

        let values = result
            .units
            .iter()
            .map(|unit| {
                Ok(ProposalValue {
                    key: FieldKey::new(unit.unit_id.clone()).map_err(|error| {
                        TranslationError::InvalidRequest(format!(
                            "machine translation returned an invalid field key: {error}"
                        ))
                    })?,
                    value: unit.translated_value.clone(),
                })
            })
            .collect::<TranslationResult<Vec<_>>>()?;
        let mut save_context = context;
        save_context.idempotency_key = Some(child_idempotency_key(
            &operation.idempotency_key,
            "save-proposal",
        )?);
        let proposal = self
            .workflow
            .save_recovered_machine_proposal(
                save_context,
                SaveProposalInput {
                    item_id: input.proposal.item_id,
                    origin: ProposalOrigin::Ai,
                    values,
                },
            )
            .await?;
        complete_operation(
            &self.database,
            tenant_id,
            operation.id,
            proposal.id,
            &result,
        )
        .await
    }

    async fn rebuild_recovery_request(
        &self,
        context: PortContext,
        tenant_id: Uuid,
        operation: &machine_operation::Model,
        input: &GenerateMachineProposalInput,
    ) -> TranslationResult<MachineTranslationBatchRequest> {
        let item = find_item(&self.database, tenant_id, input.item_id).await?;
        let snapshot: TranslationResourceSnapshot =
            serde_json::from_value(item.source_snapshot.clone())?;
        let request = self
            .build_request(tenant_id, &item, &snapshot, input, Some(operation))
            .await?;
        request.validate(&context)?;
        validate_provider_compatibility(&request, self.machine_port.descriptor())?;
        if hash_manifest(&request)? != operation.machine_request_digest {
            return Err(TranslationError::IdempotencyConflict);
        }
        Ok(request)
    }

    async fn build_request(
        &self,
        tenant_id: Uuid,
        item: &job_item::Model,
        snapshot: &TranslationResourceSnapshot,
        input: &GenerateMachineProposalInput,
        existing_operation: Option<&machine_operation::Model>,
    ) -> TranslationResult<MachineTranslationBatchRequest> {
        let selected = input.field_keys.iter().collect::<BTreeSet<_>>();
        let units = snapshot
            .fields
            .iter()
            .filter(|field| selected.contains(&field.descriptor.key))
            .map(|field| MachineTranslationUnit {
                unit_id: field.descriptor.key.as_str().to_string(),
                field_key: field.descriptor.key.as_str().to_string(),
                source_value: field.source_value.clone(),
                source_hash: field.source_hash.clone(),
                source_revision: snapshot.source_revision.as_str().to_string(),
                profile: field.descriptor.profile,
                strategy: field.descriptor.strategy,
                classification: field.descriptor.classification,
                ai_export_allowed: field.descriptor.ai_export_allowed,
                max_characters: field.descriptor.max_characters,
                preserves_whitespace: field.descriptor.preserves_whitespace,
                protected_tokens: field.protected_tokens.clone(),
            })
            .collect::<Vec<_>>();
        if units.len() != input.field_keys.len() {
            return Err(TranslationError::InvalidRequest(
                "machine translation field selection contains an unknown field".to_string(),
            ));
        }

        let job = find_job(&self.database, tenant_id, item.job_id).await?;
        let (glossary_revision, glossary_digest, glossary_terms) =
            project_glossary(&self.database, tenant_id, &job, snapshot, &selected).await?;

        let memory_suggestions = if let Some(operation) = existing_operation {
            read_pinned_memory_suggestions(&self.database, tenant_id, operation, snapshot, &units)
                .await?
        } else {
            lookup_memory_suggestions(
                &self.memory,
                tenant_id,
                snapshot,
                &units,
                input.minimum_memory_similarity_basis_points,
            )
            .await?
        };
        let memory_digest = (!memory_suggestions.is_empty())
            .then(|| hash_manifest(&memory_suggestions))
            .transpose()?;
        let descriptor = self.machine_port.descriptor();

        Ok(MachineTranslationBatchRequest {
            source_locale: snapshot.source_locale.clone(),
            target_locale: snapshot.target_locale.clone(),
            resource: MachineTranslationResourceContext {
                owner_slug: snapshot.summary.identity.owner_slug.as_str().to_string(),
                resource_kind: snapshot.summary.identity.resource_kind.as_str().to_string(),
                resource_id: snapshot.summary.identity.resource_id.as_str().to_string(),
                subresource_id: snapshot
                    .summary
                    .identity
                    .subresource_id
                    .as_ref()
                    .map(|value| value.as_str().to_string()),
            },
            units,
            glossary_revision,
            glossary_digest,
            glossary_terms,
            memory_digest,
            memory_suggestions,
            tone: input.tone.clone(),
            domain: input.domain.clone(),
            style: input.style.clone(),
            adapter_policy_digest: descriptor.policy_digest.clone(),
            evidence: [
                ("item_id".to_string(), item.id.to_string()),
                ("job_id".to_string(), item.job_id.to_string()),
                ("source_digest".to_string(), item.source_digest.clone()),
            ]
            .into_iter()
            .collect(),
        })
    }
}

async fn lookup_memory_suggestions(
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

async fn read_pinned_memory_suggestions(
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

async fn project_glossary(
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

fn validate_generation_input(input: &GenerateMachineProposalInput) -> TranslationResult<()> {
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

fn authorize_machine_generation(context: &PortContext) -> TranslationResult<Uuid> {
    context.require_policy(PortCallPolicy::write())?;
    let security = SecurityContext::try_from_port_context(context)?;
    for action in [Action::Run, Action::Update] {
        if security.get_scope(Resource::Translations, action) == PermissionScope::None {
            return Err(TranslationError::Forbidden);
        }
    }
    Uuid::parse_str(&context.tenant_id).map_err(|_| TranslationError::InvalidTenantId)
}

fn authorize_machine_recovery(context: &PortContext) -> TranslationResult<Uuid> {
    context.require_policy(PortCallPolicy::write())?;
    let security = SecurityContext::try_from_port_context(context)?;
    for action in [Action::Manage, Action::Update] {
        if security.get_scope(Resource::Translations, action) == PermissionScope::None {
            return Err(TranslationError::Forbidden);
        }
    }
    Uuid::parse_str(&context.tenant_id).map_err(|_| TranslationError::InvalidTenantId)
}

fn enforce_machine_assignment(
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

fn actor_kind(context: &PortContext) -> &'static str {
    match context.actor.kind {
        PortActorKind::User => "user",
        PortActorKind::Service => "service",
        PortActorKind::System => "system",
    }
}

fn child_write_context(context: &PortContext, operation: &str) -> TranslationResult<PortContext> {
    let parent_key = context.idempotency_key.as_deref().unwrap_or_default();
    let idempotency_key = child_idempotency_key(parent_key, operation)?;
    let mut child = context.clone();
    child.causation_id = Some(context.correlation_id.clone());
    child.idempotency_key = Some(idempotency_key);
    Ok(child)
}

fn child_idempotency_key(parent_key: &str, operation: &str) -> TranslationResult<String> {
    let digest = hash_manifest(&(parent_key, operation))?;
    Ok(format!("translation-machine:{operation}:{digest}"))
}

struct RegisterMachineOperation<'a> {
    tenant_id: Uuid,
    context: &'a PortContext,
    input: &'a GenerateMachineProposalInput,
    command_hash: &'a str,
    machine_request_digest: &'a str,
    request: &'a MachineTranslationBatchRequest,
    adapter_slug: &'a str,
    provider_policy_digest: &'a str,
}

async fn register_operation(
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

async fn insert_memory_bindings<C>(
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

async fn begin_machine_proposal_save(
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

async fn complete_operation(
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

async fn cancel_machine_operation(
    database: &DatabaseConnection,
    machine_port: Option<&dyn MachineTranslationPort>,
    context: PortContext,
    input: CancelMachineOperationInput,
) -> TranslationResult<MachineCancellationRecord> {
    context.require_policy(PortCallPolicy::write())?;
    validate_machine_cancellation_reason(&input.reason)?;
    let tenant_id =
        Uuid::parse_str(&context.tenant_id).map_err(|_| TranslationError::InvalidTenantId)?;
    let security = SecurityContext::try_from_port_context(&context)?;
    if security.get_scope(Resource::Translations, Action::Run) == PermissionScope::None {
        return Err(TranslationError::Forbidden);
    }
    let request_hash = hash_manifest(&input)?;
    let idempotency_key = context.idempotency_key.clone().unwrap_or_default();
    if let Some(existing) =
        find_cancellation_by_idempotency(database, tenant_id, &idempotency_key).await?
    {
        validate_machine_cancellation_replay(&existing, &context, &request_hash)?;
        return refresh_machine_cancellation_provider_evidence(
            database,
            machine_port,
            &context,
            existing,
        )
        .await;
    }

    let operation = find_operation(database, tenant_id, input.operation_id).await?;
    let requested_by_owner = operation.requested_by_actor_kind == actor_kind(&context)
        && operation.requested_by_actor_id == context.actor.id;
    if !requested_by_owner
        && security.get_scope(Resource::Translations, Action::Manage) == PermissionScope::None
    {
        return Err(TranslationError::Forbidden);
    }
    if operation.status == "cancelled" {
        let existing = find_cancellation_by_operation(database, tenant_id, operation.id)
            .await?
            .ok_or(TranslationError::WorkflowRevisionConflict)?;
        return replay_machine_cancellation(existing, &context, &request_hash);
    }
    if operation.status != "registered" {
        return Err(TranslationError::MachineOperationTerminal(operation.status));
    }
    let provider_evidence =
        propagate_machine_cancellation(machine_port, &context, &operation).await;

    let transaction = database.begin().await?;
    let cancellation_id = generate_id();
    let now = Utc::now().fixed_offset();
    machine_cancellation::Entity::insert(machine_cancellation::ActiveModel {
        id: Set(cancellation_id),
        tenant_id: Set(tenant_id),
        operation_id: Set(operation.id),
        reason: Set(input.reason),
        requested_by_actor_kind: Set(actor_kind(&context).to_string()),
        requested_by_actor_id: Set(context.actor.id.clone()),
        idempotency_key: Set(idempotency_key.clone()),
        request_hash: Set(request_hash.clone()),
        provider_execution_id: Set(provider_evidence.execution_id),
        provider_status: Set(provider_evidence.status),
        provider_error_code: Set(provider_evidence.error_code),
        provider_observed_at: Set(provider_evidence.observed_at),
        created_at: Set(now),
    })
    .on_conflict(OnConflict::new().do_nothing().to_owned())
    .exec_without_returning(&transaction)
    .await?;
    let persisted = if let Some(cancellation) =
        find_cancellation_by_idempotency(&transaction, tenant_id, &idempotency_key).await?
    {
        cancellation
    } else {
        transaction.rollback().await?;
        let existing = find_cancellation_by_operation(database, tenant_id, operation.id)
            .await?
            .ok_or(TranslationError::WorkflowRevisionConflict)?;
        return replay_machine_cancellation(existing, &context, &request_hash);
    };
    validate_machine_cancellation_replay(&persisted, &context, &request_hash)?;
    let update = machine_operation::Entity::update_many()
        .col_expr(machine_operation::Column::Status, Expr::value("cancelled"))
        .col_expr(machine_operation::Column::UpdatedAt, Expr::value(now))
        .filter(machine_operation::Column::TenantId.eq(tenant_id))
        .filter(machine_operation::Column::Id.eq(operation.id))
        .filter(machine_operation::Column::Status.eq("registered"))
        .exec(&transaction)
        .await?;
    if update.rows_affected != 1 {
        transaction.rollback().await?;
        let current = find_operation(database, tenant_id, operation.id).await?;
        return Err(TranslationError::MachineOperationTerminal(current.status));
    }
    machine_memory_binding::Entity::delete_many()
        .filter(machine_memory_binding::Column::TenantId.eq(tenant_id))
        .filter(machine_memory_binding::Column::OperationId.eq(operation.id))
        .exec(&transaction)
        .await?;
    transaction.commit().await?;
    Ok(machine_cancellation_record(persisted))
}

async fn read_machine_operation_status(
    database: &DatabaseConnection,
    machine_port: Option<&dyn MachineTranslationPort>,
    context: PortContext,
    operation_id: Uuid,
) -> TranslationResult<MachineOperationStatusRecord> {
    context.require_policy(PortCallPolicy::read())?;
    let tenant_id =
        Uuid::parse_str(&context.tenant_id).map_err(|_| TranslationError::InvalidTenantId)?;
    let security = SecurityContext::try_from_port_context(&context)?;
    if security.get_scope(Resource::Translations, Action::Read) == PermissionScope::None {
        return Err(TranslationError::Forbidden);
    }
    let operation = find_operation(database, tenant_id, operation_id).await?;
    if operation.status == "completed" {
        return Ok(MachineOperationStatusRecord {
            operation_id: operation.id,
            item_id: operation.item_id,
            status: operation.status,
            provider_execution_id: operation.execution_id,
            provider_status: "completed".to_string(),
            provider_error_code: None,
            updated_at: operation.updated_at,
        });
    }
    if operation.status == "cancelled" {
        let cancellation = find_cancellation_by_operation(database, tenant_id, operation.id)
            .await?
            .ok_or(TranslationError::WorkflowRevisionConflict)?;
        return Ok(MachineOperationStatusRecord {
            operation_id: operation.id,
            item_id: operation.item_id,
            status: operation.status,
            provider_execution_id: cancellation.provider_execution_id,
            provider_status: cancellation.provider_status,
            provider_error_code: cancellation.provider_error_code,
            updated_at: operation.updated_at,
        });
    }
    let Some(machine_port) = machine_port else {
        return Ok(MachineOperationStatusRecord {
            operation_id: operation.id,
            item_id: operation.item_id,
            status: operation.status,
            provider_execution_id: None,
            provider_status: "unavailable".to_string(),
            provider_error_code: None,
            updated_at: operation.updated_at,
        });
    };
    let execution_idempotency_key =
        child_idempotency_key(&operation.idempotency_key, "machine-port")?;
    let (provider_execution_id, provider_status, provider_error_code) = match machine_port
        .execution_status(context, execution_idempotency_key)
        .await
    {
        Ok(evidence) => (
            evidence.execution_id,
            machine_execution_status(evidence.status).to_string(),
            None,
        ),
        Err(error) => (
            None,
            "unavailable".to_string(),
            Some(error.code.chars().take(128).collect()),
        ),
    };
    Ok(MachineOperationStatusRecord {
        operation_id: operation.id,
        item_id: operation.item_id,
        status: operation.status,
        provider_execution_id,
        provider_status,
        provider_error_code,
        updated_at: operation.updated_at,
    })
}

fn validate_machine_cancellation_reason(reason: &str) -> TranslationResult<()> {
    if reason.trim().is_empty() || reason.trim() != reason || reason.len() > 4_096 {
        return Err(TranslationError::InvalidMachineCancellationReason);
    }
    Ok(())
}

fn validate_machine_recovery_reason(reason: &str) -> TranslationResult<()> {
    if reason.trim().is_empty() || reason.trim() != reason || reason.len() > 4_096 {
        return Err(TranslationError::InvalidMachineRecoveryReason);
    }
    Ok(())
}

fn validate_recovery_proposal_input(
    operation: &machine_operation::Model,
    input: &GenerateMachineProposalInput,
) -> TranslationResult<()> {
    if operation.item_id != input.item_id {
        return Err(TranslationError::IdempotencyConflict);
    }
    if operation.command_hash != hash_manifest(input)? {
        return Err(TranslationError::IdempotencyConflict);
    }
    Ok(())
}

fn validate_machine_recovery_replay(
    recovery: &machine_recovery::Model,
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

fn replay_machine_cancellation(
    model: machine_cancellation::Model,
    context: &PortContext,
    request_hash: &str,
) -> TranslationResult<MachineCancellationRecord> {
    validate_machine_cancellation_replay(&model, context, request_hash)?;
    Ok(machine_cancellation_record(model))
}

fn validate_machine_cancellation_replay(
    model: &machine_cancellation::Model,
    context: &PortContext,
    request_hash: &str,
) -> TranslationResult<()> {
    if model.idempotency_key != context.idempotency_key.as_deref().unwrap_or_default() {
        return Err(TranslationError::MachineOperationTerminal(
            "cancelled".to_string(),
        ));
    }
    if model.request_hash != request_hash {
        return Err(TranslationError::IdempotencyConflict);
    }
    if model.requested_by_actor_kind != actor_kind(context)
        || model.requested_by_actor_id != context.actor.id
    {
        return Err(TranslationError::IdempotencyActorMismatch);
    }
    Ok(())
}

fn machine_cancellation_record(model: machine_cancellation::Model) -> MachineCancellationRecord {
    MachineCancellationRecord {
        cancellation_id: model.id,
        operation_id: model.operation_id,
        status: "cancelled".to_string(),
        provider_execution_id: model.provider_execution_id,
        provider_status: model.provider_status,
        provider_error_code: model.provider_error_code,
        provider_observed_at: model.provider_observed_at,
        created_at: model.created_at,
    }
}

struct ProviderCancellationEvidence {
    execution_id: Option<String>,
    status: String,
    error_code: Option<String>,
    observed_at: DateTime<FixedOffset>,
}

async fn propagate_machine_cancellation(
    machine_port: Option<&dyn MachineTranslationPort>,
    context: &PortContext,
    operation: &machine_operation::Model,
) -> ProviderCancellationEvidence {
    let observed_at = Utc::now().fixed_offset();
    let Some(machine_port) = machine_port else {
        return ProviderCancellationEvidence {
            execution_id: None,
            status: "unavailable".to_string(),
            error_code: None,
            observed_at,
        };
    };
    let execution_idempotency_key =
        match child_idempotency_key(&operation.idempotency_key, "machine-port") {
            Ok(key) => key,
            Err(_) => {
                return ProviderCancellationEvidence {
                    execution_id: None,
                    status: "propagation_failed".to_string(),
                    error_code: Some("translation.machine.cancellation_key_invalid".to_string()),
                    observed_at,
                };
            }
        };
    match machine_port
        .cancel_execution(context.clone(), execution_idempotency_key)
        .await
    {
        Ok(evidence) => ProviderCancellationEvidence {
            execution_id: evidence.execution_id,
            status: provider_cancellation_status(evidence.status).to_string(),
            error_code: None,
            observed_at,
        },
        Err(error) => ProviderCancellationEvidence {
            execution_id: None,
            status: "propagation_failed".to_string(),
            error_code: Some(error.code.chars().take(128).collect()),
            observed_at,
        },
    }
}

fn provider_cancellation_status(status: MachineTranslationExecutionStatus) -> &'static str {
    match status {
        MachineTranslationExecutionStatus::NotRegistered
        | MachineTranslationExecutionStatus::Queued
        | MachineTranslationExecutionStatus::Running
        | MachineTranslationExecutionStatus::CancellationRequested => "cancellation_requested",
        MachineTranslationExecutionStatus::Completed => "completed",
        MachineTranslationExecutionStatus::Failed => "failed",
        MachineTranslationExecutionStatus::Cancelled => "cancelled",
    }
}

fn machine_execution_status(status: MachineTranslationExecutionStatus) -> &'static str {
    match status {
        MachineTranslationExecutionStatus::NotRegistered => "not_registered",
        MachineTranslationExecutionStatus::Queued => "queued",
        MachineTranslationExecutionStatus::Running => "running",
        MachineTranslationExecutionStatus::CancellationRequested => "cancellation_requested",
        MachineTranslationExecutionStatus::Completed => "completed",
        MachineTranslationExecutionStatus::Failed => "failed",
        MachineTranslationExecutionStatus::Cancelled => "cancelled",
    }
}

async fn refresh_machine_cancellation_provider_evidence(
    database: &DatabaseConnection,
    machine_port: Option<&dyn MachineTranslationPort>,
    context: &PortContext,
    existing: machine_cancellation::Model,
) -> TranslationResult<MachineCancellationRecord> {
    if machine_port.is_none()
        || matches!(
            existing.provider_status.as_str(),
            "completed" | "failed" | "cancelled"
        )
    {
        return Ok(machine_cancellation_record(existing));
    }
    let operation = find_operation(database, existing.tenant_id, existing.operation_id).await?;
    let evidence = propagate_machine_cancellation(machine_port, context, &operation).await;
    machine_cancellation::Entity::update_many()
        .col_expr(
            machine_cancellation::Column::ProviderExecutionId,
            Expr::value(evidence.execution_id),
        )
        .col_expr(
            machine_cancellation::Column::ProviderStatus,
            Expr::value(evidence.status),
        )
        .col_expr(
            machine_cancellation::Column::ProviderErrorCode,
            Expr::value(evidence.error_code),
        )
        .col_expr(
            machine_cancellation::Column::ProviderObservedAt,
            Expr::value(evidence.observed_at),
        )
        .filter(machine_cancellation::Column::TenantId.eq(existing.tenant_id))
        .filter(machine_cancellation::Column::Id.eq(existing.id))
        .exec(database)
        .await?;
    find_cancellation_by_idempotency(database, existing.tenant_id, &existing.idempotency_key)
        .await?
        .map(machine_cancellation_record)
        .ok_or(TranslationError::WorkflowRevisionConflict)
}

fn validate_machine_result(
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
        .collect::<std::collections::BTreeMap<_, _>>();
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

fn validate_provider_compatibility(
    request: &MachineTranslationBatchRequest,
    descriptor: &crate::MachineTranslationProviderDescriptor,
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

fn is_digest(value: &str) -> bool {
    value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit())
}

fn validate_operation_replay(
    operation: &machine_operation::Model,
    context: &PortContext,
    command_hash: &str,
) -> TranslationResult<()> {
    if operation.command_hash != command_hash {
        return Err(TranslationError::IdempotencyConflict);
    }
    if operation.requested_by_actor_kind != actor_kind(context)
        || operation.requested_by_actor_id != context.actor.id
    {
        return Err(TranslationError::IdempotencyActorMismatch);
    }
    Ok(())
}

async fn find_operation_by_idempotency<C>(
    database: &C,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> TranslationResult<Option<machine_operation::Model>>
where
    C: ConnectionTrait,
{
    Ok(machine_operation::Entity::find()
        .filter(machine_operation::Column::TenantId.eq(tenant_id))
        .filter(machine_operation::Column::IdempotencyKey.eq(idempotency_key))
        .one(database)
        .await?)
}

async fn find_operation<C>(
    database: &C,
    tenant_id: Uuid,
    operation_id: Uuid,
) -> TranslationResult<machine_operation::Model>
where
    C: ConnectionTrait,
{
    machine_operation::Entity::find_by_id(operation_id)
        .filter(machine_operation::Column::TenantId.eq(tenant_id))
        .one(database)
        .await?
        .ok_or(TranslationError::MachineOperationNotFound)
}

async fn find_cancellation_by_idempotency<C>(
    database: &C,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> TranslationResult<Option<machine_cancellation::Model>>
where
    C: ConnectionTrait,
{
    Ok(machine_cancellation::Entity::find()
        .filter(machine_cancellation::Column::TenantId.eq(tenant_id))
        .filter(machine_cancellation::Column::IdempotencyKey.eq(idempotency_key))
        .one(database)
        .await?)
}

async fn find_cancellation_by_operation<C>(
    database: &C,
    tenant_id: Uuid,
    operation_id: Uuid,
) -> TranslationResult<Option<machine_cancellation::Model>>
where
    C: ConnectionTrait,
{
    Ok(machine_cancellation::Entity::find()
        .filter(machine_cancellation::Column::TenantId.eq(tenant_id))
        .filter(machine_cancellation::Column::OperationId.eq(operation_id))
        .one(database)
        .await?)
}

async fn find_machine_recovery_by_idempotency<C>(
    database: &C,
    tenant_id: Uuid,
    idempotency_key: &str,
) -> TranslationResult<Option<machine_recovery::Model>>
where
    C: ConnectionTrait,
{
    Ok(machine_recovery::Entity::find()
        .filter(machine_recovery::Column::TenantId.eq(tenant_id))
        .filter(machine_recovery::Column::IdempotencyKey.eq(idempotency_key))
        .one(database)
        .await?)
}

async fn find_item(
    database: &DatabaseConnection,
    tenant_id: Uuid,
    item_id: Uuid,
) -> TranslationResult<job_item::Model> {
    job_item::Entity::find_by_id(item_id)
        .filter(job_item::Column::TenantId.eq(tenant_id))
        .one(database)
        .await?
        .ok_or(TranslationError::ItemNotFound)
}

async fn find_job(
    database: &DatabaseConnection,
    tenant_id: Uuid,
    job_id: Uuid,
) -> TranslationResult<job::Model> {
    job::Entity::find_by_id(job_id)
        .filter(job::Column::TenantId.eq(tenant_id))
        .one(database)
        .await?
        .ok_or(TranslationError::JobNotFound)
}

fn machine_proposal_record(
    model: machine_operation::Model,
) -> TranslationResult<MachineProposalRecord> {
    if model.status != "completed" {
        return Err(TranslationError::MachineTranslationInProgress);
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

#[cfg(test)]
mod tests {
    use std::{
        collections::{BTreeMap, BTreeSet},
        path::{Path, PathBuf},
        sync::atomic::{AtomicUsize, Ordering},
        time::Duration,
    };

    use async_trait::async_trait;
    use rustok_api::{Permission, PortActor, PortError, TenantLocale};
    use rustok_outbox::{OutboxTransport, TransactionalEventBus};
    use rustok_tenant::{
        ReplaceTenantLocalePolicyRequest, TenantLocalePolicyEntry, TenantLocalePolicyProjection,
    };
    use rustok_translation_targets::{
        ListTranslationResourcesRequest, OpaqueRevision, OwnerSlug, ReadTranslationResourceRequest,
        ResourceId, ResourceKind, TranslationApplicationReceipt, TranslationDataClassification,
        TranslationPatchRequest, TranslationPatchValidation, TranslationResourceIdentity,
        TranslationResourceLifecycle, TranslationResourcePage, TranslationResourceSummary,
        TranslationStrategy, TranslationTargetCapability, TranslationTargetProvider,
        TranslationTargetProviderDescriptor, TranslationValueProfile,
    };
    use sea_orm::{ConnectOptions, Database, DbBackend, PaginatorTrait, Statement};
    use sea_orm_migration::SchemaManager;
    use tokio::process::Command;

    use super::*;
    use crate::{
        MachineTranslationDiagnostic, MachineTranslationExecutionEvidence,
        MachineTranslationExecutionStatusEvidence, MachineTranslationProviderDescriptor,
        MachineTranslationUnitResult, PurgeMemoryEntryInput, TranslationMemoryService, migrations,
    };

    struct CancellationMachinePort {
        descriptor: MachineTranslationProviderDescriptor,
        calls: AtomicUsize,
    }

    struct RecoveryMachinePort {
        descriptor: MachineTranslationProviderDescriptor,
        recover_calls: AtomicUsize,
        recovered_result: Option<MachineTranslationBatchResult>,
    }

    struct EstimatingMachinePort {
        descriptor: MachineTranslationProviderDescriptor,
        estimate_calls: AtomicUsize,
    }

    struct RecoveryTargetProvider;

    struct SqliteTestFileGuard(PathBuf);

    struct RecoveryTenantLocalePolicies;

    impl Drop for SqliteTestFileGuard {
        fn drop(&mut self) {
            for path in sqlite_test_files(&self.0) {
                if path.exists() {
                    let _ = std::fs::remove_file(path);
                }
            }
        }
    }

    #[async_trait]
    impl TenantLocalePolicyPort for RecoveryTenantLocalePolicies {
        async fn read_locale_policy(
            &self,
            context: PortContext,
        ) -> Result<TenantLocalePolicyProjection, PortError> {
            Ok(TenantLocalePolicyProjection {
                tenant_id: Uuid::parse_str(&context.tenant_id).unwrap(),
                revision: 1,
                default_locale: TenantLocale::new("en").unwrap(),
                locales: ["en", "de"]
                    .into_iter()
                    .map(|locale| TenantLocalePolicyEntry {
                        locale: TenantLocale::new(locale).unwrap(),
                        name: locale.to_string(),
                        native_name: locale.to_string(),
                        is_default: locale == "en",
                        is_enabled: true,
                        fallback_locale: (locale != "en").then(|| TenantLocale::new("en").unwrap()),
                    })
                    .collect(),
            })
        }

        async fn replace_locale_policy(
            &self,
            _context: PortContext,
            _request: ReplaceTenantLocalePolicyRequest,
        ) -> Result<TenantLocalePolicyProjection, PortError> {
            unreachable!("machine recovery test does not replace locale policy")
        }
    }

    impl CancellationMachinePort {
        fn new() -> Self {
            Self {
                descriptor: descriptor(100),
                calls: AtomicUsize::new(0),
            }
        }
    }

    #[async_trait]
    impl MachineTranslationPort for CancellationMachinePort {
        fn descriptor(&self) -> &MachineTranslationProviderDescriptor {
            &self.descriptor
        }

        async fn health(
            &self,
            _context: PortContext,
        ) -> Result<crate::MachineTranslationProviderHealth, rustok_api::PortError> {
            unreachable!("cancellation test does not check health")
        }

        async fn estimate_batch(
            &self,
            _context: PortContext,
            _request: MachineTranslationBatchRequest,
        ) -> Result<crate::MachineTranslationEstimate, rustok_api::PortError> {
            unreachable!("cancellation test does not estimate")
        }

        async fn translate_batch(
            &self,
            _context: PortContext,
            _request: MachineTranslationBatchRequest,
        ) -> Result<MachineTranslationBatchResult, rustok_api::PortError> {
            unreachable!("cancellation test does not translate")
        }

        async fn execution_status(
            &self,
            _context: PortContext,
            execution_idempotency_key: String,
        ) -> Result<MachineTranslationExecutionStatusEvidence, rustok_api::PortError> {
            assert!(execution_idempotency_key.starts_with("translation-machine:machine-port:"));
            Ok(MachineTranslationExecutionStatusEvidence {
                execution_id: Some("execution-a".to_string()),
                status: MachineTranslationExecutionStatus::Running,
            })
        }

        async fn recover_batch(
            &self,
            _context: PortContext,
            _execution_idempotency_key: String,
            _request: MachineTranslationBatchRequest,
        ) -> Result<Option<MachineTranslationBatchResult>, rustok_api::PortError> {
            unreachable!("cancellation test does not recover")
        }

        async fn cancel_execution(
            &self,
            _context: PortContext,
            execution_idempotency_key: String,
        ) -> Result<MachineTranslationExecutionStatusEvidence, rustok_api::PortError> {
            assert!(execution_idempotency_key.starts_with("translation-machine:machine-port:"));
            let call = self.calls.fetch_add(1, Ordering::SeqCst);
            Ok(MachineTranslationExecutionStatusEvidence {
                execution_id: Some("execution-a".to_string()),
                status: if call == 0 {
                    MachineTranslationExecutionStatus::CancellationRequested
                } else {
                    MachineTranslationExecutionStatus::Cancelled
                },
            })
        }
    }

    #[async_trait]
    impl MachineTranslationPort for RecoveryMachinePort {
        fn descriptor(&self) -> &MachineTranslationProviderDescriptor {
            &self.descriptor
        }

        async fn health(
            &self,
            _context: PortContext,
        ) -> Result<crate::MachineTranslationProviderHealth, PortError> {
            unreachable!("machine recovery never checks provider health")
        }

        async fn estimate_batch(
            &self,
            _context: PortContext,
            _request: MachineTranslationBatchRequest,
        ) -> Result<crate::MachineTranslationEstimate, PortError> {
            unreachable!("machine recovery test does not estimate")
        }

        async fn translate_batch(
            &self,
            _context: PortContext,
            _request: MachineTranslationBatchRequest,
        ) -> Result<MachineTranslationBatchResult, PortError> {
            unreachable!("machine recovery must never start another translation")
        }

        async fn execution_status(
            &self,
            _context: PortContext,
            _execution_idempotency_key: String,
        ) -> Result<MachineTranslationExecutionStatusEvidence, PortError> {
            unreachable!("machine recovery test does not read status")
        }

        async fn recover_batch(
            &self,
            _context: PortContext,
            execution_idempotency_key: String,
            _request: MachineTranslationBatchRequest,
        ) -> Result<Option<MachineTranslationBatchResult>, PortError> {
            assert!(execution_idempotency_key.starts_with("translation-machine:machine-port:"));
            self.recover_calls.fetch_add(1, Ordering::SeqCst);
            Ok(self.recovered_result.clone())
        }

        async fn cancel_execution(
            &self,
            _context: PortContext,
            _execution_idempotency_key: String,
        ) -> Result<MachineTranslationExecutionStatusEvidence, PortError> {
            unreachable!("machine recovery test does not cancel")
        }
    }

    #[async_trait]
    impl MachineTranslationPort for EstimatingMachinePort {
        fn descriptor(&self) -> &MachineTranslationProviderDescriptor {
            &self.descriptor
        }

        async fn health(
            &self,
            _context: PortContext,
        ) -> Result<crate::MachineTranslationProviderHealth, PortError> {
            unreachable!("estimate does not use the health endpoint")
        }

        async fn estimate_batch(
            &self,
            context: PortContext,
            request: MachineTranslationBatchRequest,
        ) -> Result<crate::MachineTranslationEstimate, PortError> {
            request.validate(&context)?;
            self.estimate_calls.fetch_add(1, Ordering::SeqCst);
            Ok(crate::MachineTranslationEstimate {
                input_tokens_upper_bound: 256,
                output_tokens_upper_bound: 1_048_576,
                attempts_upper_bound: 2,
                cost_minor_units_upper_bound: 17,
                currency_code: "USD".to_string(),
                price_snapshot_digest: "9".repeat(64),
                review_required: true,
            })
        }

        async fn translate_batch(
            &self,
            _context: PortContext,
            _request: MachineTranslationBatchRequest,
        ) -> Result<MachineTranslationBatchResult, PortError> {
            unreachable!("estimate must not execute machine translation")
        }

        async fn execution_status(
            &self,
            _context: PortContext,
            _execution_idempotency_key: String,
        ) -> Result<MachineTranslationExecutionStatusEvidence, PortError> {
            unreachable!("estimate does not read execution status")
        }

        async fn recover_batch(
            &self,
            _context: PortContext,
            _execution_idempotency_key: String,
            _request: MachineTranslationBatchRequest,
        ) -> Result<Option<MachineTranslationBatchResult>, PortError> {
            unreachable!("estimate does not recover an execution")
        }

        async fn cancel_execution(
            &self,
            _context: PortContext,
            _execution_idempotency_key: String,
        ) -> Result<MachineTranslationExecutionStatusEvidence, PortError> {
            unreachable!("estimate does not cancel an execution")
        }
    }

    #[async_trait]
    impl TranslationTargetProvider for RecoveryTargetProvider {
        fn descriptor(&self) -> TranslationTargetProviderDescriptor {
            TranslationTargetProviderDescriptor {
                owner_slug: OwnerSlug::new("media").unwrap(),
                resource_kind: ResourceKind::new("asset").unwrap(),
                display_name: "Recovery test media asset".to_string(),
                capabilities: BTreeSet::from([TranslationTargetCapability::ValidatePatch]),
                read_permission_floor: BTreeSet::new(),
                apply_permission_floor: BTreeSet::new(),
            }
        }

        async fn list_resources(
            &self,
            _context: PortContext,
            _request: ListTranslationResourcesRequest,
        ) -> Result<TranslationResourcePage, PortError> {
            Err(PortError::unavailable(
                "translation.test_unavailable",
                "recovery fixture does not list resources",
            ))
        }

        async fn read_resource(
            &self,
            _context: PortContext,
            _request: ReadTranslationResourceRequest,
        ) -> Result<TranslationResourceSnapshot, PortError> {
            Err(PortError::unavailable(
                "translation.test_unavailable",
                "recovery fixture does not read resources",
            ))
        }

        async fn validate_patch(
            &self,
            _context: PortContext,
            request: TranslationPatchRequest,
        ) -> Result<TranslationPatchValidation, PortError> {
            request.validate().map_err(|error| {
                PortError::validation("translation.test_patch", error.to_string())
            })?;
            Ok(TranslationPatchValidation {
                accepted: true,
                issues: Vec::new(),
            })
        }

        async fn apply_patch(
            &self,
            _context: PortContext,
            _request: TranslationPatchRequest,
        ) -> Result<TranslationApplicationReceipt, PortError> {
            Err(PortError::unavailable(
                "translation.test_unavailable",
                "recovery fixture does not apply patches",
            ))
        }
    }

    fn recovery_registry() -> Arc<TranslationTargetRegistry> {
        let mut registry = TranslationTargetRegistry::default();
        registry.register(RecoveryTargetProvider).unwrap();
        Arc::new(registry)
    }

    fn request() -> MachineTranslationBatchRequest {
        MachineTranslationBatchRequest {
            source_locale: TenantLocale::new("en").unwrap(),
            target_locale: TenantLocale::new("de").unwrap(),
            resource: MachineTranslationResourceContext {
                owner_slug: "media".to_string(),
                resource_kind: "asset".to_string(),
                resource_id: "asset-a".to_string(),
                subresource_id: None,
            },
            units: vec![MachineTranslationUnit {
                unit_id: "alt_text".to_string(),
                field_key: "alt_text".to_string(),
                source_value: "Hello {name} {count}".to_string(),
                source_hash: "a".repeat(64),
                source_revision: "revision-a".to_string(),
                profile: TranslationValueProfile::TemplateText,
                strategy: TranslationStrategy::TranslateWithPlaceholders,
                classification: TranslationDataClassification::TenantPrivate,
                ai_export_allowed: true,
                max_characters: Some(200),
                preserves_whitespace: false,
                protected_tokens: vec!["{name}".to_string(), "{count}".to_string()],
            }],
            glossary_revision: None,
            glossary_digest: None,
            glossary_terms: Vec::new(),
            memory_digest: None,
            memory_suggestions: Vec::new(),
            tone: None,
            domain: None,
            style: None,
            adapter_policy_digest: "b".repeat(64),
            evidence: BTreeMap::new(),
        }
    }

    fn descriptor(max_batch_units: u16) -> MachineTranslationProviderDescriptor {
        MachineTranslationProviderDescriptor {
            slug: "ai".to_string(),
            display_name: "AI".to_string(),
            policy_digest: "b".repeat(64),
            supported_profiles: vec![TranslationValueProfile::TemplateText],
            supported_classifications: vec![TranslationDataClassification::TenantPrivate],
            max_batch_units,
            max_batch_characters: 1_000,
            review_required: true,
        }
    }

    fn result() -> MachineTranslationBatchResult {
        MachineTranslationBatchResult {
            provider_slug: "provider-a".to_string(),
            units: vec![MachineTranslationUnitResult {
                unit_id: "alt_text".to_string(),
                translated_value: "Hallo {name} {count}".to_string(),
                protected_tokens: vec!["{count}".to_string(), "{name}".to_string()],
                diagnostics: vec![MachineTranslationDiagnostic {
                    code: "translation.machine.review".to_string(),
                    blocking: false,
                    unit_id: Some("alt_text".to_string()),
                }],
            }],
            execution: MachineTranslationExecutionEvidence {
                execution_id: "execution-a".to_string(),
                request_digest: "c".repeat(64),
                prompt_policy_digest: "b".repeat(64),
                attempts: vec![MachineTranslationAttemptEvidence {
                    attempt: 1,
                    provider_profile_id: "profile-a".to_string(),
                    provider_slug: "provider-a".to_string(),
                    model: "model-a".to_string(),
                    fallback: false,
                }],
                usage: MachineTranslationUsage {
                    input_tokens: 10,
                    output_tokens: 5,
                    total_tokens: 15,
                    cost_minor_units: 2,
                    currency_code: "USD".to_string(),
                    price_snapshot_digest: "e".repeat(64),
                },
            },
            review_required: true,
        }
    }

    fn snapshot() -> TranslationResourceSnapshot {
        TranslationResourceSnapshot {
            summary: TranslationResourceSummary {
                identity: TranslationResourceIdentity {
                    owner_slug: OwnerSlug::new("media").unwrap(),
                    resource_kind: ResourceKind::new("asset").unwrap(),
                    resource_id: ResourceId::new("asset-a").unwrap(),
                    subresource_id: None,
                },
                display_label: "Asset".to_string(),
                lifecycle: TranslationResourceLifecycle::Active,
                resource_revision: OpaqueRevision::new("resource-a").unwrap(),
                exact_locales: vec![TenantLocale::new("en").unwrap()],
            },
            source_locale: TenantLocale::new("en").unwrap(),
            target_locale: TenantLocale::new("de").unwrap(),
            rendered_fallback_locale: None,
            source_revision: OpaqueRevision::new("source-a").unwrap(),
            target_revision: None,
            fields: vec![rustok_translation_targets::TranslationFieldSnapshot {
                descriptor: rustok_translation_targets::TranslationFieldDescriptor {
                    key: FieldKey::new("alt_text").unwrap(),
                    profile: TranslationValueProfile::TemplateText,
                    strategy: TranslationStrategy::TranslateWithPlaceholders,
                    classification: TranslationDataClassification::TenantPrivate,
                    required: true,
                    ai_export_allowed: true,
                    max_characters: Some(200),
                    preserves_whitespace: false,
                },
                source_value: "Hello {name} {count}".to_string(),
                exact_target_value: None,
                source_hash: "a".repeat(64),
                protected_tokens: vec!["{name}".to_string(), "{count}".to_string()],
            }],
        }
    }

    #[test]
    fn generation_requires_unique_explicit_fields() {
        let input = GenerateMachineProposalInput {
            item_id: Uuid::new_v4(),
            field_keys: vec![
                FieldKey::new("title").unwrap(),
                FieldKey::new("title").unwrap(),
            ],
            minimum_memory_similarity_basis_points: 7_000,
            tone: None,
            domain: None,
            style: None,
        };
        assert!(matches!(
            validate_generation_input(&input),
            Err(TranslationError::InvalidRequest(_))
        ));
    }

    #[test]
    fn provider_capacity_is_checked_before_export() {
        assert!(matches!(
            validate_provider_compatibility(&request(), &descriptor(0)),
            Err(TranslationError::Provider {
                retryable: false,
                ..
            })
        ));
    }

    #[test]
    fn result_accepts_reordered_but_exact_protected_tokens() {
        validate_machine_result(&request(), &result()).unwrap();
    }

    #[test]
    fn result_rejects_changed_protected_tokens() {
        let mut result = result();
        result.units[0].protected_tokens = vec!["{name}".to_string()];
        assert!(matches!(
            validate_machine_result(&request(), &result),
            Err(TranslationError::InvalidMachineTranslationResult)
        ));
    }

    #[test]
    fn result_rejects_duplicate_protected_token_occurrences() {
        let mut result = result();
        result.units[0].translated_value = "Hallo {name} {count} {count}".to_string();
        assert!(matches!(
            validate_machine_result(&request(), &result),
            Err(TranslationError::InvalidMachineTranslationResult)
        ));
    }

    #[test]
    fn result_rejects_changed_required_whitespace_shape() {
        let mut request = request();
        request.units[0].source_value = "  Hello {name} {count}\r\n".to_string();
        request.units[0].preserves_whitespace = true;
        let result = result();
        assert!(matches!(
            validate_machine_result(&request, &result),
            Err(TranslationError::InvalidMachineTranslationResult)
        ));
    }

    #[test]
    fn result_rejects_unbound_policy_or_missing_review_requirement() {
        let mut wrong_policy = result();
        wrong_policy.execution.prompt_policy_digest = "f".repeat(64);
        assert!(matches!(
            validate_machine_result(&request(), &wrong_policy),
            Err(TranslationError::InvalidMachineTranslationResult)
        ));

        let mut missing_review_requirement = result();
        missing_review_requirement.execution.prompt_policy_digest = request().adapter_policy_digest;
        missing_review_requirement.review_required = false;
        assert!(matches!(
            validate_machine_result(&request(), &missing_review_requirement),
            Err(TranslationError::InvalidMachineTranslationResult)
        ));
    }

    async fn persistence_fixture(tombstoned: bool) -> (DatabaseConnection, Uuid, Uuid, Uuid, Uuid) {
        let database = Database::connect("sqlite::memory:").await.unwrap();
        initialize_persistence_fixture(database, tombstoned).await
    }

    #[tokio::test]
    async fn estimate_does_not_register_operation_proposal_or_memory_pin() {
        let (database, tenant_id, actor_id, operation_id, _memory_entry_id) =
            persistence_fixture(false).await;
        let port = Arc::new(EstimatingMachinePort {
            descriptor: descriptor(100),
            estimate_calls: AtomicUsize::new(0),
        });
        let service = recovery_service(database.clone(), port.clone());
        let operation_count = machine_operation::Entity::find()
            .count(&database)
            .await
            .unwrap();
        let proposal_count = crate::entities::proposal::Entity::find()
            .count(&database)
            .await
            .unwrap();
        let binding_count = machine_memory_binding::Entity::find()
            .count(&database)
            .await
            .unwrap();
        let item_id = find_operation(&database, tenant_id, operation_id)
            .await
            .unwrap()
            .item_id;

        let context = recovery_context(tenant_id, actor_id);
        assert!(
            !context
                .claims
                .contains(&Permission::new(Resource::TranslationMemory, Action::Read).to_string())
        );
        let estimate = service
            .estimate_proposal(
                context.with_idempotency_key("estimate-machine-translation"),
                recovery_proposal(item_id),
            )
            .await
            .unwrap();

        assert_eq!(estimate.cost_minor_units_upper_bound, 17);
        assert_eq!(port.estimate_calls.load(Ordering::SeqCst), 1);
        assert_eq!(
            machine_operation::Entity::find()
                .count(&database)
                .await
                .unwrap(),
            operation_count
        );
        assert_eq!(
            crate::entities::proposal::Entity::find()
                .count(&database)
                .await
                .unwrap(),
            proposal_count
        );
        assert_eq!(
            machine_memory_binding::Entity::find()
                .count(&database)
                .await
                .unwrap(),
            binding_count
        );
    }

    async fn persistence_fixture_at(
        database_path: &Path,
        tombstoned: bool,
    ) -> (DatabaseConnection, Uuid, Uuid, Uuid, Uuid) {
        let database = connect_persistence_file(database_path, true).await;
        initialize_persistence_fixture(database, tombstoned).await
    }

    async fn connect_persistence_file(
        database_path: &Path,
        create_if_missing: bool,
    ) -> DatabaseConnection {
        let database_path = database_path.to_path_buf();
        let mut options =
            ConnectOptions::new("sqlite://translation-recovery-placeholder.sqlite?mode=rwc");
        options
            .max_connections(4)
            .min_connections(1)
            .sqlx_logging(false)
            .map_sqlx_sqlite_opts(move |options| {
                options
                    .filename(database_path.clone())
                    .create_if_missing(create_if_missing)
            });
        let database = Database::connect(options).await.unwrap();
        database
            .execute_unprepared("PRAGMA foreign_keys = ON")
            .await
            .unwrap();
        database
    }

    async fn initialize_persistence_fixture(
        database: DatabaseConnection,
        tombstoned: bool,
    ) -> (DatabaseConnection, Uuid, Uuid, Uuid, Uuid) {
        database
            .execute_unprepared("PRAGMA foreign_keys = ON")
            .await
            .unwrap();
        database
            .execute_unprepared("CREATE TABLE tenants (id TEXT PRIMARY KEY NOT NULL)")
            .await
            .unwrap();
        let manager = SchemaManager::new(&database);
        for migration in migrations::migrations() {
            migration.up(&manager).await.unwrap();
        }
        let tenant_id = Uuid::new_v4();
        database
            .execute(Statement::from_sql_and_values(
                DbBackend::Sqlite,
                "INSERT INTO tenants (id) VALUES (?)",
                [tenant_id.into()],
            ))
            .await
            .unwrap();
        let now = Utc::now().fixed_offset();
        let job_id = Uuid::new_v4();
        job::Entity::insert(job::ActiveModel {
            id: Set(job_id),
            tenant_id: Set(tenant_id),
            source_locale: Set("en".to_string()),
            target_locale: Set("de".to_string()),
            glossary_id: Set(None),
            glossary_revision: Set(None),
            status: Set("open".to_string()),
            created_by_actor_kind: Set("user".to_string()),
            created_by_actor_id: Set(Uuid::new_v4().to_string()),
            idempotency_key: Set("fixture-job".to_string()),
            request_hash: Set("a".repeat(64)),
            revision: Set(1),
            created_at: Set(now),
            updated_at: Set(now),
        })
        .exec(&database)
        .await
        .unwrap();
        let item_id = Uuid::new_v4();
        job_item::Entity::insert(job_item::ActiveModel {
            id: Set(item_id),
            tenant_id: Set(tenant_id),
            job_id: Set(job_id),
            owner_slug: Set("media".to_string()),
            resource_kind: Set("asset".to_string()),
            resource_id: Set("asset-a".to_string()),
            subresource_key: Set(String::new()),
            resource_revision: Set("resource-a".to_string()),
            source_revision: Set("source-a".to_string()),
            target_revision: Set(None),
            source_snapshot: Set(serde_json::to_value(snapshot()).unwrap()),
            source_digest: Set(hash_manifest(&snapshot()).unwrap()),
            status: Set("missing".to_string()),
            current_proposal_id: Set(None),
            active_apply_operation_id: Set(None),
            assigned_actor_kind: Set(None),
            assigned_actor_id: Set(None),
            idempotency_key: Set("fixture-item".to_string()),
            request_hash: Set("c".repeat(64)),
            revision: Set(1),
            created_at: Set(now),
            updated_at: Set(now),
        })
        .exec(&database)
        .await
        .unwrap();
        let memory_entry_id = Uuid::new_v4();
        memory_entry::Entity::insert(memory_entry::ActiveModel {
            id: Set(memory_entry_id),
            tenant_id: Set(tenant_id),
            source_locale: Set("en".to_string()),
            target_locale: Set("de".to_string()),
            owner_slug: Set("media".to_string()),
            resource_kind: Set("asset".to_string()),
            resource_id: Set("asset-memory".to_string()),
            subresource_id: Set(None),
            field_key: Set("alt_text".to_string()),
            source_text: Set("Hello".to_string()),
            target_text: Set("Hallo".to_string()),
            source_key: Set("d".repeat(64)),
            source_hash: Set("e".repeat(64)),
            target_hash: Set("f".repeat(64)),
            context_fingerprint: Set("1".repeat(64)),
            segmentation_version: Set("owner-field-v1".to_string()),
            origin: Set("manual".to_string()),
            quality_state: Set("human_approved_applied".to_string()),
            reviewer_actor_kind: Set("user".to_string()),
            reviewer_actor_id: Set(Uuid::new_v4().to_string()),
            proposal_id: Set(Uuid::new_v4()),
            apply_receipt_id: Set(Uuid::new_v4()),
            retention_policy: Set("owner_lifecycle".to_string()),
            retain_until: Set(None),
            owner_lifecycle_revision: Set(None),
            owner_deleted_at: Set(None),
            tombstoned_at: Set(tombstoned.then_some(now)),
            revision: Set(1),
            created_at: Set(now),
            updated_at: Set(now),
        })
        .exec(&database)
        .await
        .unwrap();
        let actor_id = Uuid::new_v4();
        let operation_id = Uuid::new_v4();
        machine_operation::Entity::insert(machine_operation::ActiveModel {
            id: Set(operation_id),
            tenant_id: Set(tenant_id),
            item_id: Set(item_id),
            proposal_id: Set(None),
            status: Set("registered".to_string()),
            command_hash: Set("2".repeat(64)),
            machine_request_digest: Set("3".repeat(64)),
            adapter_slug: Set("ai".to_string()),
            provider_slug: Set(None),
            provider_policy_digest: Set("4".repeat(64)),
            glossary_revision: Set(None),
            glossary_digest: Set(None),
            memory_digest: Set(Some("5".repeat(64))),
            execution_id: Set(None),
            execution_request_digest: Set(None),
            prompt_policy_digest: Set(None),
            attempts: Set(serde_json::json!([])),
            usage: Set(None),
            diagnostics: Set(serde_json::json!([])),
            review_required: Set(None),
            requested_by_actor_kind: Set("user".to_string()),
            requested_by_actor_id: Set(actor_id.to_string()),
            idempotency_key: Set("fixture-machine".to_string()),
            created_at: Set(now),
            updated_at: Set(now),
        })
        .exec(&database)
        .await
        .unwrap();
        machine_memory_binding::Entity::insert(machine_memory_binding::ActiveModel {
            id: Set(Uuid::new_v4()),
            tenant_id: Set(tenant_id),
            operation_id: Set(operation_id),
            unit_id: Set("alt_text".to_string()),
            batch_ordinal: Set(0),
            unit_ordinal: Set(0),
            memory_entry_id: Set(memory_entry_id),
            score_basis_points: Set(9_000),
            created_at: Set(now),
        })
        .exec(&database)
        .await
        .unwrap();
        (database, tenant_id, actor_id, operation_id, memory_entry_id)
    }

    fn machine_control_context(
        tenant_id: Uuid,
        actor_id: Uuid,
        idempotency_key: &str,
    ) -> PortContext {
        PortContext::new(
            tenant_id.to_string(),
            PortActor::user(actor_id.to_string()),
            "en",
            format!("machine-control-{idempotency_key}"),
        )
        .with_claim(Permission::new(Resource::Translations, Action::Run).to_string())
        .with_role("manager")
        .with_idempotency_key(idempotency_key)
        .with_deadline(Duration::from_secs(5))
    }

    fn recovery_context(tenant_id: Uuid, actor_id: Uuid) -> PortContext {
        machine_control_context(tenant_id, actor_id, "recover-machine-restart")
            .with_claim(Permission::new(Resource::Translations, Action::Manage).to_string())
            .with_claim(Permission::new(Resource::Translations, Action::Update).to_string())
    }

    fn recovery_proposal(item_id: Uuid) -> GenerateMachineProposalInput {
        GenerateMachineProposalInput {
            item_id,
            field_keys: vec![FieldKey::new("alt_text").unwrap()],
            minimum_memory_similarity_basis_points: 7_000,
            tone: None,
            domain: None,
            style: None,
        }
    }

    fn recovery_service(
        database: DatabaseConnection,
        machine_port: Arc<dyn MachineTranslationPort>,
    ) -> TranslationMachineService {
        TranslationMachineService::new(
            database.clone(),
            recovery_registry(),
            Arc::new(RecoveryTenantLocalePolicies),
            TransactionalEventBus::new(Arc::new(OutboxTransport::new(database))),
            machine_port,
        )
    }

    async fn prepare_saving_recovery(
        service: &TranslationMachineService,
        tenant_id: Uuid,
        operation_id: Uuid,
    ) -> RecoverMachineOperationInput {
        let operation = find_operation(&service.database, tenant_id, operation_id)
            .await
            .unwrap();
        let proposal = recovery_proposal(operation.item_id);
        let item = find_item(&service.database, tenant_id, proposal.item_id)
            .await
            .unwrap();
        let request = service
            .build_request(tenant_id, &item, &snapshot(), &proposal, Some(&operation))
            .await
            .unwrap();
        let observed_updated_at = Utc::now().fixed_offset();
        machine_operation::Entity::update_many()
            .col_expr(machine_operation::Column::Status, Expr::value("saving"))
            .col_expr(
                machine_operation::Column::CommandHash,
                Expr::value(hash_manifest(&proposal).unwrap()),
            )
            .col_expr(
                machine_operation::Column::MachineRequestDigest,
                Expr::value(hash_manifest(&request).unwrap()),
            )
            .col_expr(
                machine_operation::Column::ProviderPolicyDigest,
                Expr::value(descriptor(100).policy_digest),
            )
            .col_expr(
                machine_operation::Column::UpdatedAt,
                Expr::value(observed_updated_at),
            )
            .filter(machine_operation::Column::Id.eq(operation_id))
            .exec(&service.database)
            .await
            .unwrap();
        RecoverMachineOperationInput {
            operation_id,
            expected_updated_at: observed_updated_at,
            proposal,
            reason: "Recover the completed provider result after an interrupted save".to_string(),
        }
    }

    async fn save_proposal_without_completing_operation(
        service: &TranslationMachineService,
        context: PortContext,
        operation: &machine_operation::Model,
        input: &RecoverMachineOperationInput,
    ) -> Uuid {
        let mut save_context = context;
        save_context.idempotency_key =
            Some(child_idempotency_key(&operation.idempotency_key, "save-proposal").unwrap());
        service
            .workflow
            .save_recovered_machine_proposal(
                save_context,
                SaveProposalInput {
                    item_id: input.proposal.item_id,
                    origin: ProposalOrigin::Ai,
                    values: vec![ProposalValue {
                        key: FieldKey::new("alt_text").unwrap(),
                        value: "Hallo {name} {count}".to_string(),
                    }],
                },
            )
            .await
            .unwrap()
            .id
    }

    fn sqlite_test_files(database_path: &Path) -> [PathBuf; 4] {
        [
            database_path.to_path_buf(),
            PathBuf::from(format!("{}-journal", database_path.display())),
            PathBuf::from(format!("{}-shm", database_path.display())),
            PathBuf::from(format!("{}-wal", database_path.display())),
        ]
    }

    #[tokio::test]
    async fn pinned_memory_projection_survives_tombstone() {
        let (database, tenant_id, _, operation_id, _) = persistence_fixture(true).await;
        let operation = find_operation(&database, tenant_id, operation_id)
            .await
            .unwrap();
        let snapshot = snapshot();
        let suggestions = read_pinned_memory_suggestions(
            &database,
            tenant_id,
            &operation,
            &snapshot,
            &request().units,
        )
        .await
        .unwrap();
        assert_eq!(suggestions.len(), 1);
        assert_eq!(suggestions[0].target_value, "Hallo");
        assert_eq!(suggestions[0].score_basis_points, 9_000);
    }

    #[tokio::test]
    async fn pinned_memory_cannot_be_purged() {
        let (database, tenant_id, actor_id, _, memory_entry_id) = persistence_fixture(true).await;
        let context = machine_control_context(tenant_id, actor_id, "purge-pinned")
            .with_claim(Permission::new(Resource::TranslationMemory, Action::Delete).to_string())
            .with_claim(Permission::new(Resource::TranslationMemory, Action::Manage).to_string());
        let error = TranslationMemoryService::new(database)
            .purge_entry(
                context,
                PurgeMemoryEntryInput {
                    entry_id: memory_entry_id,
                    expected_revision: 1,
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(
            error,
            TranslationError::MemoryRetentionConflict(_)
        ));
    }

    #[tokio::test]
    async fn cancellation_is_actor_bound_replay_safe_and_releases_pins() {
        let (database, tenant_id, actor_id, operation_id, _) = persistence_fixture(false).await;
        let context = machine_control_context(tenant_id, actor_id, "cancel-machine");
        let input = CancelMachineOperationInput {
            operation_id,
            reason: "Operator cancelled the pending generation".to_string(),
        };
        let first = cancel_machine_operation(&database, None, context.clone(), input.clone())
            .await
            .unwrap();
        let replay = cancel_machine_operation(&database, None, context, input)
            .await
            .unwrap();
        assert_eq!(first, replay);
        assert_eq!(first.provider_status, "unavailable");
        assert_eq!(
            find_operation(&database, tenant_id, operation_id)
                .await
                .unwrap()
                .status,
            "cancelled"
        );
        assert!(
            machine_memory_binding::Entity::find()
                .filter(machine_memory_binding::Column::OperationId.eq(operation_id))
                .one(&database)
                .await
                .unwrap()
                .is_none()
        );
    }

    #[tokio::test]
    async fn cancellation_propagation_is_retried_by_the_same_receipt() {
        let (database, tenant_id, actor_id, operation_id, _) = persistence_fixture(false).await;
        let context = machine_control_context(tenant_id, actor_id, "cancel-machine-provider");
        let input = CancelMachineOperationInput {
            operation_id,
            reason: "Operator cancelled the pending generation".to_string(),
        };
        let machine_port = CancellationMachinePort::new();

        let first = cancel_machine_operation(
            &database,
            Some(&machine_port),
            context.clone(),
            input.clone(),
        )
        .await
        .unwrap();
        assert_eq!(first.provider_status, "cancellation_requested");
        assert_eq!(first.provider_execution_id.as_deref(), Some("execution-a"));

        let replay = cancel_machine_operation(&database, Some(&machine_port), context, input)
            .await
            .unwrap();
        assert_eq!(replay.cancellation_id, first.cancellation_id);
        assert_eq!(replay.provider_status, "cancelled");
        assert_eq!(machine_port.calls.load(Ordering::SeqCst), 2);
    }

    #[tokio::test]
    async fn operation_status_resolves_provider_by_stable_execution_key() {
        let (database, tenant_id, actor_id, operation_id, _) = persistence_fixture(false).await;
        let context = machine_control_context(tenant_id, actor_id, "read-machine-status")
            .with_claim(Permission::new(Resource::Translations, Action::Read).to_string());
        let machine_port = CancellationMachinePort::new();

        let status =
            read_machine_operation_status(&database, Some(&machine_port), context, operation_id)
                .await
                .unwrap();
        assert_eq!(status.status, "registered");
        assert_eq!(status.provider_status, "running");
        assert_eq!(status.provider_execution_id.as_deref(), Some("execution-a"));
    }

    #[tokio::test]
    async fn separate_process_recovers_both_machine_save_crash_boundaries() {
        let workspace = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../..")
            .canonicalize()
            .expect("workspace path");
        let evidence_dir = workspace.join("target/translation-recovery-process-tests");
        std::fs::create_dir_all(&evidence_dir)
            .expect("Translation recovery process evidence directory");
        let evidence_dir = evidence_dir
            .canonicalize()
            .expect("Translation recovery process evidence path");
        assert!(evidence_dir.starts_with(workspace.join("target")));

        for proposal_was_saved in [false, true] {
            let database_path = evidence_dir.join(format!(
                "rustok-translation-recovery-{}.sqlite",
                Uuid::new_v4()
            ));
            let _database_cleanup = SqliteTestFileGuard(database_path.clone());
            let (database, tenant_id, actor_id, operation_id, _) =
                persistence_fixture_at(&database_path, false).await;
            let machine_port = Arc::new(RecoveryMachinePort {
                descriptor: descriptor(100),
                recover_calls: AtomicUsize::new(0),
                recovered_result: Some(result()),
            });
            let service = recovery_service(database.clone(), machine_port);
            let context = recovery_context(tenant_id, actor_id);
            let input = prepare_saving_recovery(&service, tenant_id, operation_id).await;
            let expected_proposal_id = if proposal_was_saved {
                let operation = find_operation(&database, tenant_id, operation_id)
                    .await
                    .unwrap();
                Some(
                    save_proposal_without_completing_operation(
                        &service,
                        context.clone(),
                        &operation,
                        &input,
                    )
                    .await,
                )
            } else {
                None
            };
            drop(service);
            database.close().await.unwrap();

            let output =
                Command::new(std::env::current_exe().expect("Translation test executable"))
                    .args([
                        "--exact",
                        "machine_service::tests::machine_recovery_child_process",
                        "--ignored",
                        "--nocapture",
                        "--test-threads=1",
                    ])
                    .env(
                        "RUSTOK_TRANSLATION_TEST_MACHINE_RECOVERY_DB_PATH",
                        &database_path,
                    )
                    .output()
                    .await
                    .expect("Translation recovery child process");
            assert!(
                output.status.success(),
                "Translation recovery child failed for proposal_was_saved={proposal_was_saved}:\nstdout:\n{}\nstderr:\n{}",
                String::from_utf8_lossy(&output.stdout),
                String::from_utf8_lossy(&output.stderr)
            );

            let database = connect_persistence_file(&database_path, false).await;
            let operation = find_operation(&database, tenant_id, operation_id)
                .await
                .unwrap();
            assert_eq!(operation.status, "completed");
            assert_eq!(operation.provider_slug.as_deref(), Some("provider-a"));
            assert_eq!(operation.execution_id.as_deref(), Some("execution-a"));
            let proposal_id = operation.proposal_id.unwrap();
            if let Some(expected_proposal_id) = expected_proposal_id {
                assert_eq!(proposal_id, expected_proposal_id);
            }
            assert_eq!(
                crate::entities::proposal::Entity::find()
                    .filter(crate::entities::proposal::Column::TenantId.eq(tenant_id))
                    .filter(crate::entities::proposal::Column::ItemId.eq(operation.item_id))
                    .count(&database)
                    .await
                    .unwrap(),
                1
            );
            assert_eq!(
                machine_recovery::Entity::find()
                    .filter(machine_recovery::Column::OperationId.eq(operation_id))
                    .count(&database)
                    .await
                    .unwrap(),
                1
            );
            assert!(
                machine_memory_binding::Entity::find()
                    .filter(machine_memory_binding::Column::OperationId.eq(operation_id))
                    .one(&database)
                    .await
                    .unwrap()
                    .is_none()
            );
            assert_eq!(
                find_item(&database, tenant_id, operation.item_id)
                    .await
                    .unwrap()
                    .current_proposal_id,
                Some(proposal_id)
            );

            let replay_machine_port = Arc::new(RecoveryMachinePort {
                descriptor: descriptor(100),
                recover_calls: AtomicUsize::new(0),
                recovered_result: Some(result()),
            });
            let replay_service = recovery_service(database.clone(), replay_machine_port.clone());
            let replay = replay_service
                .recover_operation(context, input)
                .await
                .unwrap();
            assert_eq!(replay.proposal_id, proposal_id);
            assert_eq!(replay_machine_port.recover_calls.load(Ordering::SeqCst), 0);
            drop(replay_service);
            database.close().await.unwrap();
        }
    }

    #[tokio::test]
    #[ignore = "internal child process for Translation machine recovery evidence"]
    async fn machine_recovery_child_process() {
        let Some(database_path) =
            std::env::var_os("RUSTOK_TRANSLATION_TEST_MACHINE_RECOVERY_DB_PATH")
        else {
            return;
        };
        let database = connect_persistence_file(Path::new(&database_path), false).await;
        let operation = machine_operation::Entity::find()
            .filter(machine_operation::Column::Status.eq("saving"))
            .one(&database)
            .await
            .unwrap()
            .expect("child process must observe a saving operation");
        let tenant_id = operation.tenant_id;
        let actor_id = Uuid::parse_str(&operation.requested_by_actor_id)
            .expect("fixture machine operation actor");
        let context = recovery_context(tenant_id, actor_id);
        let input = RecoverMachineOperationInput {
            operation_id: operation.id,
            expected_updated_at: operation.updated_at,
            proposal: recovery_proposal(operation.item_id),
            reason: "Recover the completed provider result after an interrupted save".to_string(),
        };
        let machine_port = Arc::new(RecoveryMachinePort {
            descriptor: descriptor(100),
            recover_calls: AtomicUsize::new(0),
            recovered_result: Some(result()),
        });
        let service = recovery_service(database.clone(), machine_port.clone());

        let recovered = service.recover_operation(context, input).await.unwrap();
        assert_eq!(recovered.operation_id, operation.id);
        assert_eq!(machine_port.recover_calls.load(Ordering::SeqCst), 1);
        drop(service);
        database.close().await.unwrap();
    }

    #[tokio::test]
    async fn stuck_save_recovery_is_audited_and_never_starts_a_new_execution() {
        let (database, tenant_id, actor_id, operation_id, _) = persistence_fixture(false).await;
        let machine_port = Arc::new(RecoveryMachinePort {
            descriptor: descriptor(100),
            recover_calls: AtomicUsize::new(0),
            recovered_result: None,
        });
        let service = TranslationMachineService::new(
            database.clone(),
            Arc::new(TranslationTargetRegistry::default()),
            Arc::new(RecoveryTenantLocalePolicies),
            TransactionalEventBus::new(Arc::new(OutboxTransport::new(database.clone()))),
            machine_port.clone(),
        );
        let context = machine_control_context(tenant_id, actor_id, "recover-machine")
            .with_claim(Permission::new(Resource::Translations, Action::Manage).to_string())
            .with_claim(Permission::new(Resource::Translations, Action::Update).to_string());
        let operation = find_operation(&database, tenant_id, operation_id)
            .await
            .unwrap();
        let proposal = GenerateMachineProposalInput {
            item_id: operation.item_id,
            field_keys: vec![FieldKey::new("alt_text").unwrap()],
            minimum_memory_similarity_basis_points: 7_000,
            tone: None,
            domain: None,
            style: None,
        };
        let item = find_item(&database, tenant_id, proposal.item_id)
            .await
            .unwrap();
        let request = service
            .build_request(tenant_id, &item, &snapshot(), &proposal, Some(&operation))
            .await
            .unwrap();
        let observed_updated_at = Utc::now().fixed_offset();
        machine_operation::Entity::update_many()
            .col_expr(machine_operation::Column::Status, Expr::value("saving"))
            .col_expr(
                machine_operation::Column::CommandHash,
                Expr::value(hash_manifest(&proposal).unwrap()),
            )
            .col_expr(
                machine_operation::Column::MachineRequestDigest,
                Expr::value(hash_manifest(&request).unwrap()),
            )
            .col_expr(
                machine_operation::Column::ProviderPolicyDigest,
                Expr::value(descriptor(100).policy_digest),
            )
            .col_expr(
                machine_operation::Column::UpdatedAt,
                Expr::value(observed_updated_at),
            )
            .filter(machine_operation::Column::Id.eq(operation_id))
            .exec(&database)
            .await
            .unwrap();
        let input = RecoverMachineOperationInput {
            operation_id,
            expected_updated_at: observed_updated_at,
            proposal,
            reason: "Recover the completed provider result after an interrupted save".to_string(),
        };

        for attempt_context in [context.clone(), context] {
            let error = service
                .recover_operation(attempt_context, input.clone())
                .await
                .unwrap_err();
            assert!(matches!(
                error,
                TranslationError::MachineRecoveryResultUnavailable
            ));
        }
        assert_eq!(machine_port.recover_calls.load(Ordering::SeqCst), 2);
        assert_eq!(
            machine_recovery::Entity::find()
                .filter(machine_recovery::Column::OperationId.eq(operation_id))
                .count(&database)
                .await
                .unwrap(),
            1
        );
    }
}
