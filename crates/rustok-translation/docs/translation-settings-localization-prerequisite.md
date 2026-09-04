---
id: doc://crates/rustok-translation/docs/translation-settings-localization-prerequisite.md
kind: implementation_handoff
language: en
status: in_progress
last_reviewed: 2026-09-04
---

# Translation Settings localization prerequisite

Status: **owner persistence/read/source/progress plus stable provider resource+field identity source-ready / field descriptors, mutation adapter, and registration open**

Base reviewed before this slice: `main@132a2b09026f5e305a9c2d5ec06328c2658e1e37`.

## Existing owner foundation

The Settings owner boundary remains layered outside Translation:

- #3825 typed stable localized field IDs, string-leaf eligibility and sensitivity fences;
- #3831 parallel exact-locale storage, exact reads, target-row CAS, shared owner CAS, and replay-safe exact apply;
- #3832 content-free `change_seq` repair evidence;
- #3833 explicit source-locale provenance bound to the latest `base_projection` revision;
- #3834 bounded owner change reads plus stable exact-locale snapshot/progress facts.

Language-neutral Settings stay in `tenant_modules.settings`. Localized copy, repair evidence, source-locale provenance, and exact progress stay owner data. Runtime fallback is not exact coverage.

## What this slice adds: stable provider identity

`rustok-modules-translation` is a small owner-adapter crate between `rustok-modules` and the neutral `rustok-translation-targets` SPI. It intentionally has no database dependency, performs no owner persistence reads, implements no `TranslationTargetProvider`, and registers nothing at runtime.

`StaticSettingsTranslationIdentity::from_registry` maps one already validated `StaticSettingsLocalizationRegistry` to exactly one neutral resource identity:

- owner slug: `modules`;
- resource kind: `static_settings`;
- resource ID: the canonical static module slug;
- no subresource identity.

The only Translation field identities are the registry's stable localized field IDs. Because the registry stores them in a `BTreeMap`, the adapter exposes a deterministic sorted field-key inventory. It does not derive identities from schema paths, display labels, timestamps, JSON positions, or translated values.

Reverse mapping is fail-closed. `module_slug_from_identity` rejects a foreign owner slug, foreign resource kind, any subresource identity, or a resource ID that is not a valid static module slug. `contains_field` additionally requires exact resource identity plus an admitted stable field key before a future read/mutation adapter may resolve the field.

## Why field descriptors remain separate

This slice intentionally does not invent semantic metadata that the current owner registry does not yet expose through a provider contract. In particular it does not guess:

- required-vs-optional Translation units from path shape;
- AI-export permission for tenant-private Settings copy;
- protected-token or whitespace policy;
- one aggregate target revision for a resource whose exact values are stored in independently revisioned field rows.

The next bounded slice should map owner schema semantics into neutral `TranslationFieldDescriptor` values and define the resource/source/target revision encoding used by validate/apply. Only after those semantics are explicit should a full provider be registered.

## Bounded reader and exact progress remain authoritative

`StaticSettingsTranslationReadService::read_changes` remains the only owner repair reader. It freezes one inclusive `through_seq` high-water mark and drains by exclusive `after_seq`, so later commits cannot extend an in-progress scan.

`StaticSettingsTranslationReadService::exact_locale_snapshot` remains the exact source/target read boundary. It combines explicit source-locale provenance with exact target rows under a stable shared owner revision. `progress()` counts exact rows only; rendered fallback, tenant defaults, and negotiated runtime locales are never consulted.

## Remaining provider work

Three bounded pieces remain before Settings can be registered as a Translation target:

1. map owner field semantics to neutral field descriptors and pin revision encoding;
2. map neutral validate/apply requests to the existing exact owner services while preserving owner CAS, per-field target CAS, source revision checks, and idempotency;
3. register the provider only after those mappings are source-proven.

The adapter must consume public owner contracts. It must never read owner tables directly or bypass source-locale provenance, exact-row semantics, owner CAS, or operation receipts.

## Forbidden shortcuts

Do not store localized values in base Settings JSON, count fallback as exact coverage, localize sensitivity-fenced paths, put content in repair evidence, infer source locale, use timestamps for repair order, invent provider field semantics in an identity-only layer, collapse independent field target revisions without an explicit contract, or register a provider that reaches into owner persistence directly.

## Scope

This slice adds only the standalone Settings Translation identity adapter crate plus synchronized source evidence. It does not change migrations, owner persistence, runtime fallback, Settings command inputs, field descriptor semantics, validate/apply behavior, or provider registration.
