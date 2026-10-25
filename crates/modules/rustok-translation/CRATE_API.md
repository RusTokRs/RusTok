# Translation Crate API Specification

## Public surface

### Public modules
- `entities`
- `graphql` (behind feature `graphql`)
- `graphql_runtime` (behind feature `graphql`)
- `migrations`
- `module`

## Platform Capability Bindings
- **RusToKModule Implementation**: `TranslationModule` in `src/module.rs`
- **Module Slug**: `translation`
- **Module Name**: `Translation`
- **Module Kind**: `ModuleKind::Optional`
- **Dependencies**: `[]` (Layer 1/2 capabilities below are consumed via ports, not `modules.toml` deps)
- **Layer 1 — Core Platform Capabilities**:
  - `rustok-tenant::TenantLocalePolicyPort` — required; Translation never reads `tenant_locales` directly, validates source/target `TenantLocale` and `required_target_locales` against current Tenant locale-policy revision (AGENTS §5).
  - `rustok-outbox::TransactionalEventBus` — required for atomic workflow transitions + `TranslationWorkflowEvent` persistence.
  - `rustok-events` — canonical `DomainEvent` envelope for `TranslationJobCreated/ProposalApproved/Applied` etc.
  - `rustok-storage` (ObjectStorage, `StorageRuntime`) — optional/bounded; used only for `TranslationInterchangeArtifact` private object lifecycle (tenant-scoped keys, SHA256 integrity, expiry cleanup); absence fails that lifecycle closed without in-memory fallback.
- **Layer 2 — Attached Capabilities**:
  - `rustok-ai-translation` (optional adapter) — bounded `MachineTranslationPort` SPI; AI routing/inference stays outside Translation.
  - `flex`, `rustok-media`, `rustok-taxonomy` etc. — **not** consumed; Translation dispatches to owner providers via neutral `rustok-translation-targets`.
- **Permissions**:
  - `Permission::new(Resource::Translations, action)` (`Create`, `Read`, `Update`, `List`, `Resolve`, `Publish`, `Import`, `Export`, `Run`, `Manage`)
  - `Permission::new(Resource::TranslationMemory, action)` (`Create`, `Read`, `Update`, `Delete`, `List`, `Import`, `Export`, `Manage`)
  - `Permission::new(Resource::TranslationGlossaries, action)` (`Create`, `Read`, `Update`, `Delete`, `List`, `Import`, `Export`, `Manage`)
- **Runtime Extensions**:
  - When `feature = "runtime"`: Registers `TranslationMemoryRetentionWorkRegistration` and `TranslationInterchangeArtifactExpiryWorkRegistration` with `rustok_runtime::ModuleWorkRegistrations`.
- **Database Migrations**: Sea-ORM migrations defined in `src/migrations/mod.rs` (workflow jobs, items, proposals, memory segments, glossaries, and interchange artifacts).

## Domain Collaborations
- **rustok-tenant (peer business module)** — tenant-scoped localization policies and `required_target_locales`; interacts via `TenantLocalePolicyPort` port, not direct table reads.
- **rustok-translation-targets (neutral SPI, peer boundary)** — owner modules (`media`, `taxonomy`, `navigation`, `pages`, `modules/static_settings`) register `TranslationTargetProvider`; Translation dispatches exact patch application/receipt and reads aggregate progress via this seam only.
- **rustok-ai-translation (optional peer)** — AI adapter implementing `MachineTranslationPort`; Translation validates `tenant_private` classification & digests before/after dispatch.
- **rustok-outbox / rustok-events-module (platform events)** — emits `DomainEvent::TranslationJobCreated/ProposalApproved/Applied` atomically with state transitions.
- **Runtime Scheduler (`rustok_runtime::ModuleWorkRegistrations`)** — background periodic maintenance for memory retention tombstone/purge and interchange artifact expiry cleanup.

## Minimum Contract Set
- `TranslationWorkflowService`: Core lifecycle management for translation jobs, items, proposals, assignments, and approvals.
- `TranslationMemoryService`: Translation memory lookup, fuzzy matching, segment ingestion, and retention tombstoning.
- `TranslationGlossaryService`: Glossary term management, scoped variant matching, and policy enforcement.
- `TranslationExchangeService`: Interchange artifact export and import processing.
- `TranslationMachineService` & `TranslationMachineControlService`: Machine translation proposal generation, provider dispatch, and diagnostic recovery.
- `TranslationProgressService`: Reviewer queue metrics, workload tracking, and provider progress projections.

### Input DTOs/Commands
- `CreateJobInput`, `AddItemInput`, `AssignItemInput`, `UnassignItemInput`, `SubmitProposalInput`, `ApproveProposalInput`, `ApplyProposalInput`, `CancelJobInput`.
- `MemoryLookupInput`, `MemoryListInput`, `TombstoneMemoryEntryInput`, `PurgeMemoryEntryInput`, `SetMemoryRetentionInput`.
- `CreateGlossaryInput`, `UpdateGlossaryInput`, `ReplaceGlossaryTermsInput`, `SetGlossaryActiveInput`.
- `GenerateMachineProposalInput`, `CancelMachineOperationInput`, `RecoverMachineOperationInput`.
- `CreateInterchangeExportArtifactInput`, `StoreInterchangeImportArtifactInput`, `ProcessInterchangeImportArtifactInput`.

### Domain Invariants
- **Owner-Safe Application**: Only owner modules apply exact translation updates; the translation module never directly mutates downstream domain tables.
- **Protected Token Preservation**: Machine translation outputs must preserve exact protected tokens and required whitespace shapes.
- **Deterministic QA Gate**: Proposals undergo deterministic QA checks; forbidden or non-translatable glossary terms strictly block approval/application.
- **Memory Retention Pinned Protection**: Pinned translation memory segments are protected from background purging.
- **Crash Recovery Boundaries**: Stuck machine generation and apply operations are tracked by execution receipts and recoverable without duplication.

### Events / Outbox Side Effects
- `DomainEvent::TranslationJobCreated { job_id }`
- `DomainEvent::TranslationProposalApproved { proposal_id }`
- `DomainEvent::TranslationProposalApplied { proposal_id }`

### Errors / Failure Codes
- `TranslationError::NotFound`: Target job, proposal, memory segment, or glossary not found.
- `TranslationError::Validation`: Input parameters or format constraints violated.
- `TranslationError::Conflict`: Operation lease collision or optimistic concurrency revision mismatch.
- `TranslationError::QualityGateFailed`: QA checks or glossary invariant violation rejected proposal.
- `TranslationError::Database`: Database storage failure.
- `TranslationPublicError`: Client-facing error projection with redacted internal details.
