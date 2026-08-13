//! Headless GraphQL adapter for the shared Translation admin contract.

#[cfg(target_arch = "wasm32")]
use leptos::web_sys;
use rustok_graphql::{GraphqlHttpError, GraphqlRequest, execute as execute_graphql};
use serde::{Deserialize, de::DeserializeOwned};
use serde_json::{Value, json};

use crate::model::{
    Actor, ActorKind, JobItem, MemoryRetentionPolicy, ProposalOrigin, ReviewerQueueItem,
    TranslationAdminOperation, TranslationAdminResponse, TranslationAdminTransportContext,
    TranslationResourceIdentity,
};

pub type TransportError = GraphqlHttpError;

const POLICY_FIELDS: &str = "tenantId requiredTargetLocales tenantLocalePolicyRevision revision freshness disabledRequiredTargetLocales";
const JOB_PROGRESS_FIELDS: &str = "jobId sourceDigest totalItems assignedItems terminalItems missingItems draftItems inReviewItems approvedItems applyingItems appliedItems staleItems conflictItems blockedItems excludedItems cancelledItems requiredUnits optionalUnits appliedRequiredUnits appliedOptionalUnits approvedRequiredUnits approvedOptionalUnits completeResources sourceCharacters translatedCharacters revision updatedAt";
const PROVIDER_PROGRESS_FIELDS: &str = "ownerSlug resourceKind sourceLocale targetLocale requiredUnits exactRequiredUnits optionalUnits exactOptionalUnits resources completeResources ownerChangeCursor projectedCursor checkpointRevision checkpointUpdatedAt freshness";
const PROPOSAL_FIELDS: &str = "id itemId proposalRevision origin values { key value expectedSourceHash } qaIssues { field severity code message } qaAccepted status approvalReceiptId";
const MACHINE_ESTIMATE_FIELDS: &str = "inputTokensUpperBound outputTokensUpperBound attemptsUpperBound costMinorUnitsUpperBound currencyCode priceSnapshotDigest reviewRequired";
const MACHINE_PROPOSAL_FIELDS: &str = "operationId itemId proposalId adapterSlug providerSlug providerPolicyDigest machineRequestDigest glossaryRevision glossaryDigest memoryDigest executionId executionRequestDigest promptPolicyDigest attempts { attempt providerProfileId providerSlug model fallback } usage { inputTokens outputTokens totalTokens costMinorUnits currencyCode priceSnapshotDigest } diagnostics { code blocking unitId } reviewRequired createdAt updatedAt";
const MACHINE_CANCELLATION_FIELDS: &str = "cancellationId operationId status providerExecutionId providerStatus providerErrorCode providerObservedAt createdAt";
const MACHINE_OPERATION_STATUS_FIELDS: &str =
    "operationId itemId status providerExecutionId providerStatus providerErrorCode updatedAt";
const GLOSSARY_SUMMARY_FIELDS: &str = "id name description sourceLocale targetLocale scope { ownerSlug resourceKind fieldKey } isActive revision";
const GLOSSARY_FIELDS: &str = "id name description sourceLocale targetLocale scope { ownerSlug resourceKind fieldKey } isActive revision concepts { conceptKey sourceTerm variants { value policy } matchKind caseSensitive notes }";
const MEMORY_ENTRY_FIELDS: &str = "id tenantId sourceLocale targetLocale ownerSlug resourceKind resourceId subresourceId fieldKey sourceText targetText sourceHash targetHash contextFingerprint segmentationVersion origin qualityState reviewerActorKind reviewerActorId proposalId applyReceiptId retentionPolicy retainUntil tombstonedAt revision createdAt updatedAt";
const MEMORY_SUGGESTION_FIELDS: &str = "entryId sourceText targetText sourceHash ownerSlug resourceKind resourceId fieldKey origin proposalId applyReceiptId evidence { kind sourceExact contextMatch baseSimilarityBasisPoints contextBonusBasisPoints finalSimilarityBasisPoints segmentationVersion }";
const MEMORY_MUTATION_FIELDS: &str =
    "entryId revision state retentionPolicy retainUntil tombstonedAt";
const JOB_FIELDS: &str =
    "id sourceLocale targetLocale glossary { glossaryId revision } status revision";
const INTERCHANGE_DOCUMENT_FIELDS: &str = "schemaVersion jobId sourceLocale targetLocale items { itemId identity { ownerSlug resourceKind resourceId subresourceId } sourceDigest sourceRevision targetRevision fields { key sourceValue exactTargetValue proposedValue sourceHash required maxCharacters protectedTokens } }";
const INTERCHANGE_ARTIFACT_FIELDS: &str = "id jobId direction status contentLength checksumSha256 expiresAt processedAt report { totalItems acceptedItems conflictItems rejectedItems outcomes { itemId status } } createdAt updatedAt";
const INTERCHANGE_ARTIFACT_CONTENT_FIELDS: &str = "artifact { id jobId direction status contentLength checksumSha256 expiresAt processedAt report { totalItems acceptedItems conflictItems rejectedItems outcomes { itemId status } } createdAt updatedAt } document { schemaVersion jobId sourceLocale targetLocale items { itemId identity { ownerSlug resourceKind resourceId subresourceId } sourceDigest sourceRevision targetRevision fields { key sourceValue exactTargetValue proposedValue sourceHash required maxCharacters protectedTokens } } }";
const REVIEWER_QUEUE_FIELDS: &str = "item { id jobId ownerSlug resourceKind resourceId subresourceId status assignee { kind id } sourceDigest revision } proposalId proposalRevision submittedAt";
const REVIEWER_WORKLOAD_FIELDS: &str = "jobId assignee { kind id } openItems missingItems draftItems inReviewItems approvedItems applyingItems rebaseRequiredItems blockedItems sourceCharacters";
const WORKFLOW_NOTE_FIELDS: &str = "id jobId itemId body author { kind id } revision resolvedAt resolvedBy { kind id } createdAt updatedAt";

pub async fn execute(
    context: TranslationAdminTransportContext,
    operation: TranslationAdminOperation,
) -> Result<TranslationAdminResponse, TransportError> {
    let (query, variables, field) = operation_graphql(&operation);
    let data = request(query, variables, context).await?;

    match operation {
        TranslationAdminOperation::ReadPolicy | TranslationAdminOperation::ReplacePolicy { .. } => {
            Ok(TranslationAdminResponse::Policy(field_value(&data, field)?))
        }
        TranslationAdminOperation::ReadMachineOperationStatus { .. } => Ok(
            TranslationAdminResponse::MachineOperationStatus(field_value(&data, field)?),
        ),
        TranslationAdminOperation::ListTargets => Ok(TranslationAdminResponse::Targets(
            field_value(&data, field)?,
        )),
        TranslationAdminOperation::ListGlossaries { .. } => Ok(
            TranslationAdminResponse::Glossaries(field_value(&data, field)?),
        ),
        TranslationAdminOperation::ReadGlossary { .. }
        | TranslationAdminOperation::CreateGlossary { .. }
        | TranslationAdminOperation::UpdateGlossary { .. }
        | TranslationAdminOperation::ReplaceGlossaryTerms { .. }
        | TranslationAdminOperation::SetGlossaryActive { .. } => Ok(
            TranslationAdminResponse::Glossary(field_value(&data, field)?),
        ),
        TranslationAdminOperation::ListMemoryEntries { .. } => Ok(
            TranslationAdminResponse::MemoryEntries(field_value(&data, field)?),
        ),
        TranslationAdminOperation::ReadMemoryEntry { .. } => Ok(
            TranslationAdminResponse::MemoryEntry(field_value(&data, field)?),
        ),
        TranslationAdminOperation::LookupMemory { .. } => Ok(
            TranslationAdminResponse::MemorySuggestions(field_value(&data, field)?),
        ),
        TranslationAdminOperation::SetMemoryRetention { .. }
        | TranslationAdminOperation::TombstoneMemoryEntry { .. }
        | TranslationAdminOperation::PurgeMemoryEntry { .. } => Ok(
            TranslationAdminResponse::MemoryMutation(field_value(&data, field)?),
        ),
        TranslationAdminOperation::ReadJobProgress { .. }
        | TranslationAdminOperation::RebuildJobProgress { .. } => Ok(
            TranslationAdminResponse::JobProgress(field_value(&data, field)?),
        ),
        TranslationAdminOperation::ReadReviewerQueue { .. } => {
            let queue: Vec<GraphqlReviewerQueueItem> = field_value(&data, field)?;
            Ok(TranslationAdminResponse::ReviewerQueue(
                queue.into_iter().map(Into::into).collect(),
            ))
        }
        TranslationAdminOperation::ReadReviewerWorkload { .. } => Ok(
            TranslationAdminResponse::ReviewerWorkloads(field_value(&data, field)?),
        ),
        TranslationAdminOperation::ListWorkflowNotes { .. } => Ok(
            TranslationAdminResponse::WorkflowNotes(field_value(&data, field)?),
        ),
        TranslationAdminOperation::ExportJob { .. } => Ok(
            TranslationAdminResponse::InterchangeDocument(field_value(&data, field)?),
        ),
        TranslationAdminOperation::ListInterchangeArtifacts { .. } => Ok(
            TranslationAdminResponse::InterchangeArtifacts(field_value(&data, field)?),
        ),
        TranslationAdminOperation::ReadInterchangeArtifact { .. } => Ok(
            TranslationAdminResponse::InterchangeArtifactContent(field_value(&data, field)?),
        ),
        TranslationAdminOperation::ReadProviderProgress { .. } => Ok(
            TranslationAdminResponse::ProviderProgress(field_value(&data, field)?),
        ),
        TranslationAdminOperation::ReadRequiredProviderProgress { .. } => Ok(
            TranslationAdminResponse::RequiredProviderProgress(field_value(&data, field)?),
        ),
        TranslationAdminOperation::CreateJob { .. } => {
            Ok(TranslationAdminResponse::Job(field_value(&data, field)?))
        }
        TranslationAdminOperation::CreateWorkflowNote { .. }
        | TranslationAdminOperation::ResolveWorkflowNote { .. } => Ok(
            TranslationAdminResponse::WorkflowNote(field_value(&data, field)?),
        ),
        TranslationAdminOperation::CreateInterchangeExportArtifact { .. }
        | TranslationAdminOperation::StoreInterchangeImportArtifact { .. }
        | TranslationAdminOperation::ProcessInterchangeImportArtifact { .. } => Ok(
            TranslationAdminResponse::InterchangeArtifact(field_value(&data, field)?),
        ),
        TranslationAdminOperation::AddItem { .. } => {
            let item: GraphqlJobItem = field_value(&data, field)?;
            Ok(TranslationAdminResponse::Item(item.into()))
        }
        TranslationAdminOperation::SaveProposal { .. }
        | TranslationAdminOperation::ImportItem { .. }
        | TranslationAdminOperation::SubmitProposal { .. }
        | TranslationAdminOperation::ApproveProposal { .. } => Ok(
            TranslationAdminResponse::Proposal(field_value(&data, field)?),
        ),
        TranslationAdminOperation::EstimateMachineTranslation { .. } => Ok(
            TranslationAdminResponse::MachineEstimate(field_value(&data, field)?),
        ),
        TranslationAdminOperation::GenerateMachineProposal { .. } => {
            let outcome: GraphqlMachineProposalOutcome = field_value(&data, field)?;
            Ok(match outcome {
                GraphqlMachineProposalOutcome::Completed(proposal) => {
                    TranslationAdminResponse::MachineProposal(*proposal)
                }
                GraphqlMachineProposalOutcome::InProgress(status) => {
                    TranslationAdminResponse::MachineOperationStatus(status)
                }
            })
        }
        TranslationAdminOperation::RecoverMachineOperation { .. } => Ok(
            TranslationAdminResponse::MachineProposal(field_value(&data, field)?),
        ),
        TranslationAdminOperation::CancelMachineOperation { .. } => Ok(
            TranslationAdminResponse::MachineCancellation(field_value(&data, field)?),
        ),
        TranslationAdminOperation::ApplyProposal { .. }
        | TranslationAdminOperation::RecoverApply { .. } => {
            Ok(TranslationAdminResponse::Apply(field_value(&data, field)?))
        }
        TranslationAdminOperation::AssignItem { .. }
        | TranslationAdminOperation::UnassignItem { .. } => Ok(
            TranslationAdminResponse::Assignment(field_value(&data, field)?),
        ),
        TranslationAdminOperation::CancelJob { .. } => Ok(TranslationAdminResponse::Cancellation(
            field_value(&data, field)?,
        )),
        TranslationAdminOperation::RetryItem { .. } => {
            Ok(TranslationAdminResponse::Retry(field_value(&data, field)?))
        }
        TranslationAdminOperation::SyncProviderInventory { .. }
        | TranslationAdminOperation::RebuildProviderInventory { .. } => Ok(
            TranslationAdminResponse::Inventory(field_value(&data, field)?),
        ),
    }
}

fn operation_graphql(operation: &TranslationAdminOperation) -> (String, Value, &'static str) {
    match operation {
        TranslationAdminOperation::ReadPolicy => (
            format!("query TranslationPolicy {{ translationPolicy {{ {POLICY_FIELDS} }} }}"),
            json!({}),
            "translationPolicy",
        ),
        TranslationAdminOperation::ReadMachineOperationStatus { operation_id } => (
            format!(
                "query MachineTranslationOperationStatus($operationId: UUID!) {{ machineTranslationOperationStatus(operationId: $operationId) {{ {MACHINE_OPERATION_STATUS_FIELDS} }} }}"
            ),
            json!({ "operationId": operation_id }),
            "machineTranslationOperationStatus",
        ),
        TranslationAdminOperation::ListTargets => (
            "query TranslationTargets { translationTargets { ownerSlug resourceKind displayName capabilities readPermissionFloor applyPermissionFloor } }".to_string(),
            json!({}),
            "translationTargets",
        ),
        TranslationAdminOperation::ListGlossaries { limit } => (
            format!("query TranslationGlossaries($limit: Int!) {{ translationGlossaries(limit: $limit) {{ {GLOSSARY_SUMMARY_FIELDS} }} }}"),
            json!({ "limit": limit }),
            "translationGlossaries",
        ),
        TranslationAdminOperation::ReadGlossary {
            glossary_id,
            revision,
        } => (
            format!("query TranslationGlossary($glossaryId: UUID!, $revision: Int) {{ translationGlossary(glossaryId: $glossaryId, revision: $revision) {{ {GLOSSARY_FIELDS} }} }}"),
            json!({ "glossaryId": glossary_id, "revision": revision }),
            "translationGlossary",
        ),
        TranslationAdminOperation::ListMemoryEntries {
            source_locale,
            target_locale,
            include_tombstoned,
            limit,
        } => (
            format!("query TranslationMemoryEntries($sourceLocale: String, $targetLocale: String, $includeTombstoned: Boolean!, $limit: Int!) {{ translationMemoryEntries(sourceLocale: $sourceLocale, targetLocale: $targetLocale, includeTombstoned: $includeTombstoned, limit: $limit) {{ {MEMORY_ENTRY_FIELDS} }} }}"),
            json!({
                "sourceLocale": source_locale,
                "targetLocale": target_locale,
                "includeTombstoned": include_tombstoned,
                "limit": limit,
            }),
            "translationMemoryEntries",
        ),
        TranslationAdminOperation::ReadMemoryEntry { entry_id } => (
            format!("query TranslationMemoryEntry($entryId: UUID!) {{ translationMemoryEntry(entryId: $entryId) {{ {MEMORY_ENTRY_FIELDS} }} }}"),
            json!({ "entryId": entry_id }),
            "translationMemoryEntry",
        ),
        TranslationAdminOperation::LookupMemory {
            source_locale,
            target_locale,
            identity,
            field_key,
            source_text,
            minimum_similarity_basis_points,
            limit,
        } => (
            format!("query TranslationMemorySuggestions($input: LookupTranslationMemoryInput!) {{ translationMemorySuggestions(input: $input) {{ {MEMORY_SUGGESTION_FIELDS} }} }}"),
            json!({ "input": {
                "sourceLocale": source_locale,
                "targetLocale": target_locale,
                "identity": identity,
                "fieldKey": field_key,
                "sourceText": source_text,
                "minimumSimilarityBasisPoints": minimum_similarity_basis_points,
                "limit": limit,
            }}),
            "translationMemorySuggestions",
        ),
        TranslationAdminOperation::ReadJobProgress { job_id } => (
            format!("query TranslationJobProgress($jobId: UUID!) {{ translationJobProgress(jobId: $jobId) {{ {JOB_PROGRESS_FIELDS} }} }}"),
            json!({ "jobId": job_id }),
            "translationJobProgress",
        ),
        TranslationAdminOperation::ReadReviewerQueue {
            job_id,
            assignee,
            include_unassigned,
            limit,
        } => (
            format!("query TranslationReviewerQueue($input: TranslationReviewerQueueInput!) {{ translationReviewerQueue(input: $input) {{ {REVIEWER_QUEUE_FIELDS} }} }}"),
            json!({ "input": {
                "jobId": job_id,
                "assignee": assignee.as_ref().map(|actor| json!({
                    "kind": actor_kind_name(actor.kind),
                    "id": actor.id,
                })),
                "includeUnassigned": include_unassigned,
                "limit": limit,
            }}),
            "translationReviewerQueue",
        ),
        TranslationAdminOperation::ReadReviewerWorkload { job_id } => (
            format!("query TranslationReviewerWorkload($input: TranslationReviewerWorkloadInput!) {{ translationReviewerWorkload(input: $input) {{ {REVIEWER_WORKLOAD_FIELDS} }} }}"),
            json!({ "input": { "jobId": job_id } }),
            "translationReviewerWorkload",
        ),
        TranslationAdminOperation::ListWorkflowNotes {
            job_id,
            item_id,
            include_resolved,
            limit,
        } => (
            format!("query TranslationWorkflowNotes($input: TranslationWorkflowNotesInput!) {{ translationWorkflowNotes(input: $input) {{ {WORKFLOW_NOTE_FIELDS} }} }}"),
            json!({ "input": {
                "jobId": job_id,
                "itemId": item_id,
                "includeResolved": include_resolved,
                "limit": limit,
            }}),
            "translationWorkflowNotes",
        ),
        TranslationAdminOperation::ExportJob { job_id, max_items } => (
            format!(
                "query ExportTranslationJob($input: ExportTranslationJobInput!) {{ exportTranslationJob(input: $input) {{ {INTERCHANGE_DOCUMENT_FIELDS} }} }}"
            ),
            json!({ "input": {
                "jobId": job_id,
                "maxItems": max_items,
            }}),
            "exportTranslationJob",
        ),
        TranslationAdminOperation::ListInterchangeArtifacts {
            job_id,
            include_expired,
            limit,
        } => (
            format!(
                "query TranslationInterchangeArtifacts($input: TranslationInterchangeArtifactsInput!) {{ translationInterchangeArtifacts(input: $input) {{ {INTERCHANGE_ARTIFACT_FIELDS} }} }}"
            ),
            json!({ "input": {
                "jobId": job_id,
                "includeExpired": include_expired,
                "limit": limit,
            }}),
            "translationInterchangeArtifacts",
        ),
        TranslationAdminOperation::ReadInterchangeArtifact { artifact_id } => (
            format!(
                "query TranslationInterchangeArtifact($input: ReadTranslationInterchangeArtifactInput!) {{ translationInterchangeArtifact(input: $input) {{ {INTERCHANGE_ARTIFACT_CONTENT_FIELDS} }} }}"
            ),
            json!({ "input": { "artifactId": artifact_id } }),
            "translationInterchangeArtifact",
        ),
        TranslationAdminOperation::ReadProviderProgress {
            owner_slug,
            resource_kind,
            source_locale,
            target_locale,
        } => (
            format!("query TranslationProviderProgress($ownerSlug: String!, $resourceKind: String!, $sourceLocale: String!, $targetLocale: String!) {{ translationProviderProgress(ownerSlug: $ownerSlug, resourceKind: $resourceKind, sourceLocale: $sourceLocale, targetLocale: $targetLocale) {{ {PROVIDER_PROGRESS_FIELDS} }} }}"),
            json!({
                "ownerSlug": owner_slug,
                "resourceKind": resource_kind,
                "sourceLocale": source_locale,
                "targetLocale": target_locale,
            }),
            "translationProviderProgress",
        ),
        TranslationAdminOperation::ReadRequiredProviderProgress {
            owner_slug,
            resource_kind,
            source_locale,
        } => (
            format!("query TranslationRequiredProviderProgress($ownerSlug: String!, $resourceKind: String!, $sourceLocale: String!) {{ translationRequiredProviderProgress(ownerSlug: $ownerSlug, resourceKind: $resourceKind, sourceLocale: $sourceLocale) {{ ownerSlug resourceKind sourceLocale requiredTargetLocales translationPolicyRevision tenantLocalePolicyRevision requiredUnits exactRequiredUnits optionalUnits exactOptionalUnits resourceLocalePairs completeResourceLocalePairs freshness targets {{ {PROVIDER_PROGRESS_FIELDS} }} }} }}"),
            json!({
                "ownerSlug": owner_slug,
                "resourceKind": resource_kind,
                "sourceLocale": source_locale,
            }),
            "translationRequiredProviderProgress",
        ),
        TranslationAdminOperation::ReplacePolicy {
            expected_revision,
            required_target_locales,
            idempotency_key,
        } => (
            format!("mutation ReplaceTranslationPolicy($input: ReplaceTranslationPolicyInput!) {{ replaceTranslationPolicy(input: $input) {{ {POLICY_FIELDS} }} }}"),
            json!({ "input": {
                "expectedRevision": expected_revision,
                "requiredTargetLocales": required_target_locales,
                "idempotencyKey": idempotency_key,
            }}),
            "replaceTranslationPolicy",
        ),
        TranslationAdminOperation::CreateGlossary {
            name,
            description,
            source_locale,
            target_locale,
            scope,
            idempotency_key,
        } => (
            format!("mutation CreateTranslationGlossary($input: CreateTranslationGlossaryInput!) {{ createTranslationGlossary(input: $input) {{ {GLOSSARY_FIELDS} }} }}"),
            json!({ "input": {
                "name": name,
                "description": description,
                "sourceLocale": source_locale,
                "targetLocale": target_locale,
                "scope": scope,
                "idempotencyKey": idempotency_key,
            }}),
            "createTranslationGlossary",
        ),
        TranslationAdminOperation::UpdateGlossary {
            glossary_id,
            expected_revision,
            name,
            description,
            idempotency_key,
        } => (
            format!("mutation UpdateTranslationGlossary($input: UpdateTranslationGlossaryInput!) {{ updateTranslationGlossary(input: $input) {{ {GLOSSARY_FIELDS} }} }}"),
            json!({ "input": {
                "glossaryId": glossary_id,
                "expectedRevision": expected_revision,
                "name": name,
                "description": description,
                "idempotencyKey": idempotency_key,
            }}),
            "updateTranslationGlossary",
        ),
        TranslationAdminOperation::ReplaceGlossaryTerms {
            glossary_id,
            expected_revision,
            concepts,
            idempotency_key,
        } => (
            format!("mutation ReplaceTranslationGlossaryTerms($input: ReplaceTranslationGlossaryTermsInput!) {{ replaceTranslationGlossaryTerms(input: $input) {{ {GLOSSARY_FIELDS} }} }}"),
            json!({ "input": {
                "glossaryId": glossary_id,
                "expectedRevision": expected_revision,
                "concepts": concepts,
                "idempotencyKey": idempotency_key,
            }}),
            "replaceTranslationGlossaryTerms",
        ),
        TranslationAdminOperation::SetGlossaryActive {
            glossary_id,
            expected_revision,
            is_active,
            idempotency_key,
        } => (
            format!("mutation SetTranslationGlossaryActive($input: SetTranslationGlossaryActiveInput!) {{ setTranslationGlossaryActive(input: $input) {{ {GLOSSARY_FIELDS} }} }}"),
            json!({ "input": {
                "glossaryId": glossary_id,
                "expectedRevision": expected_revision,
                "isActive": is_active,
                "idempotencyKey": idempotency_key,
            }}),
            "setTranslationGlossaryActive",
        ),
        TranslationAdminOperation::SetMemoryRetention {
            entry_id,
            expected_revision,
            policy,
            retain_until,
            idempotency_key,
        } => (
            format!("mutation SetTranslationMemoryRetention($input: SetTranslationMemoryRetentionInput!) {{ setTranslationMemoryRetention(input: $input) {{ {MEMORY_MUTATION_FIELDS} }} }}"),
            json!({ "input": {
                "entryId": entry_id,
                "expectedRevision": expected_revision,
                "policy": memory_retention_name(*policy),
                "retainUntil": retain_until,
                "idempotencyKey": idempotency_key,
            }}),
            "setTranslationMemoryRetention",
        ),
        TranslationAdminOperation::TombstoneMemoryEntry {
            entry_id,
            expected_revision,
            idempotency_key,
        } => (
            format!("mutation TombstoneTranslationMemoryEntry($input: TransitionTranslationMemoryEntryInput!) {{ tombstoneTranslationMemoryEntry(input: $input) {{ {MEMORY_MUTATION_FIELDS} }} }}"),
            json!({ "input": {
                "entryId": entry_id,
                "expectedRevision": expected_revision,
                "idempotencyKey": idempotency_key,
            }}),
            "tombstoneTranslationMemoryEntry",
        ),
        TranslationAdminOperation::PurgeMemoryEntry {
            entry_id,
            expected_revision,
            idempotency_key,
        } => (
            format!("mutation PurgeTranslationMemoryEntry($input: TransitionTranslationMemoryEntryInput!) {{ purgeTranslationMemoryEntry(input: $input) {{ {MEMORY_MUTATION_FIELDS} }} }}"),
            json!({ "input": {
                "entryId": entry_id,
                "expectedRevision": expected_revision,
                "idempotencyKey": idempotency_key,
            }}),
            "purgeTranslationMemoryEntry",
        ),
        TranslationAdminOperation::CreateJob {
            source_locale,
            target_locale,
            glossary,
            idempotency_key,
        } => (
            format!("mutation CreateTranslationJob($input: CreateTranslationJobInput!) {{ createTranslationJob(input: $input) {{ {JOB_FIELDS} }} }}"),
            json!({ "input": {
                "sourceLocale": source_locale,
                "targetLocale": target_locale,
                "glossary": glossary,
                "idempotencyKey": idempotency_key,
            }}),
            "createTranslationJob",
        ),
        TranslationAdminOperation::CreateWorkflowNote {
            job_id,
            item_id,
            body,
            idempotency_key,
        } => (
            format!("mutation CreateTranslationWorkflowNote($input: CreateTranslationWorkflowNoteInput!) {{ createTranslationWorkflowNote(input: $input) {{ {WORKFLOW_NOTE_FIELDS} }} }}"),
            json!({ "input": {
                "jobId": job_id,
                "itemId": item_id,
                "body": body,
                "idempotencyKey": idempotency_key,
            }}),
            "createTranslationWorkflowNote",
        ),
        TranslationAdminOperation::ResolveWorkflowNote {
            note_id,
            expected_revision,
            idempotency_key,
        } => (
            format!("mutation ResolveTranslationWorkflowNote($input: ResolveTranslationWorkflowNoteInput!) {{ resolveTranslationWorkflowNote(input: $input) {{ {WORKFLOW_NOTE_FIELDS} }} }}"),
            json!({ "input": {
                "noteId": note_id,
                "expectedRevision": expected_revision,
                "idempotencyKey": idempotency_key,
            }}),
            "resolveTranslationWorkflowNote",
        ),
        TranslationAdminOperation::CreateInterchangeExportArtifact {
            job_id,
            max_items,
            expires_in_seconds,
            idempotency_key,
        } => (
            format!(
                "mutation CreateTranslationInterchangeExportArtifact($input: CreateTranslationInterchangeExportArtifactInput!) {{ createTranslationInterchangeExportArtifact(input: $input) {{ {INTERCHANGE_ARTIFACT_FIELDS} }} }}"
            ),
            json!({ "input": {
                "jobId": job_id,
                "maxItems": max_items,
                "expiresInSeconds": expires_in_seconds,
                "idempotencyKey": idempotency_key,
            }}),
            "createTranslationInterchangeExportArtifact",
        ),
        TranslationAdminOperation::StoreInterchangeImportArtifact {
            job_id,
            document_json,
            expires_in_seconds,
            idempotency_key,
        } => (
            format!(
                "mutation StoreTranslationInterchangeImportArtifact($input: StoreTranslationInterchangeImportArtifactInput!) {{ storeTranslationInterchangeImportArtifact(input: $input) {{ {INTERCHANGE_ARTIFACT_FIELDS} }} }}"
            ),
            json!({ "input": {
                "jobId": job_id,
                "documentJson": document_json,
                "expiresInSeconds": expires_in_seconds,
                "idempotencyKey": idempotency_key,
            }}),
            "storeTranslationInterchangeImportArtifact",
        ),
        TranslationAdminOperation::ProcessInterchangeImportArtifact {
            artifact_id,
            idempotency_key,
        } => (
            format!(
                "mutation ProcessTranslationInterchangeImportArtifact($input: ProcessTranslationInterchangeImportArtifactInput!) {{ processTranslationInterchangeImportArtifact(input: $input) {{ {INTERCHANGE_ARTIFACT_FIELDS} }} }}"
            ),
            json!({ "input": {
                "artifactId": artifact_id,
                "idempotencyKey": idempotency_key,
            }}),
            "processTranslationInterchangeImportArtifact",
        ),
        TranslationAdminOperation::AddItem {
            job_id,
            identity,
            idempotency_key,
        } => (
            "mutation AddTranslationJobItem($input: AddTranslationJobItemInput!) { addTranslationJobItem(input: $input) { id jobId ownerSlug resourceKind resourceId subresourceId status assignee { kind id } sourceDigest revision } }".to_string(),
            json!({ "input": {
                "jobId": job_id,
                "identity": identity,
                "idempotencyKey": idempotency_key,
            }}),
            "addTranslationJobItem",
        ),
        TranslationAdminOperation::SaveProposal {
            item_id,
            origin,
            values,
            idempotency_key,
        } => (
            format!("mutation SaveTranslationProposal($input: SaveTranslationProposalInput!) {{ saveTranslationProposal(input: $input) {{ {PROPOSAL_FIELDS} }} }}"),
            json!({ "input": {
                "itemId": item_id,
                "origin": enum_name(origin),
                "values": values,
                "idempotencyKey": idempotency_key,
            }}),
            "saveTranslationProposal",
        ),
        TranslationAdminOperation::ImportItem {
            schema_version,
            job_id,
            item_id,
            identity,
            source_digest,
            values,
            idempotency_key,
        } => (
            format!(
                "mutation ImportTranslationItem($input: ImportTranslationItemInput!) {{ importTranslationItem(input: $input) {{ {PROPOSAL_FIELDS} }} }}"
            ),
            json!({ "input": {
                "schemaVersion": schema_version,
                "jobId": job_id,
                "itemId": item_id,
                "identity": identity,
                "sourceDigest": source_digest,
                "values": values,
                "idempotencyKey": idempotency_key,
            }}),
            "importTranslationItem",
        ),
        TranslationAdminOperation::EstimateMachineTranslation {
            item_id,
            field_keys,
            minimum_memory_similarity_basis_points,
            tone,
            domain,
            style,
            idempotency_key,
        } => (
            format!("mutation EstimateMachineTranslation($input: GenerateMachineTranslationProposalInput!) {{ estimateMachineTranslation(input: $input) {{ {MACHINE_ESTIMATE_FIELDS} }} }}"),
            json!({ "input": {
                "itemId": item_id,
                "fieldKeys": field_keys,
                "minimumMemorySimilarityBasisPoints": minimum_memory_similarity_basis_points,
                "tone": tone,
                "domain": domain,
                "style": style,
                "idempotencyKey": idempotency_key,
            }}),
            "estimateMachineTranslation",
        ),
        TranslationAdminOperation::GenerateMachineProposal {
            item_id,
            field_keys,
            minimum_memory_similarity_basis_points,
            tone,
            domain,
            style,
            idempotency_key,
        } => (
            format!(
                "mutation GenerateMachineTranslationProposal($input: GenerateMachineTranslationProposalInput!) {{ generateMachineTranslationProposal(input: $input) {{ __typename ... on MachineTranslationProposal {{ {MACHINE_PROPOSAL_FIELDS} }} ... on MachineTranslationOperationStatus {{ {MACHINE_OPERATION_STATUS_FIELDS} }} }} }}"
            ),
            json!({ "input": {
                "itemId": item_id,
                "fieldKeys": field_keys,
                "minimumMemorySimilarityBasisPoints": minimum_memory_similarity_basis_points,
                "tone": tone,
                "domain": domain,
                "style": style,
                "idempotencyKey": idempotency_key,
            }}),
            "generateMachineTranslationProposal",
        ),
        TranslationAdminOperation::CancelMachineOperation {
            operation_id,
            reason,
            idempotency_key,
        } => (
            format!("mutation CancelMachineTranslationOperation($input: CancelMachineTranslationOperationInput!) {{ cancelMachineTranslationOperation(input: $input) {{ {MACHINE_CANCELLATION_FIELDS} }} }}"),
            json!({ "input": {
                "operationId": operation_id,
                "reason": reason,
                "idempotencyKey": idempotency_key,
            }}),
            "cancelMachineTranslationOperation",
        ),
        TranslationAdminOperation::RecoverMachineOperation {
            operation_id,
            expected_updated_at,
            item_id,
            field_keys,
            minimum_memory_similarity_basis_points,
            tone,
            domain,
            style,
            reason,
            idempotency_key,
        } => (
            format!("mutation RecoverMachineTranslationOperation($input: RecoverMachineTranslationOperationInput!) {{ recoverMachineTranslationOperation(input: $input) {{ {MACHINE_PROPOSAL_FIELDS} }} }}"),
            json!({ "input": {
                "operationId": operation_id,
                "expectedUpdatedAt": expected_updated_at,
                "proposal": {
                    "itemId": item_id,
                    "fieldKeys": field_keys,
                    "minimumMemorySimilarityBasisPoints": minimum_memory_similarity_basis_points,
                    "tone": tone,
                    "domain": domain,
                    "style": style,
                },
                "reason": reason,
                "idempotencyKey": idempotency_key,
            }}),
            "recoverMachineTranslationOperation",
        ),
        TranslationAdminOperation::SubmitProposal {
            item_id,
            proposal_id,
            idempotency_key,
        } => transition_query(
            "SubmitTranslationProposal",
            "submitTranslationProposal",
            item_id,
            proposal_id,
            idempotency_key,
            PROPOSAL_FIELDS,
        ),
        TranslationAdminOperation::ApproveProposal {
            item_id,
            proposal_id,
            idempotency_key,
        } => transition_query(
            "ApproveTranslationProposal",
            "approveTranslationProposal",
            item_id,
            proposal_id,
            idempotency_key,
            PROPOSAL_FIELDS,
        ),
        TranslationAdminOperation::ApplyProposal {
            item_id,
            proposal_id,
            idempotency_key,
        } => transition_query(
            "ApplyTranslationProposal",
            "applyTranslationProposal",
            item_id,
            proposal_id,
            idempotency_key,
            "operationId itemId proposalId providerReceiptId resourceRevision targetRevision appliedFieldKeys",
        ),
        TranslationAdminOperation::AssignItem {
            item_id,
            expected_revision,
            assignee,
            idempotency_key,
        } => (
            "mutation AssignTranslationItem($input: AssignTranslationItemInput!) { assignTranslationItem(input: $input) { operationId itemId assignee { kind id } itemRevision } }".to_string(),
            json!({ "input": {
                "itemId": item_id,
                "expectedRevision": expected_revision,
                "assignee": { "kind": actor_kind_name(assignee.kind), "id": assignee.id },
                "idempotencyKey": idempotency_key,
            }}),
            "assignTranslationItem",
        ),
        TranslationAdminOperation::UnassignItem {
            item_id,
            expected_revision,
            idempotency_key,
        } => (
            "mutation UnassignTranslationItem($input: UnassignTranslationItemInput!) { unassignTranslationItem(input: $input) { operationId itemId assignee { kind id } itemRevision } }".to_string(),
            json!({ "input": {
                "itemId": item_id,
                "expectedRevision": expected_revision,
                "idempotencyKey": idempotency_key,
            }}),
            "unassignTranslationItem",
        ),
        TranslationAdminOperation::CancelJob {
            job_id,
            expected_revision,
            reason,
            idempotency_key,
        } => (
            "mutation CancelTranslationJob($input: CancelTranslationJobInput!) { cancelTranslationJob(input: $input) { cancellationId jobId jobRevision cancelledItemCount } }".to_string(),
            json!({ "input": {
                "jobId": job_id,
                "expectedRevision": expected_revision,
                "reason": reason,
                "idempotencyKey": idempotency_key,
            }}),
            "cancelTranslationJob",
        ),
        TranslationAdminOperation::RetryItem {
            item_id,
            expected_revision,
            reason,
            idempotency_key,
        } => (
            "mutation RetryTranslationItem($input: RetryTranslationItemInput!) { retryTranslationItem(input: $input) { retryId itemId itemRevision status } }".to_string(),
            json!({ "input": {
                "itemId": item_id,
                "expectedRevision": expected_revision,
                "reason": reason,
                "idempotencyKey": idempotency_key,
            }}),
            "retryTranslationItem",
        ),
        TranslationAdminOperation::RecoverApply {
            operation_id,
            expected_attempt_count,
            reason,
            idempotency_key,
        } => (
            "mutation RecoverTranslationApply($input: RecoverTranslationApplyInput!) { recoverTranslationApply(input: $input) { operationId itemId proposalId providerReceiptId resourceRevision targetRevision appliedFieldKeys } }".to_string(),
            json!({ "input": {
                "operationId": operation_id,
                "expectedAttemptCount": expected_attempt_count,
                "reason": reason,
                "idempotencyKey": idempotency_key,
            }}),
            "recoverTranslationApply",
        ),
        TranslationAdminOperation::RebuildJobProgress {
            job_id,
            idempotency_key,
        } => (
            format!("mutation RebuildTranslationJobProgress($jobId: UUID!, $idempotencyKey: String!) {{ rebuildTranslationJobProgress(jobId: $jobId, idempotencyKey: $idempotencyKey) {{ {JOB_PROGRESS_FIELDS} }} }}"),
            json!({ "jobId": job_id, "idempotencyKey": idempotency_key }),
            "rebuildTranslationJobProgress",
        ),
        TranslationAdminOperation::SyncProviderInventory {
            owner_slug,
            resource_kind,
            limit,
        } => (
            "mutation SyncTranslationProviderInventory($ownerSlug: String!, $resourceKind: String!, $limit: Int!) { syncTranslationProviderInventory(ownerSlug: $ownerSlug, resourceKind: $resourceKind, limit: $limit) { observedResources checkpoint checkpointRevision } }".to_string(),
            json!({ "ownerSlug": owner_slug, "resourceKind": resource_kind, "limit": limit }),
            "syncTranslationProviderInventory",
        ),
        TranslationAdminOperation::RebuildProviderInventory {
            owner_slug,
            resource_kind,
            source_locale,
            target_locale,
            page_size,
        } => (
            "mutation RebuildTranslationProviderInventory($ownerSlug: String!, $resourceKind: String!, $sourceLocale: String!, $targetLocale: String!, $pageSize: Int!) { rebuildTranslationProviderInventory(ownerSlug: $ownerSlug, resourceKind: $resourceKind, sourceLocale: $sourceLocale, targetLocale: $targetLocale, pageSize: $pageSize) { observedResources checkpoint checkpointRevision } }".to_string(),
            json!({
                "ownerSlug": owner_slug,
                "resourceKind": resource_kind,
                "sourceLocale": source_locale,
                "targetLocale": target_locale,
                "pageSize": page_size,
            }),
            "rebuildTranslationProviderInventory",
        ),
    }
}

fn transition_query(
    operation_name: &str,
    field: &'static str,
    item_id: &str,
    proposal_id: &str,
    idempotency_key: &str,
    selection: &str,
) -> (String, Value, &'static str) {
    (
        format!(
            "mutation {operation_name}($input: TransitionTranslationProposalInput!) {{ {field}(input: $input) {{ {selection} }} }}"
        ),
        json!({ "input": {
            "itemId": item_id,
            "proposalId": proposal_id,
            "idempotencyKey": idempotency_key,
        }}),
        field,
    )
}

fn enum_name(origin: &ProposalOrigin) -> &'static str {
    match origin {
        ProposalOrigin::Manual => "MANUAL",
        ProposalOrigin::Import => "IMPORT",
        ProposalOrigin::Memory => "MEMORY",
        ProposalOrigin::Ai => "AI",
    }
}

fn actor_kind_name(kind: ActorKind) -> &'static str {
    match kind {
        ActorKind::User => "USER",
        ActorKind::Service => "SERVICE",
    }
}

fn memory_retention_name(policy: MemoryRetentionPolicy) -> &'static str {
    match policy {
        MemoryRetentionPolicy::OwnerLifecycle => "OWNER_LIFECYCLE",
        MemoryRetentionPolicy::RetainUntil => "RETAIN_UNTIL",
        MemoryRetentionPolicy::LegalHold => "LEGAL_HOLD",
    }
}

async fn request(
    query: String,
    variables: Value,
    context: TranslationAdminTransportContext,
) -> Result<Value, TransportError> {
    execute_graphql(
        &graphql_url(),
        GraphqlRequest::new(query, Some(variables)),
        context.token,
        context.tenant_slug,
        context.locale,
    )
    .await
}

fn field_value<T: DeserializeOwned>(data: &Value, field: &str) -> Result<T, TransportError> {
    let value = data
        .get(field)
        .cloned()
        .ok_or_else(|| GraphqlHttpError::Graphql(format!("Missing response field `{field}`")))?;
    serde_json::from_value(value)
        .map_err(|error| GraphqlHttpError::Graphql(format!("Invalid `{field}` response: {error}")))
}

#[derive(Deserialize)]
#[serde(tag = "__typename")]
enum GraphqlMachineProposalOutcome {
    #[serde(rename = "MachineTranslationProposal")]
    Completed(Box<crate::model::MachineProposal>),
    #[serde(rename = "MachineTranslationOperationStatus")]
    InProgress(crate::model::MachineOperationStatus),
}

fn graphql_endpoint_from_base(base: &str) -> String {
    format!("{}/api/graphql", base.trim_end_matches('/'))
}

fn graphql_url() -> String {
    if let Some(url) = option_env!("RUSTOK_GRAPHQL_URL") {
        return url.to_string();
    }
    #[cfg(target_arch = "wasm32")]
    {
        let origin = web_sys::window()
            .and_then(|window| window.location().origin().ok())
            .unwrap_or_else(|| "http://localhost:5150".to_string());
        graphql_endpoint_from_base(&origin)
    }
    #[cfg(not(target_arch = "wasm32"))]
    {
        let base =
            std::env::var("RUSTOK_API_URL").unwrap_or_else(|_| "http://localhost:5150".to_string());
        graphql_endpoint_from_base(&base)
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphqlJobItem {
    id: String,
    job_id: String,
    owner_slug: String,
    resource_kind: String,
    resource_id: String,
    subresource_id: Option<String>,
    status: String,
    assignee: Option<Actor>,
    source_digest: String,
    revision: i64,
}

impl From<GraphqlJobItem> for JobItem {
    fn from(value: GraphqlJobItem) -> Self {
        Self {
            id: value.id,
            job_id: value.job_id,
            identity: TranslationResourceIdentity {
                owner_slug: value.owner_slug,
                resource_kind: value.resource_kind,
                resource_id: value.resource_id,
                subresource_id: value.subresource_id,
            },
            status: value.status,
            assignee: value.assignee,
            source_digest: value.source_digest,
            revision: value.revision,
        }
    }
}

#[derive(serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct GraphqlReviewerQueueItem {
    item: GraphqlJobItem,
    proposal_id: String,
    proposal_revision: i64,
    submitted_at: String,
}

impl From<GraphqlReviewerQueueItem> for ReviewerQueueItem {
    fn from(value: GraphqlReviewerQueueItem) -> Self {
        Self {
            item: value.item.into(),
            proposal_id: value.proposal_id,
            proposal_revision: value.proposal_revision,
            submitted_at: value.submitted_at,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::ProposalValueInput;

    fn operations() -> Vec<TranslationAdminOperation> {
        let resource = TranslationResourceIdentity {
            owner_slug: "media".to_string(),
            resource_kind: "asset".to_string(),
            resource_id: "asset-1".to_string(),
            subresource_id: None,
        };
        vec![
            TranslationAdminOperation::ReadPolicy,
            TranslationAdminOperation::ReadMachineOperationStatus {
                operation_id: "00000000-0000-0000-0000-000000000020".to_string(),
            },
            TranslationAdminOperation::ListTargets,
            TranslationAdminOperation::ListGlossaries { limit: 50 },
            TranslationAdminOperation::ReadGlossary {
                glossary_id: "00000000-0000-0000-0000-000000000010".to_string(),
                revision: Some(1),
            },
            TranslationAdminOperation::ListMemoryEntries {
                source_locale: Some("en".to_string()),
                target_locale: Some("de".to_string()),
                include_tombstoned: true,
                limit: 50,
            },
            TranslationAdminOperation::ReadMemoryEntry {
                entry_id: "00000000-0000-0000-0000-000000000011".to_string(),
            },
            TranslationAdminOperation::LookupMemory {
                source_locale: "en".to_string(),
                target_locale: "de".to_string(),
                identity: resource.clone(),
                field_key: "title".to_string(),
                source_text: "Hero".to_string(),
                minimum_similarity_basis_points: 7_000,
                limit: 10,
            },
            TranslationAdminOperation::ReadJobProgress {
                job_id: "00000000-0000-0000-0000-000000000001".to_string(),
            },
            TranslationAdminOperation::ReadReviewerQueue {
                job_id: "00000000-0000-0000-0000-000000000001".to_string(),
                assignee: Some(Actor {
                    kind: ActorKind::User,
                    id: "reviewer-1".to_string(),
                }),
                include_unassigned: true,
                limit: 50,
            },
            TranslationAdminOperation::ReadReviewerWorkload {
                job_id: "00000000-0000-0000-0000-000000000001".to_string(),
            },
            TranslationAdminOperation::ListWorkflowNotes {
                job_id: "00000000-0000-0000-0000-000000000001".to_string(),
                item_id: Some("00000000-0000-0000-0000-000000000002".to_string()),
                include_resolved: true,
                limit: 50,
            },
            TranslationAdminOperation::ExportJob {
                job_id: "00000000-0000-0000-0000-000000000001".to_string(),
                max_items: 200,
            },
            TranslationAdminOperation::ListInterchangeArtifacts {
                job_id: Some("00000000-0000-0000-0000-000000000001".to_string()),
                include_expired: true,
                limit: 50,
            },
            TranslationAdminOperation::ReadInterchangeArtifact {
                artifact_id: "00000000-0000-0000-0000-000000000007".to_string(),
            },
            TranslationAdminOperation::ReadProviderProgress {
                owner_slug: "media".to_string(),
                resource_kind: "asset".to_string(),
                source_locale: "en".to_string(),
                target_locale: "de".to_string(),
            },
            TranslationAdminOperation::ReadRequiredProviderProgress {
                owner_slug: "media".to_string(),
                resource_kind: "asset".to_string(),
                source_locale: "en".to_string(),
            },
            TranslationAdminOperation::ReplacePolicy {
                expected_revision: 0,
                required_target_locales: vec!["de".to_string()],
                idempotency_key: "policy-1".to_string(),
            },
            TranslationAdminOperation::CreateGlossary {
                name: "Product terminology".to_string(),
                description: String::new(),
                source_locale: "en".to_string(),
                target_locale: "de".to_string(),
                scope: crate::model::GlossaryScope::default(),
                idempotency_key: "glossary-create-1".to_string(),
            },
            TranslationAdminOperation::UpdateGlossary {
                glossary_id: "00000000-0000-0000-0000-000000000010".to_string(),
                expected_revision: 1,
                name: "Updated terminology".to_string(),
                description: String::new(),
                idempotency_key: "glossary-update-1".to_string(),
            },
            TranslationAdminOperation::ReplaceGlossaryTerms {
                glossary_id: "00000000-0000-0000-0000-000000000010".to_string(),
                expected_revision: 2,
                concepts: Vec::new(),
                idempotency_key: "glossary-terms-1".to_string(),
            },
            TranslationAdminOperation::SetGlossaryActive {
                glossary_id: "00000000-0000-0000-0000-000000000010".to_string(),
                expected_revision: 3,
                is_active: false,
                idempotency_key: "glossary-active-1".to_string(),
            },
            TranslationAdminOperation::SetMemoryRetention {
                entry_id: "00000000-0000-0000-0000-000000000011".to_string(),
                expected_revision: 1,
                policy: MemoryRetentionPolicy::LegalHold,
                retain_until: None,
                idempotency_key: "memory-retention-1".to_string(),
            },
            TranslationAdminOperation::TombstoneMemoryEntry {
                entry_id: "00000000-0000-0000-0000-000000000011".to_string(),
                expected_revision: 2,
                idempotency_key: "memory-tombstone-1".to_string(),
            },
            TranslationAdminOperation::PurgeMemoryEntry {
                entry_id: "00000000-0000-0000-0000-000000000011".to_string(),
                expected_revision: 3,
                idempotency_key: "memory-purge-1".to_string(),
            },
            TranslationAdminOperation::CreateJob {
                source_locale: "en".to_string(),
                target_locale: "de".to_string(),
                glossary: None,
                idempotency_key: "job-1".to_string(),
            },
            TranslationAdminOperation::CreateWorkflowNote {
                job_id: "00000000-0000-0000-0000-000000000001".to_string(),
                item_id: Some("00000000-0000-0000-0000-000000000002".to_string()),
                body: "Private reviewer context".to_string(),
                idempotency_key: "workflow-note-create-1".to_string(),
            },
            TranslationAdminOperation::ResolveWorkflowNote {
                note_id: "00000000-0000-0000-0000-000000000006".to_string(),
                expected_revision: 0,
                idempotency_key: "workflow-note-resolve-1".to_string(),
            },
            TranslationAdminOperation::CreateInterchangeExportArtifact {
                job_id: "00000000-0000-0000-0000-000000000001".to_string(),
                max_items: 50,
                expires_in_seconds: 86_400,
                idempotency_key: "interchange-export-artifact-1".to_string(),
            },
            TranslationAdminOperation::StoreInterchangeImportArtifact {
                job_id: "00000000-0000-0000-0000-000000000001".to_string(),
                document_json: r#"{"schemaVersion":1,"jobId":"00000000-0000-0000-0000-000000000001","sourceLocale":"en","targetLocale":"de","items":[]}"#.to_string(),
                expires_in_seconds: 86_400,
                idempotency_key: "interchange-import-artifact-1".to_string(),
            },
            TranslationAdminOperation::ProcessInterchangeImportArtifact {
                artifact_id: "00000000-0000-0000-0000-000000000007".to_string(),
                idempotency_key: "interchange-process-artifact-1".to_string(),
            },
            TranslationAdminOperation::AddItem {
                job_id: "00000000-0000-0000-0000-000000000001".to_string(),
                identity: resource,
                idempotency_key: "item-1".to_string(),
            },
            TranslationAdminOperation::SaveProposal {
                item_id: "00000000-0000-0000-0000-000000000002".to_string(),
                origin: ProposalOrigin::Manual,
                values: vec![ProposalValueInput {
                    key: "alt".to_string(),
                    value: "Alt".to_string(),
                }],
                idempotency_key: "proposal-1".to_string(),
            },
            TranslationAdminOperation::ImportItem {
                schema_version: 1,
                job_id: "00000000-0000-0000-0000-000000000001".to_string(),
                item_id: "00000000-0000-0000-0000-000000000002".to_string(),
                identity: TranslationResourceIdentity {
                    owner_slug: "media".to_string(),
                    resource_kind: "asset".to_string(),
                    resource_id: "asset-1".to_string(),
                    subresource_id: None,
                },
                source_digest: "source-digest".to_string(),
                values: vec![ProposalValueInput {
                    key: "alt".to_string(),
                    value: "Alt".to_string(),
                }],
                idempotency_key: "import-item-1".to_string(),
            },
            TranslationAdminOperation::EstimateMachineTranslation {
                item_id: "00000000-0000-0000-0000-000000000002".to_string(),
                field_keys: vec!["alt".to_string()],
                minimum_memory_similarity_basis_points: 7_000,
                tone: Some("neutral".to_string()),
                domain: Some("media".to_string()),
                style: None,
                idempotency_key: "machine-estimate-1".to_string(),
            },
            TranslationAdminOperation::GenerateMachineProposal {
                item_id: "00000000-0000-0000-0000-000000000002".to_string(),
                field_keys: vec!["alt".to_string()],
                minimum_memory_similarity_basis_points: 7_000,
                tone: Some("neutral".to_string()),
                domain: Some("media".to_string()),
                style: None,
                idempotency_key: "machine-proposal-1".to_string(),
            },
            TranslationAdminOperation::CancelMachineOperation {
                operation_id: "00000000-0000-0000-0000-000000000020".to_string(),
                reason: "Operator cancelled the pending generation".to_string(),
                idempotency_key: "machine-cancel-1".to_string(),
            },
            TranslationAdminOperation::RecoverMachineOperation {
                operation_id: "00000000-0000-0000-0000-000000000020".to_string(),
                expected_updated_at: "2026-07-29T12:00:00Z".to_string(),
                item_id: "00000000-0000-0000-0000-000000000002".to_string(),
                field_keys: vec!["alt".to_string()],
                minimum_memory_similarity_basis_points: 7_000,
                tone: Some("neutral".to_string()),
                domain: Some("media".to_string()),
                style: None,
                reason: "Recover a completed provider result".to_string(),
                idempotency_key: "machine-recover-1".to_string(),
            },
            TranslationAdminOperation::SubmitProposal {
                item_id: "00000000-0000-0000-0000-000000000002".to_string(),
                proposal_id: "00000000-0000-0000-0000-000000000003".to_string(),
                idempotency_key: "submit-1".to_string(),
            },
            TranslationAdminOperation::ApproveProposal {
                item_id: "00000000-0000-0000-0000-000000000002".to_string(),
                proposal_id: "00000000-0000-0000-0000-000000000003".to_string(),
                idempotency_key: "approve-1".to_string(),
            },
            TranslationAdminOperation::ApplyProposal {
                item_id: "00000000-0000-0000-0000-000000000002".to_string(),
                proposal_id: "00000000-0000-0000-0000-000000000003".to_string(),
                idempotency_key: "apply-1".to_string(),
            },
            TranslationAdminOperation::AssignItem {
                item_id: "00000000-0000-0000-0000-000000000002".to_string(),
                expected_revision: 1,
                assignee: Actor {
                    kind: ActorKind::User,
                    id: "00000000-0000-0000-0000-000000000004".to_string(),
                },
                idempotency_key: "assign-1".to_string(),
            },
            TranslationAdminOperation::UnassignItem {
                item_id: "00000000-0000-0000-0000-000000000002".to_string(),
                expected_revision: 2,
                idempotency_key: "unassign-1".to_string(),
            },
            TranslationAdminOperation::CancelJob {
                job_id: "00000000-0000-0000-0000-000000000001".to_string(),
                expected_revision: 1,
                reason: "cancel".to_string(),
                idempotency_key: "cancel-1".to_string(),
            },
            TranslationAdminOperation::RetryItem {
                item_id: "00000000-0000-0000-0000-000000000002".to_string(),
                expected_revision: 3,
                reason: "retry".to_string(),
                idempotency_key: "retry-1".to_string(),
            },
            TranslationAdminOperation::RecoverApply {
                operation_id: "00000000-0000-0000-0000-000000000005".to_string(),
                expected_attempt_count: 1,
                reason: "recover".to_string(),
                idempotency_key: "recover-1".to_string(),
            },
            TranslationAdminOperation::RebuildJobProgress {
                job_id: "00000000-0000-0000-0000-000000000001".to_string(),
                idempotency_key: "progress-1".to_string(),
            },
            TranslationAdminOperation::SyncProviderInventory {
                owner_slug: "media".to_string(),
                resource_kind: "asset".to_string(),
                limit: 50,
            },
            TranslationAdminOperation::RebuildProviderInventory {
                owner_slug: "media".to_string(),
                resource_kind: "asset".to_string(),
                source_locale: "en".to_string(),
                target_locale: "de".to_string(),
                page_size: 50,
            },
        ]
    }

    #[test]
    fn every_operation_has_a_graphql_field() {
        for operation in operations() {
            let (query, _, field) = operation_graphql(&operation);
            assert!(
                query.contains(field),
                "{field} missing from GraphQL document"
            );
        }
    }

    #[test]
    fn decodes_in_progress_machine_proposal_outcome() {
        let outcome: GraphqlMachineProposalOutcome = serde_json::from_value(serde_json::json!({
            "__typename": "MachineTranslationOperationStatus",
            "operationId": "00000000-0000-0000-0000-000000000020",
            "itemId": "00000000-0000-0000-0000-000000000021",
            "status": "registered",
            "providerExecutionId": "execution-in-progress",
            "providerStatus": "running",
            "providerErrorCode": null,
            "updatedAt": "2026-08-13T00:00:00+00:00"
        }))
        .expect("machine proposal polling outcome must deserialize");

        let GraphqlMachineProposalOutcome::InProgress(status) = outcome else {
            panic!("expected an in-progress machine proposal outcome");
        };
        assert_eq!(status.status, "registered");
        assert_eq!(status.provider_status, "running");
        assert_eq!(
            status.provider_execution_id.as_deref(),
            Some("execution-in-progress")
        );
    }

    #[test]
    fn generate_machine_proposal_requests_both_outcome_members() {
        let operation = TranslationAdminOperation::GenerateMachineProposal {
            item_id: "00000000-0000-0000-0000-000000000020".to_string(),
            field_keys: vec!["title".to_string()],
            minimum_memory_similarity_basis_points: 0,
            tone: None,
            domain: None,
            style: None,
            idempotency_key: "machine-proposal-outcome".to_string(),
        };

        let (document, _, _) = operation_graphql(&operation);
        for marker in [
            "__typename",
            "... on MachineTranslationProposal",
            "... on MachineTranslationOperationStatus",
        ] {
            assert!(
                document.contains(marker),
                "missing GraphQL outcome field {marker}"
            );
        }
    }

    #[tokio::test]
    async fn every_graphql_document_validates_against_translation_roots() {
        let schema = async_graphql::Schema::build(
            rustok_translation::graphql::TranslationQuery,
            rustok_translation::graphql::TranslationMutation,
            async_graphql::EmptySubscription,
        )
        .finish();

        for operation in operations() {
            let (query, variables, field) = operation_graphql(&operation);
            let response = schema
                .execute(
                    async_graphql::Request::new(query)
                        .variables(async_graphql::Variables::from_json(variables)),
                )
                .await;
            let messages = response
                .errors
                .iter()
                .map(|error| error.message.as_str())
                .collect::<Vec<_>>();
            assert!(
                messages.iter().all(|message| {
                    !message.contains("Unknown field")
                        && !message.contains("Unknown type")
                        && !message.contains("Invalid value")
                        && !message.contains("required type")
                }),
                "{field} failed schema validation: {messages:?}"
            );
        }
    }

    #[test]
    fn graphql_endpoint_is_stable() {
        assert_eq!(
            graphql_endpoint_from_base("http://localhost:5150/"),
            "http://localhost:5150/api/graphql"
        );
    }
}
