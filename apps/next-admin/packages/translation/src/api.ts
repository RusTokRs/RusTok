import type {
  AdminGraphqlExecutor,
  ApplyResult,
  Assignment,
  Cancellation,
  Glossary,
  GlossarySummary,
  InventoryResult,
  InterchangeArtifact,
  InterchangeArtifactContent,
  InterchangeDocument,
  Job,
  JobItem,
  JobProgress,
  MemoryEntry,
  MemoryMutation,
  MemorySuggestion,
  MachineCancellation,
  MachineTranslationEstimate,
  MachineOperationStatus,
  MachineProposal,
  Proposal,
  ProviderProgress,
  RequiredProviderProgress,
  ReviewerQueueItem,
  ReviewerWorkload,
  Retry,
  TranslationOperation,
  TranslationPolicy,
  TranslationResponse,
  TranslationTarget,
  WorkflowNote
} from './types';

const POLICY_FIELDS =
  'tenantId requiredTargetLocales tenantLocalePolicyRevision revision freshness disabledRequiredTargetLocales';
const JOB_PROGRESS_FIELDS =
  'jobId sourceDigest totalItems assignedItems terminalItems missingItems draftItems inReviewItems approvedItems applyingItems appliedItems staleItems conflictItems blockedItems excludedItems cancelledItems requiredUnits optionalUnits appliedRequiredUnits appliedOptionalUnits approvedRequiredUnits approvedOptionalUnits completeResources sourceCharacters translatedCharacters revision updatedAt';
const PROVIDER_PROGRESS_FIELDS =
  'ownerSlug resourceKind sourceLocale targetLocale requiredUnits exactRequiredUnits optionalUnits exactOptionalUnits resources completeResources ownerChangeCursor projectedCursor checkpointRevision checkpointUpdatedAt freshness';
const PROPOSAL_FIELDS =
  'id itemId proposalRevision origin values { key value expectedSourceHash } qaIssues { field severity code message } qaAccepted status approvalReceiptId';
const MACHINE_ESTIMATE_FIELDS =
  'inputTokensUpperBound outputTokensUpperBound attemptsUpperBound costMinorUnitsUpperBound currencyCode priceSnapshotDigest reviewRequired';
const MACHINE_PROPOSAL_FIELDS =
  'operationId itemId proposalId adapterSlug providerSlug providerPolicyDigest machineRequestDigest glossaryRevision glossaryDigest memoryDigest executionId executionRequestDigest promptPolicyDigest attempts { attempt providerProfileId providerSlug model fallback } usage { inputTokens outputTokens totalTokens costMinorUnits currencyCode priceSnapshotDigest } diagnostics { code blocking unitId } reviewRequired createdAt updatedAt';
const MACHINE_CANCELLATION_FIELDS =
  'cancellationId operationId status providerExecutionId providerStatus providerErrorCode providerObservedAt createdAt';
const MACHINE_OPERATION_STATUS_FIELDS =
  'operationId itemId status providerExecutionId providerStatus providerErrorCode updatedAt';
const APPLY_RESULT_FIELDS =
  'operationId itemId proposalId providerReceiptId resourceRevision targetRevision appliedFieldKeys';
const ASSIGNMENT_FIELDS =
  'operationId itemId assignee { kind id } itemRevision';
const CANCELLATION_FIELDS =
  'cancellationId jobId jobRevision cancelledItemCount';
const RETRY_FIELDS = 'retryId itemId itemRevision status';
const GLOSSARY_SUMMARY_FIELDS =
  'id name description sourceLocale targetLocale scope { ownerSlug resourceKind fieldKey } isActive revision';
const GLOSSARY_FIELDS =
  'id name description sourceLocale targetLocale scope { ownerSlug resourceKind fieldKey } isActive revision concepts { conceptKey sourceTerm variants { value policy } matchKind caseSensitive notes }';
const MEMORY_ENTRY_FIELDS =
  'id tenantId sourceLocale targetLocale ownerSlug resourceKind resourceId subresourceId fieldKey sourceText targetText sourceHash targetHash contextFingerprint segmentationVersion origin qualityState reviewerActorKind reviewerActorId proposalId applyReceiptId retentionPolicy retainUntil tombstonedAt revision createdAt updatedAt';
const MEMORY_SUGGESTION_FIELDS =
  'entryId sourceText targetText sourceHash ownerSlug resourceKind resourceId fieldKey origin proposalId applyReceiptId evidence { kind sourceExact contextMatch baseSimilarityBasisPoints contextBonusBasisPoints finalSimilarityBasisPoints segmentationVersion }';
const MEMORY_MUTATION_FIELDS =
  'entryId revision state retentionPolicy retainUntil tombstonedAt';
const JOB_FIELDS =
  'id sourceLocale targetLocale glossary { glossaryId revision } status revision';
const INTERCHANGE_DOCUMENT_FIELDS =
  'schemaVersion jobId sourceLocale targetLocale items { itemId identity { ownerSlug resourceKind resourceId subresourceId } sourceDigest sourceRevision targetRevision fields { key sourceValue exactTargetValue proposedValue sourceHash required maxCharacters protectedTokens } }';
const INTERCHANGE_ARTIFACT_FIELDS =
  'id jobId direction status contentLength checksumSha256 expiresAt processedAt report { totalItems acceptedItems conflictItems rejectedItems outcomes { itemId status } } createdAt updatedAt';
const INTERCHANGE_ARTIFACT_CONTENT_FIELDS = `artifact { ${INTERCHANGE_ARTIFACT_FIELDS} } document { ${INTERCHANGE_DOCUMENT_FIELDS} }`;
const REVIEWER_QUEUE_FIELDS =
  'item { id jobId ownerSlug resourceKind resourceId subresourceId status assignee { kind id } sourceDigest revision } proposalId proposalRevision submittedAt';
const REVIEWER_WORKLOAD_FIELDS =
  'jobId assignee { kind id } openItems missingItems draftItems inReviewItems approvedItems applyingItems rebaseRequiredItems blockedItems sourceCharacters';
const WORKFLOW_NOTE_FIELDS =
  'id jobId itemId body author { kind id } revision resolvedAt resolvedBy { kind id } createdAt updatedAt';

type RequestContext = {
  graphql: AdminGraphqlExecutor;
  token?: string | null;
  tenantSlug?: string | null;
  graphqlUrl?: string;
};

export async function executeTranslationOperation(
  context: RequestContext,
  operation: TranslationOperation
): Promise<TranslationResponse> {
  switch (operation.kind) {
    case 'read_policy': {
      const data = await request<
        undefined,
        { translationPolicy: TranslationPolicy }
      >(
        context,
        `query TranslationPolicy { translationPolicy { ${POLICY_FIELDS} } }`
      );
      return { kind: 'policy', value: data.translationPolicy };
    }
    case 'read_machine_operation_status': {
      const data = await request<
        { operationId: string },
        { machineTranslationOperationStatus: MachineOperationStatus }
      >(
        context,
        `query MachineTranslationOperationStatus($operationId: UUID!) {
          machineTranslationOperationStatus(operationId: $operationId) {
            ${MACHINE_OPERATION_STATUS_FIELDS}
          }
        }`,
        { operationId: operation.operationId }
      );
      return {
        kind: 'machine_operation_status',
        value: data.machineTranslationOperationStatus
      };
    }
    case 'list_targets': {
      const data = await request<
        undefined,
        { translationTargets: TranslationTarget[] }
      >(
        context,
        `query TranslationTargets {
          translationTargets {
            ownerSlug resourceKind displayName capabilities
            readPermissionFloor applyPermissionFloor
          }
        }`
      );
      return { kind: 'targets', value: data.translationTargets };
    }
    case 'list_glossaries': {
      const data = await request<
        { limit: number },
        { translationGlossaries: GlossarySummary[] }
      >(
        context,
        `query TranslationGlossaries($limit: Int!) {
          translationGlossaries(limit: $limit) { ${GLOSSARY_SUMMARY_FIELDS} }
        }`,
        { limit: operation.limit }
      );
      return { kind: 'glossaries', value: data.translationGlossaries };
    }
    case 'read_glossary': {
      const data = await request<
        { glossaryId: string; revision?: number },
        { translationGlossary: Glossary }
      >(
        context,
        `query TranslationGlossary($glossaryId: UUID!, $revision: Int) {
          translationGlossary(glossaryId: $glossaryId, revision: $revision) {
            ${GLOSSARY_FIELDS}
          }
        }`,
        withoutKind(operation)
      );
      return { kind: 'glossary', value: data.translationGlossary };
    }
    case 'list_memory_entries': {
      const variables = withoutKind(operation);
      const data = await request<
        typeof variables,
        { translationMemoryEntries: MemoryEntry[] }
      >(
        context,
        `query TranslationMemoryEntries(
          $sourceLocale: String, $targetLocale: String,
          $includeTombstoned: Boolean!, $limit: Int!
        ) {
          translationMemoryEntries(
            sourceLocale: $sourceLocale, targetLocale: $targetLocale,
            includeTombstoned: $includeTombstoned, limit: $limit
          ) { ${MEMORY_ENTRY_FIELDS} }
        }`,
        variables
      );
      return {
        kind: 'memory_entries',
        value: data.translationMemoryEntries
      };
    }
    case 'read_memory_entry': {
      const data = await request<
        { entryId: string },
        { translationMemoryEntry: MemoryEntry }
      >(
        context,
        `query TranslationMemoryEntry($entryId: UUID!) {
          translationMemoryEntry(entryId: $entryId) {
            ${MEMORY_ENTRY_FIELDS}
          }
        }`,
        { entryId: operation.entryId }
      );
      return {
        kind: 'memory_entry',
        value: data.translationMemoryEntry
      };
    }
    case 'lookup_memory': {
      const { kind: _kind, ...input } = operation;
      const data = await request<
        { input: typeof input },
        { translationMemorySuggestions: MemorySuggestion[] }
      >(
        context,
        `query TranslationMemorySuggestions(
          $input: LookupTranslationMemoryInput!
        ) {
          translationMemorySuggestions(input: $input) {
            ${MEMORY_SUGGESTION_FIELDS}
          }
        }`,
        { input }
      );
      return {
        kind: 'memory_suggestions',
        value: data.translationMemorySuggestions
      };
    }
    case 'replace_policy': {
      const data = await request<
        {
          input: Omit<
            TranslationOperation & { kind: 'replace_policy' },
            'kind'
          >;
        },
        { replaceTranslationPolicy: TranslationPolicy }
      >(
        context,
        `mutation ReplaceTranslationPolicy($input: ReplaceTranslationPolicyInput!) {
          replaceTranslationPolicy(input: $input) { ${POLICY_FIELDS} }
        }`,
        { input: withoutKind(operation) }
      );
      return { kind: 'policy', value: data.replaceTranslationPolicy };
    }
    case 'create_glossary': {
      const data = await request<
        { input: Omit<typeof operation, 'kind'> },
        { createTranslationGlossary: Glossary }
      >(
        context,
        `mutation CreateTranslationGlossary($input: CreateTranslationGlossaryInput!) {
          createTranslationGlossary(input: $input) { ${GLOSSARY_FIELDS} }
        }`,
        { input: withoutKind(operation) }
      );
      return { kind: 'glossary', value: data.createTranslationGlossary };
    }
    case 'update_glossary': {
      const data = await request<
        { input: Omit<typeof operation, 'kind'> },
        { updateTranslationGlossary: Glossary }
      >(
        context,
        `mutation UpdateTranslationGlossary($input: UpdateTranslationGlossaryInput!) {
          updateTranslationGlossary(input: $input) { ${GLOSSARY_FIELDS} }
        }`,
        { input: withoutKind(operation) }
      );
      return { kind: 'glossary', value: data.updateTranslationGlossary };
    }
    case 'replace_glossary_terms': {
      const data = await request<
        { input: Omit<typeof operation, 'kind'> },
        { replaceTranslationGlossaryTerms: Glossary }
      >(
        context,
        `mutation ReplaceTranslationGlossaryTerms(
          $input: ReplaceTranslationGlossaryTermsInput!
        ) {
          replaceTranslationGlossaryTerms(input: $input) { ${GLOSSARY_FIELDS} }
        }`,
        { input: withoutKind(operation) }
      );
      return {
        kind: 'glossary',
        value: data.replaceTranslationGlossaryTerms
      };
    }
    case 'set_glossary_active': {
      const data = await request<
        { input: Omit<typeof operation, 'kind'> },
        { setTranslationGlossaryActive: Glossary }
      >(
        context,
        `mutation SetTranslationGlossaryActive(
          $input: SetTranslationGlossaryActiveInput!
        ) {
          setTranslationGlossaryActive(input: $input) { ${GLOSSARY_FIELDS} }
        }`,
        { input: withoutKind(operation) }
      );
      return {
        kind: 'glossary',
        value: data.setTranslationGlossaryActive
      };
    }
    case 'set_memory_retention': {
      const data = await request<
        { input: Omit<typeof operation, 'kind'> },
        { setTranslationMemoryRetention: MemoryMutation }
      >(
        context,
        `mutation SetTranslationMemoryRetention(
          $input: SetTranslationMemoryRetentionInput!
        ) {
          setTranslationMemoryRetention(input: $input) {
            ${MEMORY_MUTATION_FIELDS}
          }
        }`,
        { input: withoutKind(operation) }
      );
      return {
        kind: 'memory_mutation',
        value: data.setTranslationMemoryRetention
      };
    }
    case 'tombstone_memory_entry':
    case 'purge_memory_entry': {
      const field =
        operation.kind === 'tombstone_memory_entry'
          ? 'tombstoneTranslationMemoryEntry'
          : 'purgeTranslationMemoryEntry';
      const data = await request<
        { input: Omit<typeof operation, 'kind'> },
        Record<string, MemoryMutation>
      >(
        context,
        `mutation TranslationMemoryLifecycle(
          $input: TransitionTranslationMemoryEntryInput!
        ) {
          ${field}(input: $input) { ${MEMORY_MUTATION_FIELDS} }
        }`,
        { input: withoutKind(operation) }
      );
      return { kind: 'memory_mutation', value: data[field] };
    }
    case 'create_job': {
      const data = await request<
        { input: Omit<TranslationOperation & { kind: 'create_job' }, 'kind'> },
        { createTranslationJob: Job }
      >(
        context,
        `mutation CreateTranslationJob($input: CreateTranslationJobInput!) {
          createTranslationJob(input: $input) { ${JOB_FIELDS} }
        }`,
        { input: withoutKind(operation) }
      );
      return { kind: 'job', value: data.createTranslationJob };
    }
    case 'create_workflow_note': {
      const input = withoutKind(operation);
      const data = await request<
        { input: typeof input },
        { createTranslationWorkflowNote: WorkflowNote }
      >(
        context,
        `mutation CreateTranslationWorkflowNote(
          $input: CreateTranslationWorkflowNoteInput!
        ) {
          createTranslationWorkflowNote(input: $input) {
            ${WORKFLOW_NOTE_FIELDS}
          }
        }`,
        { input }
      );
      return {
        kind: 'workflow_note',
        value: data.createTranslationWorkflowNote
      };
    }
    case 'resolve_workflow_note': {
      const input = withoutKind(operation);
      const data = await request<
        { input: typeof input },
        { resolveTranslationWorkflowNote: WorkflowNote }
      >(
        context,
        `mutation ResolveTranslationWorkflowNote(
          $input: ResolveTranslationWorkflowNoteInput!
        ) {
          resolveTranslationWorkflowNote(input: $input) {
            ${WORKFLOW_NOTE_FIELDS}
          }
        }`,
        { input }
      );
      return {
        kind: 'workflow_note',
        value: data.resolveTranslationWorkflowNote
      };
    }
    case 'read_job_progress': {
      const data = await request<
        { jobId: string },
        { translationJobProgress: JobProgress }
      >(
        context,
        `query TranslationJobProgress($jobId: UUID!) {
          translationJobProgress(jobId: $jobId) { ${JOB_PROGRESS_FIELDS} }
        }`,
        { jobId: operation.jobId }
      );
      return { kind: 'job_progress', value: data.translationJobProgress };
    }
    case 'read_reviewer_queue': {
      const input = {
        jobId: operation.jobId,
        assignee: operation.assignee ?? null,
        includeUnassigned: operation.includeUnassigned,
        limit: operation.limit
      };
      const data = await request<
        { input: typeof input },
        { translationReviewerQueue: ReviewerQueueItem[] }
      >(
        context,
        `query TranslationReviewerQueue($input: TranslationReviewerQueueInput!) {
          translationReviewerQueue(input: $input) { ${REVIEWER_QUEUE_FIELDS} }
        }`,
        { input }
      );
      return { kind: 'reviewer_queue', value: data.translationReviewerQueue };
    }
    case 'read_reviewer_workload': {
      const input = { jobId: operation.jobId };
      const data = await request<
        { input: typeof input },
        { translationReviewerWorkload: ReviewerWorkload[] }
      >(
        context,
        `query TranslationReviewerWorkload($input: TranslationReviewerWorkloadInput!) {
          translationReviewerWorkload(input: $input) { ${REVIEWER_WORKLOAD_FIELDS} }
        }`,
        { input }
      );
      return {
        kind: 'reviewer_workload',
        value: data.translationReviewerWorkload
      };
    }
    case 'list_workflow_notes': {
      const input = {
        jobId: operation.jobId,
        itemId: operation.itemId ?? null,
        includeResolved: operation.includeResolved,
        limit: operation.limit
      };
      const data = await request<
        { input: typeof input },
        { translationWorkflowNotes: WorkflowNote[] }
      >(
        context,
        `query TranslationWorkflowNotes($input: TranslationWorkflowNotesInput!) {
          translationWorkflowNotes(input: $input) { ${WORKFLOW_NOTE_FIELDS} }
        }`,
        { input }
      );
      return {
        kind: 'workflow_notes',
        value: data.translationWorkflowNotes
      };
    }
    case 'export_job': {
      const input = {
        jobId: operation.jobId,
        maxItems: operation.maxItems
      };
      const data = await request<
        { input: typeof input },
        { exportTranslationJob: InterchangeDocument }
      >(
        context,
        `query ExportTranslationJob($input: ExportTranslationJobInput!) {
          exportTranslationJob(input: $input) {
            ${INTERCHANGE_DOCUMENT_FIELDS}
          }
        }`,
        { input }
      );
      return {
        kind: 'interchange_document',
        value: data.exportTranslationJob
      };
    }
    case 'list_interchange_artifacts': {
      const input = {
        jobId: operation.jobId ?? null,
        includeExpired: operation.includeExpired,
        limit: operation.limit
      };
      const data = await request<
        { input: typeof input },
        { translationInterchangeArtifacts: InterchangeArtifact[] }
      >(
        context,
        `query TranslationInterchangeArtifacts($input: TranslationInterchangeArtifactsInput!) {
          translationInterchangeArtifacts(input: $input) { ${INTERCHANGE_ARTIFACT_FIELDS} }
        }`,
        { input }
      );
      return {
        kind: 'interchange_artifacts',
        value: data.translationInterchangeArtifacts
      };
    }
    case 'read_interchange_artifact': {
      const input = { artifactId: operation.artifactId };
      const data = await request<
        { input: typeof input },
        { translationInterchangeArtifact: InterchangeArtifactContent }
      >(
        context,
        `query TranslationInterchangeArtifact($input: ReadTranslationInterchangeArtifactInput!) {
          translationInterchangeArtifact(input: $input) { ${INTERCHANGE_ARTIFACT_CONTENT_FIELDS} }
        }`,
        { input }
      );
      return {
        kind: 'interchange_artifact_content',
        value: data.translationInterchangeArtifact
      };
    }
    case 'create_interchange_export_artifact': {
      const input = {
        jobId: operation.jobId,
        maxItems: operation.maxItems,
        expiresInSeconds: operation.expiresInSeconds,
        idempotencyKey: operation.idempotencyKey
      };
      const data = await request<
        { input: typeof input },
        { createTranslationInterchangeExportArtifact: InterchangeArtifact }
      >(
        context,
        `mutation CreateTranslationInterchangeExportArtifact(
          $input: CreateTranslationInterchangeExportArtifactInput!
        ) {
          createTranslationInterchangeExportArtifact(input: $input) {
            ${INTERCHANGE_ARTIFACT_FIELDS}
          }
        }`,
        { input }
      );
      return {
        kind: 'interchange_artifact',
        value: data.createTranslationInterchangeExportArtifact
      };
    }
    case 'store_interchange_import_artifact': {
      const input = {
        jobId: operation.jobId,
        documentJson: operation.documentJson,
        expiresInSeconds: operation.expiresInSeconds,
        idempotencyKey: operation.idempotencyKey
      };
      const data = await request<
        { input: typeof input },
        { storeTranslationInterchangeImportArtifact: InterchangeArtifact }
      >(
        context,
        `mutation StoreTranslationInterchangeImportArtifact(
          $input: StoreTranslationInterchangeImportArtifactInput!
        ) {
          storeTranslationInterchangeImportArtifact(input: $input) {
            ${INTERCHANGE_ARTIFACT_FIELDS}
          }
        }`,
        { input }
      );
      return {
        kind: 'interchange_artifact',
        value: data.storeTranslationInterchangeImportArtifact
      };
    }
    case 'process_interchange_import_artifact': {
      const input = {
        artifactId: operation.artifactId,
        idempotencyKey: operation.idempotencyKey
      };
      const data = await request<
        { input: typeof input },
        { processTranslationInterchangeImportArtifact: InterchangeArtifact }
      >(
        context,
        `mutation ProcessTranslationInterchangeImportArtifact(
          $input: ProcessTranslationInterchangeImportArtifactInput!
        ) {
          processTranslationInterchangeImportArtifact(input: $input) {
            ${INTERCHANGE_ARTIFACT_FIELDS}
          }
        }`,
        { input }
      );
      return {
        kind: 'interchange_artifact',
        value: data.processTranslationInterchangeImportArtifact
      };
    }
    case 'rebuild_job_progress': {
      const data = await request<
        { jobId: string; idempotencyKey: string },
        { rebuildTranslationJobProgress: JobProgress }
      >(
        context,
        `mutation RebuildTranslationJobProgress($jobId: UUID!, $idempotencyKey: String!) {
          rebuildTranslationJobProgress(jobId: $jobId, idempotencyKey: $idempotencyKey) {
            ${JOB_PROGRESS_FIELDS}
          }
        }`,
        { jobId: operation.jobId, idempotencyKey: operation.idempotencyKey }
      );
      return {
        kind: 'job_progress',
        value: data.rebuildTranslationJobProgress
      };
    }
    case 'sync_inventory': {
      const data = await request<
        { ownerSlug: string; resourceKind: string; limit: number },
        { syncTranslationProviderInventory: InventoryResult }
      >(
        context,
        `mutation SyncTranslationProviderInventory(
          $ownerSlug: String!, $resourceKind: String!, $limit: Int!
        ) {
          syncTranslationProviderInventory(
            ownerSlug: $ownerSlug, resourceKind: $resourceKind, limit: $limit
          ) { observedResources checkpoint checkpointRevision }
        }`,
        withoutKind(operation)
      );
      return {
        kind: 'inventory',
        value: data.syncTranslationProviderInventory
      };
    }
    case 'rebuild_inventory': {
      const data = await request<
        {
          ownerSlug: string;
          resourceKind: string;
          sourceLocale: string;
          targetLocale: string;
          pageSize: number;
        },
        { rebuildTranslationProviderInventory: InventoryResult }
      >(
        context,
        `mutation RebuildTranslationProviderInventory(
          $ownerSlug: String!, $resourceKind: String!, $sourceLocale: String!,
          $targetLocale: String!, $pageSize: Int!
        ) {
          rebuildTranslationProviderInventory(
            ownerSlug: $ownerSlug, resourceKind: $resourceKind,
            sourceLocale: $sourceLocale, targetLocale: $targetLocale,
            pageSize: $pageSize
          ) { observedResources checkpoint checkpointRevision }
        }`,
        withoutKind(operation)
      );
      return {
        kind: 'inventory',
        value: data.rebuildTranslationProviderInventory
      };
    }
    case 'read_provider_progress': {
      const data = await request<
        Omit<typeof operation, 'kind'>,
        { translationProviderProgress: ProviderProgress }
      >(
        context,
        `query TranslationProviderProgress(
          $ownerSlug: String!, $resourceKind: String!, $sourceLocale: String!,
          $targetLocale: String!
        ) {
          translationProviderProgress(
            ownerSlug: $ownerSlug, resourceKind: $resourceKind,
            sourceLocale: $sourceLocale, targetLocale: $targetLocale
          ) { ${PROVIDER_PROGRESS_FIELDS} }
        }`,
        withoutKind(operation)
      );
      return {
        kind: 'provider_progress',
        value: data.translationProviderProgress
      };
    }
    case 'read_required_progress': {
      const data = await request<
        Omit<typeof operation, 'kind'>,
        { translationRequiredProviderProgress: RequiredProviderProgress }
      >(
        context,
        `query TranslationRequiredProviderProgress(
          $ownerSlug: String!, $resourceKind: String!, $sourceLocale: String!
        ) {
          translationRequiredProviderProgress(
            ownerSlug: $ownerSlug, resourceKind: $resourceKind,
            sourceLocale: $sourceLocale
          ) {
            ownerSlug resourceKind sourceLocale requiredTargetLocales
            translationPolicyRevision tenantLocalePolicyRevision
            requiredUnits exactRequiredUnits optionalUnits exactOptionalUnits
            resourceLocalePairs completeResourceLocalePairs freshness
            targets { ${PROVIDER_PROGRESS_FIELDS} }
          }
        }`,
        withoutKind(operation)
      );
      return {
        kind: 'required_progress',
        value: data.translationRequiredProviderProgress
      };
    }
    case 'add_item': {
      const input = {
        jobId: operation.jobId,
        identity: {
          ownerSlug: operation.ownerSlug,
          resourceKind: operation.resourceKind,
          resourceId: operation.resourceId,
          subresourceId: operation.subresourceId || null
        },
        idempotencyKey: operation.idempotencyKey
      };
      const data = await request<
        { input: typeof input },
        { addTranslationJobItem: JobItem }
      >(
        context,
        `mutation AddTranslationJobItem($input: AddTranslationJobItemInput!) {
          addTranslationJobItem(input: $input) {
            id jobId ownerSlug resourceKind resourceId subresourceId
            status assignee { kind id } sourceDigest revision
          }
        }`,
        { input }
      );
      return { kind: 'item', value: data.addTranslationJobItem };
    }
    case 'save_proposal': {
      const input = {
        itemId: operation.itemId,
        origin: 'MANUAL',
        values: [{ key: operation.fieldKey, value: operation.value }],
        idempotencyKey: operation.idempotencyKey
      };
      const data = await request<
        { input: typeof input },
        { saveTranslationProposal: Proposal }
      >(
        context,
        `mutation SaveTranslationProposal($input: SaveTranslationProposalInput!) {
          saveTranslationProposal(input: $input) { ${PROPOSAL_FIELDS} }
        }`,
        { input }
      );
      return { kind: 'proposal', value: data.saveTranslationProposal };
    }
    case 'import_item': {
      const input = {
        ...operation.input,
        idempotencyKey: operation.idempotencyKey
      };
      const data = await request<
        { input: typeof input },
        { importTranslationItem: Proposal }
      >(
        context,
        `mutation ImportTranslationItem($input: ImportTranslationItemInput!) {
          importTranslationItem(input: $input) { ${PROPOSAL_FIELDS} }
        }`,
        { input }
      );
      return { kind: 'proposal', value: data.importTranslationItem };
    }
    case 'estimate_machine_translation': {
      const input = withoutKind(operation);
      const data = await request<
        { input: typeof input },
        { estimateMachineTranslation: MachineTranslationEstimate }
      >(
        context,
        `mutation EstimateMachineTranslation(
          $input: GenerateMachineTranslationProposalInput!
        ) {
          estimateMachineTranslation(input: $input) {
            ${MACHINE_ESTIMATE_FIELDS}
          }
        }`,
        { input }
      );
      return {
        kind: 'machine_estimate',
        value: data.estimateMachineTranslation
      };
    }
    case 'generate_machine_proposal': {
      const input = withoutKind(operation);
      const data = await request<
        { input: typeof input },
        { generateMachineTranslationProposal: MachineProposal }
      >(
        context,
        `mutation GenerateMachineTranslationProposal(
          $input: GenerateMachineTranslationProposalInput!
        ) {
          generateMachineTranslationProposal(input: $input) {
            ${MACHINE_PROPOSAL_FIELDS}
          }
        }`,
        { input }
      );
      return {
        kind: 'machine_proposal',
        value: data.generateMachineTranslationProposal
      };
    }
    case 'cancel_machine_operation': {
      const input = withoutKind(operation);
      const data = await request<
        { input: typeof input },
        { cancelMachineTranslationOperation: MachineCancellation }
      >(
        context,
        `mutation CancelMachineTranslationOperation(
          $input: CancelMachineTranslationOperationInput!
        ) {
          cancelMachineTranslationOperation(input: $input) {
            ${MACHINE_CANCELLATION_FIELDS}
          }
        }`,
        { input }
      );
      return {
        kind: 'machine_cancellation',
        value: data.cancelMachineTranslationOperation
      };
    }
    case 'recover_machine_operation': {
      const input = {
        operationId: operation.operationId,
        expectedUpdatedAt: operation.expectedUpdatedAt,
        proposal: {
          itemId: operation.itemId,
          fieldKeys: operation.fieldKeys,
          minimumMemorySimilarityBasisPoints:
            operation.minimumMemorySimilarityBasisPoints,
          tone: operation.tone,
          domain: operation.domain,
          style: operation.style
        },
        reason: operation.reason,
        idempotencyKey: operation.idempotencyKey
      };
      const data = await request<
        { input: typeof input },
        { recoverMachineTranslationOperation: MachineProposal }
      >(
        context,
        `mutation RecoverMachineTranslationOperation(
          $input: RecoverMachineTranslationOperationInput!
        ) {
          recoverMachineTranslationOperation(input: $input) {
            ${MACHINE_PROPOSAL_FIELDS}
          }
        }`,
        { input }
      );
      return {
        kind: 'machine_proposal',
        value: data.recoverMachineTranslationOperation
      };
    }
    case 'assign_item':
    case 'unassign_item': {
      const input = withoutKind(operation);
      const field =
        operation.kind === 'assign_item'
          ? 'assignTranslationItem'
          : 'unassignTranslationItem';
      const inputType =
        operation.kind === 'assign_item'
          ? 'AssignTranslationItemInput'
          : 'UnassignTranslationItemInput';
      const data = await request<
        { input: typeof input },
        Record<typeof field, Assignment>
      >(
        context,
        `mutation TranslationItemAssignment($input: ${inputType}!) {
          ${field}(input: $input) { ${ASSIGNMENT_FIELDS} }
        }`,
        { input }
      );
      return { kind: 'assignment', value: data[field] };
    }
    case 'cancel_job': {
      const input = withoutKind(operation);
      const data = await request<
        { input: typeof input },
        { cancelTranslationJob: Cancellation }
      >(
        context,
        `mutation CancelTranslationJob($input: CancelTranslationJobInput!) {
          cancelTranslationJob(input: $input) { ${CANCELLATION_FIELDS} }
        }`,
        { input }
      );
      return { kind: 'cancellation', value: data.cancelTranslationJob };
    }
    case 'retry_item': {
      const input = withoutKind(operation);
      const data = await request<
        { input: typeof input },
        { retryTranslationItem: Retry }
      >(
        context,
        `mutation RetryTranslationItem($input: RetryTranslationItemInput!) {
          retryTranslationItem(input: $input) { ${RETRY_FIELDS} }
        }`,
        { input }
      );
      return { kind: 'retry', value: data.retryTranslationItem };
    }
    case 'recover_apply': {
      const input = withoutKind(operation);
      const data = await request<
        { input: typeof input },
        { recoverTranslationApply: ApplyResult }
      >(
        context,
        `mutation RecoverTranslationApply($input: RecoverTranslationApplyInput!) {
          recoverTranslationApply(input: $input) { ${APPLY_RESULT_FIELDS} }
        }`,
        { input }
      );
      return { kind: 'apply', value: data.recoverTranslationApply };
    }
    case 'submit_proposal':
    case 'approve_proposal':
    case 'apply_proposal':
      return transitionProposal(context, operation);
  }
}

async function transitionProposal(
  context: RequestContext,
  operation: Extract<
    TranslationOperation,
    { kind: 'submit_proposal' | 'approve_proposal' | 'apply_proposal' }
  >
): Promise<TranslationResponse> {
  const input = {
    itemId: operation.itemId,
    proposalId: operation.proposalId,
    idempotencyKey: operation.idempotencyKey
  };
  const field =
    operation.kind === 'submit_proposal'
      ? 'submitTranslationProposal'
      : operation.kind === 'approve_proposal'
        ? 'approveTranslationProposal'
        : 'applyTranslationProposal';
  const selection =
    operation.kind === 'apply_proposal' ? APPLY_RESULT_FIELDS : PROPOSAL_FIELDS;
  const data = await request<
    { input: typeof input },
    Record<string, Proposal | ApplyResult>
  >(
    context,
    `mutation TranslationProposalTransition($input: TransitionTranslationProposalInput!) {
      ${field}(input: $input) { ${selection} }
    }`,
    { input }
  );
  return operation.kind === 'apply_proposal'
    ? { kind: 'apply', value: data[field] as ApplyResult }
    : { kind: 'proposal', value: data[field] as Proposal };
}

async function request<V, T>(
  context: RequestContext,
  query: string,
  variables?: V
): Promise<T> {
  return context.graphql<V, T>(
    query,
    variables,
    context.token,
    context.tenantSlug,
    { graphqlUrl: context.graphqlUrl }
  );
}

function withoutKind<T extends { kind: string }>(value: T): Omit<T, 'kind'> {
  const { kind: _kind, ...rest } = value;
  return rest;
}
