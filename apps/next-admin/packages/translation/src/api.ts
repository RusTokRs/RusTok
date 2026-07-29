import type {
  AdminGraphqlExecutor,
  ApplyResult,
  Glossary,
  GlossarySummary,
  InventoryResult,
  Job,
  JobItem,
  JobProgress,
  MemoryEntry,
  MemoryMutation,
  MemorySuggestion,
  Proposal,
  ProviderProgress,
  RequiredProviderProgress,
  TranslationOperation,
  TranslationPolicy,
  TranslationResponse,
  TranslationTarget
} from './types';

const POLICY_FIELDS =
  'tenantId requiredTargetLocales tenantLocalePolicyRevision revision freshness disabledRequiredTargetLocales';
const JOB_PROGRESS_FIELDS =
  'jobId sourceDigest totalItems assignedItems terminalItems missingItems draftItems inReviewItems approvedItems applyingItems appliedItems staleItems conflictItems blockedItems excludedItems cancelledItems requiredUnits optionalUnits appliedRequiredUnits appliedOptionalUnits approvedRequiredUnits approvedOptionalUnits completeResources sourceCharacters translatedCharacters revision updatedAt';
const PROVIDER_PROGRESS_FIELDS =
  'ownerSlug resourceKind sourceLocale targetLocale requiredUnits exactRequiredUnits optionalUnits exactOptionalUnits resources completeResources ownerChangeCursor projectedCursor checkpointRevision checkpointUpdatedAt freshness';
const PROPOSAL_FIELDS =
  'id itemId proposalRevision origin values { key value expectedSourceHash } qaIssues { field severity code message } qaAccepted status approvalReceiptId';
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
    operation.kind === 'apply_proposal'
      ? 'operationId itemId proposalId providerReceiptId resourceRevision targetRevision appliedFieldKeys'
      : PROPOSAL_FIELDS;
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
