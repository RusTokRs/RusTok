---
id: doc://crates/rustok-translation/docs/translation-settings-localization-prerequisite.md
kind: implementation_handoff
language: en
status: in_progress
last_reviewed: 2026-09-04
---

# Translation Settings localization prerequisite

Status: **exact-locale owner storage/apply, transactional repair cursor, explicit source-locale provenance, bounded owner reader, and exact progress source-ready / provider identity-apply-registration open**

Base reviewed before this slice: `main@4b8da7e78fc6c4c4db7c5921b3007fee94e979ef`.

## Existing owner foundation

The Settings owner boundary remains layered outside Translation:

- #3825 typed stable localized field IDs, string-leaf eligibility and sensitivity fences;
- #3831 parallel exact-locale storage in `module_static_localized_settings`, exact reads, target-row CAS, shared `StaticTenantLifecycleStore` owner CAS, and replay-safe owner apply;
- #3832 content-free transactional repair evidence in `module_static_settings_changes`, ordered by durable `change_seq`;
- #3833 explicit canonical source-locale provenance in `module_static_settings_source_locales`, bound to the latest `base_projection` revision rather than every shared owner revision.

Language-neutral Settings remain in `tenant_modules.settings`. Localized copy, repair evidence and locale provenance remain owner data. Runtime fallback is not exact coverage.

## What this slice adds: bounded owner reader

`StaticSettingsTranslationReadService::read_changes` exposes the repair journal without exposing its table to Translation.

The reader is intentionally bounded and keyset-based:

- `limit` is constrained to `1..=200`;
- `after_seq` is an exclusive keyset cursor over durable `change_seq`;
- the first page captures the current inclusive `through_seq` high-water mark;
- every continuation reuses that exact `through_seq`;
- rows committed after the high-water mark cannot extend an in-progress repair scan;
- future/invalid upper bounds fail closed;
- records preserve only content-free change identity, owner revision and optional exact-target revision;
- stored localized change locales are revalidated as canonical `TenantLocale` data before being returned.

This gives the eventual Translation adapter a finite owner repair window without direct SQL, timestamp ordering, or an ever-moving tail.

## Exact-locale snapshot and progress

`StaticSettingsTranslationReadService::exact_locale_snapshot` combines:

1. `StaticSettingsSourceLocaleService::authoritative_source_snapshot` for deterministic source copy plus explicit source-locale provenance;
2. the exact target rows for one canonical target locale;
3. the current owner change-sequence high-water mark.

The shared static owner revision is checked before and after the exact-target read. An active mutation, source/target race, or revision movement fails closed instead of returning mixed facts.

The snapshot includes only owner-declared localized fields that currently contain source copy. Missing optional source leaves therefore do not become phantom Translation work units. `progress()` counts coverage only when an exact target row exists for that field and target locale. Rendered fallback, tenant defaults and negotiated runtime locales are never consulted.

The owner progress contract exposes `source_units`, `exact_units`, `missing_units`, `complete`, and the owner `change_seq` high-water mark. Translation can later map these facts into its neutral progress contract without reading Settings persistence directly.

## Why provider registration still remains open

The persistence/read prerequisites are now source-ready, but the adapter still needs three bounded pieces before registration:

1. map one static module Settings resource plus stable field IDs into `rustok-translation-targets` identities and descriptors;
2. map neutral validate/apply requests onto the existing owner exact-read/apply services, preserving source/target/owner CAS and idempotency;
3. register the provider only after those identity and mutation mappings are proven.

The provider must consume the owner contracts. It must not infer source locale, read owner tables directly, treat fallback as exact coverage, or bypass owner CAS/idempotency.

## Forbidden shortcuts

Do not store localized values in base Settings JSON, count rendered fallback as exact coverage, localize sensitivity-fenced paths, include content in the change journal, infer repair order from timestamps, infer authoring locale from tenant defaults, bind source locale to every shared-owner revision, allow new writes to extend a captured repair window, or register a provider that reaches into owner persistence directly.

## Scope

This slice adds only the public Settings Translation owner read model plus synchronized source evidence. It does not register a Translation provider, define runtime fallback, change existing Settings command inputs, change migrations, touch artifact Settings persistence, or overlap Forum UGC onboarding.
