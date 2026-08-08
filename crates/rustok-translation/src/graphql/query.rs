use async_graphql::{Context, Object, Result};
use rustok_api::Action;
use uuid::Uuid;

use crate::{MemoryListInput, MemoryLookupInput, ReviewerQueueInput, ReviewerWorkloadInput};

use super::{
    context::{read_port_context, require_translation_permission, runtime, translation_error},
    types::{
        ExportTranslationJobInput, LookupTranslationMemoryInput, MachineTranslationOperationStatus,
        TranslationGlossary, TranslationGlossarySummary, TranslationInterchangeDocument,
        TranslationJobProgress, TranslationMemoryEntry, TranslationMemorySuggestion,
        TranslationPolicy, TranslationProviderProgress, TranslationRequiredProviderProgress,
        TranslationReviewerQueueInput, TranslationReviewerQueueItem, TranslationReviewerWorkload,
        TranslationReviewerWorkloadInput, TranslationTargetDescriptor, parse_field_key,
        parse_locale, parse_owner_slug, parse_resource_kind,
    },
};

#[derive(Default)]
pub struct TranslationQuery;

#[Object]
impl TranslationQuery {
    async fn translation_policy(&self, ctx: &Context<'_>) -> Result<TranslationPolicy> {
        let context = read_port_context(ctx, "policy")?;
        runtime(ctx)?
            .policy_service()
            .read_policy(context)
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn translation_targets(
        &self,
        ctx: &Context<'_>,
    ) -> Result<Vec<TranslationTargetDescriptor>> {
        let context = read_port_context(ctx, "targets")?;
        require_translation_permission(&context, Action::Read)?;
        Ok(runtime(ctx)?
            .providers()
            .descriptors()
            .into_iter()
            .map(Into::into)
            .collect())
    }

    async fn machine_translation_operation_status(
        &self,
        ctx: &Context<'_>,
        operation_id: Uuid,
    ) -> Result<MachineTranslationOperationStatus> {
        let context = read_port_context(ctx, "machine-operation-status")?;
        runtime(ctx)?
            .machine_control_service()
            .operation_status(context, operation_id)
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn translation_glossaries(
        &self,
        ctx: &Context<'_>,
        #[graphql(default = 50)] limit: u16,
    ) -> Result<Vec<TranslationGlossarySummary>> {
        let context = read_port_context(ctx, "glossaries")?;
        runtime(ctx)?
            .glossary_service()
            .list_glossaries(context, limit)
            .await
            .map(|records| records.into_iter().map(Into::into).collect())
            .map_err(translation_error)
    }

    async fn translation_glossary(
        &self,
        ctx: &Context<'_>,
        glossary_id: Uuid,
        revision: Option<i64>,
    ) -> Result<TranslationGlossary> {
        let context = read_port_context(ctx, "glossary")?;
        runtime(ctx)?
            .glossary_service()
            .read_glossary(context, glossary_id, revision)
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn translation_memory_entries(
        &self,
        ctx: &Context<'_>,
        source_locale: Option<String>,
        target_locale: Option<String>,
        #[graphql(default = false)] include_tombstoned: bool,
        #[graphql(default = 50)] limit: u16,
    ) -> Result<Vec<TranslationMemoryEntry>> {
        let context = read_port_context(ctx, "memory-entries")?;
        runtime(ctx)?
            .memory_service()
            .list_entries(
                context,
                MemoryListInput {
                    source_locale: source_locale.map(parse_locale).transpose()?,
                    target_locale: target_locale.map(parse_locale).transpose()?,
                    include_tombstoned,
                    limit,
                },
            )
            .await
            .map(|records| records.into_iter().map(Into::into).collect())
            .map_err(translation_error)
    }

    async fn translation_memory_entry(
        &self,
        ctx: &Context<'_>,
        entry_id: Uuid,
    ) -> Result<TranslationMemoryEntry> {
        let context = read_port_context(ctx, "memory-entry")?;
        runtime(ctx)?
            .memory_service()
            .read_entry(context, entry_id)
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn translation_memory_suggestions(
        &self,
        ctx: &Context<'_>,
        input: LookupTranslationMemoryInput,
    ) -> Result<Vec<TranslationMemorySuggestion>> {
        let context = read_port_context(ctx, "memory-suggestions")?;
        runtime(ctx)?
            .memory_service()
            .lookup(
                context,
                MemoryLookupInput {
                    source_locale: parse_locale(input.source_locale)?,
                    target_locale: parse_locale(input.target_locale)?,
                    identity: input.identity.try_into()?,
                    field_key: parse_field_key(input.field_key)?,
                    source_text: input.source_text,
                    minimum_similarity_basis_points: input.minimum_similarity_basis_points,
                    limit: input.limit,
                },
            )
            .await
            .map(|records| records.into_iter().map(Into::into).collect())
            .map_err(translation_error)
    }

    async fn translation_job_progress(
        &self,
        ctx: &Context<'_>,
        job_id: Uuid,
    ) -> Result<TranslationJobProgress> {
        let context = read_port_context(ctx, "job-progress")?;
        runtime(ctx)?
            .progress_service()
            .read_job_progress(context, job_id)
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn translation_reviewer_queue(
        &self,
        ctx: &Context<'_>,
        input: TranslationReviewerQueueInput,
    ) -> Result<Vec<TranslationReviewerQueueItem>> {
        let context = read_port_context(ctx, "reviewer-queue")?;
        runtime(ctx)?
            .progress_service()
            .list_reviewer_queue(
                context,
                ReviewerQueueInput {
                    job_id: input.job_id,
                    assignee: input.assignee.map(Into::into),
                    include_unassigned: input.include_unassigned,
                    limit: input.limit,
                },
            )
            .await
            .map(|records| records.into_iter().map(Into::into).collect())
            .map_err(translation_error)
    }

    async fn translation_reviewer_workload(
        &self,
        ctx: &Context<'_>,
        input: TranslationReviewerWorkloadInput,
    ) -> Result<Vec<TranslationReviewerWorkload>> {
        let context = read_port_context(ctx, "reviewer-workload")?;
        runtime(ctx)?
            .progress_service()
            .list_reviewer_workload(
                context,
                ReviewerWorkloadInput {
                    job_id: input.job_id,
                },
            )
            .await
            .map(|records| records.into_iter().map(Into::into).collect())
            .map_err(translation_error)
    }

    async fn export_translation_job(
        &self,
        ctx: &Context<'_>,
        input: ExportTranslationJobInput,
    ) -> Result<TranslationInterchangeDocument> {
        let context = read_port_context(ctx, "export-job")?;
        runtime(ctx)?
            .workflow_service()
            .interchange_service()
            .export_job(
                context,
                crate::ExportTranslationJobInput {
                    job_id: input.job_id,
                    max_items: input.max_items,
                },
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn translation_provider_progress(
        &self,
        ctx: &Context<'_>,
        owner_slug: String,
        resource_kind: String,
        source_locale: String,
        target_locale: String,
    ) -> Result<TranslationProviderProgress> {
        let context = read_port_context(ctx, "provider-progress")?;
        runtime(ctx)?
            .progress_service()
            .read_provider_progress(
                context,
                parse_owner_slug(owner_slug)?,
                parse_resource_kind(resource_kind)?,
                parse_locale(source_locale)?,
                parse_locale(target_locale)?,
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }

    async fn translation_required_provider_progress(
        &self,
        ctx: &Context<'_>,
        owner_slug: String,
        resource_kind: String,
        source_locale: String,
    ) -> Result<TranslationRequiredProviderProgress> {
        let context = read_port_context(ctx, "required-provider-progress")?;
        runtime(ctx)?
            .progress_service()
            .read_required_provider_progress(
                context,
                parse_owner_slug(owner_slug)?,
                parse_resource_kind(resource_kind)?,
                parse_locale(source_locale)?,
            )
            .await
            .map(Into::into)
            .map_err(translation_error)
    }
}
