---
id: doc://crates/rustok-translation/docs/translation-settings-localization-prerequisite.md
kind: implementation_handoff
language: en
status: in_progress
last_reviewed: 2026-09-04
---

# Translation Settings localization prerequisite

Status: **owner persistence/read/source/progress, stable provider identity, conservative field descriptors, and opaque revision mapping source-ready / validate-apply adapter and registration open**

Base reviewed before this slice: `main@808d86f6146406dbe61cb266a37978d538326b96`.

## Existing owner foundation

The Settings owner boundary remains layered outside Translation:

- #3825 typed stable localized field IDs, string-leaf eligibility and sensitivity fences;
- #3831 parallel exact-locale storage, exact reads, target-row CAS, shared owner CAS, and replay-safe exact apply;
- #3832 content-free `change_seq` repair evidence;
- #3833 explicit source-locale provenance bound to the latest `base_projection` revision;
- #3834 bounded owner change reads plus stable exact-locale snapshot/progress facts;
- #3835 stable neutral resource and field identities in the persistence-free `rustok-modules-translation` adapter crate;
- #3836 conservative `LocalizedScalar` / `TenantPrivate` field descriptors with required exact source-present units and AI export default-denied.

Language-neutral Settings stay in `tenant_modules.settings`. Localized copy, repair evidence, source-locale provenance, exact progress, and per-field target revisions stay owner data. Runtime fallback is not exact coverage.

## What this slice adds: explicit opaque revision mapping

`StaticSettingsTranslationIdentity::revisions_for_snapshot` maps one already stable `StaticSettingsExactLocaleSnapshot` into the neutral resource/source/target revision contract. The adapter still has no database dependency, implements no `TranslationTargetProvider`, and registers nothing at runtime.

### Resource revision

`resource_revision` is `settings-owner-v1:<owner_revision>` and therefore follows the shared static owner revision exactly. Any base Settings write, source-locale assignment, or exact localized target write that advances the owner aggregate also advances the neutral resource revision.

This is the coarse resource CAS clock. It does not replace per-field target CAS.

### Source revision

`source_revision` is a deterministic SHA-256 digest over length-framed canonical source facts:

- revision namespace/version;
- static module slug;
- canonical source locale;
- sorted current source field IDs;
- each current source field value.

It deliberately does **not** include the shared owner revision. A target-only localized write therefore advances `resource_revision` but leaves `source_revision` unchanged when source copy is unchanged. A base source-copy change changes the source digest even if the localized target rows are untouched.

The digest is an opaque neutral precondition, not persisted owner state.

### Target revision

`target_revision` is `None` while none of the current source fields has an exact target row. Once any exact target exists, the adapter returns a deterministic SHA-256 digest over:

- revision namespace/version;
- static module slug;
- canonical target locale;
- sorted current source field IDs;
- for each field, either its positive owner target-row revision or an explicit `missing` marker.

Target values are not hashed into this revision because the owner exact write contract already advances each target-row revision when target copy changes. The digest therefore represents the current aggregate target precondition without fabricating a new numeric owner revision.

Critically, this digest does **not** replace the per-field revisions carried by `StaticSettingsExactLocaleField`. A future apply adapter must first compare the neutral aggregate precondition, then use each field's actual `target_revision` as the `expected_target_revision` passed to owner `apply_exact`.

### Fail-closed snapshot checks

Before producing revisions, the adapter rejects:

- a snapshot whose module slug does not match the neutral resource identity;
- zero shared owner revision;
- duplicate or owner-unadmitted source fields;
- target value/revision/target-owner-revision triples that are only partially populated;
- zero target revisions;
- target-owner revisions newer than the enclosing stable owner snapshot.

Fields are sorted before digesting, so revision values do not depend on snapshot row order.

## Descriptor and identity policy remain authoritative

The neutral Settings identity remains one resource per static module: owner `modules`, kind `static_settings`, canonical module slug resource ID, no subresource. Field keys remain the registry's stable localized field IDs.

Descriptors remain `LocalizedScalar`, `Translate`, `TenantPrivate`, required for source-present units, AI export default-denied, with owner schema validation still authoritative. `max_characters = None` is not relaxed validation.

## Bounded reader and exact progress remain authoritative

`StaticSettingsTranslationReadService::read_changes` remains the only owner repair reader. It freezes one inclusive `through_seq` high-water mark and drains by exclusive `after_seq`, so later commits cannot extend an in-progress scan.

`StaticSettingsTranslationReadService::exact_locale_snapshot` remains the exact source/target read boundary. It combines explicit source-locale provenance with exact target rows under a stable shared owner revision. `progress()` counts exact rows only; rendered fallback, tenant defaults, and negotiated runtime locales are never consulted.

## Remaining provider work

Two bounded pieces remain before Settings can be registered as a Translation target:

1. map neutral `read_resource`, `validate_patch`, and `apply_patch` semantics onto the existing exact owner services while checking the new resource/source/target revision preconditions and preserving per-field target CAS, shared owner CAS, owner schema validation, and idempotency;
2. register the provider only after that adapter mapping is source-proven.

The mutation adapter must not treat the target digest as a substitute for row CAS. Multi-field apply must deliberately advance the shared owner revision between field writes and preserve one replay-safe provider operation contract rather than bypassing owner receipts.

## Forbidden shortcuts

Do not store localized values in base Settings JSON, count fallback as exact coverage, localize sensitivity-fenced paths, put content in repair evidence, infer source locale, use timestamps for repair order, tie source revision to every target-only owner revision, hash target values instead of owner target-row revisions, treat aggregate target digest as a replacement for per-field CAS, weaken owner schema validation, enable AI export without explicit owner metadata, or register a provider that reaches into owner persistence directly.

## Scope

This slice changes only the persistence-free Settings Translation adapter revision encoding plus its small hashing dependencies and synchronized source evidence/handoff/verifier. It does not change migrations, owner persistence, runtime fallback, Settings command inputs, validate/apply behavior, or provider registration.
