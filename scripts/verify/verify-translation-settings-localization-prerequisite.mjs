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
  modulesLib: 'crates/rustok-modules/src/lib.rs',
  settings: 'crates/rustok-modules/src/settings.rs',
  lifecycle: 'crates/rustok-modules/src/lifecycle_writer.rs',
  localizedOwner: 'crates/rustok-modules/src/static_settings_localization.rs',
  sourceLocaleOwner: 'crates/rustok-modules/src/static_settings_source_locale.rs',
  translationRead: 'crates/rustok-modules/src/static_settings_translation_read.rs',
  localizedMigration:
    'crates/rustok-modules/src/migrations/m20260904_000051_static_localized_settings.rs',
  changeMigration:
    'crates/rustok-modules/src/migrations/m20260904_000052_static_settings_change_cursor.rs',
  sourceLocaleMigration:
    'crates/rustok-modules/src/migrations/m20260904_000053_static_settings_source_locale.rs',
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
  `${paths.centralPlan}: broad Settings provider onboarding remains open`,
);

for (const marker of [
  'Remaining ownership drift, settings, additional',
  'provider onboarding',
]) requireText(
  sources.localPlan,
  marker,
  `${paths.localPlan}: broad Settings completion must remain open`,
);

requireText(
  sources.modulesLib,
  'pub mod static_settings_translation_read;',
  `${paths.modulesLib}: Settings Translation read model must be public owner API`,
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
  `${paths.localizedOwner}: exact owner must not become a Translation/fallback adapter`,
);

for (const marker of [
  'pub struct StaticSettingsSourceLocaleRecord',
  'pub struct StaticSettingsSourceLocaleAssignCommand',
  'pub struct StaticSettingsAuthoritativeSourceSnapshot',
  'pub struct StaticSettingsSourceLocaleService',
  'pub async fn read_source_locale(',
  'pub async fn authoritative_source_snapshot(',
  'pub async fn assign_source_locale(',
  'TenantLocale::new(locale)',
  'base_projection_revision',
  'load_latest_base_projection_revision(',
  "change_kind = 'base_projection'",
  'StaticTenantLifecycleStore::claim(',
  'StaticTenantLifecycleStore::advance(',
  'idempotency::admit(',
  'idempotency::complete(',
  'module_static_settings_source_locales',
  'module_static_settings_changes',
]) requireText(
  sources.sourceLocaleOwner,
  marker,
  `${paths.sourceLocaleOwner}: explicit source-locale owner contract`,
);

for (const forbidden of [
  'RuntimeLocale',
  'fallback_chain',
  'tenant_default_locale',
  'TranslationTarget',
  'TranslationProvider',
]) forbidText(
  sources.sourceLocaleOwner,
  forbidden,
  `${paths.sourceLocaleOwner}: source locale must not be inferred or become a provider adapter`,
);

for (const marker of [
  'pub const MAX_STATIC_SETTINGS_CHANGE_PAGE_SIZE: u16 = 200;',
  'pub struct StaticSettingsChangeReadRequest',
  'pub struct StaticSettingsChangePage',
  'pub struct StaticSettingsExactLocaleSnapshot',
  'pub struct StaticSettingsExactLocaleProgress',
  'pub struct StaticSettingsTranslationReadService',
  'pub async fn read_changes(',
  'pub async fn exact_locale_snapshot(',
  'through_seq',
  'after_seq',
  'load_high_watermark(',
  'change_seq > $3 AND change_seq <= $4',
  'ORDER BY change_seq ASC LIMIT $5',
  'StaticSettingsSourceLocaleService::new(self.db.clone())',
  '.authoritative_source_snapshot(tenant_id, registry)',
  'StaticTenantLifecycleStore::snapshot(',
  'owner_before.revision != authoritative.source.owner_revision',
  'owner_after.revision != owner_before.revision',
  'module_static_localized_settings',
  'exact_target_value: exact.map(|row| row.value.clone())',
  'filter(|field| field.exact_target_value.is_some())',
]) requireText(
  sources.translationRead,
  marker,
  `${paths.translationRead}: bounded owner reader and exact-locale progress`,
);

for (const forbidden of [
  'RuntimeLocale',
  'fallback_chain',
  'tenant_default_locale',
  'rustok_translation::',
  'TranslationTargetProvider',
]) forbidText(
  sources.translationRead,
  forbidden,
  `${paths.translationRead}: read model must stay owner-local and fallback-free`,
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
  'CREATE TABLE module_static_settings_source_locales',
  'locale TEXT NOT NULL CHECK (length(locale) BETWEEN 2 AND 32)',
  'base_projection_revision BIGINT NOT NULL CHECK (base_projection_revision > 0)',
  'PRIMARY KEY (tenant_id, module_slug)',
  'ENABLE ROW LEVEL SECURITY',
  'module_static_settings_source_locales_scope',
]) requireText(
  sources.sourceLocaleMigration,
  marker,
  `${paths.sourceLocaleMigration}: source-locale provenance storage`,
);

for (const marker of [
  'mod m20260904_000051_static_localized_settings;',
  'Box::new(m20260904_000051_static_localized_settings::Migration)',
  'mod m20260904_000052_static_settings_change_cursor;',
  'Box::new(m20260904_000052_static_settings_change_cursor::Migration)',
  'mod m20260904_000053_static_settings_source_locale;',
  'Box::new(m20260904_000053_static_settings_source_locale::Migration)',
]) requireText(
  sources.migrationRegistry,
  marker,
  `${paths.migrationRegistry}: Settings migration registration`,
);

if (evidence.schema_version !== 5) {
  failures.push(`${paths.evidence}: schema_version must be 5`);
}
if (evidence.status !== 'owner_reader_progress_source_ready') {
  failures.push(`${paths.evidence}: status must be owner_reader_progress_source_ready`);
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
  authoritative_source_locale_policy_present: true,
  explicit_source_locale_assignment_present: true,
  source_locale_bound_to_latest_base_projection_present: true,
  target_only_writes_preserve_source_locale_provenance: true,
  base_settings_writes_invalidate_source_locale_provenance: true,
  legacy_without_source_locale_provenance_fails_closed: true,
  source_locale_assignment_emits_repair_evidence: true,
  bounded_owner_change_reader_present: true,
  bounded_change_reader_freezes_high_watermark: true,
  bounded_change_reader_uses_keyset_cursor: true,
  exact_locale_owner_snapshot_present: true,
  exact_locale_snapshot_rejects_source_target_equality: true,
  exact_locale_snapshot_stability_guard_present: true,
  exact_locale_progress_counts_exact_rows_only: true,
  exact_locale_progress_ignores_runtime_fallback: true,
  settings_translation_provider_registered: false,
})) {
  if (evidence.source_facts?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_facts.${key} must be ${expected}`);
  }
}

for (const key of [
  'provider_identity_mapping',
  'provider_validate_apply_adapter',
  'translation_provider_registration',
]) {
  if (evidence.remaining_owner_contract?.[key] !== true) {
    failures.push(`${paths.evidence}: remaining_owner_contract.${key} must be true`);
  }
}

for (const [key, expected] of Object.entries({
  localized_storage_source_proven: true,
  localized_owner_apply_source_proven: true,
  change_cursor_source_proven: true,
  source_locale_owner_source_proven: true,
  bounded_reader_source_proven: true,
  exact_progress_source_proven: true,
  runtime_database_execution_proven: false,
  translation_provider_proven: false,
})) {
  if (evidence.validation?.[key] !== expected) {
    failures.push(`${paths.evidence}: validation.${key} must be ${expected}`);
  }
}

for (const marker of [
  'bounded owner reader, and exact progress source-ready',
  '`StaticSettingsTranslationReadService::read_changes`',
  '`through_seq` high-water mark',
  'cannot extend an in-progress repair scan',
  '`StaticSettingsTranslationReadService::exact_locale_snapshot`',
  'exact target rows for one canonical target locale',
  'checked before and after the exact-target read',
  '`progress()` counts coverage only when an exact target row exists',
  'Rendered fallback, tenant defaults and negotiated runtime locales are never consulted',
  'provider identity-apply-registration open',
]) requireText(
  sources.handoff,
  marker,
  `${paths.handoff}: Settings bounded-reader/progress handoff`,
);

if (failures.length > 0) {
  console.error('Translation Settings localization prerequisite verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Settings owner storage/apply, repair cursor, source-locale provenance, bounded reader, and exact progress are source-ready; provider identity/apply/registration remain open',
);
