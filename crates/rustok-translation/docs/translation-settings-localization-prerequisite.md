---
id: doc://crates/rustok-translation/docs/translation-settings-localization-prerequisite.md
kind: implementation_handoff
language: en
status: in_progress
last_reviewed: 2026-09-04
---

# Translation Settings localization prerequisite

Status: **owner persistence/read/source/progress, stable provider identity, and conservative field descriptors source-ready / revision mapping, mutation adapter, and registration open**

Base reviewed before this slice: `main@1ea468d3541e961a76dccffb5877f8eb82d3fdbb`.

## Existing owner foundation

The Settings owner boundary remains layered outside Translation:

- #3825 typed stable localized field IDs, string-leaf eligibility and sensitivity fences;
- #3831 parallel exact-locale storage, exact reads, target-row CAS, shared owner CAS, and replay-safe exact apply;
- #3832 content-free `change_seq` repair evidence;
- #3833 explicit source-locale provenance bound to the latest `base_projection` revision;
- #3834 bounded owner change reads plus stable exact-locale snapshot/progress facts;
- #3835 stable neutral resource and field identities in the persistence-free `rustok-modules-translation` adapter crate.

Language-neutral Settings stay in `tenant_modules.settings`. Localized copy, repair evidence, source-locale provenance, and exact progress stay owner data. Runtime fallback is not exact coverage.

## What this slice adds: conservative field descriptors

`StaticSettingsTranslationIdentity::field_descriptors` and `descriptor_for_field` map only owner-admitted stable field IDs to neutral `TranslationFieldDescriptor` values. The adapter still has no persistence dependency, implements no `TranslationTargetProvider`, and registers nothing at runtime.

The descriptor policy is intentionally conservative and aligned with the exact owner progress contract:

- profile: `LocalizedScalar`;
- strategy: `Translate`;
- classification: `TenantPrivate`;
- `required = true` for every field that actually appears in the authoritative source snapshot;
- `ai_export_allowed = false` unless a future explicit owner metadata contract opts a field in;
- `max_characters = None` because the owner schema validator and exact apply boundary remain authoritative for concrete min/max constraints;
- `preserves_whitespace = false`; no protected-token or whitespace promise is invented by this slice.

The important distinction is that registry membership alone does not create a progress unit. `StaticSettingsTranslationReadService::exact_locale_snapshot` includes only owner-declared fields that currently contain source copy. Once a source field is present, exact target copy is required for that resource to be complete, which matches the existing owner `progress()` semantics.

`max_characters = None` is not permission to skip validation. A future provider adapter must still call the existing owner validate/apply boundary, which enforces the underlying Settings schema. Likewise `TenantPrivate` plus `ai_export_allowed = false` prevents provider onboarding from silently making tenant Settings copy eligible for AI export.

## Stable identity remains authoritative

The neutral Settings identity remains exactly one resource per static module:

- owner slug: `modules`;
- resource kind: `static_settings`;
- resource ID: canonical static module slug;
- no subresource identity;
- field keys: the registry's deterministic stable localized field IDs.

`module_slug_from_identity` rejects foreign owner/kind/subresource identities, and `contains_field` requires exact resource identity plus an admitted field key before later read/mutation mapping may resolve it.

## Bounded reader and exact progress remain authoritative

`StaticSettingsTranslationReadService::read_changes` remains the only owner repair reader. It freezes one inclusive `through_seq` high-water mark and drains by exclusive `after_seq`, so later commits cannot extend an in-progress scan.

`StaticSettingsTranslationReadService::exact_locale_snapshot` remains the exact source/target read boundary. It combines explicit source-locale provenance with exact target rows under a stable shared owner revision. `progress()` counts exact rows only; rendered fallback, tenant defaults, and negotiated runtime locales are never consulted.

## Why revision mapping remains separate

Exact localized Settings are stored in independently revisioned field rows while all writes also advance one shared static owner revision. The neutral target SPI exposes resource/source/target opaque revisions, so the adapter must define an explicit encoding rather than collapsing field revisions by accident.

The next bounded slice must pin:

1. the neutral resource revision derived from the shared owner revision;
2. the source revision tied to authoritative source-locale/base-projection provenance;
3. the target revision representation for a set of independent exact field rows;
4. how stale source, stale owner, missing target, and per-field target CAS are surfaced during validate/apply.

Only after that revision contract is source-proven should neutral validate/apply methods delegate to the existing owner exact apply service and provider registration become possible.

## Remaining provider work

Three bounded pieces remain before Settings can be registered as a Translation target:

1. pin neutral resource/source/target revision encoding without inventing aggregate state;
2. map neutral validate/apply requests to the existing exact owner services while preserving owner CAS, per-field target CAS, source revision checks, and idempotency;
3. register the provider only after those mappings are source-proven.

The adapter must consume public owner contracts. It must never read owner tables directly or bypass source-locale provenance, exact-row semantics, owner CAS, schema validation, or operation receipts.

## Forbidden shortcuts

Do not store localized values in base Settings JSON, count fallback as exact coverage, localize sensitivity-fenced paths, put content in repair evidence, infer source locale, use timestamps for repair order, treat `max_characters = None` as relaxed owner validation, enable AI export without explicit owner metadata, collapse independent target-row revisions into an invented aggregate revision, or register a provider that reaches into owner persistence directly.

## Scope

This slice changes only the persistence-free Settings Translation adapter descriptor policy plus synchronized source evidence/handoff/verifier. It does not change migrations, owner persistence, runtime fallback, Settings command inputs, revision encoding, validate/apply behavior, or provider registration.
