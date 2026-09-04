#!/usr/bin/env node

import { readFileSync, readdirSync } from 'node:fs';
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
  migrations: 'crates/rustok-modules/src/migrations/',
  evidence:
    'crates/rustok-translation/contracts/evidence/translation-settings-localization-prerequisite-source.json',
  handoff:
    'crates/rustok-translation/docs/translation-settings-localization-prerequisite.md',
};

const centralPlan = read(paths.centralPlan);
const localPlan = read(paths.localPlan);
const settings = read(paths.settings);
const evidence = JSON.parse(read(paths.evidence));
const handoff = read(paths.handoff);
const migrationNames = readdirSync(new URL(paths.migrations, root));

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const marker of [
  '| Type localized settings |',
  'parallel localized rows, CAS, events, and secret-safe validation',
]) requireText(centralPlan, marker, `${paths.centralPlan}: Settings P0 gate must remain open`);

for (const marker of [
  'Remaining ownership drift, settings, additional',
  'provider onboarding',
]) requireText(localPlan, marker, `${paths.localPlan}: remaining Settings work`);

for (const marker of [
  'pub fn validate_localization_registry(',
  'localized field IDs must be canonical',
  'localized settings must be string leaves',
  'localized settings cannot use options',
  'localized setting is fenced by sensitive path',
  'pub fn localized_field_paths(',
  'pub fn localized_value_snapshot(',
]) requireText(settings, marker, `${paths.settings}: merged typed-localization prerequisite`);

for (const forbidden of [
  'DatabaseConnection',
  'TenantLocale',
  'translation_target',
  'TranslationTarget',
]) forbidText(
  settings,
  forbidden,
  `${paths.settings}: metadata helper must not masquerade as persisted exact-locale owner`,
);

const suspiciousMigration = migrationNames.find((name) =>
  /localized.*setting|setting.*translation/i.test(name),
);
if (suspiciousMigration) {
  failures.push(
    `${paths.migrations}: localized Settings persistence appeared (${suspiciousMigration}); replace this prerequisite with executable owner evidence`,
  );
}

if (evidence.schema_version !== 1) {
  failures.push(`${paths.evidence}: schema_version must be 1`);
}
if (evidence.status !== 'source_prerequisite_only') {
  failures.push(`${paths.evidence}: status must remain source_prerequisite_only`);
}

for (const [key, expected] of Object.entries({
  localized_field_registry_present: true,
  stable_field_ids_present: true,
  localized_string_leaf_validation_present: true,
  sensitive_path_fences_present: true,
  deterministic_source_value_snapshot_present: true,
  parallel_exact_locale_storage_present: false,
  localized_row_revision_cas_present: false,
  settings_owner_idempotency_receipt_present: false,
  settings_translation_change_cursor_or_event_present: false,
  settings_translation_provider_registered: false,
})) {
  if (evidence.source_facts?.[key] !== expected) {
    failures.push(`${paths.evidence}: source_facts.${key} must be ${expected}`);
  }
}

for (const key of [
  'named_settings_owner',
  'tenant_module_field_locale_identity',
  'canonical_tenant_locale',
  'exact_locale_reads_without_runtime_fallback',
  'base_settings_revision_cas',
  'localized_row_revision_cas',
  'idempotent_owner_apply_receipt',
  'content_free_change_evidence',
  'sensitive_fields_never_persisted_as_localized_copy',
  'translation_provider_registration_only_after_owner_contract',
]) {
  if (evidence.required_next_owner_contract?.[key] !== true) {
    failures.push(`${paths.evidence}: required_next_owner_contract.${key} must be true`);
  }
}

for (const key of [
  'localized_storage_proven',
  'owner_apply_proven',
  'translation_provider_proven',
]) {
  if (evidence.validation?.[key] !== false) {
    failures.push(`${paths.evidence}: validation.${key} must remain false`);
  }
}

for (const marker of [
  'Status: **typed metadata complete / owner persistence and provider onboarding open**',
  'main@bd732fbf80c9169af2d86888a7c44cfc5b9486e8',
  'store localized values outside the language-neutral settings JSON',
  'expose exact-locale reads that never substitute runtime fallback',
  'make apply idempotent with a durable owner receipt',
  'register a Translation target only after the owner read/validate/apply/progress contract',
]) requireText(handoff, marker, `${paths.handoff}: truthful Settings handoff`);

if (failures.length > 0) {
  console.error('Translation Settings localization prerequisite verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Settings localized-field metadata is typed; exact-locale persistence/CAS/idempotency/provider onboarding remain open',
);
