# rustok-translation-targets / CRATE_API

## Public surface

### Public modules
- `provider_support`: Source hashing, CAS patch validation, opaque revision mapping, and patch merging utilities.

### Primary Public Types & Contracts
- `TranslationTargetProvider`: Async trait for target providers (`list_resources`, `read_resource`, `validate_patch`, `apply_patch`, `read_progress`, `read_changes`, `export_resources`, `validate_import`).
- `TranslationTargetRegistry`: In-memory thread-safe registry of providers keyed by `(OwnerSlug, ResourceKind)`.
- `register_translation_target_provider`: Host extension registration helper.
- `TranslationResourceSnapshot`, `TranslationResourceSummary`, `TranslationResourceIdentity`: Authoritative resource metadata and field snapshots.
- `TranslationFieldDescriptor`, `TranslationFieldSnapshot`, `TranslationFieldPatch`: Field-level translation contracts, value profiles, and classifications.
- `TranslationPatchRequest`, `TranslationPatchValidation`, `TranslationApplicationReceipt`: Idempotent patch application and receipt verification.
- `TranslationTargetProgressRequest`, `TranslationTargetProgressFacts`: Progress aggregation contracts.
- Identifiers: `OwnerSlug`, `ResourceKind`, `ResourceId`, `FieldKey`, `OpaqueRevision`, `OpaqueCursor`.
- Token & Whitespace verification: `protected_token_ledger_matches`, `protected_token_multiplicities_match`, `whitespace_shape_matches`.

## Platform Capability Bindings
- **Provider SPI**: Defines the target contract implemented by domain modules (`rustok-blog`, `rustok-forum`, `rustok-commerce`, `rustok-pages`, etc.).
- **Registry Integration**: Stores `Arc<dyn TranslationTargetProvider>` in `rustok_core::ModuleRuntimeExtensions`.
- **Context Validation**: Enforces `PortCallPolicy::read()` for reads and `PortCallPolicy::write()` with idempotency key and deadline for mutations.

## Domain Collaborations
- **`rustok-translation`**: Primary consumer of target providers for inventory synchronization, translation job dispatch, and patch application.
- **`rustok-ai-translation`**: Consumes fields marked with `ai_export_allowed = true` and verifies protected token ledgers.
- **Domain Modules**: Implement `TranslationTargetProvider` without exposing internal database schemas or domain tables.

## Minimum Contract Set

### Input DTOs/Commands
- `ListTranslationResourcesRequest`: Page limit (1..=200), source/target locales, pagination cursor.
- `ReadTranslationResourceRequest`: Identity, source locale, target locale.
- `TranslationPatchRequest`: Identity, expected revisions, proposal ID, approval receipt ID, field patches.
- `TranslationTargetProgressRequest`: Source/target locale pair.
- `TranslationTargetChangesRequest`: Cursor and page limit.
- `TranslationTargetImportRequest`: Sequence of patch requests.

### Domain Invariants
- **Owner Sovereignty**: Owner modules retain all canonical storage and business invariants. Translation targets never mutate owner persistence directly.
- **Locale Distinctness**: Source locale and target locale must be strictly different (`source_locale != target_locale`).
- **Exact Locales**: The source locale must belong to the exact locales defined for the resource.
- **Protected Token Preservation**: Protected tokens must not be duplicated, empty, or absent from the source value.
- **Whitespace Shape Invariance**: Owner-declared whitespace shape (leading, trailing, and line-breaks) must be preserved between source and translated values.
- **AI Safety**: Fields classified as `Secret` or `ImmutableTransaction` must never allow AI export.

### Events / Outbox Side Effects
- Providers emit module-specific domain events upon patch application via `rustok-outbox`.
- Receipts (`TranslationApplicationReceipt`) confirm durable target mutation.

### Errors / Failure Codes
- `TranslationTargetContractError`:
  - `InvalidIdentity` (1..=191 characters)
  - `EmptyDisplayName`, `MissingCapability`, `InvalidCapabilityCombination`
  - `EqualSourceAndTargetLocale`, `InvalidPageLimit`, `SourceLocaleIsNotExact`
  - `DuplicateExactLocale`, `DuplicateFieldKey`, `EmptyPatch`, `DuplicatePatchFieldKey`
  - `EmptyReceiptIdentity`, `UnsafeAiExport`, `EmptyProtectedToken`, `DuplicateProtectedToken`
  - `ProtectedTokenMissingFromSource`, `PatchValidationAcceptanceMismatch`
- `TranslationTargetRegistryError`:
  - `InvalidDescriptor`
  - `DuplicateProvider`
