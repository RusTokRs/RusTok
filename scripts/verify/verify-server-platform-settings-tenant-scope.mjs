#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const read = (relativePath) => readFileSync(new URL(relativePath, root), 'utf8');

const moduleSource = read('apps/server/src/graphql/settings/mod.rs');
const querySource = read('apps/server/src/graphql/settings/query.rs');
const mutationSource = read('apps/server/src/graphql/settings/mutation.rs');
const failures = [];

const requireText = (content, value, label) => {
  if (!content.includes(value)) failures.push(`${label}: missing ${value}`);
};
const between = (content, start, end, label) => {
  const startIndex = content.indexOf(start);
  const endIndex = end ? content.indexOf(end, startIndex + start.length) : content.length;
  if (startIndex < 0 || endIndex < 0) {
    failures.push(`${label}: unable to isolate source block`);
    return '';
  }
  return content.slice(startIndex, endIndex);
};
const requireOrder = (content, values, label) => {
  let cursor = -1;
  for (const value of values) {
    const next = content.indexOf(value, cursor + 1);
    if (next < 0 || next <= cursor) {
      failures.push(`${label}: expected ordered marker ${value}`);
      return;
    }
    cursor = next;
  }
};

for (const [value, label] of [
  ['fn require_tenant_settings_scope(', 'shared tenant scope helper'],
  ['if auth.tenant_id == resolved_tenant_id', 'tenant equality'],
  ['Settings access is denied', 'static public denial'],
  ['settings.tenant_scope_mismatch', 'stable diagnostic code'],
  ['auth_tenant_id = %auth.tenant_id', 'authenticated tenant diagnostic'],
  ['resolved_tenant_id = %resolved_tenant_id', 'resolved tenant diagnostic'],
  ['boundary = "server_settings_graphql"', 'GraphQL boundary diagnostic'],
]) {
  requireText(moduleSource, value, label);
}

const platformSettings = between(
  querySource,
  'async fn platform_settings(',
  'async fn all_platform_settings(',
  'platform settings query',
);
const allPlatformSettings = between(
  querySource,
  'async fn all_platform_settings(',
  null,
  'all platform settings query',
);
const updatePlatformSettings = between(
  mutationSource,
  'async fn update_platform_settings(',
  null,
  'update platform settings mutation',
);

requireOrder(
  platformSettings,
  [
    'ctx.data::<TenantContext>()?',
    'require_tenant_settings_scope(auth, tenant.id)?;',
    'has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ)',
    'SettingsService::get(runtime_ctx, tenant.id, &category)',
  ],
  'single-category settings read',
);
requireOrder(
  allPlatformSettings,
  [
    'ctx.data::<TenantContext>()?',
    'require_tenant_settings_scope(auth, tenant.id)?;',
    'has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ)',
    'SettingsService::get_all(runtime_ctx, tenant.id)',
  ],
  'all settings read',
);
requireOrder(
  updatePlatformSettings,
  [
    'ctx.data::<TenantContext>()?',
    'require_tenant_settings_scope(auth, tenant.id)?;',
    'has_effective_permission(&auth.permissions, &Permission::SETTINGS_MANAGE)',
    'SettingsService::update(',
    'event_bus',
    '.publish(',
  ],
  'settings write and event publication',
);

const queryCalls = querySource.match(/require_tenant_settings_scope\(auth, tenant\.id\)\?;/g) ?? [];
const mutationCalls = mutationSource.match(/require_tenant_settings_scope\(auth, tenant\.id\)\?;/g) ?? [];
if (queryCalls.length !== 2 || mutationCalls.length !== 1) {
  failures.push(
    `expected two scoped tenant settings queries and one scoped mutation; found ${queryCalls.length}/${mutationCalls.length}`,
  );
}

for (const [content, name] of [
  [between(querySource, 'async fn iggy_connector_configuration(', 'async fn event_delivery_configuration(', 'Iggy query'), 'Iggy connector query'],
  [between(querySource, 'async fn event_delivery_configuration(', 'async fn platform_settings(', 'event query'), 'event delivery query'],
  [between(mutationSource, 'async fn update_iggy_connector_configuration(', 'async fn update_event_delivery_configuration(', 'Iggy mutation'), 'Iggy connector mutation'],
  [between(mutationSource, 'async fn update_event_delivery_configuration(', 'async fn update_platform_settings(', 'event mutation'), 'event delivery mutation'],
]) {
  if (content.includes('require_tenant_settings_scope(')) {
    failures.push(`${name}: host-global resource must not be disguised as tenant-owned`);
  }
}

if (failures.length > 0) {
  console.error('Server platform settings tenant-scope verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Tenant platform-settings GraphQL reads/writes bind authority before storage and events; host-global controls remain separate',
);
