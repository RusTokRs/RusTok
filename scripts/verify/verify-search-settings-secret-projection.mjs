#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { fileURLToPath } from 'node:url';

const root = path.resolve(path.dirname(fileURLToPath(import.meta.url)), '../..');
const read = (relative) => readFileSync(path.join(root, relative), 'utf8');
const fail = (message) => {
  console.error(`[verify-search-settings-secret-projection] ${message}`);
  process.exit(1);
};
const requireMarkers = (relative, markers) => {
  const source = read(relative);
  for (const marker of markers) {
    if (!source.includes(marker)) fail(`${relative} is missing ${marker}`);
  }
  return source;
};

const service = requireMarkers('apps/server/src/services/settings_service.rs', [
  'const SEARCH_API_KEY_FIELD: &str = "api_key";',
  'const SEARCH_API_KEY_CONFIGURED_FIELD: &str = "api_key_configured";',
  'pub struct SearchSettingsValidator;',
  'search.api_key is bootstrap-only and cannot be stored in tenant platform settings',
  'let raw = Self::load_raw(ctx, tenant_id, cat).await?;',
  'Ok(Self::public_projection(cat, raw))',
  'Self::public_projection(&category, row.settings)',
  'let settings = Self::normalize_for_storage(cat, settings)?;',
  'Ok(Self::public_projection(cat, settings))',
  'object.remove(SEARCH_API_KEY_CONFIGURED_FIELD);',
  '.remove(SEARCH_API_KEY_FIELD)',
  'Value::Bool(configured)',
  'search_public_projection_redacts_api_key_and_reports_configuration',
  'search_storage_normalization_rejects_secret_and_drops_public_marker',
]);

const query = requireMarkers('apps/server/src/graphql/settings/query.rs', [
  'SettingsService::get(runtime_ctx, tenant.id, &category)',
  'SettingsService::get_all(runtime_ctx, tenant.id)',
  'has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ)',
]);

const mutation = requireMarkers('apps/server/src/graphql/settings/mutation.rs', [
  'SettingsService::update(',
  'has_effective_permission(&auth.permissions, &Permission::SETTINGS_MANAGE)',
  'let settings_str = serde_json::to_string(&stored)',
]);

for (const [label, source] of [
  ['settings query', query],
  ['settings mutation', mutation],
]) {
  if (source.includes('ctx.settings().search.api_key') || source.includes('runtime_ctx.settings().search.api_key')) {
    fail(`${label} reads the bootstrap Search API key directly`);
  }
  if (source.includes('SEARCH_API_KEY_FIELD') || source.includes('"api_key"')) {
    fail(`${label} owns secret-field projection instead of delegating to SettingsService`);
  }
}

if (!service.includes('reg.register(SearchSettingsValidator);')) {
  fail('default validator registry does not enforce the Search secret boundary');
}

console.log('[verify-search-settings-secret-projection] OK');
