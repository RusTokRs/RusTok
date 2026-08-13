export type AdminGraphqlExecutor = <V, T>(
  query: string,
  variables?: V,
  token?: string | null,
  tenantSlug?: string | null,
  options?: { graphqlUrl?: string; tenantId?: string | null }
) => Promise<T>;

export type TranslationAdminPageProps = {
  graphql: AdminGraphqlExecutor;
  token?: string | null;
  tenantSlug?: string | null;
  graphqlUrl?: string;
};

export type TranslationPolicy = {
  tenantId: string;
  requiredTargetLocales: string[];
  tenantLocalePolicyRevision: number;
  revision: number;
  freshness: string;
  disabledRequiredTargetLocales: string[];
};

export type TranslationTarget = {
  ownerSlug: string;
  resourceKind: string;
  displayName: string;
  capabilities: string[];
  readPermissionFloor: string[];
  applyPermissionFloor: string[];
};

export type GlossaryBinding = {
  glossaryId: string;
  revision: number;
};

export type GlossaryScope = {
  ownerSlug?: string | null;
  resourceKind?: string | null;
  fieldKey?: string | null;
};

export type GlossaryTermPolicy =
  'PREFERRED' | 'ALLOWED' | 'FORBIDDEN' | 'DO_NOT_TRANSLATE';

export type GlossaryMatchKind = 'EXACT' | 'WHOLE_WORD' | 'SUBSTRING';

export type GlossaryVariant = {
  value: string;
  policy: GlossaryTermPolicy;
};

export type GlossaryConcept = {
  conceptKey: string;
  sourceTerm: string;
  variants: GlossaryVariant[];
  matchKind: GlossaryMatchKind;
  caseSensitive: boolean;
  notes: string;
};

export type GlossarySummary = {
  id: string;
  name: string;
  description: string;
  sourceLocale: string;
  targetLocale: string;
  scope: GlossaryScope;
  isActive: boolean;
  revision: number;
};

export type Glossary = GlossarySummary & {
  concepts: GlossaryConcept[];
};

export type MemoryRetentionPolicy =
  'OWNER_LIFECYCLE' | 'RETAIN_UNTIL' | 'LEGAL_HOLD';

export type MemoryMatchKind = 'EXACT' | 'CONTEXTUAL_FUZZY' | 'FUZZY';

export type MemoryMatchEvidence = {
  kind: MemoryMatchKind;
  sourceExact: boolean;
  contextMatch: boolean;
  baseSimilarityBasisPoints: number;
  contextBonusBasisPoints: number;
  finalSimilarityBasisPoints: number;
  segmentationVersion: string;
};

export type MemorySuggestion = {
  entryId: string;
  sourceText: string;
  targetText: string;
  sourceHash: string;
  ownerSlug: string;
  resourceKind: string;
  resourceId: string;
  fieldKey: string;
  origin: string;
  proposalId: string;
  applyReceiptId: string;
  evidence: MemoryMatchEvidence;
};

export type MemoryEntry = {
  id: string;
  tenantId: string;
  sourceLocale: string;
  targetLocale: string;
  ownerSlug: string;
  resourceKind: string;
  resourceId: string;
  subresourceId: string | null;
  fieldKey: string;
  sourceText: string;
  targetText: string;
  sourceHash: string;
  targetHash: string;
  contextFingerprint: string;
  segmentationVersion: string;
  origin: string;
  qualityState: string;
  reviewerActorKind: string;
  reviewerActorId: string;
  proposalId: string;
  applyReceiptId: string;
  retentionPolicy: MemoryRetentionPolicy;
  retainUntil: string | null;
  tombstonedAt: string | null;
  revision: number;
  createdAt: string;
  updatedAt: string;
};

export type MemoryMutation = {
  entryId: string;
  revision: number;
  state: string;
  retentionPolicy: MemoryRetentionPolicy;
  retainUntil: string | null;
  tombstonedAt: string | null;
};

export type Job = {
  id: string;
  sourceLocale: string;
  targetLocale: string;
  glossary: GlossaryBinding | null;
  status: string;
  revision: number;
};

export type JobProgress = {
  jobId: string;
  totalItems: number;
  appliedItems: number;
  blockedItems: number;
  revision: number;
  [key: string]: unknown;
};

export type TranslationResourceIdentity = {
  ownerSlug: string;
  resourceKind: string;
  resourceId: string;
  subresourceId: string | null;
};

export type InterchangeField = {
  key: string;
  sourceValue: string;
  exactTargetValue: string | null;
  proposedValue: string | null;
  sourceHash: string;
  required: boolean;
  maxCharacters: number | null;
  protectedTokens: string[];
};

export type InterchangeItem = {
  itemId: string;
  identity: TranslationResourceIdentity;
  sourceDigest: string;
  sourceRevision: string;
  targetRevision: string | null;
  fields: InterchangeField[];
};

export type InterchangeDocument = {
  schemaVersion: number;
  jobId: string;
  sourceLocale: string;
  targetLocale: string;
  items: InterchangeItem[];
};

export type InterchangeArtifactItemOutcome = {
  itemId: string;
  status: string;
};

export type InterchangeConflictReport = {
  totalItems: number;
  acceptedItems: number;
  conflictItems: number;
  rejectedItems: number;
  outcomes: InterchangeArtifactItemOutcome[];
};

export type InterchangeArtifact = {
  id: string;
  jobId: string;
  direction: string;
  status: string;
  contentLength: number;
  checksumSha256: string;
  expiresAt: string;
  processedAt: string | null;
  report: InterchangeConflictReport | null;
  createdAt: string;
  updatedAt: string;
};

export type InterchangeArtifactContent = {
  artifact: InterchangeArtifact;
  document: InterchangeDocument;
};

export type ImportItemInput = {
  schemaVersion: number;
  jobId: string;
  itemId: string;
  identity: TranslationResourceIdentity;
  sourceDigest: string;
  values: Array<{ key: string; value: string }>;
};

export type ProviderProgress = {
  ownerSlug: string;
  resourceKind: string;
  sourceLocale: string;
  targetLocale: string;
  resources: number;
  completeResources: number;
  freshness: string;
  [key: string]: unknown;
};

export type RequiredProviderProgress = {
  ownerSlug: string;
  resourceKind: string;
  sourceLocale: string;
  requiredTargetLocales: string[];
  resourceLocalePairs: number;
  completeResourceLocalePairs: number;
  freshness: string;
  targets: ProviderProgress[];
  [key: string]: unknown;
};

export type JobItem = {
  id: string;
  jobId: string;
  ownerSlug: string;
  resourceKind: string;
  resourceId: string;
  subresourceId: string | null;
  status: string;
  assignee: Actor | null;
  sourceDigest: string;
  revision: number;
  [key: string]: unknown;
};

export type ReviewerQueueItem = {
  item: JobItem;
  proposalId: string;
  proposalRevision: number;
  submittedAt: string;
};

export type ReviewerWorkload = {
  jobId: string;
  assignee: Actor | null;
  openItems: number;
  missingItems: number;
  draftItems: number;
  inReviewItems: number;
  approvedItems: number;
  applyingItems: number;
  rebaseRequiredItems: number;
  blockedItems: number;
  sourceCharacters: number;
};

export type WorkflowNote = {
  id: string;
  jobId: string;
  itemId: string | null;
  body: string;
  author: Actor;
  revision: number;
  resolvedAt: string | null;
  resolvedBy: Actor | null;
  createdAt: string;
  updatedAt: string;
};

export type Proposal = {
  id: string;
  itemId: string;
  status: string;
  qaIssues: Array<{ severity: string; code: string; message: string }>;
  [key: string]: unknown;
};

export type MachineTranslationAttempt = {
  attempt: number;
  providerProfileId: string;
  providerSlug: string;
  model: string;
  fallback: boolean;
};

export type MachineTranslationUsage = {
  inputTokens: number;
  outputTokens: number;
  totalTokens: number;
  costMinorUnits: number;
  currencyCode: string;
  priceSnapshotDigest: string;
};

export type MachineTranslationEstimate = {
  inputTokensUpperBound: number;
  outputTokensUpperBound: number;
  attemptsUpperBound: number;
  costMinorUnitsUpperBound: number;
  currencyCode: string;
  priceSnapshotDigest: string;
  reviewRequired: boolean;
};

export type MachineTranslationDiagnostic = {
  code: string;
  blocking: boolean;
  unitId: string | null;
};

export type MachineProposal = {
  operationId: string;
  itemId: string;
  proposalId: string;
  adapterSlug: string;
  providerSlug: string;
  providerPolicyDigest: string;
  machineRequestDigest: string;
  glossaryRevision: string | null;
  glossaryDigest: string | null;
  memoryDigest: string | null;
  executionId: string;
  executionRequestDigest: string;
  promptPolicyDigest: string;
  attempts: MachineTranslationAttempt[];
  usage: MachineTranslationUsage;
  diagnostics: MachineTranslationDiagnostic[];
  reviewRequired: boolean;
  createdAt: string;
  updatedAt: string;
};

export type MachineCancellation = {
  cancellationId: string;
  operationId: string;
  status: string;
  providerExecutionId: string | null;
  providerStatus: string;
  providerErrorCode: string | null;
  providerObservedAt: string;
  createdAt: string;
};

export type MachineOperationStatus = {
  operationId: string;
  itemId: string;
  status: string;
  providerExecutionId: string | null;
  providerStatus: string;
  providerErrorCode: string | null;
  updatedAt: string;
};

export type MachineProposalOutcome =
  | ({ __typename: 'MachineTranslationProposal' } & MachineProposal)
  | ({ __typename: 'MachineTranslationOperationStatus' } & MachineOperationStatus);

export type ApplyResult = {
  operationId: string;
  itemId: string;
  proposalId: string;
  providerReceiptId: string;
  resourceRevision: string;
  targetRevision: string;
  appliedFieldKeys: string[];
};

export type ActorKind = 'USER' | 'SERVICE';

export type Actor = {
  kind: ActorKind;
  id: string;
};

export type Assignment = {
  operationId: string;
  itemId: string;
  assignee: Actor | null;
  itemRevision: number;
};

export type Cancellation = {
  cancellationId: string;
  jobId: string;
  jobRevision: number;
  cancelledItemCount: number;
};

export type Retry = {
  retryId: string;
  itemId: string;
  itemRevision: number;
  status: string;
};

export type InventoryResult = {
  observedResources: number;
  checkpoint: string | null;
  checkpointRevision: number;
};

export type TranslationOperation =
  | { kind: 'read_policy' }
  | { kind: 'read_machine_operation_status'; operationId: string }
  | { kind: 'list_targets' }
  | { kind: 'list_glossaries'; limit: number }
  | { kind: 'read_glossary'; glossaryId: string; revision?: number }
  | {
      kind: 'list_memory_entries';
      sourceLocale?: string;
      targetLocale?: string;
      includeTombstoned: boolean;
      limit: number;
    }
  | { kind: 'read_memory_entry'; entryId: string }
  | {
      kind: 'lookup_memory';
      sourceLocale: string;
      targetLocale: string;
      identity: {
        ownerSlug: string;
        resourceKind: string;
        resourceId: string;
        subresourceId?: string | null;
      };
      fieldKey: string;
      sourceText: string;
      minimumSimilarityBasisPoints: number;
      limit: number;
    }
  | {
      kind: 'replace_policy';
      expectedRevision: number;
      requiredTargetLocales: string[];
      idempotencyKey: string;
    }
  | {
      kind: 'create_glossary';
      name: string;
      description: string;
      sourceLocale: string;
      targetLocale: string;
      scope: GlossaryScope;
      idempotencyKey: string;
    }
  | {
      kind: 'update_glossary';
      glossaryId: string;
      expectedRevision: number;
      name: string;
      description: string;
      idempotencyKey: string;
    }
  | {
      kind: 'replace_glossary_terms';
      glossaryId: string;
      expectedRevision: number;
      concepts: GlossaryConcept[];
      idempotencyKey: string;
    }
  | {
      kind: 'set_glossary_active';
      glossaryId: string;
      expectedRevision: number;
      isActive: boolean;
      idempotencyKey: string;
    }
  | {
      kind: 'set_memory_retention';
      entryId: string;
      expectedRevision: number;
      policy: MemoryRetentionPolicy;
      retainUntil?: string | null;
      idempotencyKey: string;
    }
  | {
      kind: 'tombstone_memory_entry' | 'purge_memory_entry';
      entryId: string;
      expectedRevision: number;
      idempotencyKey: string;
    }
  | {
      kind: 'create_job';
      sourceLocale: string;
      targetLocale: string;
      glossary?: GlossaryBinding;
      idempotencyKey: string;
    }
  | {
      kind: 'create_workflow_note';
      jobId: string;
      itemId?: string | null;
      body: string;
      idempotencyKey: string;
    }
  | {
      kind: 'resolve_workflow_note';
      noteId: string;
      expectedRevision: number;
      idempotencyKey: string;
    }
  | { kind: 'read_job_progress'; jobId: string }
  | {
      kind: 'read_reviewer_queue';
      jobId: string;
      assignee?: Actor | null;
      includeUnassigned: boolean;
      limit: number;
    }
  | { kind: 'read_reviewer_workload'; jobId: string }
  | {
      kind: 'list_workflow_notes';
      jobId: string;
      itemId?: string | null;
      includeResolved: boolean;
      limit: number;
    }
  | {
      kind: 'list_interchange_artifacts';
      jobId?: string | null;
      includeExpired: boolean;
      limit: number;
    }
  | { kind: 'read_interchange_artifact'; artifactId: string }
  | { kind: 'export_job'; jobId: string; maxItems: number }
  | {
      kind: 'create_interchange_export_artifact';
      jobId: string;
      maxItems: number;
      expiresInSeconds: number;
      idempotencyKey: string;
    }
  | {
      kind: 'store_interchange_import_artifact';
      jobId: string;
      documentJson: string;
      expiresInSeconds: number;
      idempotencyKey: string;
    }
  | {
      kind: 'process_interchange_import_artifact';
      artifactId: string;
      idempotencyKey: string;
    }
  | {
      kind: 'import_item';
      input: ImportItemInput;
      idempotencyKey: string;
    }
  | { kind: 'rebuild_job_progress'; jobId: string; idempotencyKey: string }
  | {
      kind: 'sync_inventory';
      ownerSlug: string;
      resourceKind: string;
      limit: number;
    }
  | {
      kind: 'rebuild_inventory';
      ownerSlug: string;
      resourceKind: string;
      sourceLocale: string;
      targetLocale: string;
      pageSize: number;
    }
  | {
      kind: 'read_provider_progress';
      ownerSlug: string;
      resourceKind: string;
      sourceLocale: string;
      targetLocale: string;
    }
  | {
      kind: 'read_required_progress';
      ownerSlug: string;
      resourceKind: string;
      sourceLocale: string;
    }
  | {
      kind: 'add_item';
      jobId: string;
      ownerSlug: string;
      resourceKind: string;
      resourceId: string;
      subresourceId?: string;
      idempotencyKey: string;
    }
  | {
      kind: 'save_proposal';
      itemId: string;
      fieldKey: string;
      value: string;
      idempotencyKey: string;
    }
  | {
      kind: 'estimate_machine_translation' | 'generate_machine_proposal';
      itemId: string;
      fieldKeys: string[];
      minimumMemorySimilarityBasisPoints: number;
      tone?: string | null;
      domain?: string | null;
      style?: string | null;
      idempotencyKey: string;
    }
  | {
      kind: 'cancel_machine_operation';
      operationId: string;
      reason: string;
      idempotencyKey: string;
    }
  | {
      kind: 'recover_machine_operation';
      operationId: string;
      expectedUpdatedAt: string;
      itemId: string;
      fieldKeys: string[];
      minimumMemorySimilarityBasisPoints: number;
      tone?: string | null;
      domain?: string | null;
      style?: string | null;
      reason: string;
      idempotencyKey: string;
    }
  | {
      kind: 'assign_item';
      itemId: string;
      expectedRevision: number;
      assignee: Actor;
      idempotencyKey: string;
    }
  | {
      kind: 'unassign_item';
      itemId: string;
      expectedRevision: number;
      idempotencyKey: string;
    }
  | {
      kind: 'cancel_job';
      jobId: string;
      expectedRevision: number;
      reason: string;
      idempotencyKey: string;
    }
  | {
      kind: 'retry_item';
      itemId: string;
      expectedRevision: number;
      reason: string;
      idempotencyKey: string;
    }
  | {
      kind: 'recover_apply';
      operationId: string;
      expectedAttemptCount: number;
      reason: string;
      idempotencyKey: string;
    }
  | {
      kind: 'submit_proposal' | 'approve_proposal' | 'apply_proposal';
      itemId: string;
      proposalId: string;
      idempotencyKey: string;
    };

export type TranslationResponse =
  | { kind: 'policy'; value: TranslationPolicy }
  | { kind: 'targets'; value: TranslationTarget[] }
  | { kind: 'glossaries'; value: GlossarySummary[] }
  | { kind: 'glossary'; value: Glossary }
  | { kind: 'memory_entries'; value: MemoryEntry[] }
  | { kind: 'memory_entry'; value: MemoryEntry }
  | { kind: 'memory_suggestions'; value: MemorySuggestion[] }
  | { kind: 'memory_mutation'; value: MemoryMutation }
  | { kind: 'job'; value: Job }
  | { kind: 'job_progress'; value: JobProgress }
  | { kind: 'reviewer_queue'; value: ReviewerQueueItem[] }
  | { kind: 'reviewer_workload'; value: ReviewerWorkload[] }
  | { kind: 'workflow_notes'; value: WorkflowNote[] }
  | { kind: 'workflow_note'; value: WorkflowNote }
  | { kind: 'interchange_document'; value: InterchangeDocument }
  | { kind: 'interchange_artifacts'; value: InterchangeArtifact[] }
  | { kind: 'interchange_artifact'; value: InterchangeArtifact }
  | { kind: 'interchange_artifact_content'; value: InterchangeArtifactContent }
  | { kind: 'provider_progress'; value: ProviderProgress }
  | { kind: 'required_progress'; value: RequiredProviderProgress }
  | { kind: 'item'; value: JobItem }
  | { kind: 'proposal'; value: Proposal }
  | { kind: 'machine_estimate'; value: MachineTranslationEstimate }
  | { kind: 'machine_proposal'; value: MachineProposal }
  | { kind: 'machine_operation_status'; value: MachineOperationStatus }
  | { kind: 'machine_cancellation'; value: MachineCancellation }
  | { kind: 'apply'; value: ApplyResult }
  | { kind: 'assignment'; value: Assignment }
  | { kind: 'cancellation'; value: Cancellation }
  | { kind: 'retry'; value: Retry }
  | { kind: 'inventory'; value: InventoryResult };
