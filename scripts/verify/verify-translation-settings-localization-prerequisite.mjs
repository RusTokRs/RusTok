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
  sourceLocaleOwner: 'crates/rustok-modules/src/static_settings_source_locale.rs',
  translationRead: 'crates/rustok-modules/src/static_settings_translation_read.rs',
  adapterCargo: 'crates/rustok-modules-translation/Cargo.toml',
  adapter: 'crates/rustok-modules-translation/src/lib.rs',
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

for (const marker of [
  'pub fn validate_localization_registry(',
  'localized field IDs must be canonical',
  'localized settings must be string leaves',
  'localized setting is fenced by sensitive path',
  'pub fn localized_value_snapshot(',
]) requireText(
  sources.settings,
  marker,
  `${paths.settings}: typed localized Settings metadata`,
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
  'pub fn localized_fields(&self)',
  'pub struct StaticSettingsLocalizationService',
  'pub async fn source_snapshot(',
  'pub async fn read_exact(',
  'pub async fn apply_exact(',
  'expected_owner_revision',
  'expected_target_revision',
  'StaticTenantLifecycleStore::claim(',
  'StaticTenantLifecycleStore::advance(',
  'module_static_localized_settings',
]) requireText(
  sources.localizedOwner,
  marker,
  `${paths.localizedOwner}: exact Settings owner contract`,
);
for (const forbidden of [
  'rustok_translation',
  'TranslationTargetProvider',
  'RuntimeLocale',
  'fallback_chain',
]) forbidText(
  sources.localizedOwner,
  forbidden,
  `${paths.localizedOwner}: owner exact apply must remain Translation/fallback independent`,
);

for (const marker of [
  'pub struct StaticSettingsSourceLocaleService',
  'pub async fn authoritative_source_snapshot(',
  'pub async fn assign_source_locale(',
  'base_projection_revision',
  "change_kind = 'base_projection'",
  'module_static_settings_source_locales',
  'module_static_settings_changes',
]) requireText(
  sources.sourceLocaleOwner,
  marker,
  `${paths.sourceLocaleOwner}: explicit source-locale provenance`,
);
for (const forbidden of [
  'RuntimeLocale',
  'fallback_chain',
  'tenant_default_locale',
  'TranslationTargetProvider',
]) forbidText(
  sources.sourceLocaleOwner,
  forbidden,
  `${paths.sourceLocaleOwner}: source locale must not be inferred`,
);

for (const marker of [
  'pub const MAX_STATIC_SETTINGS_CHANGE_PAGE_SIZE: u16 = 200;',
  'pub struct StaticSettingsChangeReadRequest',
  'pub struct StaticSettingsExactLocaleSnapshot',
  'pub struct StaticSettingsExactLocaleProgress',
  'pub struct StaticSettingsTranslationReadService',
  'pub async fn read_changes(',
  'pub async fn exact_locale_snapshot(',
  'through_seq',
  'after_seq',
  'change_seq > $3 AND change_seq <= $4',
  'ORDER BY change_seq ASC LIMIT $5',
  '.authoritative_source_snapshot(tenant_id, registry)',
  'owner_before.revision != authoritative.source.owner_revision',
  'owner_after.revision != owner_before.revision',
  'filter(|field| field.exact_target_value.is_some())',
]) requireText(
  sources.translationRead,
  marker,
  `${paths.translationRead}: bounded owner reader and exact progress`,
);
for (const forbidden of [
  'RuntimeLocale',
  'fallback_chain',
  'tenant_default_locale',
  'TranslationTargetProvider',
]) forbidText(
  sources.translationRead,
  forbidden,
  `${paths.translationRead}: read model must stay owner-local and fallback-free`,
);

for (const marker of [
  'name = "rustok-modules-translation"',
  'hex.workspace = true',
  'rustok-modules = { workspace = true, default-features = false }',
  'rustok-translation-targets.workspace = true',
  'sha2.workspace = true',
]) requireText(
  sources.adapterCargo,
  marker,
  `${paths.adapterCargo}: isolated owner Translation adapter crate with local revision hashing`,
);
for (const forbidden of ['sea-orm', 'rustok-translation.workspace']) forbidText(
  sources.adapterCargo,
  forbidden,
  `${paths.adapterCargo}: adapter must not gain persistence/engine coupling`,
);

for (const marker of [
  'pub const STATIC_SETTINGS_TRANSLATION_OWNER_SLUG: &str = "modules";',
  'pub const STATIC_SETTINGS_TRANSLATION_RESOURCE_KIND: &str = "static_settings";',
  'pub struct StaticSettingsTranslationIdentity',
  'pub struct StaticSettingsTranslationRevisions',
  'pub fn from_registry(',
  'registry.module_slug()',
  '.localized_fields()',
  'subresource_id: None',
  'pub fn module_slug_from_identity(',
  'pub fn contains_field(',
  'pub fn field_descriptors(&self)',
  'pub fn descriptor_for_field(',
  'profile: TranslationValueProfile::LocalizedScalar',
  'strategy: TranslationStrategy::Translate',
  'classification: TranslationDataClassification::TenantPrivate',
  'required: true',
  'ai_export_allowed: false',
  'max_characters: None',
  'preserves_whitespace: false',
  'const RESOURCE_REVISION_PREFIX: &str = "settings-owner-v1";',
  'const SOURCE_REVISION_PREFIX: &str = "settings-source-v1";',
  'const TARGET_REVISION_PREFIX: &str = "settings-target-v1";',
  'pub fn revisions_for_snapshot(',
  'snapshot.owner_revision == 0',
  'fields.sort_by(|left, right| left.field_id.cmp(&right.field_id))',
  'hash_part(&mut source_hasher, snapshot.source_locale.as_bytes())',
  'hash_part(&mut source_hasher, field.source_value.as_bytes())',
  'let has_exact_target = fields.iter().any(|field| field.target_revision.is_some())',
  'hash_part(&mut target_hasher, snapshot.target_locale.as_bytes())',
  'hash_part(&mut target_hasher, revision.to_string().as_bytes())',
  'None => hash_part(&mut target_hasher, b"missing")',
  'target_owner_revision > snapshot.owner_revision',
  'digest_revision(TARGET_REVISION_PREFIX, target_hasher)',
]) requireText(
  sources.adapter,
  marker,
  `${paths.adapter}: stable identity, descriptors, and opaque revision mapping`,
);
for (const forbidden of [
  'DatabaseConnection',
  'sea_orm',
  'module_static_localized_settings',
  'module_static_settings_changes',
  'TranslationTargetProvider for',
  'register_translation_target_provider',
  'RuntimeLocale',
  'fallback_chain',
]) forbidText(
  sources.adapter,
  forbidden,
  `${paths.adapter}: revision layer must stay persistence-free and unregistered`,
);

for (const marker of [
  'CREATE TABLE module_static_localized_settings',
  'PRIMARY KEY (tenant_id, module_slug, field_id, locale)',
  'owner_revision BIGINT NOT NULL CHECK (owner_revision > 0)',
  'ENABLE ROW LEVEL SECURITY',
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
  '(tenant_id, module_slug, change_seq)',
  'ENABLE ROW LEVEL SECURITY',
]) requireText(
  sources.changeMigration,
  marker,
  `${paths.changeMigration}: content-free transactional repair cursor`,
);
for (const forbidden of ['source_text', 'translated_text', 'payload_json']) forbidText(
  sources.changeMigration,
  forbidden,
  `${paths.changeMigration}: change evidence must remain content-free`,
);
for (const marker of [
  'CREATE TABLE module_static_settings_source_locales',
  'base_projection_revision BIGINT NOT NULL CHECK (base_projection_revision > 0)',
  'PRIMARY KEY (tenant_id, module_slug)',
  'ENABLE ROW LEVEL SECURITY',
]) requireText(
  sources.sourceLocaleMigration,
  marker,
  `${paths.sourceLocaleMigration}: source-locale provenance storage`,
);
for (const marker of [
  'mod m20260904_000051_static_localized_settings;',
  'mod m20260904_000052_static_settings_change_cursor;',
  'mod m20260904_000053_static_settings_source_locale;',
]) requireText(
  sources.migrationRegistry,
  marker,
  `${paths.migrationRegistry}: Settings migration registration`,
);

if (evidence.schema_version !== 8) {
  failures.push(`${paths.evidence}: schema_version must be 8`);
}
if (evidence.status !== 'provider_revision_source_ready') {
  failures.push(`${paths.evidence}: status must be provider_revision_source_ready`);
}

for (const [key, expected] of Object.entries({
  stable_field_ids_present: true,
  sensitive_path_fences_present: true,
  parallel_exact_locale_storage_present: true,
  localized_row_revision_cas_present: true,
  localized_apply_shares_static_owner_revision_present: true,
  localized_apply_idempotency_receipt_present: true,
  canonical_tenant_locale_enforced: true,
  bounded_monotonic_change_sequence_present: true,
  authoritative_source_locale_policy_present: true,
  bounded_owner_change_reader_present: true,
  bounded_change_reader_freezes_high_watermark: true,
  exact_locale_owner_snapshot_present: true,
  exact_locale_snapshot_stability_guard_present: true,
  exact_locale_progress_counts_exact_rows_only: true,
  exact_locale_progress_ignores_runtime_fallback: true,
  provider_resource_identity_mapping_present: true,
  provider_field_identity_mapping_present: true,
  provider_identity_reverse_mapping_fail_closed: true,
  provider_identity_adapter_has_no_persistence_access: true,
  provider_field_descriptor_mapping_present: true,
  provider_present_source_units_required: true,
  provider_descriptor_tenant_private_classification: true,
  provider_descriptor_ai_export_default_denied: true,
  provider_descriptor_owner_validation_remains_authoritative: true,
  provider_resource_revision_maps_shared_owner_clock: true,
  provider_source_revision_digest_present: true,
  provider_source_revision_stable_across_target_only_writes: true,
  provider_target_revision_digest_present: true,
  provider_target_revision_none_without_exact_rows: true,
  provider_target_revision_uses_per_field_revisions: true,
  provider_target_revision_digest_is_order_independent: true,
  provider_revision_mapping_rejects_inconsistent_snapshot: true,
  settings_translation_provider_registered: false,
})) {
  if (evidence.source_facts?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_facts.${key} must be ${expected}`);
  }
}

for (const key of [
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
  provider_identity_source_proven: true,
  provider_descriptor_source_proven: true,
  provider_revision_source_proven: true,
  runtime_database_execution_proven: false,
  translation_provider_proven: false,
})) {
  if (evidence.validation?.[key] !== expected) {
    failures.push(`${paths.evidence}: validation.${key} must be ${expected}`);
  }
}

for (const marker of [
  'opaque revision mapping source-ready',
  '`StaticSettingsTranslationIdentity::revisions_for_snapshot`',
  '`resource_revision` is `settings-owner-v1:<owner_revision>`',
  'target-only localized write therefore advances `resource_revision` but leaves `source_revision` unchanged',
  '`target_revision` is `None` while none of the current source fields has an exact target row',
  'either its positive owner target-row revision or an explicit `missing` marker',
  'does **not** replace the per-field revisions',
  'Fields are sorted before digesting',
  'target digest as a substitute for row CAS',
]) requireText(
  sources.handoff,
  marker,
  `${paths.handoff}: Settings revision handoff`,
);

if (failures.length > 0) {
  console.error('Translation Settings localization prerequisite verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Settings owner prerequisites, stable identities/descriptors, and opaque revision mapping are source-ready; validate/apply mapping and registration remain open',
);
