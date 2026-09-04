---
id: doc://crates/rustok-translation/docs/translation-settings-localization-prerequisite.md
kind: implementation_handoff
language: en
status: in_progress
last_reviewed: 2026-09-04
---

# Translation Settings localization prerequisite

Status: **owner persistence/read/source/progress, stable identity/descriptors/revisions, neutral validate/apply command mapping source-ready, and authoritative package registry resolution source-ready / runtime provider registration open**

Base reviewed before this slice: `main@66e9eedded8910c884328c0a4132d8944ac0c650`.

## Existing owner foundation

The Settings owner boundary remains layered outside Translation:

- #3825 typed stable localized field IDs, string-leaf eligibility and sensitivity fences;
- #3831 parallel exact-locale storage, exact reads, per-field target CAS, shared owner CAS, and replay-safe exact apply;
- #3832 content-free `change_seq` repair evidence;
- #3833 explicit source-locale provenance bound to the latest `base_projection` revision;
- #3834 bounded owner change reads plus stable exact-locale snapshot/progress facts;
- #3835 stable neutral resource and field identities;
- #3836 conservative field descriptors;
- #3837 opaque resource/source/target revision mapping;
- the immediately preceding slice added neutral patch validation plus deterministic owner CAS/idempotency apply planning.

Language-neutral Settings stay in `tenant_modules.settings`. Localized copy, source-locale provenance, repair evidence, exact progress, owner revisions, and row target revisions remain owner data. Runtime fallback is not exact coverage.

## Previously proven: neutral validate/apply mapping

`StaticSettingsTranslationIdentity::validate_patch_against_snapshot` validates one neutral `TranslationPatchRequest` against a stable `StaticSettingsExactLocaleSnapshot` without performing a write. It checks resource identity, source/target locales, opaque revisions, owner-admitted fields, and per-field source hashes.

`StaticSettingsTranslationIdentity::prepare_apply_plan` converts an accepted neutral patch into deterministic `StaticLocalizedSettingApplyCommand` values. Patch fields are sorted by stable `FieldKey`; the first command uses the stable snapshot owner revision; each following command expects the previous command to have advanced the owner revision by exactly one; and every command retains that field's current target-row CAS revision.

One provider patch may update multiple Settings fields, but owner `apply_exact` stores one durable receipt per field payload. The mapper therefore derives a stable per-step idempotency UUID. Replay of the same prepared operation reuses the same step IDs while different fields in that operation cannot collide.

The adapter still **does not** execute the prepared commands and still contains no Settings persistence access.

## What this slice adds: authoritative package registry resolution

Runtime registration needs a `StaticSettingsLocalizationRegistry` for an arbitrary static module slug. That registry cannot be hardcoded in Translation, and Translation must not parse another owner's `rustok-module.toml` or reach into Settings persistence.

The server now exposes `resolve_static_settings_localization_registry(module_slug)` from `static_settings_localization_registry`.

The host resolver:

1. asks `ManifestManager::module_settings_schema` for the resolved static Settings schema;
2. materializes the existing owner-owned `StaticModulePackageContract` with that schema, preserving the established package/schema boundary;
3. resolves the installed/builtin static package location inside the server manifest boundary;
4. reads only the optional `[settings_localization]` package metadata slice;
5. passes the package contract's owner-typed schema plus `localized_fields` and `sensitive_paths` into `StaticSettingsLocalizationRegistry::new`;
6. returns the fully validated authoritative registry.

Keeping `localized_fields` and `sensitive_paths` adjacent to, rather than embedded in, `ModuleSettingSpec` preserves the source-compatibility decision from #3825. The owner registry constructor remains the validation authority for stable field IDs, module slug validity, string-leaf eligibility, schema paths, duplicate claims, and sensitivity fences.

A package can declare metadata in the backward-compatible form:

```toml
[settings_localization]
localized_fields = { "checkout.title" = "title" }
sensitive_paths = ["secret"]
```

The existing full package deserializer ignores unknown fields, so this metadata slice does not widen the legacy `ModulePackageManifest` or `ModuleSettingSpec` Rust shapes. Translation never sees the package path or TOML parser.

For a valid module with no localization metadata, the host returns an empty owner registry, matching the existing empty-schema/non-localized behavior. Invalid slugs, unknown metadata paths, non-string localized leaves, duplicate paths, and sensitivity-fenced localized paths fail closed in the owner registry constructor.

Focused tests prove both a valid package metadata slice and rejection when a declared localized path is sensitivity-fenced.

## What is still not proven

This slice does not register or execute a `TranslationTargetProvider`. It only makes the authoritative registry available to runtime composition without violating ownership boundaries.

The future runtime provider must still expose list/read/progress/change/apply behavior through existing owner services, perform replay-safe multi-field execution, and register through the neutral target registry. It must not add direct Settings SQL or package-manifest parsing to Translation.

## Remaining provider work

Only the runtime registration/execution slice remains:

1. implement the Settings `TranslationTargetProvider` using `resolve_static_settings_localization_registry` plus the already-proven owner read/source/progress contracts;
2. map list/read/progress/change calls without runtime fallback or direct owner-table access;
3. execute the proven deterministic apply plan with replay-safe orchestration around per-field owner receipts;
4. register the provider through the neutral target registry only after those runtime mappings are source-proven.

## Forbidden shortcuts

Do not store localized values in base Settings JSON, count fallback as exact coverage, localize sensitivity-fenced paths, read Settings tables directly from the Translation adapter, parse `rustok-module.toml` directly from Translation, bypass shared owner CAS, replace per-field target CAS with the aggregate digest, reuse one owner idempotency key for multiple field payloads, apply fields in caller-supplied nondeterministic order, prepare commands for a mismatched tenant context, weaken owner schema validation, or treat package registry resolution as runtime provider execution evidence.

## Scope

This slice adds only the server-owned `[settings_localization]` metadata resolver, the owner package/schema handoff into `StaticSettingsLocalizationRegistry`, focused unit coverage, and synchronized source evidence/handoff. It does not change existing package structs, migrations, Settings persistence, fallback, Translation provider execution, or provider registration.
