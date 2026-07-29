use async_graphql::{Context, Object, Result};
use uuid::Uuid;

use crate::{
    AddItemInput, ApplyProposalInput, ApproveProposalInput, AssignItemInput, CancelJobInput,
    CreateGlossaryInput, CreateJobInput, ProposalValue, PurgeMemoryEntryInput, RecoverApplyInput,
    ReplaceGlossaryTermsInput, ReplaceRequiredTargetLocalesInput, RetryItemInput,
    SaveProposalInput, SetGlossaryActiveInput, SetMemoryRetentionInput, SubmitProposalInput,
    TombstoneMemoryEntryInput, UnassignItemInput, UpdateGlossaryInput,
};

use super::{
    context::{read_port_context, runtime, translation_error, write_port_context},
    types::{
        AddTranslationJobItemInput, AssignTranslationItemInput, CancelTranslationJobInput,
        CreateTranslationGlossaryInput, CreateTranslationJobInput, RecoverTranslationApplyInput,
        ReplaceTranslationGlossaryTermsInput, ReplaceTranslationPolicyInput,
        RetryTranslationItemInput, SaveTranslationProposalInput, SetTranslationGlossaryActiveInput,
        SetTranslationMemoryRetentionInput, TransitionTranslationMemoryEntryInput,
        TransitionTranslationProposalInput, TranslationApply, TranslationAssignment,
        TranslationCancellation, TranslationGlossary, TranslationInventoryRebuild,
        TranslationInventorySync, TranslationJob, TranslationJobItem, TranslationJobProgress,
        TranslationMemoryMutation, TranslationPolicy, TranslationProposal, TranslationRetry,
        UnassignTranslationItemInput, UpdateTranslationGlossaryInput, parse_field_key,
        parse_locale, parse_owner_slug, parse_resource_kind,
    },
};

#[derive(Default)]
pub struct TranslationMutation;

#[Object]
impl TranslationMutation {
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
}
