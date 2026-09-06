# rustok-translation-targets

## Purpose

`rustok-translation-targets` defines the owner-neutral contract through which
domain modules expose translatable resources to the future Translation control
plane.

## Responsibilities

- Define stable owner/resource/field identities and opaque revisions.
- Keep exact-locale state separate from rendered fallback.
- Define bounded list, read, exact aggregate progress, validate, and idempotent
  apply operations.
- Require actor-neutral apply request hashing with per-call actor
  re-authorization so unknown outcomes can be recovered safely.
- Declare field value profiles, translation strategy, data classification, and
  AI-export eligibility.
- Carry an explicit protected-token ledger and typed warning/error validation
  evidence; consumers never infer placeholder syntax. Provide shared pure
  comparison helpers for exact unique ledgers, token multiplicity, and
  owner-declared whitespace shape so every Translation path has identical
  semantics.
- Register owner-contributed providers through `ModuleRuntimeExtensions`.
- Provide contract validation and conformance fixtures.
- Provide `provider_support` helpers for contract-level source hashing, patch
  CAS validation, opaque revision conversion, sparse patch merging, lifecycle
  parsing, and receipt decoding without taking ownership of domain persistence
  or authorization.

## Interactions

- Uses `rustok-api::TenantLocale`, `PortContext`, and `PortError`.
- Owner modules implement `TranslationTargetProvider` and retain all canonical
  localized data and write validation.
- The future `rustok-translation` module consumes the registry without direct
  owner-table access.
- The future `rustok-ai-translation` adapter consumes only fields explicitly
  marked safe for AI export.

## Entry points

- `TranslationTargetProvider`
- `TranslationTargetRegistry`
- `register_translation_target_provider`
- `TranslationResourceSnapshot`
- `TranslationPatchRequest`
- `TranslationPatchIssueSeverity`
- `TranslationPatchValidation`
- `TranslationApplicationReceipt`
- `TranslationTargetProgressRequest`
- `TranslationTargetProgressFacts`
- `protected_token_ledger_matches`
- `protected_token_multiplicities_match`
- `whitespace_shape_matches`
- `provider_support`

Executable positive and negative reference-provider fixtures live in
`tests/reference_provider_conformance.rs`. They cover exact-locale discovery,
validation, revision-safe apply, idempotent replay, stale revisions, and
same-key/different-payload rejection.

## Docs

- [Contract documentation](./docs/README.md)
- [Translation implementation plan](../../docs/modules/translation-implementation-plan.md)
- [Platform documentation map](../../docs/index.md)
