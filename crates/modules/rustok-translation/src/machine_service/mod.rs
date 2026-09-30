pub mod types;
pub(crate) mod cancellation;
pub(crate) mod helpers;
pub(crate) mod operations;

#[cfg(test)]
mod tests;

use std::{
    collections::BTreeSet,
    sync::Arc,
};

use rustok_api::{PortContext, manifest_hash::hash_manifest};
use rustok_core::generate_id;
use rustok_outbox::TransactionalEventBus;
use rustok_tenant::TenantLocalePolicyPort;
use rustok_translation_targets::{
    FieldKey, TranslationResourceSnapshot, TranslationTargetRegistry,
};
use sea_orm::{
    DatabaseConnection, EntityTrait, Set, TransactionTrait, sea_query::OnConflict,
};
use uuid::Uuid;

use crate::{
    MachineTranslationBatchExecution, MachineTranslationBatchRequest,
    MachineTranslationEstimate, MachineTranslationPort, MachineTranslationProviderState,
    MachineTranslationResourceContext, MachineTranslationUnit, ProposalOrigin, ProposalValue,
    SaveProposalInput, TranslationError, TranslationMemoryService, TranslationResult,
    TranslationWorkflowService,
    entities::{job_item, machine_recovery, machine_operation},
};

pub use cancellation::{cancel_machine_operation, read_machine_operation_status};
pub use types::*;

pub(crate) use cancellation::*;
pub(crate) use helpers::*;
pub(crate) use operations::*;

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

pub struct TranslationMachineService {
    database: DatabaseConnection,
    workflow: TranslationWorkflowService,
    memory: TranslationMemoryService,
    machine_port: Arc<dyn MachineTranslationPort>,
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
    ) -> TranslationResult<MachineProposalOutcome> {
        let tenant_id = authorize_machine_generation(&context)?;
        validate_generation_input(&input)?;
        let idempotency_key = context.idempotency_key.clone().unwrap_or_default();
        let command_hash = hash_manifest(&input)?;

        let existing_operation =
            find_operation_by_idempotency(&self.database, tenant_id, &idempotency_key).await?;
        if let Some(existing) = existing_operation.as_ref() {
            validate_operation_replay(existing, &context, &command_hash)?;
            if existing.status == "completed" {
                return machine_proposal_record(existing.clone())
                    .map(Box::new)
                    .map(MachineProposalOutcome::Completed);
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
            return machine_proposal_record(operation)
                .map(Box::new)
                .map(MachineProposalOutcome::Completed);
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
            "completed" => {
                return machine_proposal_record(current)
                    .map(Box::new)
                    .map(MachineProposalOutcome::Completed);
            }
            "cancelled" => return Err(TranslationError::MachineOperationCancelled),
            _ => return Err(TranslationError::WorkflowRevisionConflict),
        }
        let machine_context = child_write_context(&context, "machine-port")?;
        let execution = self
            .machine_port
            .translate_batch(machine_context, request.clone())
            .await?;
        let result = match execution {
            MachineTranslationBatchExecution::Completed(result) => result,
            MachineTranslationBatchExecution::InProgress(evidence) => {
                let current = find_operation(&self.database, tenant_id, operation.id).await?;
                return machine_proposal_outcome(current, evidence);
            }
        };
        validate_machine_result(&request, &result)?;
        if let Some(completed) =
            begin_machine_proposal_save(&self.database, tenant_id, operation.id).await?
        {
            return Ok(MachineProposalOutcome::Completed(Box::new(completed)));
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
        .map(Box::new)
        .map(MachineProposalOutcome::Completed)
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
        let now = chrono::Utc::now().fixed_offset();
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
