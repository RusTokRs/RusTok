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
  changeMigration:
    'crates/rustok-modules/src/migrations/m20260904_000052_static_settings_change_cursor.rs',
  migrationRegistry: 'crates/rustok-modules/src/migrations/mod.rs',
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
  `${paths.centralPlan}: broad Settings onboarding remains open until source-locale/provider work`,
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
  `${paths.lifecycle}: static owner transaction/revision prerequisite`,
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
  'CREATE TABLE module_static_settings_changes',
  'change_seq BIGSERIAL PRIMARY KEY',
  "change_kind TEXT NOT NULL CHECK (change_kind IN ('base_projection', 'localized_target'))",
  'UNIQUE (tenant_id, module_slug, owner_revision)',
  'idx_module_static_settings_changes_scope',
  '(tenant_id, module_slug, change_seq)',
  'ENABLE ROW LEVEL SECURITY',
  'module_static_settings_changes_scope',
  'rustok_log_static_settings_base_projection',
  'AFTER UPDATE OF settings ON tenant_modules',
  'lifecycle.revision + 1',
  'lifecycle.active_idempotency_key IS NOT NULL',
  'rustok_log_static_settings_localized_target',
  'AFTER UPDATE OF value, revision, owner_revision ON module_static_localized_settings',
  'NEW.owner_revision',
  'NEW.revision',
  'INSERT OR IGNORE INTO module_static_settings_changes',
]) requireText(
  sources.changeMigration,
  marker,
  `${paths.changeMigration}: content-free transactional repair cursor`,
);

for (const forbidden of [
  'NEW.value,',
  'OLD.value,',
  'source_text',
  'translated_text',
  'payload_json',
]) forbidText(
  sources.changeMigration,
  forbidden,
  `${paths.changeMigration}: change evidence must remain content-free`,
);

for (const marker of [
  'mod m20260904_000051_static_localized_settings;',
  'Box::new(m20260904_000051_static_localized_settings::Migration)',
  'mod m20260904_000052_static_settings_change_cursor;',
  'Box::new(m20260904_000052_static_settings_change_cursor::Migration)',
]) requireText(
  sources.migrationRegistry,
  marker,
  `${paths.migrationRegistry}: Settings migration registration`,
);

if (evidence.schema_version !== 3) {
  failures.push(`${paths.evidence}: schema_version must be 3`);
}
if (evidence.status !== 'owner_change_cursor_source_ready') {
  failures.push(`${paths.evidence}: status must be owner_change_cursor_source_ready`);
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
  settings_translation_change_cursor_or_event_present: true,
  content_free_transactional_change_evidence_present: true,
  base_projection_change_evidence_present: true,
  localized_target_change_evidence_present: true,
  bounded_monotonic_change_sequence_present: true,
  settings_translation_provider_registered: false,
  authoritative_source_locale_policy_present: false,
})) {
  if (evidence.source_facts?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_facts.${key} must be ${expected}`);
  }
}

for (const key of [
  'authoritative_source_locale_policy',
  'translation_provider_registration_after_source_locale_policy',
  'provider_exact_locale_progress',
  'provider_bounded_change_reader',
]) {
  if (evidence.remaining_owner_contract?.[key] !== true) {
    failures.push(`${paths.evidence}: remaining_owner_contract.${key} must be true`);
  }
}

for (const [key, expected] of Object.entries({
  localized_storage_source_proven: true,
  localized_owner_apply_source_proven: true,
  change_cursor_source_proven: true,
  runtime_database_execution_proven: false,
  translation_provider_proven: false,
})) {
  if (evidence.validation?.[key] !== expected) {
    failures.push(`${paths.evidence}: validation.${key} must be ${expected}`);
  }
}

for (const marker of [
  'transactional repair cursor source-ready',
  '`module_static_settings_changes`',
  '`change_seq` is an append-only database sequence',
  '`base_projection` rows invalidate the Settings source projection',
  '`localized_target` rows identify only stable field ID',
  'same database transaction as the owner write',
  'initial static override materialization may conservatively emit `base_projection` evidence',
  'define the authoritative source-locale policy',
  'bounded owner change reader over `change_seq`',
]) requireText(
  sources.handoff,
  marker,
  `${paths.handoff}: Settings repair-cursor handoff`,
);

if (failures.length > 0) {
  console.error('Translation Settings localization prerequisite verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Settings exact-locale owner storage/apply and content-free transactional repair cursor are source-ready; source-locale policy and provider onboarding remain open',
);
