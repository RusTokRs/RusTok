#!/usr/bin/env node

import { readFileSync } from 'node:fs';
import path from 'node:path';
import { pathToFileURL } from 'node:url';

const configuredRoot = process.env.RUSTOK_VERIFY_REPO_ROOT?.trim();
const root = configuredRoot
  ? pathToFileURL(`${path.resolve(configuredRoot)}${path.sep}`)
  : new URL('../../', import.meta.url);
const source = readFileSync(
  new URL('apps/admin/src/widgets/app_shell/native_server_adapter.rs', root),
  'utf8',
);
const failures = [];

const requireText = (value, label) => {
  if (!source.includes(value)) failures.push(`${label}: missing ${value}`);
};

for (const [value, label] of [
  ['fn require_admin_global_search_tenant_scope(', 'tenant scope helper'],
  ['if auth_tenant_id == resolved_tenant_id', 'tenant equality'],
  [
    'require_admin_global_search_tenant_scope(auth.tenant_id, tenant.id)?;',
    'scoped endpoint call',
  ],
  [
    'has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ)',
    'settings read admission',
  ],
  ['Admin search access is denied', 'static public denial'],
  ['admin.global_search_tenant_scope_mismatch', 'stable diagnostic code'],
  ['auth_tenant_id = %auth_tenant_id', 'authenticated tenant diagnostic'],
  ['resolved_tenant_id = %resolved_tenant_id', 'resolved tenant diagnostic'],
  ['boundary = "admin_global_search_native_transport"', 'transport boundary diagnostic'],
  ['SearchDictionaryService::transform_query(', 'tenant dictionary read'],
  ['SearchSettingsService::load_effective(', 'tenant settings read'],
  ['tenant_id: Some(tenant.id)', 'tenant-scoped query'],
  ['record_admin_search_query_log(', 'tenant analytics write'],
]) {
  requireText(value, label);
}

const guardCall = source.indexOf(
  'require_admin_global_search_tenant_scope(auth.tenant_id, tenant.id)?;',
);
const permissionCheck = source.indexOf(
  'has_effective_permission(&auth.permissions, &Permission::SETTINGS_READ)',
);
const dictionaryRead = source.indexOf('SearchDictionaryService::transform_query(');
const analyticsWrite = source.indexOf('record_admin_search_query_log(');
if (
  guardCall < 0 ||
  permissionCheck < 0 ||
  dictionaryRead < 0 ||
  analyticsWrite < 0 ||
  !(guardCall < permissionCheck && permissionCheck < dictionaryRead && dictionaryRead < analyticsWrite)
) {
  failures.push(
    'tenant equality and SETTINGS_READ must precede tenant Search reads and analytics writes',
  );
}

const authExtracts = source.match(/leptos_axum::extract::<AuthContext>\(\)/g) ?? [];
const tenantExtracts = source.match(/leptos_axum::extract::<TenantContext>\(\)/g) ?? [];
const guardCalls = source.match(
  /require_admin_global_search_tenant_scope\(auth\.tenant_id, tenant\.id\)\?;/g,
) ?? [];
if (authExtracts.length !== 1 || tenantExtracts.length !== 1 || guardCalls.length !== 1) {
  failures.push(
    `expected one AuthContext, one TenantContext and one scoped guard call; found ${authExtracts.length}/${tenantExtracts.length}/${guardCalls.length}`,
  );
}

if (failures.length > 0) {
  console.error('Admin global search tenant-scope verification failed:');
  for (const failure of failures) console.error(`✗ ${failure}`);
  process.exit(Math.min(failures.length, 255));
}

console.log(
  '✔ Admin global search binds authenticated authority to the resolved tenant before Search reads and analytics writes',
);
