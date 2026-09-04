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

const hostResolverPath = 'apps/server/src/static_settings_localization_registry.rs';
const serverLibPath = 'apps/server/src/lib.rs';
const evidencePath =
  'crates/rustok-translation/contracts/evidence/translation-settings-localization-prerequisite-source.json';
const handoffPath = 'crates/rustok-translation/docs/translation-settings-localization-prerequisite.md';

const hostResolver = read(hostResolverPath);
const serverLib = read(serverLibPath);
const evidence = JSON.parse(read(evidencePath));
const handoff = read(handoffPath);

const requireText = (source, value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};
const forbidText = (source, value, label) => {
  if (source.includes(value)) failures.push(`${label}: forbidden ${value}`);
};

for (const marker of [
  'pub fn resolve_static_settings_localization_registry(',
  'ManifestManager::module_settings_schema(module_slug)?',
  'settings_localization: StaticSettingsLocalizationPackageMetadata',
  'rustok_modules::StaticModulePackageContract',
  'rustok_modules::StaticSettingsLocalizationRegistry::new(',
  'package_metadata_resolves_through_owner_package_contract',
  'package_metadata_cannot_localize_a_sensitive_path',
]) requireText(
  hostResolver,
  marker,
  `${hostResolverPath}: authoritative package registry resolver`,
);

for (const forbidden of [
  'rustok_translation::',
  'TranslationTargetProvider',
  'DatabaseConnection',
  'sea_orm',
]) forbidText(
  hostResolver,
  forbidden,
  `${hostResolverPath}: resolver must stay host/package-only`,
);

requireText(
  serverLib,
  'pub mod static_settings_localization_registry;',
  `${serverLibPath}: registry resolver must be exported to runtime composition`,
);

for (const [key, expected] of Object.entries({
  provider_package_registry_resolver_present: true,
  provider_package_registry_uses_owner_contract: true,
  provider_package_registry_keeps_translation_manifest_blind: true,
  settings_translation_provider_registered: false,
})) {
  if (evidence.source_facts?.[key] !== expected) {
    failures.push(`${evidencePath}: source_facts.${key} must be ${expected}`);
  }
}
if (evidence.validation?.provider_package_registry_source_proven !== true) {
  failures.push(`${evidencePath}: validation.provider_package_registry_source_proven must be true`);
}

for (const marker of [
  'authoritative package registry resolution source-ready',
  '`resolve_static_settings_localization_registry(module_slug)`',
  '`StaticSettingsLocalizationRegistry::new`',
  'Translation never sees the package path or TOML parser',
  'Only the runtime registration/execution slice remains',
]) requireText(
  handoff,
  marker,
  `${handoffPath}: package registry prerequisite handoff`,
);

if (failures.length > 0) {
  console.error('Translation Settings package-registry prerequisite verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Settings package metadata resolves to the authoritative owner localization registry without Translation manifest access',
);
