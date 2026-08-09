use async_graphql::{Context, Object, Result};
use uuid::Uuid;

use crate::{
    AddItemInput, ApplyProposalInput, ApproveProposalInput, AssignItemInput, CancelJobInput,
    CancelMachineOperationInput, CreateGlossaryInput,
    CreateInterchangeExportArtifactInput as DomainCreateInterchangeExportArtifactInput,
    CreateJobInput, CreateWorkflowNoteInput, GenerateMachineProposalInput,
    ImportTranslationItemInput as DomainImportTranslationItemInput,
    ProcessInterchangeImportArtifactInput as DomainProcessInterchangeImportArtifactInput,
    ProposalValue, PurgeMemoryEntryInput, RecoverApplyInput, RecoverMachineOperationInput,
    ReplaceGlossaryTermsInput, ReplaceRequiredTargetLocalesInput, ResolveWorkflowNoteInput,
    RetryItemInput, SaveProposalInput, SetGlossaryActiveInput, SetMemoryRetentionInput,
    StoreInterchangeImportArtifactInput as DomainStoreInterchangeImportArtifactInput,
    SubmitProposalInput, TombstoneMemoryEntryInput, UnassignItemInput, UpdateGlossaryInput,
};

use super::{
    context::{read_port_context, runtime, translation_error, write_port_context},
    types::{
        AddTranslationJobItemInput, AssignTranslationItemInput,
        CancelMachineTranslationOperationInput, CancelTranslationJobInput,
        CreateTranslationGlossaryInput, CreateTranslationInterchangeExportArtifactInput,
        CreateTranslationJobInput, CreateTranslationWorkflowNoteInput,
        GenerateMachineTranslationProposalInput, ImportTranslationItemInput,
        MachineTranslationCancellation, MachineTranslationEstimate, MachineTranslationProposal,
        ProcessTranslationInterchangeImportArtifactInput,
        RecoverMachineTranslationOperationInput as GraphqlRecoverMachineTranslationOperationInput,
        RecoverTranslationApplyInput, ReplaceTranslationGlossaryTermsInput,
        ReplaceTranslationPolicyInput, ResolveTranslationWorkflowNoteInput,
        RetryTranslationItemInput, SaveTranslationProposalInput, SetTranslationGlossaryActiveInput,
        SetTranslationMemoryRetentionInput, StoreTranslationInterchangeImportArtifactInput,
        TransitionTranslationMemoryEntryInput, TransitionTranslationProposalInput,
        TranslationApply, TranslationAssignment, TranslationCancellation, TranslationGlossary,
        TranslationInterchangeArtifact, TranslationInventoryRebuild, TranslationInventorySync,
        TranslationJob, TranslationJobItem, TranslationJobProgress, TranslationMemoryMutation,
        TranslationPolicy, TranslationProposal, TranslationRetry, TranslationWorkflowNote,
        UnassignTranslationItemInput, UpdateTranslationGlossaryInput, parse_field_key,
        parse_interchange_document, parse_locale, parse_owner_slug, parse_resource_kind,
    },
};

#[derive(Default)]
pub struct TranslationMutation;

#[Object]
impl TranslationMutation {
    async fn create_translation_workflow_note(
        &self,
        ctx: &Context<'_>,
        input: CreateTranslationWorkflowNoteInput,
    ) -> Result<TranslationWorkflowNote> {
        let context = write_port_context(ctx, "create-workflow-note", input.idempotency_key)?;
        runtime(ctx)?
            .workflow_service()
            .collaboration_service()
            .create_workflow_note(
                context,
                CreateWorkflowNoteInput {
                    job_id: input.job_id,
                    item_id: input.item_id,
                    body: input.body,
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn resolve_translation_workflow_note(
        &self,
        ctx: &Context<'_>,
        input: ResolveTranslationWorkflowNoteInput,
    ) -> Result<TranslationWorkflowNote> {
        let context = write_port_context(ctx, "resolve-workflow-note", input.idempotency_key)?;
        runtime(ctx)?
            .workflow_service()
            .collaboration_service()
            .resolve_workflow_note(
                context,
                ResolveWorkflowNoteInput {
                    note_id: input.note_id,
                    expected_revision: input.expected_revision,
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn replace_translation_policy(
        &self,
        ctx: &Context<'_>,
        input: ReplaceTranslationPolicyInput,
    ) -> Result<TranslationPolicy> {
        let context = write_port_context(ctx, "replace-policy", input.idempotency_key)?;
        let required_target_locales = input
            .required_target_locales
            .into_iter()
            .map(parse_locale)
            .collect::<Result<Vec<_>>>()?;
        runtime(ctx)?
            .policy_service()
            .replace_required_target_locales(
                context,
                ReplaceRequiredTargetLocalesInput {
                    expected_revision: input.expected_revision,
                    required_target_locales,
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn create_translation_job(
        &self,
        ctx: &Context<'_>,
        input: CreateTranslationJobInput,
    ) -> Result<TranslationJob> {
        let context = write_port_context(ctx, "create-job", input.idempotency_key)?;
        runtime(ctx)?
            .workflow_service()
            .create_job(
                context,
                CreateJobInput {
                    source_locale: parse_locale(input.source_locale)?,
                    target_locale: parse_locale(input.target_locale)?,
                    glossary: input.glossary.map(Into::into),
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn create_translation_glossary(
        &self,
        ctx: &Context<'_>,
        input: CreateTranslationGlossaryInput,
    ) -> Result<TranslationGlossary> {
        let context = write_port_context(ctx, "create-glossary", input.idempotency_key)?;
        runtime(ctx)?
            .glossary_service()
            .create_glossary(
                context,
                CreateGlossaryInput {
                    name: input.name,
                    description: input.description,
                    source_locale: parse_locale(input.source_locale)?,
                    target_locale: parse_locale(input.target_locale)?,
                    scope: input.scope.try_into()?,
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn update_translation_glossary(
        &self,
        ctx: &Context<'_>,
        input: UpdateTranslationGlossaryInput,
    ) -> Result<TranslationGlossary> {
        let context = write_port_context(ctx, "update-glossary", input.idempotency_key)?;
        runtime(ctx)?
            .glossary_service()
            .update_glossary(
                context,
                UpdateGlossaryInput {
                    glossary_id: input.glossary_id,
                    expected_revision: input.expected_revision,
                    name: input.name,
                    description: input.description,
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn replace_translation_glossary_terms(
        &self,
        ctx: &Context<'_>,
        input: ReplaceTranslationGlossaryTermsInput,
    ) -> Result<TranslationGlossary> {
        let context = write_port_context(ctx, "replace-glossary-terms", input.idempotency_key)?;
        runtime(ctx)?
            .glossary_service()
            .replace_terms(
                context,
                ReplaceGlossaryTermsInput {
                    glossary_id: input.glossary_id,
                    expected_revision: input.expected_revision,
                    concepts: input.concepts.into_iter().map(Into::into).collect(),
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn set_translation_glossary_active(
        &self,
        ctx: &Context<'_>,
        input: SetTranslationGlossaryActiveInput,
    ) -> Result<TranslationGlossary> {
        let context = write_port_context(ctx, "set-glossary-active", input.idempotency_key)?;
        runtime(ctx)?
            .glossary_service()
            .set_active(
                context,
                SetGlossaryActiveInput {
                    glossary_id: input.glossary_id,
                    expected_revision: input.expected_revision,
                    is_active: input.is_active,
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn set_translation_memory_retention(
        &self,
        ctx: &Context<'_>,
        input: SetTranslationMemoryRetentionInput,
    ) -> Result<TranslationMemoryMutation> {
        let context = write_port_context(ctx, "set-memory-retention", input.idempotency_key)?;
        runtime(ctx)?
            .memory_service()
            .set_retention(
                context,
                SetMemoryRetentionInput {
                    entry_id: input.entry_id,
                    expected_revision: input.expected_revision,
                    policy: input.policy.into(),
                    retain_until: input.retain_until,
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn tombstone_translation_memory_entry(
        &self,
        ctx: &Context<'_>,
        input: TransitionTranslationMemoryEntryInput,
    ) -> Result<TranslationMemoryMutation> {
        let context = write_port_context(ctx, "tombstone-memory-entry", input.idempotency_key)?;
        runtime(ctx)?
            .memory_service()
            .tombstone_entry(
                context,
                TombstoneMemoryEntryInput {
                    entry_id: input.entry_id,
                    expected_revision: input.expected_revision,
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn purge_translation_memory_entry(
        &self,
        ctx: &Context<'_>,
        input: TransitionTranslationMemoryEntryInput,
    ) -> Result<TranslationMemoryMutation> {
        let context = write_port_context(ctx, "purge-memory-entry", input.idempotency_key)?;
        runtime(ctx)?
            .memory_service()
            .purge_entry(
                context,
                PurgeMemoryEntryInput {
                    entry_id: input.entry_id,
                    expected_revision: input.expected_revision,
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn add_translation_job_item(
        &self,
        ctx: &Context<'_>,
        input: AddTranslationJobItemInput,
    ) -> Result<TranslationJobItem> {
        let context = write_port_context(ctx, "add-job-item", input.idempotency_key)?;
        runtime(ctx)?
            .workflow_service()
            .add_item(
                context,
                AddItemInput {
                    job_id: input.job_id,
                    identity: input.identity.try_into()?,
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn save_translation_proposal(
        &self,
        ctx: &Context<'_>,
        input: SaveTranslationProposalInput,
    ) -> Result<TranslationProposal> {
        let context = write_port_context(ctx, "save-proposal", input.idempotency_key)?;
        let values = input
            .values
            .into_iter()
            .map(|value| {
                Ok(ProposalValue {
                    key: parse_field_key(value.key)?,
                    value: value.value,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        runtime(ctx)?
            .workflow_service()
            .save_proposal(
                context,
                SaveProposalInput {
                    item_id: input.item_id,
                    origin: input.origin.into(),
                    values,
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn import_translation_item(
        &self,
        ctx: &Context<'_>,
        input: ImportTranslationItemInput,
    ) -> Result<TranslationProposal> {
        let context = write_port_context(ctx, "import-item", input.idempotency_key)?;
        let values = input
            .values
            .into_iter()
            .map(|value| {
                Ok(ProposalValue {
                    key: parse_field_key(value.key)?,
                    value: value.value,
                })
            })
            .collect::<Result<Vec<_>>>()?;
        runtime(ctx)?
            .workflow_service()
            .interchange_service()
            .import_item(
                context,
                DomainImportTranslationItemInput {
                    schema_version: input.schema_version,
                    job_id: input.job_id,
                    item_id: input.item_id,
                    identity: input.identity.try_into()?,
                    source_digest: input.source_digest,
                    values,
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn create_translation_interchange_export_artifact(
        &self,
        ctx: &Context<'_>,
        input: CreateTranslationInterchangeExportArtifactInput,
    ) -> Result<TranslationInterchangeArtifact> {
        let context = write_port_context(
            ctx,
            "create-interchange-export-artifact",
            input.idempotency_key,
        )?;
        runtime(ctx)?
            .exchange_service()
            .map_err(translation_error)?
            .create_export_artifact(
                context,
                DomainCreateInterchangeExportArtifactInput {
                    job_id: input.job_id,
                    max_items: input.max_items,
                    expires_in_seconds: input.expires_in_seconds,
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn store_translation_interchange_import_artifact(
        &self,
        ctx: &Context<'_>,
        input: StoreTranslationInterchangeImportArtifactInput,
    ) -> Result<TranslationInterchangeArtifact> {
        let document = parse_interchange_document(&input.document_json)?;
        let context = write_port_context(
            ctx,
            "store-interchange-import-artifact",
            input.idempotency_key,
        )?;
        runtime(ctx)?
            .exchange_service()
            .map_err(translation_error)?
            .store_import_artifact(
                context,
                DomainStoreInterchangeImportArtifactInput {
                    job_id: input.job_id,
                    document,
                    expires_in_seconds: input.expires_in_seconds,
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn process_translation_interchange_import_artifact(
        &self,
        ctx: &Context<'_>,
        input: ProcessTranslationInterchangeImportArtifactInput,
    ) -> Result<TranslationInterchangeArtifact> {
        let context = write_port_context(
            ctx,
            "process-interchange-import-artifact",
            input.idempotency_key,
        )?;
        runtime(ctx)?
            .exchange_service()
            .map_err(translation_error)?
            .process_import_artifact(
                context,
                DomainProcessInterchangeImportArtifactInput {
                    artifact_id: input.artifact_id,
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn generate_machine_translation_proposal(
        &self,
        ctx: &Context<'_>,
        input: GenerateMachineTranslationProposalInput,
    ) -> Result<MachineTranslationProposal> {
        let mut context =
            write_port_context(ctx, "generate-machine-proposal", input.idempotency_key)?;
        context.deadline_ms = Some(120_000);
        let field_keys = input
            .field_keys
            .into_iter()
            .map(parse_field_key)
            .collect::<Result<Vec<_>>>()?;
        runtime(ctx)?
            .machine_service()
            .map_err(translation_error)?
            .generate_proposal(
                context,
                GenerateMachineProposalInput {
                    item_id: input.item_id,
                    field_keys,
                    minimum_memory_similarity_basis_points: input
                        .minimum_memory_similarity_basis_points,
                    tone: input.tone,
                    domain: input.domain,
                    style: input.style,
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn cancel_machine_translation_operation(
        &self,
        ctx: &Context<'_>,
        input: CancelMachineTranslationOperationInput,
    ) -> Result<MachineTranslationCancellation> {
        let context = write_port_context(ctx, "cancel-machine-operation", input.idempotency_key)?;
        runtime(ctx)?
            .machine_control_service()
            .cancel_operation(
                context,
                CancelMachineOperationInput {
                    operation_id: input.operation_id,
                    reason: input.reason,
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn recover_machine_translation_operation(
        &self,
        ctx: &Context<'_>,
        input: GraphqlRecoverMachineTranslationOperationInput,
    ) -> Result<MachineTranslationProposal> {
        let mut context =
            write_port_context(ctx, "recover-machine-operation", input.idempotency_key)?;
        context.deadline_ms = Some(120_000);
        let proposal = input.proposal;
        let field_keys = proposal
            .field_keys
            .into_iter()
            .map(parse_field_key)
            .collect::<Result<Vec<_>>>()?;
        runtime(ctx)?
            .machine_service()
            .map_err(translation_error)?
            .recover_operation(
                context,
                RecoverMachineOperationInput {
                    operation_id: input.operation_id,
                    expected_updated_at: input.expected_updated_at,
                    proposal: GenerateMachineProposalInput {
                        item_id: proposal.item_id,
                        field_keys,
                        minimum_memory_similarity_basis_points: proposal
                            .minimum_memory_similarity_basis_points,
                        tone: proposal.tone,
                        domain: proposal.domain,
                        style: proposal.style,
                    },
                    reason: input.reason,
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn submit_translation_proposal(
        &self,
        ctx: &Context<'_>,
        input: TransitionTranslationProposalInput,
    ) -> Result<TranslationProposal> {
        let context = write_port_context(ctx, "submit-proposal", input.idempotency_key)?;
        runtime(ctx)?
            .workflow_service()
            .submit_proposal(
                context,
                SubmitProposalInput {
                    item_id: input.item_id,
                    proposal_id: input.proposal_id,
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn approve_translation_proposal(
        &self,
        ctx: &Context<'_>,
        input: TransitionTranslationProposalInput,
    ) -> Result<TranslationProposal> {
        let context = write_port_context(ctx, "approve-proposal", input.idempotency_key)?;
        runtime(ctx)?
            .workflow_service()
            .approve_proposal(
                context,
                ApproveProposalInput {
                    item_id: input.item_id,
                    proposal_id: input.proposal_id,
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn apply_translation_proposal(
        &self,
        ctx: &Context<'_>,
        input: TransitionTranslationProposalInput,
    ) -> Result<TranslationApply> {
        let context = write_port_context(ctx, "apply-proposal", input.idempotency_key)?;
        runtime(ctx)?
            .workflow_service()
            .apply_proposal(
                context,
                ApplyProposalInput {
                    item_id: input.item_id,
                    proposal_id: input.proposal_id,
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn assign_translation_item(
        &self,
        ctx: &Context<'_>,
        input: AssignTranslationItemInput,
    ) -> Result<TranslationAssignment> {
        let context = write_port_context(ctx, "assign-item", input.idempotency_key)?;
        runtime(ctx)?
            .workflow_service()
            .assign_item(
                context,
                AssignItemInput {
                    item_id: input.item_id,
                    expected_revision: input.expected_revision,
                    assignee: input.assignee.into(),
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn unassign_translation_item(
        &self,
        ctx: &Context<'_>,
        input: UnassignTranslationItemInput,
    ) -> Result<TranslationAssignment> {
        let context = write_port_context(ctx, "unassign-item", input.idempotency_key)?;
        runtime(ctx)?
            .workflow_service()
            .unassign_item(
                context,
                UnassignItemInput {
                    item_id: input.item_id,
                    expected_revision: input.expected_revision,
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn cancel_translation_job(
        &self,
        ctx: &Context<'_>,
        input: CancelTranslationJobInput,
    ) -> Result<TranslationCancellation> {
        let context = write_port_context(ctx, "cancel-job", input.idempotency_key)?;
        runtime(ctx)?
            .workflow_service()
            .cancel_job(
                context,
                CancelJobInput {
                    job_id: input.job_id,
                    expected_revision: input.expected_revision,
                    reason: input.reason,
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn retry_translation_item(
        &self,
        ctx: &Context<'_>,
        input: RetryTranslationItemInput,
    ) -> Result<TranslationRetry> {
        let context = write_port_context(ctx, "retry-item", input.idempotency_key)?;
        runtime(ctx)?
            .workflow_service()
            .retry_item(
                context,
                RetryItemInput {
                    item_id: input.item_id,
                    expected_revision: input.expected_revision,
                    reason: input.reason,
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn recover_translation_apply(
        &self,
        ctx: &Context<'_>,
        input: RecoverTranslationApplyInput,
    ) -> Result<TranslationApply> {
        let context = write_port_context(ctx, "recover-apply", input.idempotency_key)?;
        runtime(ctx)?
            .workflow_service()
            .recover_apply(
                context,
                RecoverApplyInput {
                    operation_id: input.operation_id,
                    expected_attempt_count: input.expected_attempt_count,
                    reason: input.reason,
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn rebuild_translation_job_progress(
        &self,
        ctx: &Context<'_>,
        job_id: Uuid,
        idempotency_key: String,
    ) -> Result<TranslationJobProgress> {
        let context = write_port_context(ctx, "rebuild-job-progress", idempotency_key)?;
        runtime(ctx)?
            .progress_service()
            .rebuild_job_progress(context, job_id)
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn sync_translation_provider_inventory(
        &self,
        ctx: &Context<'_>,
        owner_slug: String,
        resource_kind: String,
        limit: u16,
    ) -> Result<TranslationInventorySync> {
        let context = read_port_context(ctx, "sync-provider-inventory")?;
        runtime(ctx)?
            .inventory_service()
            .sync_provider_changes(
                context,
                parse_owner_slug(owner_slug)?,
                parse_resource_kind(resource_kind)?,
                limit,
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn rebuild_translation_provider_inventory(
        &self,
        ctx: &Context<'_>,
        owner_slug: String,
        resource_kind: String,
        source_locale: String,
        target_locale: String,
        page_size: u16,
    ) -> Result<TranslationInventoryRebuild> {
        let context = read_port_context(ctx, "rebuild-provider-inventory")?;
        runtime(ctx)?
            .inventory_service()
            .rebuild_provider_inventory(
                context,
                parse_owner_slug(owner_slug)?,
                parse_resource_kind(resource_kind)?,
                parse_locale(source_locale)?,
                parse_locale(target_locale)?,
                page_size,
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn estimate_machine_translation(
        &self,
        ctx: &Context<'_>,
        input: GenerateMachineTranslationProposalInput,
    ) -> Result<MachineTranslationEstimate> {
        let mut context =
            write_port_context(ctx, "estimate-machine-translation", input.idempotency_key)?;
        context.deadline_ms = Some(120_000);
        let field_keys = input
            .field_keys
            .into_iter()
            .map(parse_field_key)
            .collect::<Result<Vec<_>>>()?;
        runtime(ctx)?
            .machine_service()
            .map_err(translation_error)?
            .estimate_proposal(
                context,
                GenerateMachineProposalInput {
                    item_id: input.item_id,
                    field_keys,
                    minimum_memory_similarity_basis_points: input
                        .minimum_memory_similarity_basis_points,
                    tone: input.tone,
                    domain: input.domain,
                    style: input.style,
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }
}
