---
id: doc://crates/rustok-translation/docs/translation-settings-localization-prerequisite.md
kind: implementation_handoff
language: en
status: in_progress
last_reviewed: 2026-09-04
---

# Translation Settings localization prerequisite

Status: **exact-locale owner storage/apply, transactional repair cursor, and explicit source-locale provenance source-ready / provider reader-progress-registration open**

Base reviewed before this slice: `main@3d06cc4b4fa79fe7135ee4e7e02f9a6de5eb1074`.

## Existing owner foundation

The current Settings owner boundary is intentionally layered rather than implemented inside Translation:

- #3825 typed stable localized field IDs, string-leaf eligibility and sensitivity fences;
- #3831 added parallel exact-locale storage in `module_static_localized_settings`, exact reads, target-row CAS, shared `StaticTenantLifecycleStore` owner CAS, and replay-safe owner apply;
- #3832 added content-free transactional repair evidence in `module_static_settings_changes`, ordered by durable `change_seq` with bounded `(tenant_id, module_slug, change_seq)` keyset access.

Language-neutral Settings remain in `tenant_modules.settings`. Localized copy, repair evidence and locale provenance are separate owner data. Runtime fallback is not exact coverage.

## What this slice adds: authoritative source locale

Static Settings now have an explicit owner contract for authoring-locale provenance in `module_static_settings_source_locales`.

`StaticSettingsSourceLocaleService::assign_source_locale`:

- accepts only exact canonical `TenantLocale`; `und`, underscore aliases and non-canonical spellings fail closed;
- uses the same tenant/module `StaticTenantLifecycleStore` claim/advance/release CAS as base Settings and localized apply;
- is replay-safe through the durable owner operation-receipt ledger;
- writes only tenant/module/locale plus a `base_projection_revision`; no Settings copy is duplicated into the provenance row;
- records a content-free `base_projection` repair change at the next owner revision in the same transaction;
- advances and releases the shared owner revision only after provenance and repair evidence are durable in that transaction.

The source locale is **not** bound to the current shared owner revision. That would be wrong because a localized target-only apply also advances the shared owner clock even though source copy did not change.

Instead, provenance is bound to the latest `base_projection` change revision:

- source-locale assignment itself emits `base_projection`, so policy changes are visible to repair readers;
- a target-only localized apply emits `localized_target` and therefore preserves the source-locale assignment;
- a later base Settings write emits a newer `base_projection`, which makes the older locale assignment stale;
- reassigning/reaffirming the source locale creates the next `base_projection` revision and makes the source projection authoritative again.

This gives base source copy its own durable provenance clock without inventing a second mutable Settings aggregate.

## Authoritative source reads

`StaticSettingsSourceLocaleService::authoritative_source_snapshot` combines the existing deterministic localized-field source snapshot with explicit locale provenance and fails closed unless all of the following are true:

1. the static owner has no active mutation claim while the snapshot is verified;
2. the deterministic source snapshot still matches the current shared owner revision;
3. a source-locale provenance row exists;
4. the stored locale is canonical `TenantLocale` data;
5. its `base_projection_revision` equals the latest content-free `base_projection` change revision;
6. the shared owner revision remains stable during verification.

Legacy Settings with no provenance are therefore not silently assigned the current tenant default. A later tenant-default or runtime-fallback change cannot reinterpret existing Settings copy as a different authoring source.

## Why provider registration still remains open

The owner prerequisites are now source-ready, but the Translation adapter still needs its transport-neutral reader/progress layer:

1. expose a bounded owner change reader over `module_static_settings_changes.change_seq` without exposing the physical table;
2. map Settings module/field inventory and authoritative source snapshots into `rustok-translation-targets` identities;
3. prove exact-locale progress counts without treating rendered fallback as coverage;
4. validate/apply through the owner services and only then register the Settings Translation provider.

The provider must consume the owner contracts; it must not infer source locale, write owner tables directly, or bypass owner CAS/idempotency.

## Forbidden shortcuts

Do not store localized values in the base Settings JSON, count rendered fallback as exact coverage, localize secret/sensitivity-fenced paths, include source/target text in change evidence, infer repair ordering from timestamps, infer authoring locale from the current tenant default, bind source locale to every shared-owner revision, or register the provider before bounded reader/progress semantics exist.

## Scope

This slice adds only source-locale provenance persistence, the explicit owner assignment/read contract, migration wiring, and synchronized Translation source evidence. It does not register a Translation provider, change runtime fallback, change existing Settings command inputs, touch artifact Settings persistence, or overlap Forum UGC onboarding.
