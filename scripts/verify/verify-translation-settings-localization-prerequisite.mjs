#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');
const failures = [];

const paths = {
  centralPlan: 'docs/modules/translation-implementation-plan.md',
  localPlan: 'crates/rustok-translation/docs/implementation-plan.md',
  settings: 'crates/rustok-modules/src/settings.rs',
  lifecycle: 'crates/rustok-modules/src/lifecycle_writer.rs',
  localizedOwner: 'crates/rustok-modules/src/static_settings_localization.rs',
  localizedMigration:
    'crates/rustok-modules/src/migrations/m20260904_000051_static_localized_settings.rs',
  migrationRegistry: 'crates/rustok-modules/src/migrations/mod.rs',
  modulesLib: 'crates/rustok-modules/src/lib.rs',
  evidence:
    'crates/rustok-translation/contracts/evidence/translation-settings-localization-prerequisite-source.json',
  handoff:
    'crates/rustok-translation/docs/translation-settings-localization-prerequisite.md',
};

const sources = Object.fromEntries(
  Object.entries(paths).map(([key, relativePath]) => [key, read(relativePath)]),
);
const evidence = JSON.parse(sources.evidence);

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const marker of [
  '| Type localized settings |',
  'parallel localized rows, CAS, events, and secret-safe validation',
]) requireText(
  sources.centralPlan,
  marker,
  `${paths.centralPlan}: Settings P0 gate remains open until change evidence/provider work`,
);

for (const marker of [
  'Remaining ownership drift, settings, additional',
  'provider onboarding',
]) requireText(
  sources.localPlan,
  marker,
  `${paths.localPlan}: broad Settings completion must remain open`,
);

for (const marker of [
  'pub fn validate_localization_registry(',
  'localized field IDs must be canonical',
  'localized settings must be string leaves',
  'localized setting is fenced by sensitive path',
  'pub fn localized_value_snapshot(',
]) requireText(
  sources.settings,
  marker,
  `${paths.settings}: typed owner localization metadata`,
);

for (const marker of [
  'pub async fn update_static_normalized_settings(',
  'idempotency::admit(',
  'StaticTenantLifecycleStore::claim(',
  'StaticTenantLifecycleStore::advance(',
  'idempotency::complete(',
]) requireText(
  sources.lifecycle,
  marker,
  `${paths.lifecycle}: pre-existing static owner CAS/idempotency must stay explicit`,
);

for (const marker of [
  'pub struct StaticSettingsLocalizationRegistry',
  'pub struct StaticSettingsLocalizationService',
  'pub async fn source_snapshot(',
  'pub async fn read_exact(',
  'pub async fn apply_exact(',
  'TenantLocale::new(locale)',
  'expected_owner_revision',
  'expected_target_revision',
  'StaticTenantLifecycleStore::claim(',
  'StaticTenantLifecycleStore::advance(',
  'idempotency::admit(',
  'idempotency::complete(',
  'module_static_localized_settings',
]) requireText(
  sources.localizedOwner,
  marker,
  `${paths.localizedOwner}: exact Settings owner contract`,
);

for (const forbidden of [
  'rustok_translation',
  'TranslationTarget',
  'TranslationProvider',
  'RuntimeLocale',
  'fallback_chain',
]) forbidText(
  sources.localizedOwner,
  forbidden,
  `${paths.localizedOwner}: owner store must not become a Translation/fallback adapter`,
);

for (const marker of [
  'CREATE TABLE module_static_localized_settings',
  'PRIMARY KEY (tenant_id, module_slug, field_id, locale)',
  'revision BIGINT NOT NULL CHECK (revision > 0)',
  'owner_revision BIGINT NOT NULL CHECK (owner_revision > 0)',
  'ENABLE ROW LEVEL SECURITY',
  'module_static_localized_settings_scope',
]) requireText(
  sources.localizedMigration,
  marker,
  `${paths.localizedMigration}: tenant-scoped exact-locale storage`,
);

for (const marker of [
  'mod m20260904_000051_static_localized_settings;',
  'Box::new(m20260904_000051_static_localized_settings::Migration)',
]) requireText(
  sources.migrationRegistry,
  marker,
  `${paths.migrationRegistry}: migration registration`,
);

for (const marker of [
  'mod static_settings_localization;',
  'StaticSettingsLocalizationRegistry',
  'StaticSettingsLocalizationService',
]) requireText(
  sources.modulesLib,
  marker,
  `${paths.modulesLib}: owner API exposure`,
);

if (evidence.schema_version !== 2) {
  failures.push(`${paths.evidence}: schema_version must be 2`);
}
if (evidence.status !== 'owner_exact_locale_source_ready') {
  failures.push(`${paths.evidence}: status must be owner_exact_locale_source_ready`);
}

for (const [key, expected] of Object.entries({
  localized_field_registry_present: true,
  stable_field_ids_present: true,
  localized_string_leaf_validation_present: true,
  sensitive_path_fences_present: true,
  deterministic_source_value_snapshot_present: true,
  static_owner_lifecycle_revision_cas_present: true,
  static_owner_settings_idempotency_receipt_present: true,
  parallel_exact_locale_storage_present: true,
  localized_row_revision_cas_present: true,
  localized_exact_read_without_fallback_present: true,
  localized_apply_shares_static_owner_revision_present: true,
  localized_apply_idempotency_receipt_present: true,
  canonical_tenant_locale_enforced: true,
  settings_translation_change_cursor_or_event_present: false,
  settings_translation_provider_registered: false,
  authoritative_source_locale_policy_present: false,
})) {
  if (evidence.source_facts?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_facts.${key} must be ${expected}`);
  }
}

for (const key of [
  'content_free_transactional_change_evidence',
  'authoritative_source_locale_policy',
  'translation_provider_registration_after_change_evidence',
  'provider_exact_locale_progress',
]) {
  if (evidence.remaining_owner_contract?.[key] !== true) {
    failures.push(`${paths.evidence}: remaining_owner_contract.${key} must be true`);
  }
}

for (const [key, expected] of Object.entries({
  localized_storage_source_proven: true,
  localized_owner_apply_source_proven: true,
  runtime_database_execution_proven: false,
  translation_provider_proven: false,
})) {
  if (evidence.validation?.[key] !== expected) {
    failures.push(`${paths.evidence}: validation.${key} must be ${expected}`);
  }
}

for (const marker of [
  'Correction to the previous prerequisite',
  'already uses the shared `StaticTenantLifecycleStore` expected-revision CAS',
  '`module_static_localized_settings` stores localized copy outside',
  '`read_exact` requires canonical `TenantLocale` and never performs runtime fallback',
  '`apply_exact` requires both expected static owner revision and expected target-row revision',
  'define transactional content-free Settings localization change evidence',
  'define the authoritative source-locale policy',
]) requireText(
  sources.handoff,
  marker,
  `${paths.handoff}: corrected and advanced Settings handoff`,
);

if (failures.length > 0) {
  console.error('Translation Settings localization prerequisite verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Settings exact-locale owner storage/read/apply source contract is present; change evidence, source-locale policy and provider onboarding remain open',
);
