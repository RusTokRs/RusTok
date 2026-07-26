# rustok-translation-targets

## Purpose

`rustok-translation-targets` defines the owner-neutral contract through which
domain modules expose translatable resources to the future Translation control
plane.

## Responsibilities

- Define stable owner/resource/field identities and opaque revisions.
- Keep exact-locale state separate from rendered fallback.
- Define bounded list, read, validate, and idempotent apply operations.
- Declare field value profiles, translation strategy, data classification, and
  AI-export eligibility.
- Register owner-contributed providers through `ModuleRuntimeExtensions`.
- Provide contract validation and conformance fixtures.

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
- `TranslationApplicationReceipt`

## Docs

- [Contract documentation](./docs/README.md)
- [Translation implementation plan](../../docs/modules/translation-implementation-plan.md)
- [Platform documentation map](../../docs/index.md)

