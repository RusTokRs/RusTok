# rustok-translation

## Purpose

`rustok-translation` is the optional RusToK control-plane module for
owner-safe translation inventory, workflow, review, memory, glossaries, and
machine-translation orchestration.

## Responsibilities

- Maintain rebuildable tenant translation inventory and provider checkpoints.
- Own a revisioned required-target-locale subset validated through the
  Tenant-owned locale-policy port.
- Own tenant-scoped, locale-pair glossaries with hierarchical owner/resource/
  field scope, versioned concept snapshots, preferred/allowed/forbidden/
  do-not-translate variants, compare-and-set lifecycle changes, and durable
  actor-bound idempotency receipts.
- Read validated exact-locale owner aggregates and expose projection freshness
  by opaque cursor equality, including required-target cross-locale totals.
- Maintain a content-free, rebuildable per-job workflow progress projection
  plus bounded reviewer queue and workload read models derived from current
  workflow evidence rather than duplicated assignment state.
- Publish content-free, fixed-cardinality provider, workflow, memory, QA, and
  interchange observability through the shared telemetry registry; tenant
  content progress remains an authorized Translation read model rather than a
  process-global metric.
- Run a Translation-owned runtime worker, when private storage is configured,
  to delete expired interchange documents independently of later tenant
  requests while retaining content-free lifecycle metadata.
- Persist tenant-scoped jobs, immutable owner source snapshots, proposal and
  approval records, and owner-application receipts.
- Run deterministic platform QA on every proposal save, submission, and
  approval while preserving owner validation as the authoritative domain gate.
- Call owner modules only through `rustok-translation-targets`; never read or
  write owner translation tables directly.
- Keep machine-translation output review-required and route AI execution
  through the separate `rustok-ai-translation` adapter. Accept an adapter
  result only when it retains the requested policy digest, is review-required,
  and preserves the owner-declared protected-token multiplicity and required
  whitespace shape. Every AI packet is at least tenant-private because it
  contains tenant-scoped resource context; personal and sensitive units raise
  that classification. AI provider policy must reject an unpermitted packet
  before it is registered or sent externally.
- Remain fully usable for manual translation when the AI capability is absent.

## Entry points

- `TranslationModule`
- `TranslationInventoryService`
- `TranslationInventoryRebuildResult`
- `TranslationInventorySyncResult`
- `TranslationWorkflowService`
- `TranslationPolicyService`
- `TranslationProgressService`
- `TranslationExchangeService`
- `TranslationGlossaryService`
- `TranslationMemoryService`
- `MachineTranslationPort` and bounded machine-translation request/result
  contracts
- `TranslationMachineService` generation and audited stuck-save recovery
- `TranslationMachineControlService` status and cancellation controls
- `RecoverMachineOperationInput`
- `MemoryLookupInput`
- `MemoryListInput`
- `SetMemoryRetentionInput`
- `TombstoneMemoryEntryInput`
- `PurgeMemoryEntryInput`
- `MemoryEntryRecord`
- `MemoryMutationRecord`
- `MemoryRetentionPolicy`
- `MemorySuggestion`
- `MemoryMatchEvidence`
- `CreateGlossaryInput`
- `UpdateGlossaryInput`
- `ReplaceGlossaryTermsInput`
- `SetGlossaryActiveInput`
- `GlossaryBinding`
- `GlossaryRecord`
- `GlossarySummaryRecord`
- `GlossaryConcept`
- `GlossaryVariant`
- `JobProgressRecord`
- `ProviderProgressRecord`
- `ProviderProjectionFreshness`
- `RequiredProviderProgressRecord`
- `ReplaceRequiredTargetLocalesInput`
- `TranslationPolicyRecord`
- `TranslationPolicyFreshness`
- `evaluate_patch_qa`
- `map_translation_public_error`
- `TranslationPublicError`
- `TranslationPublicErrorKind`
- `graphql::TranslationQuery`
- `graphql::TranslationMutation`
- `graphql_runtime::TranslationGraphqlRuntimeData`
- `CreateJobInput`
- `AddItemInput`
- `SaveProposalInput`
- `SubmitProposalInput`
- `ApproveProposalInput`
- `AssignItemInput`
- `UnassignItemInput`
- `AssignmentRecord`
- `CancelJobInput`
- `CancellationRecord`
- `RetryItemInput`
- `RetryRecord`
- `ApplyProposalInput`
- `RecoverApplyInput`
- `ApplyRecord`
- `migrations::migrations`

## Interactions

- Depends on `rustok-translation-targets` for the neutral owner-provider SPI.
- Consumes `rustok-tenant::TenantLocalePolicyPort`; it never queries
  `tenant_locales` directly. Policy writes use tenant-scoped revision CAS,
  actor-bound durable idempotency receipts, and reject disabled locales. Reads
  expose stale policy plus its current CAS revision so an operator can rebase
  it safely.
- Requires the Core transactional outbox for content-free, typed workflow
  events. A workflow state transition and its event always share one database
  transaction.
- Uses owner-declared permissions and revisions in addition to Translation
  workflow permissions.
- Stores active assignment on the job item and append-only assignment commands
  separately. Assigned draft/review work is writable only by that actor or a
  Translation manager.
- Cancels remaining job work under job/item revision CAS while preserving
  already applied or excluded terminal items and rejecting unresolved owner
  apply outcomes.
- Completes a non-empty job automatically only when every item is applied,
  excluded, or cancelled. Conflict, stale, and blocked work never counts as
  successful completion.
- Allows an audited, actor-bound retry to return a blocked item with a current
  approved proposal to `approved`; stale and conflict items require explicit
  rebase instead of a blind owner retry.
- Updates per-job state, required/optional unit, applied/approved unit,
  assignment, resource-completion, and character-workload counters in the same
  transaction as workflow mutations. A Manage-authorized rebuild repairs the
  projection deterministically from workflow evidence.
- Reads provider aggregates only through the neutral SPI, rejects impossible
  facts, and reports checkpoint freshness as `current`, `behind`, or `unknown`
  without interpreting opaque cursors as numeric distances.
- Uses the required-target policy as the cross-locale progress denominator.
  The current source locale is excluded from its own target set, totals use
  checked arithmetic, and aggregate freshness is the worst target state.
- Validates a job glossary binding against tenant ownership, active lifecycle,
  current glossary revision, and the exact job locale pair before persisting
  the job. The captured glossary ID and revision remain immutable workflow
  evidence while later term replacements preserve old revision snapshots.
- Ingests reusable owner-field segments atomically with successful apply, only
  after user review and only for public or tenant-private data. Memory entries
  retain proposal, reviewer, owner-resource, source-hash, and apply-receipt
  provenance; replay cannot duplicate a proposal field.
- Provides tenant-scoped exact and context-aware fuzzy lookup with Unicode
  normalization, bounded candidates, stable ranking, and explicit score
  evidence. Unknown-locale, sensitive, secret, personal, and immutable
  transaction content does not enter the default memory path.
- Provides revision-guarded owner-lifecycle, retain-until, and legal-hold
  policies plus replay-safe tombstone and purge. Tombstoned entries leave
  lookup immediately; purge removes content while retaining content-free
  operation receipts.
- Persists typed QA warnings/errors. Required-field presence, non-empty
  required values, character bounds, protected-token multiplicity,
  owner-declared whitespace shape, excluded fields, lifecycle, and unchanged
  value warnings are deterministic. The same pass evaluates the job-captured
  glossary revision for applicable owner/resource/field scope, enforcing
  preferred, allowed, forbidden, and do-not-translate terminology. Any error
  blocks review/approval.
- Persists apply intent before invoking an owner and records `applied` only
  after validating and durably storing the owner's stable receipt.
- Allows an operator with both Translation Manage and Publish permissions to
  recover an unknown outcome through an audited, lease-guarded command. The
  owner reauthorizes that operator and reconciles the original mutation key.
- Does not own localized business data and has no dependency on Media, Product,
  Pages, Blog, Commerce, or other provider implementations.
- Publishes its operator GraphQL roots through module-manifest composition.
  The capability-owned runtime factory receives the provider registry and
  transactional event bus through neutral typed host values and constructs the
  Tenant locale-policy adapter inside Translation; the server does not contain
  owner-specific Translation wiring.
- Shares one redacted public-error classifier between GraphQL and native
  adapters, preserving stable client codes without exposing database details.
- Publishes the module-owned `rustok-translation-admin` package with one typed
  49-operation contract, native `#[server]` execution for SSR/hydrate,
  `rustok-graphql` execution for CSR/headless, and a six-tab Leptos workbench.
  The manifest mounts that package in the Leptos host; the matching
  `@rustok/translation-admin` package owns the parity Next admin workbench,
  including Translation Memory lookup/lifecycle management, bounded direct
  interchange export/import, private object-storage artifact lifecycle with
  exclusive import-processing leases, and
  machine-translation estimate, generation,
  status, cancellation, and recovery controls, revision-guarded
  assignment/unassignment, bounded reviewer queue and workload reads,
  blocked-item retry, job cancellation, owner-apply recovery, and private
  append-only job/item workflow notes. Notes are bounded, actor-bound,
  resolution-revision-guarded, and never enter Translation Memory, machine
  requests, owner application, or event bodies.

See the [local module contract](docs/README.md) and
[implementation plan](docs/implementation-plan.md).
